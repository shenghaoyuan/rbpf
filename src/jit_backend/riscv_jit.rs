#[cfg(not(feature = "shuttle-test"))]
use rand::{thread_rng, Rng};
#[cfg(feature = "shuttle-test")]
use shuttle::rand::{thread_rng, Rng};

use rand::{
    distributions::{Distribution, Uniform},
    rngs::SmallRng,
    SeedableRng,
};
use std::{fmt::Debug, mem, ptr};

use crate::{
    ebpf::{self, FIRST_SCRATCH_REG, FRAME_PTR_REG, INSN_SIZE, Insn, SCRATCH_REGS},
    elf::Executable,
    error::{EbpfError, ProgramResult},
    memory_management::{
        allocate_pages, free_pages, get_system_page_size, protect_pages, round_to_page_size,
    },
    memory_region::{AccessType, MemoryMapping},
    vm::{get_runtime_environment_key, Config, ContextObject, EbpfVm, RuntimeEnvironmentSlot},
    riscv::*,
    jit::*,
};

pub(crate) unsafe fn invoke_trampoline<C: ContextObject>(
    vm: &mut EbpfVm<C>,
    runtime_environment: *mut u64,
    instruction_meter: i64,
    entrypoint: *const u8,
    registers: &[u64; 12],
) {
    macro_rules! stmt_expr_attribute_asm {
        ($($prologue:literal,)+ cfg(not(feature = $feature:literal)), $guarded:tt, $($epilogue:tt)+) => {
            #[cfg(feature = $feature)]
            std::arch::asm!($($prologue,)+ $($epilogue)+);
            #[cfg(not(feature = $feature))]
            std::arch::asm!($($prologue,)+ $guarded, $($epilogue)+);
        }
    }
    stmt_expr_attribute_asm!(
        
        "addi sp, sp, -24",                // push s0 and s1 and ra
        "sd ra, 16(sp)",
        "sd s1, 8(sp)",                    
        "sd s0, 0(sp)",                    
        "sd sp, 0({host_stack_pointer})",  // host_stack_pointer needn't -8, because jarl don't push ra
        
        cfg(not(feature = "jit-enable-host-stack-frames")),
        "xor s0, s0, s0",

        "ld a0, 0(a7)",           
        "ld a1, 8(a7)",           
        "ld a2, 16(a7)",         
        "ld a3, 24(a7)",         
        "ld a4, 32(a7)",          
        "ld a5, 40(a7)",          
        "ld s1, 48(a7)",          
        "ld s2, 56(a7)",          
        "ld s3, 64(a7)",          
        "ld s4, 72(a7)",          
        "ld s5, 80(a7)",          
        "ld a7, 88(a7)",          
        // call the JITed code
        "1: auipc t2, %pcrel_hi(2f)",
        "addi t2, t2, %pcrel_lo(1b)",
        "mv s6, t2",                       // save return address in s6
        "jalr ra, s7",
        "2:",

        // pop s0 and s1
        "ld ra, 16(sp)",
        "ld s1, 8(sp)",
        "ld s0, 0(sp)",
        "addi sp, sp, 24",                   

        host_stack_pointer = in(reg) &mut vm.host_stack_pointer,
        inlateout("s10") runtime_environment => _,
        inlateout("a6") instruction_meter => _,
        inlateout("s7") entrypoint => _,
        inlateout("a7") registers => _,
        lateout("a1") _, lateout("a2") _, lateout("a3") _, lateout("a4") _,
        lateout("a5") _, lateout("s2") _, lateout("s3") _, lateout("s4") _, lateout("s5") _,
    );
}

pub const REGISTER_MAP: [u8; 11] = [
    CALLER_SAVED_REGISTERS[4], //a0
    ARGUMENT_REGISTERS[1],     //a1
    ARGUMENT_REGISTERS[2],     //a2
    ARGUMENT_REGISTERS[3],     //a3
    ARGUMENT_REGISTERS[4],     //a4
    ARGUMENT_REGISTERS[5],     //a5
    CALLEE_SAVED_REGISTERS[1], //s1
    CALLEE_SAVED_REGISTERS[2], //s2
    CALLEE_SAVED_REGISTERS[3], //s3
    CALLEE_SAVED_REGISTERS[4], //s4
    CALLEE_SAVED_REGISTERS[5], //s5
];

/// S10: Used together with slot_in_vm()
pub const REGISTER_PTR_TO_VM: u8 = S10;
/// A6: Program counter limit
pub const REGISTER_INSTRUCTION_METER: u8 = CALLER_SAVED_REGISTERS[7];
/// A7: Scratch register
pub const REGISTER_SCRATCH: u8 = CALLER_SAVED_REGISTERS[8];

pub(crate) fn emit_noop<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, ZERO, 0, ZERO));
}

// This function helps the optimizer to inline the machinecode emission while avoiding stack allocations
#[inline(always)]
pub(crate) fn emit_ins<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    instruction: RISCVInstruction,
) {
    jit.emit(instruction.emit());
}

pub(crate) fn emit_register_trace<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    load_immediate(jit, REGISTER_SCRATCH, jit.pc as i64);
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
    store(jit, OperandSize::S64, SP, RA, 0);
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_TRACE, 0) as i64, RA));
    load(jit, OperandSize::S64, SP, 0, RA);
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
    emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, ZERO, REGISTER_SCRATCH));
}

pub(crate) fn emit_sanitized_load_immediate<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    destination: u8,
    value: i64,
) {
    let lower_key = jit.immediate_value_key as i32 as i64;
    if value >= i32::MIN as i64 && value <= i32::MAX as i64 {
        load_immediate(jit, destination, value.wrapping_sub(lower_key));
        load_immediate(jit, T1, lower_key); // wrapping_add(lower_key)
        emit_ins(jit, RISCVInstruction::add(OperandSize::S64, destination, T1, destination));
    } else if value as u64 & u32::MAX as u64 == 0 {
        load_immediate(jit, destination, value.rotate_right(32).wrapping_sub(lower_key));
        load_immediate(jit, T1, lower_key);
        emit_ins(jit, RISCVInstruction::add(OperandSize::S64, destination, T1, destination)); // wrapping_add(key)
        emit_ins(jit, RISCVInstruction::slli(OperandSize::S64, destination, 32, destination)); // shift_left(32)
    } else if destination != REGISTER_SCRATCH {
        load_immediate(jit, destination, value.wrapping_sub(jit.immediate_value_key));
        load_immediate(jit, REGISTER_SCRATCH, jit.immediate_value_key);
        emit_ins(jit, RISCVInstruction::add(OperandSize::S64, destination, REGISTER_SCRATCH, destination)); // wrapping_add(immediate_value_key)
    } else {
        let upper_key = (jit.immediate_value_key >> 32) as i32 as i64;
        load_immediate(jit, destination, value.wrapping_sub(lower_key).rotate_right(32).wrapping_sub(upper_key));
        load_immediate(jit, T1, upper_key); // wrapping_add(upper_key)
        emit_ins(jit, RISCVInstruction::add(OperandSize::S64, destination, T1, destination));
        rotate_right(jit, OperandSize::S64, destination, 32, destination);
        load_immediate(jit, T1, lower_key); // wrapping_add(lower_key)
        emit_ins(jit, RISCVInstruction::add(OperandSize::S64, destination, T1, destination));
    } 
}

// riscv-specific
pub(crate) fn rotate_right<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    source1: u8,
    shamt: i64,
    destination: u8,
) {
    emit_ins(jit, RISCVInstruction::mov(size, source1, T2));
    emit_ins(jit, RISCVInstruction::mov(size, source1, T3));
    emit_ins(jit, RISCVInstruction::slli(size, T2, shamt, T2));
    emit_ins(jit, RISCVInstruction::srli(size, T3, shamt, T3));
    emit_ins(jit, RISCVInstruction::or(size, T2, T3, destination));
}

pub(crate) fn load_immediate<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    destination: u8,
    immediate: i64,
) {
    if immediate >= i32::MIN as i64 && immediate <= i32::MAX as i64 {
        let size = OperandSize::S32;
        load_immediate_with_lui_and_addi(jit, size, destination, immediate);
    } else {
        let size = OperandSize::S64;
        let upper_imm = immediate >> 32; // high 32 bits
        let lower_imm = immediate & 0xFFFFFFFF; // low 32 bits

        // Step 1: handle high 32 bits immediate to destination register
        load_immediate_with_lui_and_addi(jit, size, destination, upper_imm);

        // Step 2: use SLLI to shift left by 32 bits
        emit_ins(jit, RISCVInstruction::slli(size, destination, 32, destination));

        // Step 3: handle low 32 bits immediate
        load_immediate_with_lui_and_addi(jit, size, T0, lower_imm);
        zero_extend(jit, T0);

        // Step 4: use OR to combine high and low parts
        emit_ins(jit, RISCVInstruction::or(size, destination, T0, destination));
    }
}

// riscv-specific
/// Divide the immediate number into the high 20 bits and the low 12 bits
pub(crate) fn load_immediate_with_lui_and_addi<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    destination: u8,
    immediate: i64,
) {
    if immediate >= -2048 && immediate <= 2047 {
        // imm in 12-bit range, use ADDI directly
        emit_ins(jit, RISCVInstruction::addi(size, 0, immediate, destination));
    } else {
        // handel immediate number larger than 12 bits
        let upper_imm = immediate >> 12; // high 20 bits
        let lower_imm = immediate & 0xFFF; // low 12 bits
        let sign_ext = if lower_imm & 0x800 != 0 { 1 } else { 0 };

        // Step 1: load high 20 bits using LUI
        emit_ins(jit, RISCVInstruction::lui(size, upper_imm + sign_ext, destination));

        // Step 2: add low 12 bits using ADDI
        if lower_imm != 0 {
            emit_ins(jit, RISCVInstruction::addi(size, destination, lower_imm, destination));
        }
    }
}

pub(crate) fn emit_sanitized_add<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    destination: u8,
    immediate: i64,
) {
    if jit.should_sanitize_constant(immediate) {
        emit_sanitized_load_immediate(jit, T4, immediate);
        if size == OperandSize::S32 {
            emit_ins(jit, RISCVInstruction::addw(size, T4, destination, destination));
        } else {
            emit_ins(jit, RISCVInstruction::add(size, T4, destination, destination));
        }
    } else {
        load_immediate(jit, T1, immediate);
        if size == OperandSize::S32 {
            emit_ins(jit, RISCVInstruction::addw(size, T1, destination, destination));
        } else {
            emit_ins(jit, RISCVInstruction::add(size, T1, destination, destination));
        }
    }
}

