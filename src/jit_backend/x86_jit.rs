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
    ebpf::{self, FIRST_SCRATCH_REG, FRAME_PTR_REG, INSN_SIZE, Insn, SCRATCH_REGS}, elf::Executable, error::{EbpfError, ProgramResult}, jit::*, memory_management::{
        allocate_pages, free_pages, get_system_page_size, protect_pages, round_to_page_size,
    }, memory_region::MemoryMapping, vm::{Config, ContextObject, EbpfVm, RuntimeEnvironmentSlot, get_runtime_environment_key}, x86::*
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
        "push rbx",
        "push rbp",
        "mov [{host_stack_pointer}], rsp",
        "add QWORD PTR [{host_stack_pointer}], -8",
        cfg(not(feature = "jit-enable-host-stack-frames")),
        "xor rbp, rbp",
        "mov [rsp-8], rax",
        "mov rax, [r11 + 0x00]",
        "mov rsi, [r11 + 0x08]",
        "mov rdx, [r11 + 0x10]",
        "mov rcx, [r11 + 0x18]",
        "mov r8,  [r11 + 0x20]",
        "mov r9,  [r11 + 0x28]",
        "mov rbx, [r11 + 0x30]",
        "mov r12, [r11 + 0x38]",
        "mov r13, [r11 + 0x40]",
        "mov r14, [r11 + 0x48]",
        "mov r15, [r11 + 0x50]",
        "mov r11, [r11 + 0x58]",
        "call [rsp-8]",
        "pop rbp",
        "pop rbx",
        host_stack_pointer = in(reg) &mut vm.host_stack_pointer,
        inlateout("rdi") runtime_environment => _,
        inlateout("r10") instruction_meter => _,
        inlateout("rax") entrypoint => _,
        inlateout("r11") registers => _,
        lateout("rsi") _, lateout("rdx") _, lateout("rcx") _, lateout("r8") _,
        lateout("r9") _, lateout("r12") _, lateout("r13") _, lateout("r14") _, lateout("r15") _,
    );

    // unsafe {
    //     std::arch::asm!(
    //         "push rbx",
    //         "push rbp",
    //         "mov [{host_stack_pointer}], rsp",
    //         "add QWORD PTR [{host_stack_pointer}], -8",

    //         "xor rbp, rbp",
            
    //         "mov [rsp-8], rax",
    //         "mov rax, [r11 + 0x00]",
    //         "mov rsi, [r11 + 0x08]",
    //         "mov rdx, [r11 + 0x10]",
    //         "mov rcx, [r11 + 0x18]",
    //         "mov r8,  [r11 + 0x20]",
    //         "mov r9,  [r11 + 0x28]",
    //         "mov rbx, [r11 + 0x30]",
    //         "mov r12, [r11 + 0x38]",
    //         "mov r13, [r11 + 0x40]",
    //         "mov r14, [r11 + 0x48]",
    //         "mov r15, [r11 + 0x50]",
    //         "mov r11, [r11 + 0x58]",
    //         "call [rsp-8]",
    //         "pop rbp",
    //         "pop rbx",
    //         host_stack_pointer = in(reg) &mut vm.host_stack_pointer,
    //         inlateout("rdi") std::ptr::addr_of_mut!(*vm).cast::<u64>().offset(get_runtime_environment_key() as isize) => _,
    //         inlateout("r10") (vm.previous_instruction_meter as i64).wrapping_add(registers[11] as i64) => _,
    //         inlateout("rax") &jit.text_section
    //             [jit.pc_section[registers[11] as usize] as usize & (i32::MAX as u32 as usize)]
    //             as *const u8 => _,
    //         inlateout("r11") registers => _,
    //         lateout("rsi") _, lateout("rdx") _, lateout("rcx") _, lateout("r8") _,
    //         lateout("r9") _, lateout("r12") _, lateout("r13") _, lateout("r14") _, lateout("r15") _,
    //         // lateout("rbp") _, lateout("rbx") _,
    //     );
    // }
}

pub const REGISTER_MAP: [u8; 11] = [
    CALLER_SAVED_REGISTERS[0], // RAX
    ARGUMENT_REGISTERS[1],     // RSI
    ARGUMENT_REGISTERS[2],     // RDX
    ARGUMENT_REGISTERS[3],     // RCX
    ARGUMENT_REGISTERS[4],     // R8
    ARGUMENT_REGISTERS[5],     // R9
    CALLEE_SAVED_REGISTERS[1], // RBX
    CALLEE_SAVED_REGISTERS[2], // R12
    CALLEE_SAVED_REGISTERS[3], // R13
    CALLEE_SAVED_REGISTERS[4], // R14
    CALLEE_SAVED_REGISTERS[5], // R15
];

/// RDI: Used together with slot_in_vm()
pub const REGISTER_PTR_TO_VM: u8 = ARGUMENT_REGISTERS[0];
/// R10: Program counter limit
pub const REGISTER_INSTRUCTION_METER: u8 = CALLER_SAVED_REGISTERS[7];
/// R11: Scratch register
pub const REGISTER_SCRATCH: u8 = CALLER_SAVED_REGISTERS[8];

pub(crate) fn emit_noop<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    jit.emit::<u8>(0x90);
}

// This function helps the optimizer to inline the machinecode emission while avoiding stack allocations
#[inline(always)]
pub(crate) fn emit_ins<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    instruction: X86Instruction,
) {
    instruction.emit(jit);
    if jit.next_noop_insertion == 0 {
        jit.next_noop_insertion = jit.noop_range.sample(&mut jit.diversification_rng);
        // X86Instruction::noop().emit(jit)?;
        jit.emit::<u8>(0x90);
    } else {
        jit.next_noop_insertion -= 1;
    }
}

pub(crate) fn emit_variable_length<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, data: u64) {
    match size {
        OperandSize::S0 => {},
        OperandSize::S8 => jit.emit::<u8>(data as u8),
        OperandSize::S16 => jit.emit::<u16>(data as u16),
        OperandSize::S32 => jit.emit::<u32>(data as u32),
        OperandSize::S64 => jit.emit::<u64>(data),
    }
}

pub(crate) fn emit_register_trace<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    emit_ins(jit,X86Instruction::load_immediate(REGISTER_SCRATCH, jit.pc as i64));
    emit_ins(jit,X86Instruction::call_immediate(jit.relative_to_anchor(ANCHOR_TRACE, 5)));
    emit_ins(jit,X86Instruction::load_immediate(REGISTER_SCRATCH, 0));
}

// x86-specific
pub(crate) fn emit_sanitized_alu<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    opcode: u8, 
    opcode_extension: u8,
    destination: u8,
    immediate: i64,
) {
    if jit.should_sanitize_constant(immediate) {
        emit_sanitized_load_immediate(jit,REGISTER_SCRATCH, immediate);
        emit_ins(jit,X86Instruction::alu(size, opcode, REGISTER_SCRATCH, destination, None));
    } else if immediate >= i32::MIN as i64 && immediate <= i32::MAX as i64 {
        emit_ins(jit,X86Instruction::alu_immediate(size, 0x81, opcode_extension, destination, immediate, None));
    } else {
        emit_ins(jit,X86Instruction::load_immediate(REGISTER_SCRATCH, immediate));
        emit_ins(jit,X86Instruction::alu(size, opcode, REGISTER_SCRATCH, destination, None));
    }
}

pub(crate) fn emit_sanitized_load_immediate<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    destination: u8,
    value: i64,
) {
    let lower_key = jit.immediate_value_key as i32 as i64;
    if value >= i32::MIN as i64 && value <= i32::MAX as i64 {
        emit_ins(jit,X86Instruction::load_immediate(destination, value.wrapping_sub(lower_key)));
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, destination, lower_key, None)); // wrapping_add(lower_key)
    } else if value as u64 & u32::MAX as u64 == 0 {
        emit_ins(jit,X86Instruction::load_immediate(destination, value.rotate_right(32).wrapping_sub(lower_key)));
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, destination, lower_key, None)); // wrapping_add(lower_key)
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0xc1, 4, destination, 32, None)); // shift_left(32)
    } else if destination != REGISTER_SCRATCH {
        emit_ins(jit,X86Instruction::load_immediate(destination, value.wrapping_sub(jit.immediate_value_key)));
        emit_ins(jit,X86Instruction::load_immediate(REGISTER_SCRATCH, jit.immediate_value_key));
        emit_ins(jit,X86Instruction::alu(OperandSize::S64, 0x01, REGISTER_SCRATCH, destination, None)); // wrapping_add(immediate_value_key)
    } else {
        let upper_key = (jit.immediate_value_key >> 32) as i32 as i64;
        emit_ins(jit,X86Instruction::load_immediate(destination, value.wrapping_sub(lower_key).rotate_right(32).wrapping_sub(upper_key)));
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, destination, upper_key, None)); // wrapping_add(upper_key)
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0xc1, 1, destination, 32, None)); // rotate_right(32)
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, destination, lower_key, None)); // wrapping_add(lower_key)
    }
}