pub(crate) fn emit_or_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    if jit.should_sanitize_constant(imm) {
        emit_sanitized_load_immediate(jit, T4, imm);
        emit_ins(jit, RISCVInstruction::or(size, dst, T4, dst));
    } else if imm >= -2048 && imm <= 2047 {
        emit_ins(jit, RISCVInstruction::ori(size, dst, imm, dst));
    } else {
        load_immediate(jit, T1, imm);
        emit_ins(jit, RISCVInstruction::or(size, dst, T1, dst));
    }
    if size == OperandSize::S32 {
        zero_extend(jit, dst);
    }
}

pub(crate) fn emit_or_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    emit_ins(jit, RISCVInstruction::or(size, dst, src, dst));
    if size == OperandSize::S32 {
        zero_extend(jit, dst);
    }
}

pub(crate) fn emit_and_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    if jit.should_sanitize_constant(imm) {
        emit_sanitized_load_immediate(jit, T4, imm);
        emit_ins(jit, RISCVInstruction::and(size, dst, T4, dst));
    } else if imm >= -2048 && imm <= 2047 {
        emit_ins(jit, RISCVInstruction::andi(size, dst, imm, dst));
    } else {
        load_immediate(jit, T1, imm);
        emit_ins(jit, RISCVInstruction::and(size, dst, T1, dst));
    }
    if size == OperandSize::S32 {
        zero_extend(jit, dst);
    }
}

pub(crate) fn emit_and_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    emit_ins(jit, RISCVInstruction::and(size, dst, src, dst));
    if size == OperandSize::S32 {
        zero_extend(jit, dst);
    }
}

pub(crate) fn emit_lsh_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::slliw(OperandSize::S32, dst, imm, dst));
        zero_extend(jit, dst);
    } else {
        emit_ins(jit, RISCVInstruction::slli(OperandSize::S64, dst, imm, dst));
    }
}

pub(crate) fn emit_lsh_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::sllw(OperandSize::S32, dst, src, dst));
        zero_extend(jit, dst);
    } else {
        emit_ins(jit, RISCVInstruction::sll(OperandSize::S64, dst, src, dst));
    }
}

pub(crate) fn emit_rsh_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::srliw(OperandSize::S32, dst, imm, dst));
        zero_extend(jit, dst);
    } else {
        emit_ins(jit, RISCVInstruction::srli(OperandSize::S64, dst, imm, dst));
    }
}

pub(crate) fn emit_rsh_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::srlw(OperandSize::S32, dst, src, dst));
        zero_extend(jit, dst);
    } else {
        emit_ins(jit, RISCVInstruction::srl(OperandSize::S64, dst, src, dst));
    }
}

pub(crate) fn emit_arsh_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::sraiw(OperandSize::S32, dst, imm, dst));
        zero_extend(jit, dst);
    } else {
        emit_ins(jit, RISCVInstruction::srai(OperandSize::S64, dst, imm, dst));
    }
}

pub(crate) fn emit_arsh_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::sraw(OperandSize::S32, dst, src, dst));
        zero_extend(jit, dst);
    } else {
        emit_ins(jit, RISCVInstruction::sra(OperandSize::S64, dst, src, dst));
    }
}

pub(crate) fn emit_le<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    dst: u8,
    imm: i64,
) -> Result<(), EbpfError> {
    match imm {
        16 => {
            load_immediate(jit, T1, 0xffff);
            emit_ins(jit, RISCVInstruction::and(OperandSize::S32, dst, T1, dst)); // Mask to 16 bit
        }
        32 => {
            load_immediate(jit, T1, 0xffffffff);
            emit_ins(jit, RISCVInstruction::and(OperandSize::S32, dst, T1, dst)); // Mask to 32 bit
        }
        64 => {}
        _ => {
            return Err(EbpfError::InvalidInstruction);
        }
    }
    Ok(())
}

pub(crate) fn emit_be<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    dst: u8,
    imm: i64,
) -> Result<(), EbpfError> {
    match imm {
        16 => {
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S32, dst, 0xff, T1));
            emit_ins(jit, RISCVInstruction::srliw(OperandSize::S32, dst, 8, T2));
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S32, T2, 0xff, T2));
            emit_ins(jit, RISCVInstruction::slliw(OperandSize::S32, T1, 8, T1));
            emit_ins(jit, RISCVInstruction::or(OperandSize::S64, T1, T2, dst));
        }
        32 => {
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S32, dst, 0xff, T1));
            emit_ins(jit, RISCVInstruction::srliw(OperandSize::S32, dst, 8, T2));
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S32, T2, 0xff, T2));
            emit_ins(jit, RISCVInstruction::srliw(OperandSize::S32, dst, 16, T3));
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S32, T3, 0xff, T3));
            emit_ins(jit, RISCVInstruction::srliw(OperandSize::S32, dst, 24, T4));
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S32, T4, 0xff, T4));
            
            emit_ins(jit, RISCVInstruction::slliw(OperandSize::S32, T1, 24, T1));
            emit_ins(jit, RISCVInstruction::slliw(OperandSize::S32, T2, 16, T2));
            emit_ins(jit, RISCVInstruction::slliw(OperandSize::S32, T3, 8, T3));
            emit_ins(jit, RISCVInstruction::or(OperandSize::S64, T1, T2, dst));
            emit_ins(jit, RISCVInstruction::or(OperandSize::S64, dst, T3, dst));
            emit_ins(jit, RISCVInstruction::or(OperandSize::S64, dst, T4, dst));
        }
        64 => {
            //low32bits
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S64, dst, 0xff, T1));
            emit_ins(jit, RISCVInstruction::srliw(OperandSize::S64, dst, 8, T2));
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S64, T2, 0xff, T2));
            emit_ins(jit, RISCVInstruction::srliw(OperandSize::S64, dst, 16, T3));
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S64, T3, 0xff, T3));
            emit_ins(jit, RISCVInstruction::srliw(OperandSize::S64, dst, 24, T4));
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S64, T4, 0xff, T4));
            
            emit_ins(jit, RISCVInstruction::slliw(OperandSize::S64, T1, 24, T1));
            emit_ins(jit, RISCVInstruction::slliw(OperandSize::S64, T2, 16, T2));
            emit_ins(jit, RISCVInstruction::slliw(OperandSize::S64, T3, 8, T3));
            emit_ins(jit, RISCVInstruction::or(OperandSize::S64, T1, T2, T5));
            emit_ins(jit, RISCVInstruction::or(OperandSize::S64, T5, T3, T5));
            emit_ins(jit, RISCVInstruction::or(OperandSize::S64, T5, T4, T5));
            emit_ins(jit, RISCVInstruction::slli(OperandSize::S64,T5,32,T5));
            //high32bits
            emit_ins(jit, RISCVInstruction::srli(OperandSize::S64, dst, 32, T1));
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S64, T1, 0xff, T1));
            emit_ins(jit, RISCVInstruction::srli(OperandSize::S64, dst, 40, T2));
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S64, T2, 0xff, T2));
            emit_ins(jit, RISCVInstruction::srli(OperandSize::S64, dst, 48, T3));
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S64, T3, 0xff, T3));
            emit_ins(jit, RISCVInstruction::srli(OperandSize::S64, dst, 56, T4));
            emit_ins(jit, RISCVInstruction::andi(OperandSize::S64, T4, 0xff, T4));
        
            emit_ins(jit, RISCVInstruction::slli(OperandSize::S64, T1, 24, T1));
            emit_ins(jit, RISCVInstruction::slli(OperandSize::S64, T2, 16, T2));
            emit_ins(jit, RISCVInstruction::slli(OperandSize::S64, T3, 8, T3));
            emit_ins(jit, RISCVInstruction::or(OperandSize::S64, T5, T1, T5));
            emit_ins(jit, RISCVInstruction::or(OperandSize::S64, T5, T2, T5));
            emit_ins(jit, RISCVInstruction::or(OperandSize::S64, T5, T3, T5));
            emit_ins(jit, RISCVInstruction::or(OperandSize::S64, T5, T4, dst));
        }
        _ => {
            return Err(EbpfError::InvalidInstruction);
        }
    }
    Ok(())
}

pub(crate) fn emit_neg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
) {
    emit_ins(jit, RISCVInstruction::sub(size, ZERO, dst, dst));
    if size == OperandSize::S32 {
        zero_extend(jit, dst);
    }
}

pub(crate) fn emit_xor_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    if jit.should_sanitize_constant(imm) {
        emit_sanitized_load_immediate(jit, T4, imm);
        emit_ins(jit, RISCVInstruction::xor(size, dst, T4, dst));
    } else if imm >= -2048 && imm <= 2047 {
        emit_ins(jit, RISCVInstruction::xori(size, dst, imm, dst));
    } else {
        load_immediate(jit, T1, imm);
        emit_ins(jit, RISCVInstruction::xor(size, dst, T1, dst));
    }
    if size == OperandSize::S32 {
        zero_extend(jit, dst);
    }
}

pub(crate) fn emit_xor_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    emit_ins(jit, RISCVInstruction::xor(size, dst, src, dst));
    if size == OperandSize::S32 {
        zero_extend(jit, dst);
    }
}

pub(crate) fn emit_hor_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    if jit.should_sanitize_constant(imm) {
        emit_sanitized_load_immediate(jit, T4, imm);
        emit_ins(jit, RISCVInstruction::or(size, dst, T4, dst));
    } else if imm >= -2048 && imm <= 2047 {
        emit_ins(jit, RISCVInstruction::ori(size, dst, imm, dst));
    } else {
        load_immediate(jit, T1, imm);
        emit_ins(jit, RISCVInstruction::or(size, dst, T1, dst));
    }
    if size == OperandSize::S32 {
        zero_extend(jit, dst);
    }
}

pub(crate) fn emit_sanitized_sub<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    destination: u8,
    immediate: i64,
) {
    if jit.should_sanitize_constant(immediate) {
        emit_sanitized_load_immediate(jit, T4, immediate);
        if size == OperandSize::S32 {
            emit_ins(jit, RISCVInstruction::subw(size, destination, T4, destination));
        } else {
            emit_ins(jit, RISCVInstruction::sub(size, destination, T4, destination));
        }
    } else {
        load_immediate(jit, T1, immediate);
        if size == OperandSize::S32 {
            emit_ins(jit, RISCVInstruction::subw(size, destination, T1, destination));
        } else {
            emit_ins(jit, RISCVInstruction::sub(size, destination, T1, destination));
        }
    }
}

/// clear the high 32 bits of a 64-bit register
pub(crate) fn zero_extend<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    destination: u8,
) {
    emit_ins(jit, RISCVInstruction::slli(OperandSize::S64, destination, 32, destination));
    emit_ins(jit, RISCVInstruction::srli(OperandSize::S64, destination, 32, destination));
}

/// sign-extend
pub(crate) fn sign_extend<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    destination: u8,
) {
    emit_ins(jit, RISCVInstruction::addiw(OperandSize::S64, destination, 0, destination));
}

pub(crate) fn emit_add_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::addw(OperandSize::S32, src, dst, dst));
    } else {
        emit_ins(jit, RISCVInstruction::add(OperandSize::S64, src, dst, dst));
    }
}

pub(crate) fn emit_mov_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, src, dst));
        if !jit.executable.get_sbpf_version().explicit_sign_extension_of_results() {
            zero_extend(jit, dst);
        }
    } else {
        emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, src, dst));
    }
}

pub(crate) fn emit_sub_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    if size == OperandSize::S32 {
        if jit.executable.get_sbpf_version().swap_sub_reg_imm_operands() {
            emit_ins(jit, RISCVInstruction::subw(OperandSize::S32, ZERO, dst, dst));
            if imm != 0 {
                emit_sanitized_add(jit, OperandSize::S32, dst, imm);
            }
        } else {
            emit_sanitized_sub(jit, OperandSize::S32, dst, imm);
        }
        if !jit.executable.get_sbpf_version().explicit_sign_extension_of_results() {
            sign_extend(jit, dst); // sign extend i32 to i64
        }
    } else {
        if jit.executable.get_sbpf_version().swap_sub_reg_imm_operands() {
            emit_ins(jit, RISCVInstruction::sub(OperandSize::S64, ZERO, dst, dst));
            if imm != 0{
                emit_sanitized_add(jit, OperandSize::S64, dst, imm);
            }
        } else {
            emit_sanitized_sub(jit, OperandSize::S64, dst, imm);
        }
    }
}

pub(crate) fn emit_sub_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::subw(OperandSize::S32, dst, src, dst));
        if !jit.executable.get_sbpf_version().explicit_sign_extension_of_results() {
            sign_extend(jit, dst);
        }
    } else {
        emit_ins(jit, RISCVInstruction::sub(OperandSize::S64, dst, src, dst));
    }
}

pub(crate) fn emit_mul_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    if jit.should_sanitize_constant(imm) {
        emit_sanitized_load_immediate(jit, REGISTER_SCRATCH, imm);
    } else {
        load_immediate(jit, REGISTER_SCRATCH, imm);
    }
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::mulw(OperandSize::S32, dst, REGISTER_SCRATCH, dst));
        if !jit.executable.get_sbpf_version().explicit_sign_extension_of_results() {
            sign_extend(jit, dst);
        }
    } else {
        emit_ins(jit, RISCVInstruction::mul(OperandSize::S64, dst, REGISTER_SCRATCH, dst));
    }
}

pub(crate) fn emit_div_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    load_immediate(jit, T1, insn.imm);
    if size == OperandSize::S32 {
        div_err_handle(jit, OperandSize::S32, false, T1, dst);
        emit_ins(jit, RISCVInstruction::divuw(OperandSize::S32, dst, T1, dst));
        zero_extend(jit, dst);
    } else {
        div_err_handle(jit, OperandSize::S64, false, T1, dst);
        emit_ins(jit, RISCVInstruction::divu(OperandSize::S64, dst, T1, dst));
    }
}

pub(crate) fn emit_mod_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    load_immediate(jit, T1, insn.imm);
    if size == OperandSize::S32 {
        div_err_handle(jit, OperandSize::S32, false, T1, dst);
        emit_ins(jit, RISCVInstruction::remuw(OperandSize::S32, dst, T1, dst));
        zero_extend(jit, dst);
    } else {
        div_err_handle(jit, OperandSize::S64, false, T1, dst);
        emit_ins(jit, RISCVInstruction::remu(OperandSize::S64, dst, T1, dst));
    }
}

pub(crate) fn emit_div_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        div_err_handle(jit, OperandSize::S32, false, src, dst);
        emit_ins(jit, RISCVInstruction::divuw(OperandSize::S32, dst, src, dst));
        zero_extend(jit, dst);
    } else {
        div_err_handle(jit, OperandSize::S64, false, src, dst);
        emit_ins(jit, RISCVInstruction::divu(OperandSize::S64, dst, src, dst));
    }
}

pub(crate) fn emit_mod_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        div_err_handle(jit, OperandSize::S32, false, src, dst);
        emit_ins(jit, RISCVInstruction::remuw(OperandSize::S32, dst, src, dst));
        zero_extend(jit, dst);
    } else {
        div_err_handle(jit, OperandSize::S64, false, src, dst);
        emit_ins(jit, RISCVInstruction::remu(OperandSize::S64, dst, src, dst));
    }
}

pub(crate) fn emit_mul_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::mulw(OperandSize::S32, dst, src, dst));
        if !jit.executable.get_sbpf_version().explicit_sign_extension_of_results() {
            sign_extend(jit, dst);
        }
    } else {
        emit_ins(jit, RISCVInstruction::mul(OperandSize::S64, dst, src, dst));
    }
}

pub(crate) fn emit_lmul_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    load_immediate(jit, T1, insn.imm);
    if size == OperandSize::S32 {
        zero_extend(jit, T1);
        emit_ins(jit, RISCVInstruction::mulw(OperandSize::S32, dst, T1, dst));
        zero_extend(jit, dst);
    } else {
        emit_ins(jit, RISCVInstruction::mul(OperandSize::S64, dst, T1, dst));
    }
}

pub(crate) fn emit_uhlmul_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    load_immediate(jit, T1, insn.imm);
    zero_extend(jit, T1);
    emit_ins(jit, RISCVInstruction::mulhu(OperandSize::S64, dst, T1, dst));
}

pub(crate) fn emit_shlmul_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    load_immediate(jit, T1, insn.imm);
    emit_ins(jit, RISCVInstruction::mulh(OperandSize::S64, dst, T1, dst));
}

pub(crate) fn emit_udiv_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    load_immediate(jit, T1, insn.imm);
    if size == OperandSize::S32 {
        div_err_handle(jit, OperandSize::S32, false, T1, dst);
        emit_ins(jit, RISCVInstruction::divuw(OperandSize::S32, dst, T1, dst));
        zero_extend(jit, dst);
    } else {
        zero_extend(jit, T1);
        div_err_handle(jit, OperandSize::S64, false, T1, dst);
        emit_ins(jit, RISCVInstruction::divu(OperandSize::S64, dst, T1, dst));
    }
}

pub(crate) fn emit_urem_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    load_immediate(jit, T1, insn.imm);
    if size == OperandSize::S32 {
        zero_extend(jit, T1);
        div_err_handle(jit, OperandSize::S32, false, T1, dst);
        emit_ins(jit, RISCVInstruction::remuw(OperandSize::S32, dst, T1, dst));
        zero_extend(jit, dst);
    } else {
        zero_extend(jit, T1);
        div_err_handle(jit, OperandSize::S64, false, T1, dst);
        emit_ins(jit, RISCVInstruction::remu(OperandSize::S64, dst, T1, dst));
    }
}

pub(crate) fn emit_sdiv_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    load_immediate(jit, T1, insn.imm);
    if size == OperandSize::S32 {
        div_err_handle(jit, OperandSize::S32, true, T1, dst);
        emit_ins(jit, RISCVInstruction::divw(OperandSize::S32, dst, T1, dst));
        zero_extend(jit, dst);
    } else {
        div_err_handle(jit, OperandSize::S64, true, T1, dst);
        emit_ins(jit, RISCVInstruction::div(OperandSize::S64, dst, T1, dst));
    }
}

pub(crate) fn emit_srem_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    load_immediate(jit, T1, insn.imm);
    if size == OperandSize::S32 {
        div_err_handle(jit, OperandSize::S32, true, T1, dst);
        emit_ins(jit, RISCVInstruction::remw(OperandSize::S32, dst, T1, dst));
        zero_extend(jit, dst);
    } else {
        div_err_handle(jit, OperandSize::S64, true, T1, dst);
        emit_ins(jit, RISCVInstruction::rem(OperandSize::S64, dst, T1, dst));
    }
}

pub(crate) fn emit_lmul_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::mulw(OperandSize::S32, dst, src, dst));
        zero_extend(jit, dst);
    } else {
        emit_ins(jit, RISCVInstruction::mul(OperandSize::S64, dst, src, dst));
    }
}

pub(crate) fn emit_uhlmul_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    emit_ins(jit, RISCVInstruction::mulhu(OperandSize::S64, dst, src, dst));
}

pub(crate) fn emit_shlmul_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    emit_ins(jit, RISCVInstruction::mulh(OperandSize::S64, dst, src, dst));
}

pub(crate) fn emit_udiv_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        div_err_handle(jit, OperandSize::S32, false, src, dst);
        emit_ins(jit, RISCVInstruction::divuw(OperandSize::S32, dst, src, dst));
        zero_extend(jit, dst);
    } else {
        div_err_handle(jit, OperandSize::S64, false, src, dst);
        emit_ins(jit, RISCVInstruction::divu(OperandSize::S64, dst, src, dst));
    }
}

pub(crate) fn emit_urem_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        div_err_handle(jit, OperandSize::S32, false, src, dst);
        emit_ins(jit, RISCVInstruction::remuw(OperandSize::S32, dst, src, dst));
    } else {
        div_err_handle(jit, OperandSize::S64, false, src, dst);
        emit_ins(jit, RISCVInstruction::remu(OperandSize::S64, dst, src, dst));
    }
}

pub(crate) fn emit_sdiv_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        div_err_handle(jit, OperandSize::S32, true, src, dst);
        emit_ins(jit, RISCVInstruction::divw(OperandSize::S32, dst, src, dst));
        zero_extend(jit, dst);
    } else {
        div_err_handle(jit, OperandSize::S64, true, src, dst);
        emit_ins(jit, RISCVInstruction::div(OperandSize::S64, dst, src, dst));
    }
}