pub(crate) fn load_immediate<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    destination: u8,
    value: i64,
) {
    emit_ins(jit,X86Instruction::load_immediate(destination, value));
}

pub(crate) fn emit_sanitized_add<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    destination: u8,
    immediate: i64,
) {
    emit_sanitized_alu(jit, size, 0x01, 0, destination, immediate);
}

pub(crate) fn emit_or_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    emit_sanitized_alu(jit, size, 0x09, 1, dst, imm);
}

pub(crate) fn emit_or_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    emit_ins(jit, X86Instruction::alu(size, 0x09, src, dst, None));
}

pub(crate) fn emit_and_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    emit_sanitized_alu(jit, size, 0x21, 4, dst, imm);
}

pub(crate) fn emit_and_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    emit_ins(jit, X86Instruction::alu(size, 0x21, src, dst, None));
}

pub(crate) fn emit_lsh_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    emit_shift(jit, size, 4, REGISTER_SCRATCH, dst, Some(imm));
}

pub(crate) fn emit_lsh_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    emit_shift(jit, size, 4, src, dst, None);
}

pub(crate) fn emit_rsh_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    emit_shift(jit, size, 5, REGISTER_SCRATCH, dst, Some(imm));
}

pub(crate) fn emit_rsh_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    emit_shift(jit, size, 5, src, dst, None);
}

pub(crate) fn emit_arsh_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    emit_shift(jit, size, 7, REGISTER_SCRATCH, dst, Some(imm));
}

pub(crate) fn emit_arsh_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    emit_shift(jit, size, 7, src, dst, None);
}

pub(crate) fn emit_le<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    dst: u8,
    imm: i64,
) -> Result<(), EbpfError> {
    match imm {
        16 => {
            emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S32, 0x81, 4, dst, 0xffff, None)); // Mask to 16 bit
        }
        32 => {
            emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S32, 0x81, 4, dst, -1, None)); // Mask to 32 bit
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
            emit_ins(jit, X86Instruction::bswap(OperandSize::S16, dst));
            emit_ins(jit, X86Instruction::alu_immediate(OperandSize::S32, 0x81, 4, dst, 0xffff, None)); // Mask to 16 bit
        }
        32 => emit_ins(jit, X86Instruction::bswap(OperandSize::S32, dst)),
        64 => emit_ins(jit, X86Instruction::bswap(OperandSize::S64, dst)),
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
    emit_ins(jit, X86Instruction::alu_immediate(size, 0xf7, 3, dst, 0, None));
}

pub(crate) fn emit_xor_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    emit_sanitized_alu(jit, size, 0x31, 6, dst, imm);
}

pub(crate) fn emit_xor_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    emit_ins(jit, X86Instruction::alu(size, 0x31, src, dst, None));
}

pub(crate) fn emit_hor_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    imm: i64,
) {
    emit_sanitized_alu(jit, size, 0x09, 1, dst, imm);
}

/// sign-extend
pub(crate) fn sign_extend<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    destination: u8,
) {
    emit_ins(jit, X86Instruction::alu(OperandSize::S64, 0x63, destination, destination, None)); // sign extend i32 to i64
}

pub(crate) fn emit_add_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    emit_ins(jit, X86Instruction::alu(size, 0x01, src, dst, None));
}

pub(crate) fn emit_mov_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        if jit.executable.get_sbpf_version().explicit_sign_extension_of_results() {
            emit_ins(jit, X86Instruction::mov_with_sign_extension(OperandSize::S64, src, dst));
        } else {
            emit_ins(jit, X86Instruction::mov(OperandSize::S32, src, dst));
        }
    } else {
        emit_ins(jit,X86Instruction::mov(OperandSize::S64, src, dst));
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
            emit_ins(jit, X86Instruction::alu_immediate(OperandSize::S32, 0xf7, 3, dst, 0, None));
            if imm != 0 {
                emit_sanitized_alu(jit,OperandSize::S32, 0x01, 0, dst, imm);
            }
        } else {
            emit_sanitized_alu(jit, OperandSize::S32, 0x29, 5, dst, imm);
        }
        if !jit.executable.get_sbpf_version().explicit_sign_extension_of_results() {
            sign_extend(jit, dst); // sign extend i32 to i64
        }
    } else {
        if jit.executable.get_sbpf_version().swap_sub_reg_imm_operands() {
            emit_ins(jit, X86Instruction::alu_immediate(OperandSize::S64, 0xf7, 3, dst, 0, None));
            if imm != 0 {
                emit_sanitized_alu(jit,OperandSize::S64, 0x01, 0, dst, imm);
            }
        } else {
            emit_sanitized_alu(jit, OperandSize::S64, 0x29, 5, dst, imm);
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
        emit_ins(jit, X86Instruction::alu(OperandSize::S32, 0x29, src, dst, None));
        if !jit.executable.get_sbpf_version().explicit_sign_extension_of_results() {
            sign_extend(jit, dst);
        }
    } else {
        emit_ins(jit, X86Instruction::alu(OperandSize::S64, 0x29, src, dst, None));
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
        emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, imm));
    }
    if size == OperandSize::S32 {
        emit_ins(jit, X86Instruction::alu_escaped(OperandSize::S32, 1, 0xaf, dst, REGISTER_SCRATCH, None));
        if !jit.executable.get_sbpf_version().explicit_sign_extension_of_results() {
            sign_extend(jit, dst);
        }
    } else {
        emit_ins(jit, X86Instruction::alu_escaped(OperandSize::S64, 1, 0xaf, dst, REGISTER_SCRATCH, None));
    }
}

pub(crate) fn emit_div_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    emit_product_quotient_remainder(
        jit,
        size,
        (insn.opc & ebpf::BPF_ALU_OP_MASK) == ebpf::BPF_MOD,
        (insn.opc & ebpf::BPF_ALU_OP_MASK) != ebpf::BPF_MUL,
        (insn.opc & ebpf::BPF_ALU_OP_MASK) == ebpf::BPF_MUL,
        dst, dst, Some(insn.imm),
    )
}

pub(crate) fn emit_mod_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    emit_product_quotient_remainder(
        jit,
        size,
        (insn.opc & ebpf::BPF_ALU_OP_MASK) == ebpf::BPF_MOD,
        (insn.opc & ebpf::BPF_ALU_OP_MASK) != ebpf::BPF_MUL,
        (insn.opc & ebpf::BPF_ALU_OP_MASK) == ebpf::BPF_MUL,
        dst, dst, Some(insn.imm),
    )
}

pub(crate) fn emit_div_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    emit_product_quotient_remainder(
        jit,
        size,
        (insn.opc & ebpf::BPF_ALU_OP_MASK) == ebpf::BPF_MOD,
        (insn.opc & ebpf::BPF_ALU_OP_MASK) != ebpf::BPF_MUL,
        (insn.opc & ebpf::BPF_ALU_OP_MASK) == ebpf::BPF_MUL,
        src, dst, None,
    )
}

pub(crate) fn emit_mod_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    emit_product_quotient_remainder(
        jit,
        size,
        (insn.opc & ebpf::BPF_ALU_OP_MASK) == ebpf::BPF_MOD,
        (insn.opc & ebpf::BPF_ALU_OP_MASK) != ebpf::BPF_MUL,
        (insn.opc & ebpf::BPF_ALU_OP_MASK) == ebpf::BPF_MUL,
        src, dst, None,
    )
}

pub(crate) fn emit_mul_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    dst: u8,
    src: u8,
) {
    if size == OperandSize::S32 {
        emit_ins(jit, X86Instruction::alu_escaped(OperandSize::S32, 1, 0xaf, dst, src, None));
        if !jit.executable.get_sbpf_version().explicit_sign_extension_of_results() {
            sign_extend(jit, dst);
        }
    } else {
        emit_ins(jit, X86Instruction::alu_escaped(OperandSize::S64, 1, 0xaf, dst, src, None));
    }
}

pub(crate) fn emit_lmul_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    pqr_imm_helper(jit, size, insn, dst);
}

pub(crate) fn emit_uhlmul_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    pqr_imm_helper(jit, size, insn, dst);
}

pub(crate) fn emit_shlmul_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    pqr_imm_helper(jit, size, insn, dst);
}

pub(crate) fn emit_udiv_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    pqr_imm_helper(jit, size, insn, dst);
}

pub(crate) fn emit_urem_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    pqr_imm_helper(jit, size, insn, dst);
}

pub(crate) fn emit_sdiv_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    pqr_imm_helper(jit, size, insn, dst);
}

pub(crate) fn emit_srem_imm<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    pqr_imm_helper(jit, size, insn, dst);
}