pub(crate) fn emit_srem_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        div_err_handle(jit, OperandSize::S32, true, src, dst);
        emit_ins(jit, RISCVInstruction::remw(OperandSize::S32, dst, src, dst));
        zero_extend(jit, dst);
    } else {
        div_err_handle(jit, OperandSize::S64, true, src, dst);
        emit_ins(jit, RISCVInstruction::rem(OperandSize::S64, dst, src, dst));
    }
}

pub(crate) fn div_err_handle<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, signed: bool, src: u8, dst: u8) {
    emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, src, T6));
    if size == OperandSize::S32 {
        zero_extend(jit, T6);
    }

    // Prevent division by zero
    load_immediate(jit, REGISTER_SCRATCH, jit.pc as i64);// Save pc
    emit_ins(jit, RISCVInstruction::beq(OperandSize::S64, T6, ZERO, jit.relative_to_anchor(ANCHOR_DIV_BY_ZERO, 0) as i64));

    // Signed division overflows with MIN / -1.
    // If we have an immediate and it's not -1, we can skip the following check.
    if signed  == true {
        load_immediate(jit, T4, if let OperandSize::S64 = size { i64::MIN } else { i32::MIN as u32 as i64 });
        emit_ins(jit, RISCVInstruction::sltu(OperandSize::S64, dst, T4, T4));// if (dst < T4) ? 1 : 0 只有dst等于最小值时，结果为0
        emit_ins(jit, RISCVInstruction::sltiu(OperandSize::S64, T4, 1, T4));// if (T4 < 1) ? 1 : 0

        // The exception case is: dst == MIN && src == -1
        // Via De Morgan's law becomes: !(dst != MIN || src != -1)
        // Also, we know that src != 0 in here, so we can use it to set REGISTER_SCRATCH to something not zero
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, src, 1, T5));
        emit_ins(jit, RISCVInstruction::sltiu(OperandSize::S64, T5, 1, T5));

        // MIN / -1, raise EbpfError::DivideOverflow
        emit_ins(jit, RISCVInstruction::and(OperandSize::S64, T5, T4, T5));
        load_immediate(jit, REGISTER_SCRATCH, jit.pc as i64);
        emit_ins(jit, RISCVInstruction::bne(OperandSize::S64, T5, ZERO, jit.relative_to_anchor(ANCHOR_DIV_OVERFLOW, 0) as i64));
    }
}

/// Determine the offset and execute the load instruction
pub(crate) fn load<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    source1: u8,
    offset:i64,
    destination: u8,
) {
    if offset >= -2048 && offset <= 2047 {
        // offset in 12-bit range, use ld
        emit_ins(jit, RISCVInstruction::load(size, source1, offset, destination));
    } else {
        load_immediate(jit, T1, offset);
        emit_ins(jit, RISCVInstruction::add(size, source1, T1, T1));
        emit_ins(jit, RISCVInstruction::load(size, T1, 0, destination));
    }
}

/// Determine the offset and execute the store instruction
pub(crate) fn store<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    source1: u8,
    source2: u8,
    offset:i64,
) {
    if offset >= -2048 && offset <= 2047 {
        // offset in 12-bit range, use sd
        emit_ins(jit, RISCVInstruction::store(size, source1, source2, offset));
    } else {
        load_immediate(jit, T1, offset);
        emit_ins(jit, RISCVInstruction::add(size, source1, T1, T1));
        emit_ins(jit, RISCVInstruction::store(size, T1, source2, 0));
    }
}

pub(crate) fn emit_jeq_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_imm(jit, size, imm, dst, target_pc);
    emit_ins(jit, RISCVInstruction::beq(OperandSize::S64, T1, dst, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jeq_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_reg(jit, size, src, dst, target_pc);
    emit_ins(jit, RISCVInstruction::beq(OperandSize::S64, src, dst, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jgt_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_imm(jit, size, imm, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bltu(OperandSize::S64, T1, dst, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jgt_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_reg(jit, size, src, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bltu(OperandSize::S64, src, dst, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}
/////////
pub(crate) fn emit_jge_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_imm(jit, size, imm, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bgeu(OperandSize::S64, dst, T1, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jge_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_reg(jit, size, src, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bgeu(OperandSize::S64, dst, src, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jlt_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_imm(jit, size, imm, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bltu(OperandSize::S64, dst, T1, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jlt_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_reg(jit, size, src, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bltu(OperandSize::S64, dst, src, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jle_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_imm(jit, size, imm, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bgeu(OperandSize::S64, T1, dst, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jle_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_reg(jit, size, src, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bgeu(OperandSize::S64, src, dst, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jset_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    jit.emit_validate_and_profile_instruction_count(Some(target_pc));
    if size == OperandSize::S32 {
        load_immediate(jit, T1, imm as u32 as i64);
        zero_extend(jit, dst);
    } else {
        load_immediate(jit, T1, imm);
    }
    load_immediate(jit, REGISTER_SCRATCH, target_pc as i64);
    emit_ins(jit, RISCVInstruction::and(OperandSize::S64, T1, dst, T1));
    let jump_offset = jit.relative_to_target_pc(target_pc, 0);
    emit_ins(jit, RISCVInstruction::bne(OperandSize::S64, T1, ZERO, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jset_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    jit.emit_validate_and_profile_instruction_count(Some(target_pc));
    if size == OperandSize::S32 {
        zero_extend(jit, src);
        zero_extend(jit, dst);
    }
    load_immediate(jit, REGISTER_SCRATCH, target_pc as i64);
    emit_ins(jit, RISCVInstruction::and(OperandSize::S64, src, dst, T1));
    let jump_offset = jit.relative_to_target_pc(target_pc, 0);
    emit_ins(jit, RISCVInstruction::bne(OperandSize::S64, T1, ZERO, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jne_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_imm(jit, size, imm, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bne(OperandSize::S64, T1, dst, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jne_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_reg(jit, size, src, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bne(OperandSize::S64, src, dst, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jsgt_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_signed_imm(jit, size, imm, dst, target_pc);
    emit_ins(jit, RISCVInstruction::blt(OperandSize::S64, T1, dst, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jsgt_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_signed_reg(jit, size, src, dst, target_pc);
    emit_ins(jit, RISCVInstruction::blt(OperandSize::S64, src, dst, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jsge_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_signed_imm(jit, size, imm, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bge(OperandSize::S64, dst, T1, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jsge_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_signed_reg(jit, size, src, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bge(OperandSize::S64, dst, src, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jslt_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_signed_imm(jit, size, imm, dst, target_pc);
    emit_ins(jit, RISCVInstruction::blt(OperandSize::S64, dst, T1, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jslt_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_signed_reg(jit, size, src, dst, target_pc);
    emit_ins(jit, RISCVInstruction::blt(OperandSize::S64, dst, src, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jsle_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_signed_imm(jit, size, imm, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bge(OperandSize::S64, T1, dst, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_jsle_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    let jump_offset = prepare_conditional_branch_signed_reg(jit, size, src, dst, target_pc);
    emit_ins(jit, RISCVInstruction::bge(OperandSize::S64, src, dst, jump_offset as i64));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_ja<C: ContextObject>(jit: &mut JitCompiler<C>, target_pc: usize) {
    jit.emit_validate_and_profile_instruction_count(Some(target_pc));
    load_immediate(jit, REGISTER_SCRATCH, target_pc as i64);
    let jump_offset = jit.relative_to_target_pc(target_pc, 0);
    emit_ins(jit, RISCVInstruction::jal(jump_offset as i64, ZERO));
}

#[inline(always)]
fn prepare_conditional_branch_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) -> i64 {
    jit.emit_validate_and_profile_instruction_count(Some(target_pc));
    if size == OperandSize::S32 {
        load_immediate(jit, T1, imm as u32 as i64);
        zero_extend(jit, dst);
    } else {
        load_immediate(jit, T1, imm);
    }
    load_immediate(jit, REGISTER_SCRATCH, target_pc as i64);
    jit.relative_to_target_pc(target_pc, 0) as i64
}

#[inline(always)]
fn prepare_conditional_branch_reg<C: ContextObject>(jit: &mut JitCompiler<C>,size: OperandSize,src: u8,dst: u8,target_pc: usize) -> i64 {
    jit.emit_validate_and_profile_instruction_count(Some(target_pc));
    if size == OperandSize::S32 {
        zero_extend(jit, src);
        zero_extend(jit, dst);
    }
    load_immediate(jit, REGISTER_SCRATCH, target_pc as i64);
    jit.relative_to_target_pc(target_pc, 0) as i64
}

#[inline(always)]
fn prepare_conditional_branch_signed_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) -> i64 {
    jit.emit_validate_and_profile_instruction_count(Some(target_pc));
    if size == OperandSize::S32 {
        load_immediate(jit, T1, imm as i32 as i64);
        zero_extend(jit, dst);
        emit_ins(jit, RISCVInstruction::addiw(OperandSize::S64, dst, 0, dst));
    } else {
        load_immediate(jit, T1, imm);
    }
    load_immediate(jit, REGISTER_SCRATCH, target_pc as i64);
    jit.relative_to_target_pc(target_pc, 0) as i64
}

#[inline(always)]
fn prepare_conditional_branch_signed_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) -> i64 {
    jit.emit_validate_and_profile_instruction_count(Some(target_pc));
    if size == OperandSize::S32 {
        emit_ins(jit, RISCVInstruction::addiw(OperandSize::S64, dst, 0, dst));
        emit_ins(jit, RISCVInstruction::addiw(OperandSize::S64, src, 0, src));
    }
    load_immediate(jit, REGISTER_SCRATCH, target_pc as i64);
    jit.relative_to_target_pc(target_pc, 0) as i64
}

pub(crate) fn emit_call_imm<C: ContextObject>(jit: &mut JitCompiler<C>, insn: Insn){
    // For JIT, external functions MUST be registered at compile time.
    let mut resolved = false;

    // External syscall
    if !jit.executable.get_sbpf_version().static_syscalls() || insn.src == 0 {
        if let Some((_, function)) =
                jit.executable.get_loader().get_function_registry().lookup_by_key(insn.imm as u32) {
            jit.emit_validate_and_profile_instruction_count(Some(0));
            load_immediate(jit, REGISTER_SCRATCH, function as usize as i64);
            emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
            store(jit, OperandSize::S64, SP, RA, 0);
            emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_EXTERNAL_FUNCTION_CALL, 0) as i64, RA));
            load(jit, OperandSize::S64, SP, 0, RA);
            emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
            emit_undo_profile_instruction_count(jit, 0);
            resolved = true;
        }
    }
    // Internal call
    if jit.executable.get_sbpf_version().static_syscalls() {
        let target_pc = (jit.pc as i64).saturating_add(insn.imm).saturating_add(1);
        if ebpf::is_pc_in_program(jit.program, target_pc as usize) && insn.src == 1 {
            emit_internal_call(jit, Value::Constant64(target_pc as i64, true));
            resolved = true;
        }
    } else if let Some((_function_name, target_pc)) =
        jit.executable
            .get_function_registry()
            .lookup_by_key(insn.imm as u32) {
        emit_internal_call(jit, Value::Constant64(target_pc as i64, true));
        resolved = true;
    }
    if !resolved {
        load_immediate(jit, REGISTER_SCRATCH, jit.pc as i64);
        emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_CALL_UNSUPPORTED_INSTRUCTION, 0) as i64, ZERO));
    }
}

pub(crate) fn emit_internal_call<C: ContextObject>(jit: &mut JitCompiler<C>, dst: Value) {
    // Store PC in case the bounds check fails
    load_immediate(jit, REGISTER_SCRATCH, jit.pc as i64);
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
    store(jit, OperandSize::S64, SP, RA, 0);
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_INTERNAL_FUNCTION_CALL_PROLOGUE, 0) as i64, RA));
    load(jit, OperandSize::S64, SP, 0, RA);
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));

    match dst {
        Value::Register(reg) => {
            // REGISTER_SCRATCH contains self.pc, and we must store it for proper error handling.
            // We can discard the value if callx succeeds, so we are not incrementing the stack pointer (RSP).
            store(jit, OperandSize::S64, SP, REGISTER_SCRATCH, -24); 
            // Move guest_target_address into REGISTER_SCRATCH
            emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, reg, REGISTER_SCRATCH));
            
            emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
            store(jit, OperandSize::S64, SP, RA, 0);
            emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_INTERNAL_FUNCTION_CALL_REG, 0) as i64, RA));
            load(jit, OperandSize::S64, SP, 0, RA);
            emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
        },
        Value::Constant64(target_pc, user_provided) => {
            debug_assert!(user_provided);
            emit_profile_instruction_count(jit, Some(target_pc as usize));
            if user_provided && jit.should_sanitize_constant(target_pc) {
                emit_sanitized_load_immediate(jit, REGISTER_SCRATCH, target_pc);
            } else {
                load_immediate(jit, REGISTER_SCRATCH, target_pc);
            }
            
            emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
            store(jit, OperandSize::S64, SP, RA, 0);
            let jump_offset = jit.relative_to_target_pc(target_pc as usize, 0) as i64;
            emit_ins(jit, RISCVInstruction::jal(jump_offset, RA));
            load(jit, OperandSize::S64, SP, 0, RA);
            emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
        },
        _ => {
            #[cfg(debug_assertions)]
            unreachable!();
        }
    }

    emit_undo_profile_instruction_count(jit, 0);

    // Restore the previous frame pointer
    load(jit, OperandSize::S64, SP, 0, REGISTER_MAP[FRAME_PTR_REG]);
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP)); 
    let mut current_offset = 0;
    for reg in REGISTER_MAP.iter().skip(FIRST_SCRATCH_REG).take(SCRATCH_REGS).rev() {
        load(jit, OperandSize::S64, SP, current_offset, *reg);
        current_offset += 8;
    }
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, current_offset, SP));
}

pub(crate) fn emit_exit<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    let call_depth_access = jit.slot_in_vm(RuntimeEnvironmentSlot::CallDepth) as i64;
    load(jit, OperandSize::S64, REGISTER_PTR_TO_VM, call_depth_access, T5);

    // If CallDepth == 0, we've reached the exit instruction of the entry point
    emit_ins(jit, RISCVInstruction::bne(OperandSize::S64, T5, ZERO, 8)); // if call_depth != 0, jump over next instruction
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_EXIT, 0) as i64, ZERO)); // jump to exit
    // we're done

    // else decrement and update CallDepth
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, T5, -1, T5)); // env.call_depth -= 1;
    store(jit, OperandSize::S64, REGISTER_PTR_TO_VM, T5, call_depth_access);

    // and return
    emit_ins(jit, RISCVInstruction::return_near());
}

pub(crate) fn emit_address_translation<C: ContextObject>(jit: &mut JitCompiler<C>, dst: Option<u8>, vm_addr: Value, len: u64, value: Option<Value>) {
    debug_assert_ne!(dst.is_some(), value.is_some());

    let stack_slot_of_value_to_store = -96; 
    match value {
        Some(Value::Register(reg)) => {
            store(jit, OperandSize::S64, SP, reg, stack_slot_of_value_to_store);
        }
        Some(Value::Constant64(constant, user_provided)) => {
            debug_assert!(user_provided);
            // First half of emit_sanitized_load_immediate(stack_slot_of_value_to_store, constant)
            let lower_key = jit.immediate_value_key as i32 as i64;
            load_immediate(jit, REGISTER_SCRATCH, constant.wrapping_sub(lower_key));
            store(jit, OperandSize::S64, SP, REGISTER_SCRATCH, stack_slot_of_value_to_store);
        }
        _ => {}
    }

    match vm_addr {
        Value::RegisterPlusConstant64(reg, constant, user_provided) => {
            if user_provided && jit.should_sanitize_constant(constant) {
                emit_sanitized_load_immediate(jit, REGISTER_SCRATCH, constant);
            } else {
                load_immediate(jit, REGISTER_SCRATCH, constant);
            }
            emit_ins(jit, RISCVInstruction::add(OperandSize::S64, reg, REGISTER_SCRATCH, REGISTER_SCRATCH));
        },
        _ => {
            #[cfg(debug_assertions)]
            unreachable!();
        },
    }
    if jit.config.enable_address_translation {
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
        store(jit, OperandSize::S64, SP, RA, 0);
        let anchor_base = match value {
            Some(Value::Register(_reg)) => 4,
            Some(Value::Constant64(_constant, _user_provided)) => 8,
            _ => 0,
        };
        let anchor = ANCHOR_TRANSLATE_MEMORY_ADDRESS + anchor_base + len.trailing_zeros() as usize;
        load_immediate(jit, T1, jit.pc as i64);
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
        store(jit, OperandSize::S64, SP, T1, 0);
        
        emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(anchor, 0) as i64, RA));
        load(jit, OperandSize::S64, SP, 0, RA);
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
        if let Some(dst) = dst {
            emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, REGISTER_SCRATCH, dst));
        }
    } else if let Some(dst) = dst {
        match len {
            1 => load(jit, OperandSize::S8, REGISTER_SCRATCH, 0, dst),
            2 => load(jit, OperandSize::S16, REGISTER_SCRATCH, 0,dst),
            4 => load(jit, OperandSize::S32, REGISTER_SCRATCH, 0,dst),
            8 => load(jit, OperandSize::S64, REGISTER_SCRATCH, 0,dst),
            _ => unreachable!(),
        }
    } else {
        // Save REGISTER_MAP[0] and retrieve value to store
        load(jit, OperandSize::S64, SP, stack_slot_of_value_to_store, T5);
        store(jit, OperandSize::S64, SP, T5, stack_slot_of_value_to_store);
        emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, T5, REGISTER_MAP[0]));
        match len {
            1 => store(jit, OperandSize::S8, REGISTER_MAP[0], REGISTER_SCRATCH, 0),
            2 => store(jit, OperandSize::S16, REGISTER_MAP[0], REGISTER_SCRATCH, 0),
            4 => store(jit, OperandSize::S32, REGISTER_MAP[0], REGISTER_SCRATCH, 0),
            8 => store(jit, OperandSize::S64, REGISTER_MAP[0], REGISTER_SCRATCH, 0),
            _ => unreachable!(),
        }
        // Restore REGISTER_MAP[0]
        load(jit, OperandSize::S64, SP, stack_slot_of_value_to_store, T5);
        store(jit, OperandSize::S64, SP, T5, stack_slot_of_value_to_store);
        emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, T5, REGISTER_MAP[0]));
    }
}

pub(crate) fn emit_validate_instruction_count<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    pc: Option<usize>,
) {
    if !jit.config.enable_instruction_meter {
        return;
    }
    // Update `MACHINE_CODE_PER_INSTRUCTION_METER_CHECKPOINT` if you change the code generation here
    if let Some(pc) = pc {
        jit.last_instruction_meter_validation_pc = pc;
        emit_sanitized_load_immediate(jit,REGISTER_SCRATCH, pc as i64);
    }
    // If instruction_meter >= pc, throw ExceededMaxInstructions
    emit_ins(jit, RISCVInstruction::bgeu(OperandSize::S64, REGISTER_SCRATCH, REGISTER_INSTRUCTION_METER, jit.relative_to_anchor(ANCHOR_THROW_EXCEEDED_MAX_INSTRUCTIONS, 0) as i64));
}

pub(crate) fn emit_profile_instruction_count<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    target_pc: Option<usize>,
) {
    if !jit.config.enable_instruction_meter {
        return;
    }
    match target_pc {
        Some(target_pc) => {
            let immediate = target_pc as i64 - jit.pc as i64 - 1;
            emit_sanitized_add(jit, OperandSize::S64, REGISTER_INSTRUCTION_METER, immediate); // instruction_meter += target_pc - (self.pc + 1);
        },
        None => {
            emit_ins(jit, RISCVInstruction::add(OperandSize::S64, REGISTER_SCRATCH, REGISTER_INSTRUCTION_METER, REGISTER_INSTRUCTION_METER)); // instruction_meter += target_pc;
            let immediate = -(jit.pc as i64 + 1);
            load_immediate(jit, T1, immediate);
            emit_ins(jit, RISCVInstruction::add(OperandSize::S64, REGISTER_INSTRUCTION_METER, T1, REGISTER_INSTRUCTION_METER)); // instruction_meter -= self.pc + 1;
        }
    }
}

pub(crate) fn emit_undo_profile_instruction_count<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    target_pc: usize,
) {
    if jit.config.enable_instruction_meter {
        let immediate = jit.pc as i64 + 1 - target_pc as i64;
        emit_sanitized_add(jit, OperandSize::S64, REGISTER_INSTRUCTION_METER, immediate); // instruction_meter += (self.pc + 1) - target_pc;
    }
}

pub(crate) fn emit_set_exception_kind<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    err: EbpfError,
) {
    let err_kind = unsafe { *std::ptr::addr_of!(err).cast::<u64>() };
    let err_discriminant = ProgramResult::Err(err).discriminant();
    load_immediate(jit, T1, jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult) as i64);
    emit_ins(jit, RISCVInstruction::add(OperandSize::S64, REGISTER_PTR_TO_VM, T1, REGISTER_MAP[0]));
    // result.discriminant = err_discriminant;
    load_immediate(jit, T1, err_discriminant as i64);
    store(jit, OperandSize::S64, REGISTER_MAP[0], T1, 0);
    // err.kind = err_kind;
    load_immediate(jit, T1, err_kind as i64);
    store(jit, OperandSize::S64, REGISTER_MAP[0], T1, std::mem::size_of::<u64>() as i64);
}