pub(crate) fn pqr_imm_helper<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
) {
    let signed = insn.opc & (1 << 7) != 0;
    let mut imm = insn.imm;
    if !signed {
        imm &= u32::MAX as i64;
    }
    emit_product_quotient_remainder(
        jit,
        size,
        insn.opc & (1 << 5) != 0,
        insn.opc & (1 << 6) != 0,
        signed,
        dst, dst, Some(imm),
    )
}

pub(crate) fn emit_lmul_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    pqr_reg_helper(jit, size, insn, dst, src);
}

pub(crate) fn emit_uhlmul_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    pqr_reg_helper(jit, size, insn, dst, src);
}

pub(crate) fn emit_shlmul_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    pqr_reg_helper(jit, size, insn, dst, src);
}

pub(crate) fn emit_udiv_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    pqr_reg_helper(jit, size, insn, dst, src);
}

pub(crate) fn emit_urem_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    pqr_reg_helper(jit, size, insn, dst, src);
}

pub(crate) fn emit_sdiv_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    pqr_reg_helper(jit, size, insn, dst, src);
}

pub(crate) fn emit_srem_reg<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    pqr_reg_helper(jit, size, insn, dst, src);
}

pub(crate) fn pqr_reg_helper<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    insn: Insn,
    dst: u8,
    src: u8,
) {
    emit_product_quotient_remainder(
        jit,
        size,
        insn.opc & (1 << 5) != 0,
        insn.opc & (1 << 6) != 0,
        insn.opc & (1 << 7) != 0,
        src, dst, None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_product_quotient_remainder<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    size: OperandSize,
    alt_dst: bool,
    division: bool,
    signed: bool,
    src: u8,
    dst: u8,
    imm: Option<i64>,
) {
    //         LMUL UHMUL SHMUL UDIV SDIV UREM SREM
    // ALU     F7/4 F7/4  F7/5  F7/6 F7/7 F7/6 F7/7
    // src-in  REGISTER_SCRATCH  REGISTER_SCRATCH   REGISTER_SCRATCH   REGISTER_SCRATCH  REGISTER_SCRATCH  REGISTER_SCRATCH  REGISTER_SCRATCH
    // dst-in  RAX  RAX   RAX   RAX  RAX  RAX  RAX
    // dst-out RAX  RDX   RDX   RAX  RAX  RDX  RDX

    if division {
        // Prevent division by zero
        if imm.is_none() {
            emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, jit.pc as i64)); // Save pc
            emit_ins(jit, X86Instruction::test(size, src, src, None)); // src == 0
            emit_ins(jit, X86Instruction::conditional_jump_immediate(0x84, jit.relative_to_anchor(ANCHOR_DIV_BY_ZERO, 6)));
        }

        // Signed division overflows with MIN / -1.
        // If we have an immediate and it's not -1, we can skip the following check.
        if signed && imm.unwrap_or(-1) == -1 {
            emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, if let OperandSize::S64 = size { i64::MIN } else { i32::MIN as i64 }));
            emit_ins(jit, X86Instruction::cmp(size, dst, REGISTER_SCRATCH, None)); // dst == MIN

            if imm.is_none() {
                // The exception case is: dst == MIN && src == -1
                // Via De Morgan's law becomes: !(dst != MIN || src != -1)
                // Also, we know that src != 0 in here, so we can use it to set REGISTER_SCRATCH to something not zero
                emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, 0)); // No XOR here because we need to keep the status flags
                emit_ins(jit, X86Instruction::cmov(size, 0x45, src, REGISTER_SCRATCH)); // if dst != MIN { REGISTER_SCRATCH = src; }
                emit_ins(jit, X86Instruction::cmp_immediate(size, src, -1, None)); // src == -1
                emit_ins(jit, X86Instruction::cmov(size, 0x45, src, REGISTER_SCRATCH)); // if src != -1 { REGISTER_SCRATCH = src; }
                emit_ins(jit, X86Instruction::test(size, REGISTER_SCRATCH, REGISTER_SCRATCH, None)); // REGISTER_SCRATCH == 0
            }

            // MIN / -1, raise EbpfError::DivideOverflow
            emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, jit.pc as i64));
            emit_ins(jit, X86Instruction::conditional_jump_immediate(0x84, jit.relative_to_anchor(ANCHOR_DIV_OVERFLOW, 6)));
        }
    }

    if let Some(imm) = imm {
        if jit.should_sanitize_constant(imm) {
            emit_sanitized_load_immediate(jit, REGISTER_SCRATCH, imm);
        } else {
            emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, imm));
        }
    } else {
        emit_ins(jit, X86Instruction::mov(OperandSize::S64, src, REGISTER_SCRATCH));
    }
    if dst != RAX {
        emit_ins(jit, X86Instruction::push(RAX, None));
        emit_ins(jit, X86Instruction::mov(OperandSize::S64, dst, RAX));
    }
    if dst != RDX {
        emit_ins(jit, X86Instruction::push(RDX, None));
    }
    if division {
        if signed {
            emit_ins(jit, X86Instruction::sign_extend_rax_rdx(size));
        } else {
            emit_ins(jit, X86Instruction::alu(size, 0x31, RDX, RDX, None)); // RDX = 0
        }
    }

    emit_ins(jit, X86Instruction::alu_immediate(size, 0xf7, 0x4 | ((division as u8) << 1) | signed as u8, REGISTER_SCRATCH, 0, None));

    if dst != RDX {
        if alt_dst {
            emit_ins(jit, X86Instruction::mov(OperandSize::S64, RDX, dst));
        }
        emit_ins(jit, X86Instruction::pop(RDX));
    }
    if dst != RAX {
        if !alt_dst {
            emit_ins(jit, X86Instruction::mov(OperandSize::S64, RAX, dst));
        }
        emit_ins(jit, X86Instruction::pop(RAX));
    }
    if let OperandSize::S32 = size {
        if signed && !jit.executable.get_sbpf_version().explicit_sign_extension_of_results() {
            emit_ins(jit, X86Instruction::alu(OperandSize::S64, 0x63, dst, dst, None)); // sign extend i32 to i64
        }
    }
}

pub(crate) fn emit_shift<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, opcode_extension: u8, source: u8, destination: u8, immediate: Option<i64>) {
    if let Some(immediate) = immediate {
        emit_ins(jit, X86Instruction::alu_immediate(size, 0xc1, opcode_extension, destination, immediate, None));
        return;
    }
    if let OperandSize::S32 = size {
        emit_ins(jit, X86Instruction::mov(OperandSize::S32, destination, destination)); // Truncate to 32 bit
    }
    if source == RCX {
        emit_ins(jit, X86Instruction::alu_immediate(size, 0xd3, opcode_extension, destination, 0, None));
    } else if destination == RCX {
        emit_ins(jit, X86Instruction::push(source, None));
        emit_ins(jit, X86Instruction::xchg(OperandSize::S64, source, RCX, None));
        emit_ins(jit, X86Instruction::alu_immediate(size, 0xd3, opcode_extension, source, 0, None));
        emit_ins(jit, X86Instruction::mov(OperandSize::S64, source, RCX));
        emit_ins(jit, X86Instruction::pop(source));
    } else {
        emit_ins(jit, X86Instruction::push(RCX, None));
        emit_ins(jit, X86Instruction::mov(OperandSize::S64, source, RCX));
        emit_ins(jit, X86Instruction::alu_immediate(size, 0xd3, opcode_extension, destination, 0, None));
        emit_ins(jit, X86Instruction::pop(RCX));
    }
}

pub(crate) fn emit_jeq_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    emit_conditional_branch_imm(jit, size, 0x84, false,imm, dst, target_pc);
}

pub(crate) fn emit_jeq_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    emit_conditional_branch_reg(jit, size, 0x84, false, src, dst, target_pc);
}

pub(crate) fn emit_jgt_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    emit_conditional_branch_imm(jit, size, 0x87, false, imm, dst, target_pc);
}

pub(crate) fn emit_jgt_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    emit_conditional_branch_reg(jit, size, 0x87, false, src, dst, target_pc);
}

pub(crate) fn emit_jge_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    emit_conditional_branch_imm(jit, size, 0x83, false, imm, dst, target_pc);
}

pub(crate) fn emit_jge_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    emit_conditional_branch_reg(jit, size, 0x83, false, src, dst, target_pc);
}

pub(crate) fn emit_jlt_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    emit_conditional_branch_imm(jit, size, 0x82, false, imm, dst, target_pc);
}

pub(crate) fn emit_jlt_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    emit_conditional_branch_reg(jit, size, 0x82, false, src, dst, target_pc);
}

pub(crate) fn emit_jle_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    emit_conditional_branch_imm(jit, size, 0x86, false, imm, dst, target_pc);
}

pub(crate) fn emit_jle_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    emit_conditional_branch_reg(jit, size, 0x86, false, src, dst, target_pc);
}

pub(crate) fn emit_jset_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    emit_conditional_branch_imm(jit, size, 0x85, true, imm, dst, target_pc);
}

pub(crate) fn emit_jset_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    emit_conditional_branch_reg(jit, size, 0x85, true, src, dst, target_pc);
}

pub(crate) fn emit_jne_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    emit_conditional_branch_imm(jit, size, 0x85, false, imm, dst, target_pc);
}

pub(crate) fn emit_jne_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    emit_conditional_branch_reg(jit, size, 0x85, false, src, dst, target_pc);
}

pub(crate) fn emit_jsgt_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    emit_conditional_branch_imm(jit, size, 0x8f, false, imm, dst, target_pc);
}

pub(crate) fn emit_jsgt_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    emit_conditional_branch_reg(jit, size, 0x8f, false, src, dst, target_pc);
}

pub(crate) fn emit_jsge_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    emit_conditional_branch_imm(jit, size, 0x8d, false, imm, dst, target_pc);
}

pub(crate) fn emit_jsge_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    emit_conditional_branch_reg(jit, size, 0x8d, false, src, dst, target_pc);
}

pub(crate) fn emit_jslt_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    emit_conditional_branch_imm(jit, size, 0x8c, false, imm, dst, target_pc);
}

pub(crate) fn emit_jslt_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    emit_conditional_branch_reg(jit, size, 0x8c, false, src, dst, target_pc);
}

pub(crate) fn emit_jsle_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, imm: i64, dst: u8, target_pc: usize) {
    emit_conditional_branch_imm(jit, size, 0x8e, false, imm, dst, target_pc);
}

pub(crate) fn emit_jsle_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, src: u8, dst: u8, target_pc: usize) {
    emit_conditional_branch_reg(jit, size, 0x8e, false, src, dst, target_pc);
}

pub(crate) fn emit_ja<C: ContextObject>(jit: &mut JitCompiler<C>, target_pc: usize) {
    jit.emit_validate_and_profile_instruction_count(Some(target_pc));
    emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, target_pc as i64));
    let jump_offset = jit.relative_to_target_pc(target_pc, 5);
    emit_ins(jit, X86Instruction::jump_immediate(jump_offset));
}

pub(crate) fn emit_conditional_branch_imm<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, op: u8, bitwise: bool, immediate: i64, second_operand: u8, target_pc: usize) {
    jit.emit_validate_and_profile_instruction_count(Some(target_pc));
    if jit.should_sanitize_constant(immediate) {
        emit_sanitized_load_immediate(jit, REGISTER_SCRATCH, immediate);
        if bitwise { // Logical
            emit_ins(jit, X86Instruction::test(size, REGISTER_SCRATCH, second_operand, None));
        } else { // Arithmetic
            emit_ins(jit, X86Instruction::cmp(size, REGISTER_SCRATCH, second_operand, None));
        }
    } else if bitwise { // Logical
        emit_ins(jit, X86Instruction::test_immediate(size, second_operand, immediate, None));
    } else { // Arithmetic
        emit_ins(jit, X86Instruction::cmp_immediate(size, second_operand, immediate, None));
    }
    emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, target_pc as i64));
    let jump_offset = jit.relative_to_target_pc(target_pc, 6);
    emit_ins(jit, X86Instruction::conditional_jump_immediate(op, jump_offset));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_conditional_branch_reg<C: ContextObject>(jit: &mut JitCompiler<C>, size: OperandSize, op: u8, bitwise: bool, first_operand: u8, second_operand: u8, target_pc: usize) {
    jit.emit_validate_and_profile_instruction_count(Some(target_pc));
    if bitwise { // Logical
        emit_ins(jit, X86Instruction::test(size, first_operand, second_operand, None));
    } else { // Arithmetic
        emit_ins(jit, X86Instruction::cmp(size, first_operand, second_operand, None));
    }
    emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, target_pc as i64));
    let jump_offset = jit.relative_to_target_pc(target_pc, 6);
    emit_ins(jit, X86Instruction::conditional_jump_immediate(op, jump_offset));
    emit_undo_profile_instruction_count(jit, target_pc);
}

pub(crate) fn emit_call_imm<C: ContextObject>(jit: &mut JitCompiler<C>, insn: Insn){
    let mut resolved = false;
    // External syscall
    if !jit.executable.get_sbpf_version().static_syscalls() || insn.src == 0 {
        if let Some((_, function)) =
                jit.executable.get_loader().get_function_registry().lookup_by_key(insn.imm as u32) {
            jit.emit_validate_and_profile_instruction_count(Some(0));
            emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, function as usize as i64));
            emit_ins(jit, X86Instruction::call_immediate(jit.relative_to_anchor(ANCHOR_EXTERNAL_FUNCTION_CALL, 5)));
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
        emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, jit.pc as i64));
        emit_ins(jit, X86Instruction::jump_immediate(jit.relative_to_anchor(ANCHOR_CALL_UNSUPPORTED_INSTRUCTION, 5)));
    }
}

pub(crate) fn emit_internal_call<C: ContextObject>(jit: &mut JitCompiler<C>, dst: Value) {
    // Store PC in case the bounds check fails
    emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, jit.pc as i64));
    jit.last_instruction_meter_validation_pc = jit.pc;
    emit_ins(jit, X86Instruction::call_immediate(jit.relative_to_anchor(ANCHOR_INTERNAL_FUNCTION_CALL_PROLOGUE, 5)));

    match dst {
        Value::Register(reg) => {
            // REGISTER_SCRATCH contains self.pc, and we must store it for proper error handling.
            // We can discard the value if callx succeeds, so we are not incrementing the stack pointer (RSP).
            emit_ins(jit, X86Instruction::store(OperandSize::S64, REGISTER_SCRATCH, RSP, X86IndirectAccess::OffsetIndexShift(-24, RSP, 0)));
            // Move guest_target_address into REGISTER_SCRATCH
            emit_ins(jit, X86Instruction::mov(OperandSize::S64, reg, REGISTER_SCRATCH));
            emit_ins(jit, X86Instruction::call_immediate(jit.relative_to_anchor(ANCHOR_INTERNAL_FUNCTION_CALL_REG, 5)));
        },
        Value::Constant64(target_pc, user_provided) => {
            debug_assert!(user_provided);
            emit_profile_instruction_count(jit, Some(target_pc as usize));
            if user_provided && jit.should_sanitize_constant(target_pc) {
                emit_sanitized_load_immediate(jit, REGISTER_SCRATCH, target_pc);
            } else {
                emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, target_pc));
            }
            let jump_offset = jit.relative_to_target_pc(target_pc as usize, 5);
            emit_ins(jit, X86Instruction::call_immediate(jump_offset));
        },
        _ => {
            #[cfg(debug_assertions)]
            unreachable!();
        }
    }

    emit_undo_profile_instruction_count(jit, 0);

    // Restore the previous frame pointer
    emit_ins(jit, X86Instruction::pop(REGISTER_MAP[FRAME_PTR_REG]));
    for reg in REGISTER_MAP.iter().skip(FIRST_SCRATCH_REG).take(SCRATCH_REGS).rev() {
        emit_ins(jit, X86Instruction::pop(*reg));
    }
}

pub(crate) fn emit_exit<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    let call_depth_access = X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::CallDepth));
    // If env.call_depth == 0, we've reached the exit instruction of the entry point
    emit_ins(jit,X86Instruction::cmp_immediate(OperandSize::S32, REGISTER_PTR_TO_VM, 0, Some(call_depth_access)));
    // we're done
    emit_ins(jit,X86Instruction::conditional_jump_immediate(0x84, jit.relative_to_anchor(ANCHOR_EXIT, 6)));

    // else decrement and update env.call_depth
    emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 5, REGISTER_PTR_TO_VM, 1, Some(call_depth_access))); // env.call_depth -= 1;

    // and return
    emit_ins(jit,X86Instruction::return_near());
}