pub(crate) fn emit_execution_overrun_trailer<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    load_immediate(jit, REGISTER_SCRATCH, jit.pc as i64); // Save pc
    emit_set_exception_kind(jit, EbpfError::ExecutionOverrun);
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 0) as i64, ZERO));
}

pub(crate) fn emit_throw_exception<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 0) as i64, ZERO));
}

pub(crate) fn emit_subroutines<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    // Routine for instruction tracing
    if jit.config.enable_register_tracing {
        jit.set_anchor(ANCHOR_TRACE);
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
        emit_ins(jit, RISCVInstruction::store(OperandSize::S64, SP, REGISTER_SCRATCH, 0));
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, REGISTER_MAP.len() as i64 * (-8), SP));
        let mut current_offset = 0;
        for reg in REGISTER_MAP.iter() { // here the data is stored from lower addresses to higher addresses. Therefore, there is no need for .rev()
            store(jit, OperandSize::S64, SP, *reg, current_offset);
            current_offset += 8;
        }
        emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, SP, REGISTER_MAP[0]));
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8 * 3, SP));
        emit_rust_call(jit, Value::Constant64(Vec::<crate::static_analysis::RegisterTraceEntry>::push as *const u8 as i64, false), &[
            Argument { index: 1, value: Value::Register(REGISTER_MAP[0]) }, // registers
            Argument { index: 0, value: Value::RegisterPlusConstant32(REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::RegisterTrace), false) },
        ], None); 
        // Pop stack and return
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8 * 3, SP)); // RSP += 8 * 3;
        load(jit, OperandSize::S64, SP, 0, REGISTER_MAP[0]);
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8 * (REGISTER_MAP.len() - 1) as i64, SP));
        load(jit, OperandSize::S64, SP, 0, REGISTER_SCRATCH);
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
        emit_ins(jit, RISCVInstruction::return_near());
    }

    // Epilogue
    jit.set_anchor(ANCHOR_EPILOGUE);
    if jit.config.enable_instruction_meter {
        // REGISTER_INSTRUCTION_METER -= 1;
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, REGISTER_INSTRUCTION_METER, -1, REGISTER_INSTRUCTION_METER));
        // REGISTER_INSTRUCTION_METER -= pc;
        emit_ins(jit, RISCVInstruction::sub(OperandSize::S64, REGISTER_INSTRUCTION_METER, REGISTER_SCRATCH, REGISTER_INSTRUCTION_METER)); 
        // REGISTER_INSTRUCTION_METER -= *PreviousInstructionMeter;
        load(jit, OperandSize::S64, REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::PreviousInstructionMeter) as i64, T5);
        emit_ins(jit, RISCVInstruction::sub(OperandSize::S64, REGISTER_INSTRUCTION_METER, T5, REGISTER_INSTRUCTION_METER));
        // REGISTER_INSTRUCTION_METER = -REGISTER_INSTRUCTION_METER;
        emit_ins(jit, RISCVInstruction::sub(OperandSize::S64, ZERO, REGISTER_INSTRUCTION_METER, REGISTER_INSTRUCTION_METER));
        // *DueInsnCount = REGISTER_INSTRUCTION_METER;
        store(jit, OperandSize::S64, REGISTER_PTR_TO_VM, REGISTER_INSTRUCTION_METER, jit.slot_in_vm(RuntimeEnvironmentSlot::DueInsnCount) as i64);
    }

    // Restore stack pointer in case we did not exit gracefully
    load(jit, OperandSize::S64, REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::HostStackPointer) as i64, SP);
    emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, S6, RA)); // RA = REGISTER_SCRATCH
    emit_ins(jit, RISCVInstruction::return_near());
    
    // Handler for EbpfError::ExceededMaxInstructions
    jit.set_anchor(ANCHOR_THROW_EXCEEDED_MAX_INSTRUCTIONS);
    emit_set_exception_kind(jit,EbpfError::ExceededMaxInstructions);
    emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, REGISTER_INSTRUCTION_METER, REGISTER_SCRATCH)); // REGISTER_SCRATCH = REGISTER_INSTRUCTION_METER;
    // Fall through

    // Epilogue for errors
    jit.set_anchor(ANCHOR_THROW_EXCEPTION_UNCHECKED);
    store(jit, OperandSize::S64, REGISTER_PTR_TO_VM, REGISTER_SCRATCH, (jit.slot_in_vm(RuntimeEnvironmentSlot::Registers) + 11 * std::mem::size_of::<u64>() as i32) as i64); // registers[11] = pc;
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_EPILOGUE, 0) as i64, ZERO));

    // Quit gracefully
    jit.set_anchor(ANCHOR_EXIT); 
    if jit.config.enable_instruction_meter {
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, REGISTER_INSTRUCTION_METER, 1, REGISTER_INSTRUCTION_METER)); // REGISTER_INSTRUCTION_METER += 1;
    }
    load_immediate(jit, T1, jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult) as i64);
    emit_ins(jit, RISCVInstruction::add(OperandSize::S64, REGISTER_PTR_TO_VM, T1, REGISTER_SCRATCH));
    store(jit, OperandSize::S64, REGISTER_SCRATCH, REGISTER_MAP[0], std::mem::size_of::<u64>() as i64);
    emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, ZERO,  REGISTER_SCRATCH)); // REGISTER_SCRATCH ^= REGISTER_SCRATCH; // REGISTER_SCRATCH = 0;
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_EPILOGUE, 0) as i64, ZERO));

    // Handler for exceptions which report their pc
    jit.set_anchor(ANCHOR_THROW_EXCEPTION);
    // Validate that we did not reach the instruction meter limit before the exception occured
    emit_validate_instruction_count(jit, None);
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION_UNCHECKED, 0) as i64, ZERO));

    // Handler for EbpfError::CallDepthExceeded
    jit.set_anchor(ANCHOR_CALL_DEPTH_EXCEEDED);
    emit_set_exception_kind(jit,EbpfError::CallDepthExceeded);
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 0) as i64, ZERO));
    
    // Handler for EbpfError::CallOutsideTextSegment
    jit.set_anchor(ANCHOR_CALL_REG_OUTSIDE_TEXT_SEGMENT);
    emit_set_exception_kind(jit,EbpfError::CallOutsideTextSegment);
    load(jit, OperandSize::S64, SP, -8, REGISTER_SCRATCH); 
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 0) as i64, ZERO));

    // Handler for EbpfError::DivideByZero
    jit.set_anchor(ANCHOR_DIV_BY_ZERO);
    emit_set_exception_kind(jit,EbpfError::DivideByZero);
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 0) as i64, ZERO));

    // Handler for EbpfError::DivideOverflow
    jit.set_anchor(ANCHOR_DIV_OVERFLOW);
    emit_set_exception_kind(jit,EbpfError::DivideOverflow);
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 0) as i64, ZERO));

    // See `ANCHOR_INTERNAL_FUNCTION_CALL_REG` for more details.
    jit.set_anchor(ANCHOR_CALL_REG_UNSUPPORTED_INSTRUCTION);
    load(jit, OperandSize::S64, SP, -8, REGISTER_SCRATCH); // Retrieve the current program counter from the stack
    load(jit, OperandSize::S64, SP, 0, REGISTER_MAP[0]); // Restore the clobbered REGISTER_MAP[0]
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
    // Fall through

    // Handler for EbpfError::UnsupportedInstruction
    jit.set_anchor(ANCHOR_CALL_UNSUPPORTED_INSTRUCTION);
    if jit.config.enable_register_tracing {
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
        store(jit, OperandSize::S64, SP, RA, 0);
        emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_TRACE, 0) as i64, RA));
        load(jit, OperandSize::S64, SP, 0, RA);
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
    }
    emit_set_exception_kind(jit,EbpfError::UnsupportedInstruction);
    emit_ins(jit, RISCVInstruction::jal(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 0) as i64, ZERO));

    //Routine for external functions
    jit.set_anchor(ANCHOR_EXTERNAL_FUNCTION_CALL);
    load_immediate(jit, T1, -1);
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
    store(jit, OperandSize::S64, SP, T1, 0);// Used as PC value in error case, acts as stack padding otherwise
    if jit.config.enable_instruction_meter {
        store(jit, OperandSize::S64, REGISTER_PTR_TO_VM, REGISTER_INSTRUCTION_METER, jit.slot_in_vm(RuntimeEnvironmentSlot::DueInsnCount) as i64); // *DueInsnCount = REGISTER_INSTRUCTION_METER;
    }
    emit_rust_call(jit,Value::Register(REGISTER_SCRATCH), &[
        Argument { index: 5, value: Value::Register(ARGUMENT_REGISTERS[5]) },
        Argument { index: 4, value: Value::Register(ARGUMENT_REGISTERS[4]) },
        Argument { index: 3, value: Value::Register(ARGUMENT_REGISTERS[3]) },
        Argument { index: 2, value: Value::Register(ARGUMENT_REGISTERS[2]) },
        Argument { index: 1, value: Value::Register(ARGUMENT_REGISTERS[1]) },
        Argument { index: 0, value: Value::Register(REGISTER_PTR_TO_VM) },
    ], None);
    if jit.config.enable_instruction_meter {
        load(jit, OperandSize::S64, REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::PreviousInstructionMeter) as i64, REGISTER_INSTRUCTION_METER); // REGISTER_INSTRUCTION_METER = *PreviousInstructionMeter;
    }

    //Test if result indicates that an error occured
    // self.emit_result_is_err(REGISTER_SCRATCH);
    let ok = ProgramResult::Ok(0);
    let ok_discriminant = ok.discriminant();
    load_immediate(jit, T1, jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult) as i64);
    emit_ins(jit, RISCVInstruction::add(OperandSize::S64, REGISTER_PTR_TO_VM, T1, T5));
    // load(jit, OperandSize::S64, REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult) as i64, T5);
    
    load_immediate(jit, T1, ok_discriminant as i64);
    load(jit, OperandSize::S64, SP, 0, REGISTER_SCRATCH);
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
    load(jit, OperandSize::S64, T5, 0, T4);
    emit_ins(jit, RISCVInstruction::bne(OperandSize::S32, T1, T4, jit.relative_to_anchor(ANCHOR_EPILOGUE, 0) as i64));
    // Store Ok value in result register
    load_immediate(jit, T1, jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult) as i64);
    emit_ins(jit, RISCVInstruction::add(OperandSize::S64, REGISTER_PTR_TO_VM, T1, REGISTER_SCRATCH));
    // load(jit, OperandSize::S64, REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult) as i64, REGISTER_SCRATCH);
    load(jit, OperandSize::S64, REGISTER_SCRATCH, 8, REGISTER_MAP[0]);
    emit_ins(jit, RISCVInstruction::return_near());

    // Routine for prologue of emit_internal_call()
    jit.set_anchor(ANCHOR_INTERNAL_FUNCTION_CALL_PROLOGUE);
    emit_validate_instruction_count(jit, None);
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8 * (SCRATCH_REGS + 1) as i64, SP));
    store(jit, OperandSize::S64, SP, REGISTER_SCRATCH,  0); // Save original REGISTER_SCRATCH
    load(jit, OperandSize::S64, SP, 8 * (SCRATCH_REGS + 1) as i64, REGISTER_SCRATCH); // Load return address
    for (i, reg) in REGISTER_MAP.iter().skip(FIRST_SCRATCH_REG).take(SCRATCH_REGS).enumerate() {
        store(jit, OperandSize::S64, SP, *reg,  8 * (SCRATCH_REGS - i + 1) as i64); // Push SCRATCH_REG
    }
    // Push the caller's frame pointer. The code to restore it is emitted at the end of emit_internal_call().
    store(jit, OperandSize::S64, SP, REGISTER_MAP[FRAME_PTR_REG],  8);

    // Push return address and restore original REGISTER_SCRATCH
    load(jit, OperandSize::S64, SP, 0, T5);
    store(jit, OperandSize::S64, SP, REGISTER_SCRATCH, 0);
    emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, T5, REGISTER_SCRATCH));

    // Increase env.call_depth
    let call_depth_access = jit.slot_in_vm(RuntimeEnvironmentSlot::CallDepth) as i64;
    load(jit, OperandSize::S64, REGISTER_PTR_TO_VM, call_depth_access, T5);
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, T5, 1, T5)); // env.call_depth += 1;
    store(jit, OperandSize::S64, REGISTER_PTR_TO_VM, T5, call_depth_access);
    // If env.call_depth == self.config.max_call_depth, throw CallDepthExceeded
    load_immediate(jit, T1, jit.config.max_call_depth as i64);
    emit_ins(jit, RISCVInstruction::beq(OperandSize::S64, T5, T1, jit.relative_to_anchor(ANCHOR_CALL_DEPTH_EXCEEDED, 0) as i64));

    // Setup the frame pointer for the new frame. What we do depends on whether we're using dynamic or fixed frames.
    if jit.executable.get_sbpf_version().automatic_stack_frame_bump() {
        // With fixed frames we start the new frame at the next fixed offset
        let stack_frame_size = jit.config.stack_frame_size as i64 * if !jit.executable.get_sbpf_version().manual_stack_frame_bump() && jit.config.enable_stack_frame_gaps { 2 } else { 1 };
        load_immediate(jit, T1, stack_frame_size);
        emit_ins(jit, RISCVInstruction::add(OperandSize::S64, REGISTER_MAP[FRAME_PTR_REG], T1, REGISTER_MAP[FRAME_PTR_REG])); // env.stack_pointer += stack_frame_size;
    }
    // emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
    emit_ins(jit, RISCVInstruction::return_near());

    // Routine for emit_internal_call(Value::Register())
    // Inputs: Guest current pc in X86IndirectAccess::OffsetIndexShift(-16, RSP, 0), Guest target address in REGISTER_SCRATCH
    // Outputs: Guest current pc in X86IndirectAccess::OffsetIndexShift(-16, RSP, 0), Guest target pc in REGISTER_SCRATCH, Host target address in RIP
    jit.set_anchor(ANCHOR_INTERNAL_FUNCTION_CALL_REG);
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
    store(jit, OperandSize::S64, SP, REGISTER_MAP[0], 0);
    // Calculate offset relative to instruction_addresses
    load_immediate(jit, REGISTER_MAP[0], jit.program_vm_addr as i64);
    emit_ins(jit, RISCVInstruction::sub(OperandSize::S64, REGISTER_SCRATCH, REGISTER_MAP[0], REGISTER_SCRATCH)); // guest_target_pc = guest_target_address - self.program_vm_addr;
    // Force alignment of RAX
    load_immediate(jit, T5, !(INSN_SIZE as i64 - 1));
    emit_ins(jit, RISCVInstruction::and(OperandSize::S64, REGISTER_SCRATCH, T5, REGISTER_SCRATCH)); // guest_target_pc &= !(INSN_SIZE - 1);
    // Bound check
    // if(guest_target_pc >= number_of_instructions * INSN_SIZE) throw CALL_OUTSIDE_TEXT_SEGMENT;
    // if(RAX >= number_of_instructions * INSN_SIZE) throw CALL_OUTSIDE_TEXT_SEGMENT;
    let number_of_instructions = jit.result.pc_section.len();
    load_immediate(jit, T1, (number_of_instructions * INSN_SIZE) as i64);
    emit_ins(jit, RISCVInstruction::bgeu(OperandSize::S64, REGISTER_SCRATCH, T1, jit.relative_to_anchor(ANCHOR_CALL_REG_OUTSIDE_TEXT_SEGMENT, 0) as i64));
    // Calculate the target_pc (dst / INSN_SIZE) to update REGISTER_INSTRUCTION_METER
    // and as target pc for potential ANCHOR_CALL_REG_UNSUPPORTED_INSTRUCTION
    let shift_amount = INSN_SIZE.trailing_zeros();
    debug_assert_eq!(INSN_SIZE, 1 << shift_amount);
    emit_ins(jit, RISCVInstruction::srli(OperandSize::S64, REGISTER_SCRATCH, shift_amount as i64, REGISTER_SCRATCH));
    // Load host target_address from self.result.pc_section
    // debug_assert_eq!(INSN_SIZE, 8); // Because the instruction size is also the slot size we do not need to shift the offset
    load_immediate(jit, REGISTER_MAP[0], jit.result.pc_section.as_ptr() as i64); // host_target_address = self.result.pc_section;
    emit_ins(jit, RISCVInstruction::slli(OperandSize::S64, REGISTER_SCRATCH, 2, T5));
    emit_ins(jit, RISCVInstruction::add(OperandSize::S64, REGISTER_MAP[0], T5, T5)); 
    load(jit, OperandSize::S32, T5, 0, REGISTER_MAP[0]); // host_target_address = self.result.pc_section[guest_target_pc];
    // Check destination is valid
    load_immediate(jit, T1, 0x8000_0000);
    emit_ins(jit, RISCVInstruction::and(OperandSize::S64, REGISTER_MAP[0], T1, T1));
    emit_ins(jit, RISCVInstruction::bne(OperandSize::S64, T1, ZERO, jit.relative_to_anchor(ANCHOR_CALL_REG_UNSUPPORTED_INSTRUCTION, 0) as i64));
    load_immediate(jit, T1, 0x8000_0000);
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, T1, -1, T1));
    emit_ins(jit, RISCVInstruction::and(OperandSize::S64, REGISTER_MAP[0], T1, REGISTER_MAP[0]));
    // A version of `self.emit_profile_instruction_count(None);` which reads self.pc from the stack
    load(jit, OperandSize::S64, SP, -8, T5); // Load guest_current_pc
    emit_ins(jit, RISCVInstruction::sub(OperandSize::S64, REGISTER_INSTRUCTION_METER, T5, REGISTER_INSTRUCTION_METER)); // instruction_meter -= guest_current_pc;
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, REGISTER_INSTRUCTION_METER, -1, REGISTER_INSTRUCTION_METER)); // instruction_meter -= 1;
    emit_ins(jit, RISCVInstruction::add(OperandSize::S64, REGISTER_INSTRUCTION_METER, REGISTER_SCRATCH, REGISTER_INSTRUCTION_METER)); // instruction_meter += guest_target_pc;
    // Offset host_target_address by self.result.text_section
    emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, REGISTER_SCRATCH, T5));
    load_immediate(jit, REGISTER_SCRATCH, jit.result.text_section.as_ptr() as i64); // REGISTER_SCRATCH = self.result.text_section;
    emit_ins(jit, RISCVInstruction::add(OperandSize::S64, REGISTER_MAP[0], REGISTER_SCRATCH, REGISTER_MAP[0])); // host_target_address += self.result.text_section;
    emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, T5, REGISTER_SCRATCH));
    // Restore the clobbered REGISTER_MAP[0]
    load(jit, OperandSize::S64, SP, 0, T5);
    store(jit, OperandSize::S64, SP, REGISTER_MAP[0], 0);
    emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, REGISTER_MAP[0], T6)); // save host_target_address in T6
    emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, T5, REGISTER_MAP[0]));

    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
    emit_ins(jit, RISCVInstruction::jalr(T6, 0, ZERO)); // Tail call to host_target_address

    // Translates a vm memory address to a host memory address
    let lower_key = jit.immediate_value_key as i32 as i64;
    for (anchor_base, len) in &[
        (0, 1i32), (0, 2i32), (0, 4i32), (0, 8i32),
        (4, 1i32), (4, 2i32), (4, 4i32), (4, 8i32),
        (8, 1i32), (8, 2i32), (8, 4i32), (8, 8i32),
    ] {
        let target_offset = *anchor_base + len.trailing_zeros() as usize;
        jit.set_anchor(ANCHOR_TRANSLATE_MEMORY_ADDRESS + target_offset);
        // call MemoryMapping::(load|store) storing the result in RuntimeEnvironmentSlot::ProgramResult
        if *anchor_base == 0 {
            let load = match len {
                1 => MemoryMapping::load::<u8> as *const u8 as i64,
                2 => MemoryMapping::load::<u16> as *const u8 as i64,
                4 => MemoryMapping::load::<u32> as *const u8 as i64,
                8 => MemoryMapping::load::<u64> as *const u8 as i64,
                _ => unreachable!()
            };
            emit_rust_call(jit, Value::Constant64(load, false), &[
                Argument { index: 2, value: Value::Register(REGISTER_SCRATCH) }, // Specify first as the src register could be overwritten by other arguments
                Argument { index: 3, value: Value::Constant64(0, false) }, // self.pc is set later
                Argument { index: 1, value: Value::RegisterPlusConstant32(REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::MemoryMapping), false) },
                Argument { index: 0, value: Value::RegisterPlusConstant32(REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult), false) },
            ], None);
        } else {
            if *anchor_base == 8 {
                // Second half of emit_sanitized_load_immediate(stack_slot_of_value_to_store, constant)
                load(jit, OperandSize::S64, SP, -80, T5);
                load_immediate(jit, T1, lower_key);
                emit_ins(jit, RISCVInstruction::add(OperandSize::S64, T5, T1, T5));
                store(jit, OperandSize::S64, SP, T5, -80);
            }
            let store = match len {
                1 => MemoryMapping::store::<u8> as *const u8 as i64,
                2 => MemoryMapping::store::<u16> as *const u8 as i64,
                4 => MemoryMapping::store::<u32> as *const u8 as i64,
                8 => MemoryMapping::store::<u64> as *const u8 as i64,
                _ => unreachable!()
            };
            emit_rust_call(jit, Value::Constant64(store, false), &[
                Argument { index: 3, value: Value::Register(REGISTER_SCRATCH) }, // Specify first as the src register could be overwritten by other arguments
                Argument { index: 2, value: Value::RegisterIndirect(SP, -8, false) },
                Argument { index: 4, value: Value::Constant64(0, false) }, // self.pc is set later
                Argument { index: 1, value: Value::RegisterPlusConstant32(REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::MemoryMapping), false) },
                Argument { index: 0, value: Value::RegisterPlusConstant32(REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult), false) },
            ], None);
        }

        // Throw error if the result indicates one
        // self.emit_result_is_err(REGISTER_SCRATCH);
        let ok = ProgramResult::Ok(0);
        let ok_discriminant = ok.discriminant();
        load_immediate(jit, T1, jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult) as i64);
        emit_ins(jit, RISCVInstruction::add(OperandSize::S64, REGISTER_PTR_TO_VM, T1, T5));
        
        load_immediate(jit, T1, ok_discriminant as i64);
        load(jit, OperandSize::S64, SP, 0, REGISTER_SCRATCH); // REGISTER_SCRATCH = self.pc
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));

        load(jit, OperandSize::S64, T5, 0, T4);
        emit_ins(jit, RISCVInstruction::bne(OperandSize::S32, T1, T4, jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 0) as i64));

        if *anchor_base == 0 { // AccessType::Load
            // unwrap() the result into REGISTER_SCRATCH
            load(jit, OperandSize::S64, REGISTER_PTR_TO_VM, (jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult) + std::mem::size_of::<u64>() as i32) as i64, REGISTER_SCRATCH);
        }
        
        emit_ins(jit, RISCVInstruction::return_near());
    }
}