pub(crate) fn emit_address_translation<C: ContextObject>(jit: &mut JitCompiler<C>, dst: Option<u8>, vm_addr: Value, len: u64, value: Option<Value>) {
    debug_assert_ne!(dst.is_some(), value.is_some());

    let stack_slot_of_value_to_store = X86IndirectAccess::OffsetIndexShift(-96, RSP, 0);
    match value {
        Some(Value::Register(reg)) => {
            emit_ins(jit, X86Instruction::store(OperandSize::S64, reg, RSP, stack_slot_of_value_to_store));
        }
        Some(Value::Constant64(constant, user_provided)) => {
            debug_assert!(user_provided);
            // First half of emit_sanitized_load_immediate(stack_slot_of_value_to_store, constant)
            let lower_key = jit.immediate_value_key as i32 as i64;
            emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, constant.wrapping_sub(lower_key)));
            emit_ins(jit, X86Instruction::store(OperandSize::S64, REGISTER_SCRATCH, RSP, stack_slot_of_value_to_store));
        }
        _ => {}
    }

    match vm_addr {
        Value::RegisterPlusConstant64(reg, constant, user_provided) => {
            if user_provided && jit.should_sanitize_constant(constant) {
                emit_sanitized_load_immediate(jit, REGISTER_SCRATCH, constant);
            } else {
                emit_ins(jit, X86Instruction::load_immediate(REGISTER_SCRATCH, constant));
            }
            emit_ins(jit, X86Instruction::alu(OperandSize::S64, 0x01, reg, REGISTER_SCRATCH, None));
        },
        _ => {
            #[cfg(debug_assertions)]
            unreachable!();
        },
    }

    if jit.config.enable_address_translation {
        let anchor_base = match value {
            Some(Value::Register(_reg)) => 4,
            Some(Value::Constant64(_constant, _user_provided)) => 8,
            _ => 0,
        };
        let anchor = ANCHOR_TRANSLATE_MEMORY_ADDRESS + anchor_base + len.trailing_zeros() as usize;
        emit_ins(jit, X86Instruction::push_immediate(OperandSize::S64, jit.pc as i32));
        emit_ins(jit, X86Instruction::call_immediate(jit.relative_to_anchor(anchor, 5)));
        if let Some(dst) = dst {
            emit_ins(jit, X86Instruction::mov(OperandSize::S64, REGISTER_SCRATCH, dst));
        }
    } else if let Some(dst) = dst {
        match len {
            1 => emit_ins(jit, X86Instruction::load(OperandSize::S8, REGISTER_SCRATCH, dst, X86IndirectAccess::Offset(0))),
            2 => emit_ins(jit, X86Instruction::load(OperandSize::S16, REGISTER_SCRATCH, dst, X86IndirectAccess::Offset(0))),
            4 => emit_ins(jit, X86Instruction::load(OperandSize::S32, REGISTER_SCRATCH, dst, X86IndirectAccess::Offset(0))),
            8 => emit_ins(jit, X86Instruction::load(OperandSize::S64, REGISTER_SCRATCH, dst, X86IndirectAccess::Offset(0))),
            _ => unreachable!(),
        }
    } else {
        emit_ins(jit, X86Instruction::xchg(OperandSize::S64, RSP, REGISTER_MAP[0], Some(stack_slot_of_value_to_store))); // Save REGISTER_MAP[0] and retrieve value to store
        match len {
            1 => emit_ins(jit, X86Instruction::store(OperandSize::S8, REGISTER_MAP[0], REGISTER_SCRATCH, X86IndirectAccess::Offset(0))),
            2 => emit_ins(jit, X86Instruction::store(OperandSize::S16, REGISTER_MAP[0], REGISTER_SCRATCH, X86IndirectAccess::Offset(0))),
            4 => emit_ins(jit, X86Instruction::store(OperandSize::S32, REGISTER_MAP[0], REGISTER_SCRATCH, X86IndirectAccess::Offset(0))),
            8 => emit_ins(jit, X86Instruction::store(OperandSize::S64, REGISTER_MAP[0], REGISTER_SCRATCH, X86IndirectAccess::Offset(0))),
            _ => unreachable!(),
        }
        emit_ins(jit, X86Instruction::xchg(OperandSize::S64, RSP, REGISTER_MAP[0], Some(stack_slot_of_value_to_store))); // Restore REGISTER_MAP[0]
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
    emit_ins(jit,X86Instruction::cmp(OperandSize::S64, REGISTER_SCRATCH, REGISTER_INSTRUCTION_METER, None));
    emit_ins(jit,X86Instruction::conditional_jump_immediate(0x86, jit.relative_to_anchor(ANCHOR_THROW_EXCEEDED_MAX_INSTRUCTIONS, 6)));
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
            emit_sanitized_alu(jit,OperandSize::S64, 0x01, 0, REGISTER_INSTRUCTION_METER, target_pc as i64 - jit.pc as i64 - 1); // instruction_meter += target_pc - (self.pc + 1);
            // jit.emit_sanitized_alu(OperandSize::S64, 0x01, 0, REGISTER_INSTRUCTION_METER, target_pc as i64 - jit.pc as i64 - 1); // instruction_meter += target_pc - (self.pc + 1);
        },
        None => {
            emit_ins(jit,X86Instruction::alu(OperandSize::S64, 0x01, REGISTER_SCRATCH, REGISTER_INSTRUCTION_METER, None)); // instruction_meter += target_pc;
            emit_sanitized_alu(jit,OperandSize::S64, 0x81, 5, REGISTER_INSTRUCTION_METER, jit.pc as i64 + 1); // instruction_meter -= self.pc + 1;
        },
    }
}

pub(crate) fn emit_undo_profile_instruction_count<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    target_pc: usize,
) {
    if jit.config.enable_instruction_meter {
        emit_sanitized_alu(jit,OperandSize::S64, 0x01, 0, REGISTER_INSTRUCTION_METER, jit.pc as i64 + 1 - target_pc as i64); // instruction_meter += (self.pc + 1) - target_pc;
    }
}

pub(crate) fn emit_set_exception_kind<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    err: EbpfError,
) {
    let err_kind = unsafe { *std::ptr::addr_of!(err).cast::<u64>() };
    let err_discriminant = ProgramResult::Err(err).discriminant();
    emit_ins(jit,X86Instruction::lea(OperandSize::S64, REGISTER_PTR_TO_VM, REGISTER_MAP[0], Some(X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult)))));
    emit_ins(jit,X86Instruction::store_immediate(OperandSize::S64, REGISTER_MAP[0], X86IndirectAccess::Offset(0), err_discriminant as i64)); // result.discriminant = err_discriminant;
    emit_ins(jit,X86Instruction::store_immediate(OperandSize::S64, REGISTER_MAP[0], X86IndirectAccess::Offset(std::mem::size_of::<u64>() as i32), err_kind as i64)); // err.kind = err_kind;
}

pub(crate) fn emit_execution_overrun_trailer<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    emit_ins(jit,X86Instruction::load_immediate(REGISTER_SCRATCH, jit.pc as i64)); // Save pc
    emit_set_exception_kind(jit,EbpfError::ExecutionOverrun);
    emit_ins(jit,X86Instruction::jump_immediate(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 5)));
}

pub(crate) fn emit_throw_exception<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    emit_ins(jit, X86Instruction::jump_immediate(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 5)));
}

#[allow(dead_code)]
pub(crate) fn emit_stopwatch<C: ContextObject>(jit: &mut JitCompiler<C>, begin: bool) {
    jit.stopwatch_is_active = true;
    emit_ins(jit, X86Instruction::push(RDX, None));
    emit_ins(jit, X86Instruction::push(RAX, None));
    emit_ins(jit, X86Instruction::fence(FenceType::Load)); // lfence
    emit_ins(jit, X86Instruction::cycle_count()); // rdtsc
    emit_ins(jit, X86Instruction::fence(FenceType::Load)); // lfence
    emit_ins(jit, X86Instruction::alu_immediate(OperandSize::S64, 0xc1, 4, RDX, 32, None)); // RDX <<= 32;
    emit_ins(jit, X86Instruction::alu(OperandSize::S64, 0x09, RDX, RAX, None)); // RAX |= RDX;
    if begin {
        emit_ins(jit, X86Instruction::alu(OperandSize::S64, 0x29, RAX, REGISTER_PTR_TO_VM, Some(X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::StopwatchNumerator))))); // *numerator -= RAX;
    } else {
        emit_ins(jit, X86Instruction::alu(OperandSize::S64, 0x01, RAX, REGISTER_PTR_TO_VM, Some(X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::StopwatchNumerator))))); // *numerator += RAX;
        emit_ins(jit, X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, REGISTER_PTR_TO_VM, 1, Some(X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::StopwatchDenominator))))); // *denominator += 1;
    }
    emit_ins(jit, X86Instruction::pop(RAX));
    emit_ins(jit, X86Instruction::pop(RDX));
}

pub(crate) fn emit_subroutines<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    // Routine for instruction tracing
    if jit.config.enable_register_tracing {
        jit.set_anchor(ANCHOR_TRACE);
        // Save registers on stack
        emit_ins(jit,X86Instruction::push(REGISTER_SCRATCH, None));
        for reg in REGISTER_MAP.iter().rev() {
            emit_ins(jit,X86Instruction::push(*reg, None));
        }
        emit_ins(jit,X86Instruction::mov(OperandSize::S64, RSP, REGISTER_MAP[0]));
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, RSP, - 8 * 3, None)); // RSP -= 8 * 3;
        emit_rust_call(jit, Value::Constant64(Vec::<crate::static_analysis::RegisterTraceEntry>::push as *const u8 as i64, false), &[
            Argument { index: 1, value: Value::Register(REGISTER_MAP[0]) }, // registers
            Argument { index: 0, value: Value::RegisterPlusConstant32(REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::RegisterTrace), false) },
        ], None);
        // Pop stack and return
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, RSP, 8 * 3, None)); // RSP += 8 * 3;
        emit_ins(jit,X86Instruction::pop(REGISTER_MAP[0]));
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, RSP, 8 * (REGISTER_MAP.len() - 1) as i64, None)); // RSP += 8 * (REGISTER_MAP.len() - 1);
        emit_ins(jit,X86Instruction::pop(REGISTER_SCRATCH));
        emit_ins(jit,X86Instruction::return_near());
    }

    // Epilogue
    jit.set_anchor(ANCHOR_EPILOGUE);
    if jit.config.enable_instruction_meter {
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 5, REGISTER_INSTRUCTION_METER, 1, None)); // REGISTER_INSTRUCTION_METER -= 1;
        emit_ins(jit,X86Instruction::alu(OperandSize::S64, 0x29, REGISTER_SCRATCH, REGISTER_INSTRUCTION_METER, None)); // REGISTER_INSTRUCTION_METER -= pc;
        // *DueInsnCount = *PreviousInstructionMeter - REGISTER_INSTRUCTION_METER;
        emit_ins(jit,X86Instruction::alu(OperandSize::S64, 0x2B, REGISTER_INSTRUCTION_METER, REGISTER_PTR_TO_VM, Some(X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::PreviousInstructionMeter))))); // REGISTER_INSTRUCTION_METER -= *PreviousInstructionMeter;
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0xf7, 3, REGISTER_INSTRUCTION_METER, 0, None)); // REGISTER_INSTRUCTION_METER = -REGISTER_INSTRUCTION_METER;
        emit_ins(jit,X86Instruction::store(OperandSize::S64, REGISTER_INSTRUCTION_METER, REGISTER_PTR_TO_VM, X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::DueInsnCount)))); // *DueInsnCount = REGISTER_INSTRUCTION_METER;
    }
    // // Print stop watch value
    // fn stopwatch_result(numerator: u64, denominator: u64) {
    //     println!("Stop watch: {} / {} = {}", numerator, denominator, if denominator == 0 { 0.0 } else { numerator as f64 / denominator as f64 });
    // }
    // if stopwatch_is_active {
    //     emit_rust_call(Value::Constant64(stopwatch_result as *const u8 as i64, false), &[
    //         Argument { index: 1, value: Value::RegisterIndirect(REGISTER_PTR_TO_VM, slot_in_vm(RuntimeEnvironmentSlot::StopwatchDenominator), false) },
    //         Argument { index: 0, value: Value::RegisterIndirect(REGISTER_PTR_TO_VM, slot_in_vm(RuntimeEnvironmentSlot::StopwatchNumerator), false) },
    //     ], None);
    // }
    // Restore stack pointer in case we did not exit gracefully
    emit_ins(jit,X86Instruction::load(OperandSize::S64, REGISTER_PTR_TO_VM, RSP, X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::HostStackPointer))));
    emit_ins(jit,X86Instruction::return_near());

    // Handler for EbpfError::ExceededMaxInstructions
    jit.set_anchor(ANCHOR_THROW_EXCEEDED_MAX_INSTRUCTIONS);
    emit_set_exception_kind(jit,EbpfError::ExceededMaxInstructions);
    emit_ins(jit,X86Instruction::mov(OperandSize::S64, REGISTER_INSTRUCTION_METER, REGISTER_SCRATCH)); // REGISTER_SCRATCH = REGISTER_INSTRUCTION_METER;
    // Fall through

    // Epilogue for errors
    jit.set_anchor(ANCHOR_THROW_EXCEPTION_UNCHECKED);
    emit_ins(jit,X86Instruction::store(OperandSize::S64, REGISTER_SCRATCH, REGISTER_PTR_TO_VM, X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::Registers) + 11 * std::mem::size_of::<u64>() as i32))); // registers[11] = pc;
    emit_ins(jit,X86Instruction::jump_immediate(jit.relative_to_anchor(ANCHOR_EPILOGUE, 5)));

    // Quit gracefully
    jit.set_anchor(ANCHOR_EXIT);
    if jit.config.enable_instruction_meter {
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, REGISTER_INSTRUCTION_METER, 1, None)); // REGISTER_INSTRUCTION_METER += 1;
    }
    emit_ins(jit,X86Instruction::lea(OperandSize::S64, REGISTER_PTR_TO_VM, REGISTER_SCRATCH, Some(X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult)))));
    emit_ins(jit,X86Instruction::store(OperandSize::S64, REGISTER_MAP[0], REGISTER_SCRATCH, X86IndirectAccess::Offset(std::mem::size_of::<u64>() as i32))); // result.return_value = R0;
    emit_ins(jit,X86Instruction::alu(OperandSize::S64, 0x31, REGISTER_SCRATCH, REGISTER_SCRATCH, None)); // REGISTER_SCRATCH ^= REGISTER_SCRATCH; // REGISTER_SCRATCH = 0;
    emit_ins(jit,X86Instruction::jump_immediate(jit.relative_to_anchor(ANCHOR_EPILOGUE, 5)));

    // Handler for exceptions which report their pc
    jit.set_anchor(ANCHOR_THROW_EXCEPTION);
    // Validate that we did not reach the instruction meter limit before the exception occured
    emit_validate_instruction_count(jit,None);
    emit_ins(jit,X86Instruction::jump_immediate(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION_UNCHECKED, 5)));

    // Handler for EbpfError::CallDepthExceeded
    jit.set_anchor(ANCHOR_CALL_DEPTH_EXCEEDED);
    emit_set_exception_kind(jit,EbpfError::CallDepthExceeded);
    emit_ins(jit,X86Instruction::jump_immediate(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 5)));

    // Handler for EbpfError::CallOutsideTextSegment
    jit.set_anchor(ANCHOR_CALL_REG_OUTSIDE_TEXT_SEGMENT);
    emit_set_exception_kind(jit,EbpfError::CallOutsideTextSegment);
    emit_ins(jit,X86Instruction::load(OperandSize::S64, RSP, REGISTER_SCRATCH, X86IndirectAccess::OffsetIndexShift(-8, RSP, 0)));
    emit_ins(jit,X86Instruction::jump_immediate(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 5)));

    // Handler for EbpfError::DivideByZero
    jit.set_anchor(ANCHOR_DIV_BY_ZERO);
    emit_set_exception_kind(jit,EbpfError::DivideByZero);
    emit_ins(jit,X86Instruction::jump_immediate(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 5)));

    // Handler for EbpfError::DivideOverflow
    jit.set_anchor(ANCHOR_DIV_OVERFLOW);
    emit_set_exception_kind(jit,EbpfError::DivideOverflow);
    emit_ins(jit,X86Instruction::jump_immediate(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 5)));

    // See `ANCHOR_INTERNAL_FUNCTION_CALL_REG` for more details.
    jit.set_anchor(ANCHOR_CALL_REG_UNSUPPORTED_INSTRUCTION);
    emit_ins(jit,X86Instruction::load(OperandSize::S64, RSP, REGISTER_SCRATCH, X86IndirectAccess::OffsetIndexShift(-8, RSP, 0))); // Retrieve the current program counter from the stack
    emit_ins(jit,X86Instruction::pop(REGISTER_MAP[0])); // Restore the clobbered REGISTER_MAP[0]
    // Fall through

    // Handler for EbpfError::UnsupportedInstruction
    jit.set_anchor(ANCHOR_CALL_UNSUPPORTED_INSTRUCTION);
    if jit.config.enable_register_tracing {
        emit_ins(jit,X86Instruction::call_immediate(jit.relative_to_anchor(ANCHOR_TRACE, 5)));
    }
    emit_set_exception_kind(jit,EbpfError::UnsupportedInstruction);
    emit_ins(jit,X86Instruction::jump_immediate(jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 5)));

    // Routine for external functions
    jit.set_anchor(ANCHOR_EXTERNAL_FUNCTION_CALL);
    emit_ins(jit,X86Instruction::push_immediate(OperandSize::S64, -1)); // Used as PC value in error case, acts as stack padding otherwise
    if jit.config.enable_instruction_meter {
        emit_ins(jit,X86Instruction::store(OperandSize::S64, REGISTER_INSTRUCTION_METER, REGISTER_PTR_TO_VM, X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::DueInsnCount)))); // *DueInsnCount = REGISTER_INSTRUCTION_METER;
    }
    emit_rust_call(jit, Value::Register(REGISTER_SCRATCH), &[
        Argument { index: 5, value: Value::Register(ARGUMENT_REGISTERS[5]) },
        Argument { index: 4, value: Value::Register(ARGUMENT_REGISTERS[4]) },
        Argument { index: 3, value: Value::Register(ARGUMENT_REGISTERS[3]) },
        Argument { index: 2, value: Value::Register(ARGUMENT_REGISTERS[2]) },
        Argument { index: 1, value: Value::Register(ARGUMENT_REGISTERS[1]) },
        Argument { index: 0, value: Value::Register(REGISTER_PTR_TO_VM) },
    ], None);
    if jit.config.enable_instruction_meter {
        emit_ins(jit,X86Instruction::load(OperandSize::S64, REGISTER_PTR_TO_VM, REGISTER_INSTRUCTION_METER, X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::PreviousInstructionMeter)))); // REGISTER_INSTRUCTION_METER = *PreviousInstructionMeter;
    }

    // Test if result indicates that an error occured
    emit_result_is_err(jit,REGISTER_SCRATCH);
    emit_ins(jit,X86Instruction::pop(REGISTER_SCRATCH));
    emit_ins(jit,X86Instruction::conditional_jump_immediate(0x85, jit.relative_to_anchor(ANCHOR_EPILOGUE, 6)));
    // Store Ok value in result register
    emit_ins(jit,X86Instruction::lea(OperandSize::S64, REGISTER_PTR_TO_VM, REGISTER_SCRATCH, Some(X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult)))));
    emit_ins(jit,X86Instruction::load(OperandSize::S64, REGISTER_SCRATCH, REGISTER_MAP[0], X86IndirectAccess::Offset(8)));
    emit_ins(jit,X86Instruction::return_near());

    // Routine for prologue of emit_internal_call()
    jit.set_anchor(ANCHOR_INTERNAL_FUNCTION_CALL_PROLOGUE);
    emit_validate_instruction_count(jit,None);
    emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 5, RSP, 8 * (SCRATCH_REGS + 1) as i64, None)); // alloca
    emit_ins(jit,X86Instruction::store(OperandSize::S64, REGISTER_SCRATCH, RSP, X86IndirectAccess::OffsetIndexShift(0, RSP, 0))); // Save original REGISTER_SCRATCH
    emit_ins(jit,X86Instruction::load(OperandSize::S64, RSP, REGISTER_SCRATCH, X86IndirectAccess::OffsetIndexShift(8 * (SCRATCH_REGS + 1) as i32, RSP, 0))); // Load return address
    for (i, reg) in REGISTER_MAP.iter().skip(FIRST_SCRATCH_REG).take(SCRATCH_REGS).enumerate() {
        emit_ins(jit,X86Instruction::store(OperandSize::S64, *reg, RSP, X86IndirectAccess::OffsetIndexShift(8 * (SCRATCH_REGS - i + 1) as i32, RSP, 0))); // Push SCRATCH_REG
    }
    // Push the caller's frame pointer. The code to restore it is emitted at the end of emit_internal_call().
    emit_ins(jit,X86Instruction::store(OperandSize::S64, REGISTER_MAP[FRAME_PTR_REG], RSP, X86IndirectAccess::OffsetIndexShift(8, RSP, 0)));
    emit_ins(jit,X86Instruction::xchg(OperandSize::S64, REGISTER_SCRATCH, RSP, Some(X86IndirectAccess::OffsetIndexShift(0, RSP, 0)))); // Push return address and restore original REGISTER_SCRATCH
    // Increase env.call_depth
    let call_depth_access = X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::CallDepth));
    emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, REGISTER_PTR_TO_VM, 1, Some(call_depth_access))); // env.call_depth += 1;
    // If env.call_depth == config.max_call_depth, throw CallDepthExceeded
    emit_ins(jit,X86Instruction::cmp_immediate(OperandSize::S32, REGISTER_PTR_TO_VM, jit.config.max_call_depth as i64, Some(call_depth_access)));
    emit_ins(jit,X86Instruction::conditional_jump_immediate(0x83, jit.relative_to_anchor(ANCHOR_CALL_DEPTH_EXCEEDED, 6)));
    // Setup the frame pointer for the new frame. What we do depends on whether we're using dynamic or fixed frames.
    if jit.executable.get_sbpf_version().automatic_stack_frame_bump() {
        // With fixed frames we start the new frame at the next fixed offset
        let stack_frame_size = jit.config.stack_frame_size as i64 * if !jit.executable.get_sbpf_version().manual_stack_frame_bump() && jit.config.enable_stack_frame_gaps { 2 } else { 1 };
        emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, REGISTER_MAP[FRAME_PTR_REG], stack_frame_size, None)); // REGISTER_MAP[FRAME_PTR_REG] += stack_frame_size;
    }
    emit_ins(jit,X86Instruction::return_near());

    // Routine for emit_internal_call(Value::Register())
    // Inputs: Guest current pc in X86IndirectAccess::OffsetIndexShift(-16, RSP, 0), Guest target address in REGISTER_SCRATCH
    // Outputs: Guest current pc in X86IndirectAccess::OffsetIndexShift(-16, RSP, 0), Guest target pc in REGISTER_SCRATCH, Host target address in RIP
    jit.set_anchor(ANCHOR_INTERNAL_FUNCTION_CALL_REG);
    emit_ins(jit,X86Instruction::push(REGISTER_MAP[0], None));
    // Calculate offset relative to program_vm_addr
    emit_ins(jit,X86Instruction::load_immediate(REGISTER_MAP[0], jit.program_vm_addr as i64));
    emit_ins(jit,X86Instruction::alu(OperandSize::S64, 0x29, REGISTER_MAP[0], REGISTER_SCRATCH, None)); // guest_target_pc = guest_target_address - program_vm_addr;
    // Force alignment of guest_target_pc
    emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 4, REGISTER_SCRATCH, !(INSN_SIZE as i64 - 1), None)); // guest_target_pc &= !(INSN_SIZE - 1);
    // Bound check
    // if(guest_target_pc >= number_of_instructions * INSN_SIZE) throw CALL_OUTSIDE_TEXT_SEGMENT;
    let number_of_instructions = jit.result.pc_section.len();
    emit_ins(jit,X86Instruction::cmp_immediate(OperandSize::S64, REGISTER_SCRATCH, (number_of_instructions * INSN_SIZE) as i64, None)); // guest_target_pc.cmp(number_of_instructions * INSN_SIZE)
    emit_ins(jit,X86Instruction::conditional_jump_immediate(0x83, jit.relative_to_anchor(ANCHOR_CALL_REG_OUTSIDE_TEXT_SEGMENT, 6)));
    // Calculate the guest_target_pc (dst / INSN_SIZE) to update REGISTER_INSTRUCTION_METER
    // and as target_pc for potential ANCHOR_CALL_REG_UNSUPPORTED_INSTRUCTION
    let shift_amount = INSN_SIZE.trailing_zeros();
    debug_assert_eq!(INSN_SIZE, 1 << shift_amount);
    emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0xc1, 5, REGISTER_SCRATCH, shift_amount as i64, None)); // guest_target_pc /= INSN_SIZE;
    // Load host_target_address offset from result.pc_section
    emit_ins(jit,X86Instruction::load_immediate(REGISTER_MAP[0], jit.result.pc_section.as_ptr() as i64)); // host_target_address = result.pc_section;
    emit_ins(jit,X86Instruction::load(OperandSize::S32, REGISTER_MAP[0], REGISTER_MAP[0], X86IndirectAccess::OffsetIndexShift(0, REGISTER_SCRATCH, 2))); // host_target_address = result.pc_section[guest_target_pc];
    // Check destination is valid
    emit_ins(jit,X86Instruction::test_immediate(OperandSize::S32, REGISTER_MAP[0], 1 << 31, None)); // host_target_address & (1 << 31)
    emit_ins(jit,X86Instruction::conditional_jump_immediate(0x85, jit.relative_to_anchor(ANCHOR_CALL_REG_UNSUPPORTED_INSTRUCTION, 6))); // If host_target_address & (1 << 31) != 0, throw UnsupportedInstruction
    emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S32, 0x81, 4, REGISTER_MAP[0], i32::MAX as i64, None)); // host_target_address &= (1 << 31) - 1;
    // A version of `emit_profile_instruction_count(None);` which reads pc from the stack
    emit_ins(jit,X86Instruction::alu(OperandSize::S64, 0x2b, REGISTER_INSTRUCTION_METER, RSP, Some(X86IndirectAccess::OffsetIndexShift(-8, RSP, 0)))); // instruction_meter -= guest_current_pc;
    emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 5, REGISTER_INSTRUCTION_METER, 1, None)); // instruction_meter -= 1;
    emit_ins(jit,X86Instruction::alu(OperandSize::S64, 0x01, REGISTER_SCRATCH, REGISTER_INSTRUCTION_METER, None)); // instruction_meter += guest_target_pc;
    // Offset host_target_address by result.text_section
    emit_ins(jit,X86Instruction::mov_mmx(OperandSize::S64, REGISTER_SCRATCH, MM0));
    emit_ins(jit,X86Instruction::load_immediate(REGISTER_SCRATCH, jit.result.text_section.as_ptr() as i64)); // REGISTER_SCRATCH = result.text_section;
    emit_ins(jit,X86Instruction::alu(OperandSize::S64, 0x01, REGISTER_SCRATCH, REGISTER_MAP[0], None)); // host_target_address += result.text_section;
    emit_ins(jit,X86Instruction::mov_mmx(OperandSize::S64, MM0, REGISTER_SCRATCH));
    // Restore the clobbered REGISTER_MAP[0]
    emit_ins(jit,X86Instruction::xchg(OperandSize::S64, REGISTER_MAP[0], RSP, Some(X86IndirectAccess::OffsetIndexShift(0, RSP, 0)))); // Swap REGISTER_MAP[0] and host_target_address
    emit_ins(jit,X86Instruction::return_near()); // Tail call to host_target_address

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
        if *anchor_base == 0 { // AccessType::Load
            let load = match len {
                1 => MemoryMapping::load::<u8> as *const u8 as i64,
                2 => MemoryMapping::load::<u16> as *const u8 as i64,
                4 => MemoryMapping::load::<u32> as *const u8 as i64,
                8 => MemoryMapping::load::<u64> as *const u8 as i64,
                _ => unreachable!()
            };
            emit_rust_call(jit, Value::Constant64(load, false), &[
                Argument { index: 2, value: Value::Register(REGISTER_SCRATCH) }, // Specify first as the src register could be overwritten by other arguments
                Argument { index: 3, value: Value::Constant64(0, false) }, // pc is set later
                Argument { index: 1, value: Value::RegisterPlusConstant32(REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::MemoryMapping), false) },
                Argument { index: 0, value: Value::RegisterPlusConstant32(REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult), false) },
            ], None);
        } else { // AccessType::Store
            if *anchor_base == 8 {
                // Second half of emit_sanitized_load_immediate(stack_slot_of_value_to_store, constant)
                emit_ins(jit,X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, RSP, lower_key, Some(X86IndirectAccess::OffsetIndexShift(-80, RSP, 0))));
            }
            let store = match len {
                1 => MemoryMapping::store::<u8> as *const u8 as i64,
                2 => MemoryMapping::store::<u16> as *const u8 as i64,
                4 => MemoryMapping::store::<u32> as *const u8 as i64,
                8 => MemoryMapping::store::<u64> as *const u8 as i64,
                _ => unreachable!()
            };
            emit_rust_call(jit,Value::Constant64(store, false), &[
                Argument { index: 3, value: Value::Register(REGISTER_SCRATCH) }, // Specify first as the src register could be overwritten by other arguments
                Argument { index: 2, value: Value::RegisterIndirect(RSP, -8, false) },
                Argument { index: 4, value: Value::Constant64(0, false) }, // pc is set later
                Argument { index: 1, value: Value::RegisterPlusConstant32(REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::MemoryMapping), false) },
                Argument { index: 0, value: Value::RegisterPlusConstant32(REGISTER_PTR_TO_VM, jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult), false) },
            ], None);
        }

        // Throw error if the result indicates one
        emit_result_is_err(jit, REGISTER_SCRATCH);
        emit_ins(jit,X86Instruction::pop(REGISTER_SCRATCH)); // REGISTER_SCRATCH = pc
        emit_ins(jit,X86Instruction::xchg(OperandSize::S64, REGISTER_SCRATCH, RSP, Some(X86IndirectAccess::OffsetIndexShift(0, RSP, 0)))); // Swap return address and pc
        emit_ins(jit,X86Instruction::conditional_jump_immediate(0x85, jit.relative_to_anchor(ANCHOR_THROW_EXCEPTION, 6)));

        if *anchor_base == 0 { // AccessType::Load
            // unwrap() the result into REGISTER_SCRATCH
            emit_ins(jit,X86Instruction::load(OperandSize::S64, REGISTER_PTR_TO_VM, REGISTER_SCRATCH, X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult) + std::mem::size_of::<u64>() as i32)));
        }

        emit_ins(jit,X86Instruction::return_near());
    }
}