pub(crate) fn emit_rust_call<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    target: Value,
    arguments: &[Argument], 
    result_reg: Option<u8>,
) {
    let mut saved_registers = CALLER_SAVED_REGISTERS.to_vec();
    if let Some(reg) = result_reg {
        if let Some(dst) = saved_registers.iter().position(|x| *x == reg) {
            saved_registers.remove(dst);
        }
    }

    // Save registers on stack
    let saved_registers_len = saved_registers.len();
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8 * saved_registers_len as i64, SP));
    let mut current_offset = 0;
    for reg in saved_registers.iter() {
        store(jit, OperandSize::S64, SP, *reg, current_offset);
        current_offset += 8;
    }

    let stack_arguments = arguments.len().saturating_sub(ARGUMENT_REGISTERS.len()) as i64;
    if stack_arguments % 2 != 0 {
        // If we're going to pass an odd number of stack args we need to pad
        // to preserve alignment
        emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
    }

    // Pass arguments
    for argument in arguments {
        let is_stack_argument = argument.index >= ARGUMENT_REGISTERS.len();
        let dst = if is_stack_argument {
            SP // Never used
        } else {
            ARGUMENT_REGISTERS[argument.index]
        };
        match argument.value {
            Value::Register(reg) => {
                if is_stack_argument {
                    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
                    store(jit, OperandSize::S64, SP, reg, 0);
                } else if reg != dst {
                    emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, reg, dst));
                }
            },
            Value::RegisterIndirect(reg, offset, user_provided) => {
                debug_assert!(!user_provided);
                if is_stack_argument {
                    debug_assert!(reg != SP);
                    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
                    store(jit, OperandSize::S64, SP, reg, offset as i64);
                } else if reg == SP {
                    load(jit, OperandSize::S64, SP, offset as i64, dst); 
                } else {
                    load(jit, OperandSize::S64, reg, offset as i64, dst);
                }
            },
            Value::RegisterPlusConstant32(reg, offset, user_provided) => {
                debug_assert!(!user_provided);
                if is_stack_argument {
                    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
                    store(jit, OperandSize::S64, SP, reg, 0);
                    load(jit, OperandSize::S64, SP, 0, T5);
                    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, T5, 1, T5));
                    store(jit, OperandSize::S64, SP, T5, 0);
                } else if reg == SP {
                    load_immediate(jit, T1, offset as i64);
                    emit_ins(jit, RISCVInstruction::add(OperandSize::S64, SP, T1, dst));
                } else {
                    load_immediate(jit, T1, offset as i64);
                    emit_ins(jit, RISCVInstruction::add(OperandSize::S64, reg, T1, dst));
                }
            },
            Value::RegisterPlusConstant64(reg, offset, user_provided) => {
                debug_assert!(!user_provided);
                if is_stack_argument {
                    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
                    store(jit, OperandSize::S64, SP, reg, 0);
                    load(jit, OperandSize::S64, SP, 0, T5);
                    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, T5, 1, T5));
                    store(jit, OperandSize::S64, SP, T5, 0);
                } else {
                    load_immediate(jit, T1, offset as i64);
                    emit_ins(jit, RISCVInstruction::add(OperandSize::S64, reg, T1, dst));
                }
            },
            Value::Constant64(value, user_provided) => {
                debug_assert!(!user_provided && !is_stack_argument);
                load_immediate(jit, dst, value);
            },
        }
    }
    match target {
        Value::Register(reg) => {
            emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
            store(jit, OperandSize::S64, SP, RA, 0);
            emit_ins(jit, RISCVInstruction::jalr(reg, 0, RA));
            load(jit, OperandSize::S64, SP, 0, RA);
            emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
        },
        Value::Constant64(value, user_provided) => {
            debug_assert!(!user_provided);
            load_immediate(jit, T1, value);
            emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, -8, SP));
            store(jit, OperandSize::S64, SP, RA, 0);
            emit_ins(jit, RISCVInstruction::jalr(T1, 0, RA));
            load(jit, OperandSize::S64, SP, 0, RA);
            emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8, SP));
        },
        _ => {
            #[cfg(debug_assertions)]
            unreachable!();
        }
    }

    // Save returned value in result register
    if let Some(reg) = result_reg {
        emit_ins(jit, RISCVInstruction::mov(OperandSize::S64, A0, reg));
    }

    // Restore registers from stack
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, if stack_arguments % 2 != 0 { stack_arguments + 1 } else { stack_arguments } * 8, SP));

    let mut current_offset = 0;
    for reg in saved_registers.iter() {
        load(jit, OperandSize::S64, SP, current_offset, *reg);
        current_offset += 8;
    }
    emit_ins(jit, RISCVInstruction::addi(OperandSize::S64, SP, 8 * saved_registers_len as i64, SP));
}

pub(crate) fn resolve_jumps<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    // Relocate forward jumps
    for jump in &jit.text_section_jumps {
        let destination = &jit.result.text_section[jit.result.pc_section[jump.target_pc] as usize & (i32::MAX as u32 as usize)] as *const u8;
        let offset_value = 
            unsafe { destination.offset_from(jump.location) } as i32 ;// Relative jump
        let address: *const u32 = jump.location as *const u32; 

        unsafe {
            // Rewrite the instruction at jump.location with the correct offset
            let original_instruction = *address;
            
            let op = original_instruction & 0x7f;
            if op != 0x6f as u32 {
                // B type instrustion (Branch)
                let imm_11 = (offset_value & 0x800) >> 4; // Extract the 12th bit of the immediate number and move it to bit[7]
                let imm_4_1 = (offset_value & 0x1E) << 7; // Extract bits [1:4] of the immediate number and move them to bits[8:11]
                let imm_10_5 = (offset_value & 0x7E0) << 20; // Extract bits [5:10] of the immediate number and move them to bits[25:30]
                let imm_12 = (offset_value & 0x1000) << 19; // Extract the 13th bit of the immediate number and move it to bit[31]
                let rs2_rs1_funct3 = (original_instruction & 0x1FFF000) as i32;
                let opcode = (original_instruction & 0x7F) as i32;
                let instruction = imm_12 | imm_10_5 | rs2_rs1_funct3 | imm_4_1 | imm_11 | opcode;
                unsafe { ptr::write_unaligned(jump.location as *mut i32, instruction); }
            } else {
                // J type instruction (JAL)
                let imm_19_12 = offset_value & 0xFF000; // Extract immediate bits [12:19]
                let imm_11 = (offset_value & 0x800) << 9; // Extract immediate bit [11]
                let imm_10_1 = (offset_value & 0x7FE) << 20; // Extract immediate bits [1:10]
                let imm_20 = (offset_value & 0x100000) << 11; // Extract immediate bit [20]
                let rd = (original_instruction & 0xF80) as i32;
                let opcode = 0x6f & 0x7F;
                let instruction= imm_20 | imm_10_1 | imm_11 | imm_19_12 | rd | opcode;
                unsafe { ptr::write_unaligned(jump.location as *mut i32, instruction); }
            }
        }         
    }
}