pub(crate) fn emit_result_is_err<C: ContextObject>(
    jit: &mut JitCompiler<C>,
    destination: u8,
) {
    let ok = ProgramResult::Ok(0);
    let ok_discriminant = ok.discriminant();
    emit_ins(jit, X86Instruction::lea(OperandSize::S64, REGISTER_PTR_TO_VM, destination, Some(X86IndirectAccess::Offset(jit.slot_in_vm(RuntimeEnvironmentSlot::ProgramResult)))));
    emit_ins(jit, X86Instruction::cmp_immediate(OperandSize::S64, destination, ok_discriminant as i64, Some(X86IndirectAccess::Offset(0))));
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
    for reg in saved_registers.iter() {
        emit_ins(jit, X86Instruction::push(*reg, None));
    }

    let stack_arguments = arguments.len().saturating_sub(ARGUMENT_REGISTERS.len()) as i64;
    if stack_arguments % 2 != 0 {
        // If we're going to pass an odd number of stack args we need to pad
        // to preserve alignment
        emit_ins(jit, X86Instruction::alu_immediate(OperandSize::S64, 0x81, 5, RSP, 8, None));
    }

    // Pass arguments
    for argument in arguments {
        let is_stack_argument = argument.index >= ARGUMENT_REGISTERS.len();
        let dst = if is_stack_argument {
            RSP // Never used
        } else {
            ARGUMENT_REGISTERS[argument.index]
        };
        match argument.value {
            Value::Register(reg) => {
                if is_stack_argument {
                    emit_ins(jit, X86Instruction::push(reg, None));
                } else if reg != dst {
                    emit_ins(jit, X86Instruction::mov(OperandSize::S64, reg, dst));
                }
            },
            Value::RegisterIndirect(reg, offset, user_provided) => {
                debug_assert!(!user_provided);
                if is_stack_argument {
                    debug_assert!(reg != RSP);
                    emit_ins(jit, X86Instruction::push(reg, Some(X86IndirectAccess::Offset(offset))));
                } else if reg == RSP {
                    emit_ins(jit, X86Instruction::load(OperandSize::S64, RSP, dst, X86IndirectAccess::OffsetIndexShift(offset, RSP, 0)));
                } else {
                    emit_ins(jit, X86Instruction::load(OperandSize::S64, reg, dst, X86IndirectAccess::Offset(offset)));
                }
            },
            Value::RegisterPlusConstant32(reg, offset, user_provided) => {
                debug_assert!(!user_provided);
                if is_stack_argument {
                    emit_ins(jit, X86Instruction::push(reg, None));
                    emit_ins(jit, X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, RSP, offset as i64, Some(X86IndirectAccess::OffsetIndexShift(0, RSP, 0))));
                } else if reg == RSP {
                    emit_ins(jit, X86Instruction::lea(OperandSize::S64, RSP, dst, Some(X86IndirectAccess::OffsetIndexShift(offset, RSP, 0))));
                } else {
                    emit_ins(jit, X86Instruction::lea(OperandSize::S64, reg, dst, Some(X86IndirectAccess::Offset(offset))));
                }
            },
            Value::RegisterPlusConstant64(reg, offset, user_provided) => {
                debug_assert!(!user_provided);
                if is_stack_argument {
                    emit_ins(jit, X86Instruction::push(reg, None));
                    emit_ins(jit, X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, RSP, offset, Some(X86IndirectAccess::OffsetIndexShift(0, RSP, 0))));
                } else {
                    emit_ins(jit, X86Instruction::load_immediate(dst, offset));
                    emit_ins(jit, X86Instruction::alu(OperandSize::S64, 0x01, reg, dst, None));
                }
            },
            Value::Constant64(value, user_provided) => {
                debug_assert!(!user_provided && !is_stack_argument);
                emit_ins(jit, X86Instruction::load_immediate(dst, value));
            },
        }
    }

    match target {
        Value::Register(reg) => {
            emit_ins(jit, X86Instruction::call_reg(reg, None));
        },
        Value::Constant64(value, user_provided) => {
            debug_assert!(!user_provided);
            emit_ins(jit, X86Instruction::load_immediate(RAX, value));
            emit_ins(jit, X86Instruction::call_reg(RAX, None));
        },
        _ => {
            #[cfg(debug_assertions)]
            unreachable!();
        }
    }

    // Save returned value in result register
    if let Some(reg) = result_reg {
        emit_ins(jit, X86Instruction::mov(OperandSize::S64, RAX, reg));
    }

    // Restore registers from stack
    emit_ins(jit, X86Instruction::alu_immediate(OperandSize::S64, 0x81, 0, RSP,
        if stack_arguments % 2 != 0 { stack_arguments + 1 } else { stack_arguments } * 8, None));

    for reg in saved_registers.iter().rev() {
        emit_ins(jit, X86Instruction::pop(*reg));
    }
}


pub(crate) fn resolve_jumps<C: ContextObject>(
    jit: &mut JitCompiler<C>,
) {
    // Relocate forward jumps
    for jump in &jit.text_section_jumps {
        let destination = &jit.result.text_section[jit.result.pc_section[jump.target_pc] as usize & (i32::MAX as u32 as usize)] as *const u8;
        let offset_value = 
            unsafe { destination.offset_from(jump.location) } as i32 // Relative jump
            - mem::size_of::<i32>() as i32; // Jump from end of instruction
        unsafe { ptr::write_unaligned(jump.location as *mut i32, offset_value); }
    }
}

