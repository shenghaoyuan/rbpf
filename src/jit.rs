//! Just-in-time compiler (Linux x86, macOS x86)

// Derived from uBPF <https://github.com/iovisor/ubpf>
// Copyright 2015 Big Switch Networks, Inc
//      (uBPF: JIT algorithm, originally in C)
// Copyright 2016 6WIND S.A. <quentin.monnet@6wind.com>
//      (Translation to Rust, MetaBuff addition)
// Copyright 2020 Solana Maintainers <maintainers@solana.com>
//
// Licensed under the Apache License, Version 2.0 <http://www.apache.org/licenses/LICENSE-2.0> or
// the MIT license <http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

#![allow(clippy::arithmetic_side_effects)]

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

#[cfg(target_arch = "x86_64")]
use crate::x86::*;

#[cfg(target_arch = "riscv64")]
use crate::riscv::*;

use crate::{
    ebpf::{self, FIRST_SCRATCH_REG, FRAME_PTR_REG, INSN_SIZE, SCRATCH_REGS},
    elf::Executable,
    error::{EbpfError, ProgramResult},
    jit_backend::arch_backend::*,
    memory_management::{
        allocate_pages, free_pages, get_system_page_size, protect_pages, round_to_page_size,
    },
    memory_region::MemoryMapping,
    vm::{get_runtime_environment_key, Config, ContextObject, EbpfVm, RuntimeEnvironmentSlot},
};

/// The maximum machine code length in bytes of a program with no guest instructions
pub const MAX_EMPTY_PROGRAM_MACHINE_CODE_LENGTH: usize = 4096;
/// The maximum machine code length in bytes of a single guest instruction
pub const MAX_MACHINE_CODE_LENGTH_PER_INSTRUCTION: usize = 110;
/// The maximum machine code length in bytes of an instruction meter checkpoint
pub const MACHINE_CODE_PER_INSTRUCTION_METER_CHECKPOINT: usize = 24;
/// The maximum machine code length of the randomized padding
pub const MAX_START_PADDING_LENGTH: usize = 256;

/// The program compiled to native host machinecode
pub struct JitProgram {
    /// OS page size in bytes and the alignment of the sections
    page_size: usize,
    /// Byte offset in the text_section for each BPF instruction
    pub pc_section: &'static mut [u32],
    /// The arch machinecode
    pub text_section: &'static mut [u8],
    /// Number of emitted host machine instructions
    pub machine_instruction_count: usize,
}

impl JitProgram {
    fn new(pc: usize, code_size: usize) -> Result<Self, EbpfError> {
        let page_size = get_system_page_size();
        let pc_loc_table_size = round_to_page_size(pc * std::mem::size_of::<u32>(), page_size);
        let over_allocated_code_size = round_to_page_size(code_size, page_size);
        unsafe {
            let raw = allocate_pages(pc_loc_table_size + over_allocated_code_size)?;
            Ok(Self {
                page_size,
                pc_section: std::slice::from_raw_parts_mut(raw.cast::<u32>(), pc),
                text_section: std::slice::from_raw_parts_mut(
                    raw.add(pc_loc_table_size),
                    over_allocated_code_size,
                ),
                machine_instruction_count: 0,
            })
        }
    }

    fn seal(&mut self, text_section_usage: usize) -> Result<(), EbpfError> {
        if self.page_size == 0 {
            return Ok(());
        }
        let raw = self.pc_section.as_ptr() as *mut u8;
        let pc_loc_table_size =
            round_to_page_size(std::mem::size_of_val(self.pc_section), self.page_size);
        let over_allocated_code_size = round_to_page_size(self.text_section.len(), self.page_size);
        let code_size = round_to_page_size(text_section_usage, self.page_size);
        unsafe {
            // Fill with debugger traps
            std::ptr::write_bytes(
                raw.add(pc_loc_table_size).add(text_section_usage),
                0xcc,
                code_size - text_section_usage,
            );
            if over_allocated_code_size > code_size {
                free_pages(
                    raw.add(pc_loc_table_size).add(code_size),
                    over_allocated_code_size - code_size,
                )?;
            }
            self.text_section =
                std::slice::from_raw_parts_mut(raw.add(pc_loc_table_size), text_section_usage);
            protect_pages(
                self.pc_section.as_mut_ptr().cast::<u8>(),
                pc_loc_table_size,
                false,
            )?;
            protect_pages(self.text_section.as_mut_ptr(), code_size, true)?;
        }
        Ok(())
    }

    pub(crate) fn invoke<C: ContextObject>(
        &self,
        _config: &Config,
        vm: &mut EbpfVm<C>,
        registers: [u64; 12],
    ) {
        unsafe {
            let runtime_environment = std::ptr::addr_of_mut!(*vm)
                .cast::<u64>()
                .offset(get_runtime_environment_key() as isize);
            let instruction_meter =
                (vm.previous_instruction_meter as i64).wrapping_add(registers[11] as i64);
            let entrypoint = &self.text_section
                [self.pc_section[registers[11] as usize] as usize & (i32::MAX as u32 as usize)]
                as *const u8;
            invoke_trampoline(
                vm,
                runtime_environment,
                instruction_meter,
                entrypoint,
                &registers,
            );
        }
    }

    /// The length of the host machinecode in bytes
    pub fn machine_code_length(&self) -> usize {
        self.text_section.len()
    }

    /// The number of emitted host machine instructions
    pub fn machine_instruction_count(&self) -> usize {
        self.machine_instruction_count
    }

    /// The total memory used in bytes rounded up to page boundaries
    pub fn mem_size(&self) -> usize {
        let pc_loc_table_size =
            round_to_page_size(std::mem::size_of_val(self.pc_section), self.page_size);
        let code_size = round_to_page_size(self.text_section.len(), self.page_size);
        pc_loc_table_size + code_size
    }
}

impl Drop for JitProgram {
    fn drop(&mut self) {
        let pc_loc_table_size =
            round_to_page_size(std::mem::size_of_val(self.pc_section), self.page_size);
        let code_size = round_to_page_size(self.text_section.len(), self.page_size);
        if pc_loc_table_size + code_size > 0 {
            unsafe {
                let _ = free_pages(
                    self.pc_section.as_ptr() as *mut u8,
                    pc_loc_table_size + code_size,
                );
            }
        }
    }
}

impl Debug for JitProgram {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.write_fmt(format_args!("JitProgram {:?}", self as *const _))
    }
}

impl PartialEq for JitProgram {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self as *const _, other as *const _)
    }
}

// Used to define subroutines and then call them
// See JitCompiler::set_anchor() and JitCompiler::relative_to_anchor()
pub const ANCHOR_TRACE: usize = 0;
pub const ANCHOR_THROW_EXCEEDED_MAX_INSTRUCTIONS: usize = 1;
pub const ANCHOR_EPILOGUE: usize = 2;
pub const ANCHOR_THROW_EXCEPTION_UNCHECKED: usize = 3;
pub const ANCHOR_EXIT: usize = 4;
pub const ANCHOR_THROW_EXCEPTION: usize = 5;
pub const ANCHOR_CALL_DEPTH_EXCEEDED: usize = 6;
pub const ANCHOR_CALL_REG_OUTSIDE_TEXT_SEGMENT: usize = 7;
pub const ANCHOR_DIV_BY_ZERO: usize = 8;
pub const ANCHOR_DIV_OVERFLOW: usize = 9;
pub const ANCHOR_CALL_REG_UNSUPPORTED_INSTRUCTION: usize = 10;
pub const ANCHOR_CALL_UNSUPPORTED_INSTRUCTION: usize = 11;
pub const ANCHOR_EXTERNAL_FUNCTION_CALL: usize = 12;
pub const ANCHOR_INTERNAL_FUNCTION_CALL_PROLOGUE: usize = 13;
pub const ANCHOR_INTERNAL_FUNCTION_CALL_REG: usize = 14;
pub const ANCHOR_TRANSLATE_MEMORY_ADDRESS: usize = 21;
pub const ANCHOR_COUNT: usize = 34; // Update me when adding or removing anchors

/// Bit width of an instruction operand
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum OperandSize {
    /// Empty
    S0 = 0,
    /// 8 bit
    S8 = 8,
    /// 16 bit
    S16 = 16,
    /// 32 bit
    S32 = 32,
    /// 64 bit
    S64 = 64,
}

pub enum Value {
    Register(u8),
    RegisterIndirect(u8, i32, bool),
    RegisterPlusConstant32(u8, i32, bool),
    RegisterPlusConstant64(u8, i64, bool),
    Constant64(i64, bool),
}

pub struct Argument {
    pub index: usize,
    pub value: Value,
}

#[derive(Debug)]
pub struct Jump {
    pub location: *const u8,
    pub target_pc: usize,
}

/* Explanation of the Instruction Meter

    The instruction meter serves two purposes: First, measure how many BPF instructions are
    executed (profiling) and second, limit this number by stopping the program with an exception
    once a given threshold is reached (validation). One approach would be to increment and
    validate the instruction meter before each instruction. However, this would heavily impact
    performance. Thus, we only profile and validate the instruction meter at branches.

    For this, we implicitly sum up all the instructions between two branches.
    It is easy to know the end of such a slice of instructions, but how do we know where it
    started? There could be multiple ways to jump onto a path which all lead to the same final
    branch. This is, where the integral technique comes in. The program is basically a sequence
    of instructions with the x-axis being the program counter (short "pc"). The cost function is
    a constant function which returns one for every point on the x axis. Now, the instruction
    meter needs to calculate the definite integral of the cost function between the start and the
    end of the current slice of instructions. For that we need the indefinite integral of the cost
    function. Fortunately, the derivative of the pc is the cost function (it increases by one for
    every instruction), thus the pc is an antiderivative of the the cost function and a valid
    indefinite integral. So, to calculate an definite integral of the cost function, we just need
    to subtract the start pc from the end pc of the slice. This difference can then be subtracted
    from the remaining instruction counter until it goes below zero at which point it reaches
    the instruction meter limit. Ok, but how do we know the start of the slice at the end?

    The trick is: We do not need to know. As subtraction and addition are associative operations,
    we can reorder them, even beyond the current branch. Thus, we can simply account for the
    amount the start will subtract at the next branch by already adding that to the remaining
    instruction counter at the current branch. So, every branch just subtracts its current pc
    (the end of the slice) and adds the target pc (the start of the next slice) to the remaining
    instruction counter. This way, no branch needs to know the pc of the last branch explicitly.
    Another way to think about this trick is as follows: The remaining instruction counter now
    measures what the maximum pc is, that we can reach with the remaining budget after the last
    branch.

    One problem are conditional branches. There are basically two ways to handle them: Either,
    only do the profiling if the branch is taken, which requires two jumps (one for the profiling
    and one to get to the target pc). Or, always profile it as if the jump to the target pc was
    taken, but then behind the conditional branch, undo the profiling (as it was not taken). We
    use the second method and the undo profiling is the same as the normal profiling, just with
    reversed plus and minus signs.

    Another special case to keep in mind are return instructions. They would require us to know
    the return address (target pc), but in the JIT we already converted that to be a host address.
    Of course, one could also save the BPF return address on the stack, but an even simpler
    solution exists: Just count as if you were jumping to an specific target pc before the exit,
    and then after returning use the undo profiling. The trick is, that the undo profiling now
    has the current pc which is the BPF return address. The virtual target pc we count towards
    and undo again can be anything, so we just set it to zero.
*/

/// Temporary object which stores the compilation context
pub struct JitCompiler<'a, C: ContextObject> {
    pub result: JitProgram,
    pub text_section_jumps: Vec<Jump>,
    pub anchors: [*const u8; ANCHOR_COUNT],
    pub offset_in_text_section: usize,
    pub executable: &'a Executable<C>,
    pub program: &'a [u8],
    pub program_vm_addr: u64,
    pub config: &'a Config,
    pub pc: usize,
    pub last_instruction_meter_validation_pc: usize,
    pub next_noop_insertion: u32,
    pub noop_range: Uniform<u32>,
    runtime_environment_key: i32,
    pub immediate_value_key: i64,
    pub diversification_rng: SmallRng,
    pub stopwatch_is_active: bool,
}

#[rustfmt::skip]
impl<'a, C: ContextObject> JitCompiler<'a, C> {
    /// Constructs a new compiler and allocates memory for the compilation output
    pub fn new(executable: &'a Executable<C>)->Result<Self,EbpfError>{
        let config = executable.get_config();
        let (program_vm_addr, program) = executable.get_text_bytes();

        // Scan through program to find actual number of instructions
        let mut pc = 0;
        if !executable.get_sbpf_version().disable_lddw() {
            while (pc + 1) * ebpf::INSN_SIZE <= program.len() {
                let insn = ebpf::get_insn_unchecked(program, pc);
                pc += match insn.opc {
                    ebpf::LD_DW_IMM => 2,
                    _ => 1,
                };
            }
        } else {
            pc = program.len() / ebpf::INSN_SIZE;
        }

        let mut code_length_estimate = MAX_EMPTY_PROGRAM_MACHINE_CODE_LENGTH + MAX_START_PADDING_LENGTH + MAX_MACHINE_CODE_LENGTH_PER_INSTRUCTION * pc;
        if config.noop_instruction_rate != 0 {
            code_length_estimate += code_length_estimate / config.noop_instruction_rate as usize;
        }
        if config.instruction_meter_checkpoint_distance != 0 {
            code_length_estimate += pc / config.instruction_meter_checkpoint_distance * MACHINE_CODE_PER_INSTRUCTION_METER_CHECKPOINT;
        }
        // Relative jump destinations limit the maximum output size
        debug_assert!(code_length_estimate < (i32::MAX as usize));

        let runtime_environment_key = get_runtime_environment_key();
        let mut diversification_rng = SmallRng::from_rng(thread_rng()).map_err(|_| EbpfError::JitNotCompiled)?;
        // let immediate_value_key = diversification_rng.gen::<i64>();
        let immediate_value_key = 0; // fixed value to avoid the influence of the sanitization of immediate values on the performance measurements of the JIT

        Ok(Self {
            result: JitProgram::new(pc, code_length_estimate)?,
            text_section_jumps: vec![],
            anchors: [std::ptr::null(); ANCHOR_COUNT],
            offset_in_text_section: 0,
            executable,
            program_vm_addr,
            program,
            config,
            pc: 0,
            last_instruction_meter_validation_pc: 0,
            next_noop_insertion: if config.noop_instruction_rate == 0 { u32::MAX } else { diversification_rng.gen_range(0..config.noop_instruction_rate * 2) },
            noop_range: Uniform::new_inclusive(0, config.noop_instruction_rate * 2),
            runtime_environment_key,
            immediate_value_key,
            diversification_rng,
            stopwatch_is_active: false,
        })
    }

    /// Compiles the given executable, consuming the compiler
    pub fn compile(mut self) -> Result<JitProgram, EbpfError> {
        // Randomized padding at the start before random intervals begin
        if self.config.noop_instruction_rate != 0 {
            for _ in 0..self.diversification_rng.gen_range(0..MAX_START_PADDING_LENGTH) {
                emit_noop(&mut self);
            }
        }
        
        emit_subroutines(&mut self);

        while self.pc * ebpf::INSN_SIZE < self.program.len(){
            if self.offset_in_text_section + MAX_MACHINE_CODE_LENGTH_PER_INSTRUCTION * 2 >= self.result.text_section.len() {
                return Err(EbpfError::ExhaustedTextSegment(self.pc));
            }
            let mut insn = ebpf::get_insn_unchecked(self.program, self.pc);
            self.result.pc_section[self.pc] = self.offset_in_text_section as u32;

            // Regular instruction meter checkpoints to prevent long linear runs from exceeding their budget
            if self.last_instruction_meter_validation_pc + self.config.instruction_meter_checkpoint_distance <= self.pc {
                let current_pc = self.pc;
                emit_validate_instruction_count(&mut self, Some(current_pc));
            }
            
            if self.config.enable_register_tracing {
                emit_register_trace(&mut self);
            }

            let dst = REGISTER_MAP[insn.dst as usize];
            let src = REGISTER_MAP[insn.src as usize];
            let target_pc = (self.pc as isize + insn.off as isize + 1) as usize;

            match insn.opc{
                ebpf::LD_DW_IMM if !self.executable.get_sbpf_version().disable_lddw() => {
                    self.emit_validate_and_profile_instruction_count(Some(self.pc + 2));
                    self.pc += 1;
                    self.result.pc_section[self.pc] = unsafe { self.anchors[ANCHOR_CALL_UNSUPPORTED_INSTRUCTION].offset_from(self.result.text_section.as_ptr()) as u32 };
                    ebpf::augment_lddw_unchecked(self.program, &mut insn);
                    if self.should_sanitize_constant(insn.imm) {
                        emit_sanitized_load_immediate(&mut self, dst, insn.imm);
                    } else {
                        load_immediate(&mut self, dst, insn.imm);
                    }
                }

                // BPF_LDX class
                ebpf::LD_B_REG   if !self.executable.get_sbpf_version().move_memory_instruction_classes() =>  {
                    emit_address_translation(&mut self, Some(dst), Value::RegisterPlusConstant64(src, insn.off as i64, true), 1, None);
                },
                ebpf::LD_H_REG   if !self.executable.get_sbpf_version().move_memory_instruction_classes() =>  {
                    emit_address_translation(&mut self, Some(dst), Value::RegisterPlusConstant64(src, insn.off as i64, true), 2, None);
                },
                ebpf::LD_W_REG   if !self.executable.get_sbpf_version().move_memory_instruction_classes() =>  {
                    emit_address_translation(&mut self, Some(dst), Value::RegisterPlusConstant64(src, insn.off as i64, true), 4, None);
                },
                ebpf::LD_DW_REG  if !self.executable.get_sbpf_version().move_memory_instruction_classes() =>  {
                    emit_address_translation(&mut self, Some(dst), Value::RegisterPlusConstant64(src, insn.off as i64, true), 8, None);
                },

                // BPF_ST class
                ebpf::ST_B_IMM   if !self.executable.get_sbpf_version().move_memory_instruction_classes() =>  {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 1, Some(Value::Constant64(insn.imm, true)));
                },
                ebpf::ST_H_IMM   if !self.executable.get_sbpf_version().move_memory_instruction_classes() =>  {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 2, Some(Value::Constant64(insn.imm, true)));
                },
                ebpf::ST_W_IMM   if !self.executable.get_sbpf_version().move_memory_instruction_classes() =>  {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 4, Some(Value::Constant64(insn.imm, true)));
                },
                ebpf::ST_DW_IMM  if !self.executable.get_sbpf_version().move_memory_instruction_classes() =>  {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 8, Some(Value::Constant64(insn.imm, true)));
                },

                // BPF_STX class
                ebpf::ST_B_REG  if !self.executable.get_sbpf_version().move_memory_instruction_classes() =>  {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 1, Some(Value::Register(src)));
                },
                ebpf::ST_H_REG  if !self.executable.get_sbpf_version().move_memory_instruction_classes() =>  {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 2, Some(Value::Register(src)));
                },
                ebpf::ST_W_REG  if !self.executable.get_sbpf_version().move_memory_instruction_classes() =>  {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 4, Some(Value::Register(src)));
                },
                ebpf::ST_DW_REG  if !self.executable.get_sbpf_version().move_memory_instruction_classes() =>  {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 8, Some(Value::Register(src)));
                },

                // BPF_ALU class
                ebpf::ADD32_IMM  => {
                    emit_sanitized_add(&mut self,OperandSize::S32, dst, insn.imm);
                }
                ebpf::ADD32_REG  => {
                    emit_add_reg(&mut self, OperandSize::S32, dst, src);
                }
                ebpf::SUB32_IMM  => {
                    emit_sub_imm(&mut self, OperandSize::S32, dst, insn.imm);
                }
                ebpf::SUB32_REG  => {
                    emit_sub_reg(&mut self, OperandSize::S32, dst, src);
                }
                ebpf::MUL32_IMM if !self.executable.get_sbpf_version().enable_pqr() => {
                    emit_mul_imm(&mut self, OperandSize::S32, dst, insn.imm);
                }
                ebpf::DIV32_IMM if !self.executable.get_sbpf_version().enable_pqr() => {
                    emit_div_imm(&mut self, OperandSize::S32, insn, dst);
                },
                ebpf::MOD32_IMM if !self.executable.get_sbpf_version().enable_pqr() => {
                    emit_mod_imm(&mut self, OperandSize::S32, insn, dst);
                }
                ebpf::LD_1B_REG  if self.executable.get_sbpf_version().move_memory_instruction_classes() => {
                    emit_address_translation(&mut self, Some(dst), Value::RegisterPlusConstant64(src, insn.off as i64, true), 1, None);
                },
                ebpf::MUL32_REG if !self.executable.get_sbpf_version().enable_pqr() => {
                    emit_mul_reg(&mut self, OperandSize::S32, dst, src);
                }
                ebpf::DIV32_REG if !self.executable.get_sbpf_version().enable_pqr() => {
                    emit_div_reg(&mut self, OperandSize::S32, insn, dst, src);
                },
                ebpf::MOD32_REG if !self.executable.get_sbpf_version().enable_pqr() => {
                    emit_mod_reg(&mut self, OperandSize::S32, insn, dst, src);
                }
                ebpf::LD_2B_REG  if self.executable.get_sbpf_version().move_memory_instruction_classes() => {
                    emit_address_translation(&mut self, Some(dst), Value::RegisterPlusConstant64(src, insn.off as i64, true), 2, None);
                },
                ebpf::OR32_IMM   => emit_or_imm(&mut self, OperandSize::S32, dst, insn.imm),
                ebpf::OR32_REG   => emit_or_reg(&mut self, OperandSize::S32, dst, src),
                ebpf::AND32_IMM   => emit_and_imm(&mut self, OperandSize::S32, dst, insn.imm),
                ebpf::AND32_REG   => emit_and_reg(&mut self, OperandSize::S32, dst, src),
                ebpf::LSH32_IMM   => emit_lsh_imm(&mut self, OperandSize::S32, dst, insn.imm),
                ebpf::LSH32_REG   => emit_lsh_reg(&mut self, OperandSize::S32, dst, src),
                ebpf::RSH32_IMM   => emit_rsh_imm(&mut self, OperandSize::S32, dst, insn.imm),
                ebpf::RSH32_REG   => emit_rsh_reg(&mut self, OperandSize::S32, dst, src),
                ebpf::NEG32      if !self.executable.get_sbpf_version().disable_neg() => emit_neg(&mut self, OperandSize::S32, dst),
                ebpf::LD_4B_REG  if self.executable.get_sbpf_version().move_memory_instruction_classes() => {
                    emit_address_translation(&mut self, Some(dst), Value::RegisterPlusConstant64(src, insn.off as i64, true), 4, None);
                },
                ebpf::LD_8B_REG  if self.executable.get_sbpf_version().move_memory_instruction_classes() => {
                    emit_address_translation(&mut self, Some(dst), Value::RegisterPlusConstant64(src, insn.off as i64, true), 8, None);
                },
                ebpf::XOR32_IMM   => emit_xor_imm(&mut self, OperandSize::S32, dst, insn.imm),
                ebpf::XOR32_REG   => emit_xor_reg(&mut self, OperandSize::S32, dst, src),
                ebpf::MOV32_IMM  => {
                    if self.should_sanitize_constant(insn.imm) {
                        emit_sanitized_load_immediate(&mut self,dst, insn.imm as u32 as u64 as i64);
                    } else {
                        load_immediate(&mut self,dst, insn.imm as u32 as u64 as i64);
                    }
                }
                ebpf::MOV32_REG  => {
                    emit_mov_reg(&mut self, OperandSize::S32, dst, src);
                }
                ebpf::ARSH32_IMM   => emit_arsh_imm(&mut self, OperandSize::S32, dst, insn.imm),
                ebpf::ARSH32_REG   => emit_arsh_reg(&mut self, OperandSize::S32, dst, src),
                ebpf::LE if !self.executable.get_sbpf_version().disable_le() => {
                    emit_le(&mut self, dst, insn.imm)?;
                },
                ebpf::BE         => {
                    emit_be(&mut self, dst, insn.imm)?;
                },
                
                // BPF_ALU64 class
                ebpf::ADD64_IMM  => {
                    emit_sanitized_add(&mut self, OperandSize::S64, dst, insn.imm);
                }
                ebpf::ADD64_REG  => {
                    emit_add_reg(&mut self, OperandSize::S64, dst, src);
                }
                ebpf::SUB64_IMM  =>{
                    emit_sub_imm(&mut self, OperandSize::S64, dst, insn.imm);
                }
                ebpf::SUB64_REG  => {
                    emit_sub_reg(&mut self, OperandSize::S64, dst, src);
                }
                ebpf::MUL64_IMM if !self.executable.get_sbpf_version().enable_pqr() => {
                    emit_mul_imm(&mut self, OperandSize::S64, dst, insn.imm);
                }
                ebpf::DIV64_IMM if !self.executable.get_sbpf_version().enable_pqr() => {
                    emit_div_imm(&mut self, OperandSize::S64, insn, dst);
                },
                ebpf::MOD64_IMM if !self.executable.get_sbpf_version().enable_pqr() => {
                    emit_mod_imm(&mut self, OperandSize::S64, insn, dst);
                }
                ebpf::ST_1B_IMM  if self.executable.get_sbpf_version().move_memory_instruction_classes() => {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 1, Some(Value::Constant64(insn.imm, true)));
                },
                ebpf::ST_2B_IMM  if self.executable.get_sbpf_version().move_memory_instruction_classes() => {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 2, Some(Value::Constant64(insn.imm, true)));
                },
                ebpf::MUL64_REG if !self.executable.get_sbpf_version().enable_pqr() => {
                    emit_mul_reg(&mut self, OperandSize::S64, dst, src);
                }
                ebpf::DIV64_REG if !self.executable.get_sbpf_version().enable_pqr() => {
                    emit_div_reg(&mut self, OperandSize::S64, insn, dst, src);
                },
                ebpf::MOD64_REG if !self.executable.get_sbpf_version().enable_pqr() => {
                    emit_mod_reg(&mut self, OperandSize::S64, insn, dst, src);
                }
                ebpf::ST_1B_REG  if self.executable.get_sbpf_version().move_memory_instruction_classes() => {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 1, Some(Value::Register(src)));
                },
                ebpf::ST_2B_REG  if self.executable.get_sbpf_version().move_memory_instruction_classes() => {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 2, Some(Value::Register(src)));
                },
                ebpf::OR64_IMM   => emit_or_imm(&mut self, OperandSize::S64, dst, insn.imm),
                ebpf::OR64_REG   => emit_or_reg(&mut self, OperandSize::S64, dst, src),
                ebpf::AND64_IMM   => emit_and_imm(&mut self, OperandSize::S64, dst, insn.imm),
                ebpf::AND64_REG   => emit_and_reg(&mut self, OperandSize::S64, dst, src),
                ebpf::LSH64_IMM   => emit_lsh_imm(&mut self, OperandSize::S64, dst, insn.imm),
                ebpf::LSH64_REG   => emit_lsh_reg(&mut self, OperandSize::S64, dst, src),
                ebpf::RSH64_IMM   => emit_rsh_imm(&mut self, OperandSize::S64, dst, insn.imm),
                ebpf::RSH64_REG   => emit_rsh_reg(&mut self, OperandSize::S64, dst, src),
                ebpf::ST_4B_IMM  if self.executable.get_sbpf_version().move_memory_instruction_classes() => {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 4, Some(Value::Constant64(insn.imm, true)));
                },
                ebpf::NEG64      if !self.executable.get_sbpf_version().disable_neg() => emit_neg(&mut self, OperandSize::S64, dst),
                ebpf::ST_4B_REG  if self.executable.get_sbpf_version().move_memory_instruction_classes() => {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 4, Some(Value::Register(src)));
                },
                ebpf::ST_8B_IMM  if self.executable.get_sbpf_version().move_memory_instruction_classes() => {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 8, Some(Value::Constant64(insn.imm, true)));
                },
                ebpf::ST_8B_REG  if self.executable.get_sbpf_version().move_memory_instruction_classes() => {
                    emit_address_translation(&mut self, None, Value::RegisterPlusConstant64(dst, insn.off as i64, true), 8, Some(Value::Register(src)));
                },
                ebpf::XOR64_IMM   => emit_xor_imm(&mut self, OperandSize::S64, dst, insn.imm),
                ebpf::XOR64_REG   => emit_xor_reg(&mut self, OperandSize::S64, dst, src),
                ebpf::MOV64_IMM  => {
                    if self.should_sanitize_constant(insn.imm) {
                        emit_sanitized_load_immediate(&mut self, dst, insn.imm);
                    } else {
                        load_immediate(&mut self, dst, insn.imm);
                    }
                }
                ebpf::MOV64_REG  => emit_mov_reg(&mut self, OperandSize::S64, dst, src),
                ebpf::ARSH64_IMM   => emit_arsh_imm(&mut self, OperandSize::S64, dst, insn.imm),
                ebpf::ARSH64_REG   => emit_arsh_reg(&mut self, OperandSize::S64, dst, src),
                ebpf::HOR64_IMM if self.executable.get_sbpf_version().disable_lddw() => {
                    emit_hor_imm(&mut self, OperandSize::S64, dst, (insn.imm as u64).wrapping_shl(32) as i64);
                }

                // BPF_PQR class
                ebpf::LMUL32_IMM if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_lmul_imm(&mut self, OperandSize::S32, insn, dst);
                }
                ebpf::LMUL64_IMM if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_lmul_imm(&mut self, OperandSize::S64, insn, dst);
                }
                ebpf::UHMUL64_IMM if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_uhlmul_imm(&mut self, OperandSize::S64, insn, dst);
                }
                ebpf::SHMUL64_IMM if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_shlmul_imm(&mut self, OperandSize::S64, insn, dst);
                }
                ebpf::UDIV32_IMM if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_udiv_imm(&mut self, OperandSize::S32, insn, dst);
                }
                ebpf::UDIV64_IMM if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_udiv_imm(&mut self, OperandSize::S64, insn, dst);
                }
                ebpf::UREM32_IMM if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_urem_imm(&mut self, OperandSize::S32, insn, dst);
                }
                ebpf::UREM64_IMM if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_urem_imm(&mut self, OperandSize::S64, insn, dst);
                }
                ebpf::SDIV32_IMM if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_sdiv_imm(&mut self, OperandSize::S32, insn, dst);
                }
                ebpf::SDIV64_IMM if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_sdiv_imm(&mut self, OperandSize::S64, insn, dst);
                }
                ebpf::SREM32_IMM if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_srem_imm(&mut self, OperandSize::S32, insn, dst);
                }
                ebpf::SREM64_IMM if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_srem_imm(&mut self, OperandSize::S64, insn, dst);
                }
                ebpf::LMUL32_REG if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_lmul_reg(&mut self, OperandSize::S32, insn, dst, src);
                }
                ebpf::LMUL64_REG if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_lmul_reg(&mut self, OperandSize::S64, insn, dst, src);
                }
                ebpf::UHMUL64_REG if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_uhlmul_reg(&mut self, OperandSize::S64, insn, dst, src);
                }
                ebpf::SHMUL64_REG if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_shlmul_reg(&mut self, OperandSize::S64, insn, dst, src);
                }
                ebpf::UDIV32_REG if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_udiv_reg(&mut self, OperandSize::S32, insn, dst, src);
                }
                ebpf::UDIV64_REG if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_udiv_reg(&mut self, OperandSize::S64, insn, dst, src);
                }
                ebpf::UREM32_REG if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_urem_reg(&mut self, OperandSize::S32, insn, dst, src);
                }
                ebpf::UREM64_REG if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_urem_reg(&mut self, OperandSize::S64, insn, dst, src);
                }
                ebpf::SDIV32_REG if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_sdiv_reg(&mut self, OperandSize::S32, insn, dst, src);
                }
                ebpf::SDIV64_REG if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_sdiv_reg(&mut self, OperandSize::S64, insn, dst, src);
                }
                ebpf::SREM32_REG if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_srem_reg(&mut self, OperandSize::S32, insn, dst, src);
                }
                ebpf::SREM64_REG if self.executable.get_sbpf_version().enable_pqr() => {
                    emit_srem_reg(&mut self, OperandSize::S64, insn, dst, src);
                }
                
                // BPF_JMP32 class 
                ebpf::JEQ32_IMM  if self.executable.get_sbpf_version().enable_jmp32() => emit_jeq_imm(&mut self, OperandSize::S32, insn.imm, dst, target_pc),
                ebpf::JEQ32_REG  if self.executable.get_sbpf_version().enable_jmp32() => emit_jeq_reg(&mut self, OperandSize::S32, src, dst, target_pc),
                ebpf::JGT32_IMM  if self.executable.get_sbpf_version().enable_jmp32() => emit_jgt_imm(&mut self, OperandSize::S32, insn.imm, dst, target_pc),
                ebpf::JGT32_REG  if self.executable.get_sbpf_version().enable_jmp32() => emit_jgt_reg(&mut self, OperandSize::S32, src, dst, target_pc),
                ebpf::JGE32_IMM  if self.executable.get_sbpf_version().enable_jmp32() => emit_jge_imm(&mut self, OperandSize::S32, insn.imm, dst, target_pc),
                ebpf::JGE32_REG  if self.executable.get_sbpf_version().enable_jmp32() => emit_jge_reg(&mut self, OperandSize::S32, src, dst, target_pc),
                ebpf::JLT32_IMM  if self.executable.get_sbpf_version().enable_jmp32() => emit_jlt_imm(&mut self, OperandSize::S32, insn.imm, dst, target_pc),
                ebpf::JLT32_REG  if self.executable.get_sbpf_version().enable_jmp32() => emit_jlt_reg(&mut self, OperandSize::S32, src, dst, target_pc),
                ebpf::JLE32_IMM  if self.executable.get_sbpf_version().enable_jmp32() => emit_jle_imm(&mut self, OperandSize::S32, insn.imm, dst, target_pc),
                ebpf::JLE32_REG  if self.executable.get_sbpf_version().enable_jmp32() => emit_jle_reg(&mut self, OperandSize::S32, src, dst, target_pc),
                ebpf::JSET32_IMM if self.executable.get_sbpf_version().enable_jmp32() => emit_jset_imm(&mut self, OperandSize::S32, insn.imm, dst, target_pc),
                ebpf::JSET32_REG if self.executable.get_sbpf_version().enable_jmp32() => emit_jset_reg(&mut self, OperandSize::S32, src, dst, target_pc),
                ebpf::JNE32_IMM  if self.executable.get_sbpf_version().enable_jmp32() => emit_jne_imm(&mut self, OperandSize::S32, insn.imm, dst, target_pc),
                ebpf::JNE32_REG  if self.executable.get_sbpf_version().enable_jmp32() => emit_jne_reg(&mut self, OperandSize::S32, src, dst, target_pc),
                ebpf::JSGT32_IMM if self.executable.get_sbpf_version().enable_jmp32() => emit_jsgt_imm(&mut self, OperandSize::S32, insn.imm, dst, target_pc),
                ebpf::JSGT32_REG if self.executable.get_sbpf_version().enable_jmp32() => emit_jsgt_reg(&mut self, OperandSize::S32, src, dst, target_pc),
                ebpf::JSGE32_IMM if self.executable.get_sbpf_version().enable_jmp32() => emit_jsge_imm(&mut self, OperandSize::S32, insn.imm, dst, target_pc),
                ebpf::JSGE32_REG if self.executable.get_sbpf_version().enable_jmp32() => emit_jsge_reg(&mut self, OperandSize::S32, src, dst, target_pc),
                ebpf::JSLT32_IMM if self.executable.get_sbpf_version().enable_jmp32() => emit_jslt_imm(&mut self, OperandSize::S32, insn.imm, dst, target_pc),
                ebpf::JSLT32_REG if self.executable.get_sbpf_version().enable_jmp32() => emit_jslt_reg(&mut self, OperandSize::S32, src, dst, target_pc),
                ebpf::JSLE32_IMM if self.executable.get_sbpf_version().enable_jmp32() => emit_jsle_imm(&mut self, OperandSize::S32, insn.imm, dst, target_pc),
                ebpf::JSLE32_REG if self.executable.get_sbpf_version().enable_jmp32() => emit_jsle_reg(&mut self, OperandSize::S32, src, dst, target_pc),

                // Jump if Equal
                ebpf::JA         => emit_ja(&mut self, target_pc),
                ebpf::JEQ64_IMM  => emit_jeq_imm(&mut self, OperandSize::S64, insn.imm, dst, target_pc),
                ebpf::JEQ64_REG  => emit_jeq_reg(&mut self, OperandSize::S64, src, dst, target_pc),
                ebpf::JGT64_IMM  => emit_jgt_imm(&mut self, OperandSize::S64, insn.imm, dst, target_pc),
                ebpf::JGT64_REG  => emit_jgt_reg(&mut self, OperandSize::S64, src, dst, target_pc),
                ebpf::JGE64_IMM  => emit_jge_imm(&mut self, OperandSize::S64, insn.imm, dst, target_pc),
                ebpf::JGE64_REG  => emit_jge_reg(&mut self, OperandSize::S64, src, dst, target_pc),
                ebpf::JLT64_IMM  => emit_jlt_imm(&mut self, OperandSize::S64, insn.imm, dst, target_pc),
                ebpf::JLT64_REG  => emit_jlt_reg(&mut self, OperandSize::S64, src, dst, target_pc),
                ebpf::JLE64_IMM  => emit_jle_imm(&mut self, OperandSize::S64, insn.imm, dst, target_pc),
                ebpf::JLE64_REG  => emit_jle_reg(&mut self, OperandSize::S64, src, dst, target_pc),
                ebpf::JSET64_IMM => emit_jset_imm(&mut self, OperandSize::S64, insn.imm, dst, target_pc),
                ebpf::JSET64_REG => emit_jset_reg(&mut self, OperandSize::S64, src, dst, target_pc),
                ebpf::JNE64_IMM  => emit_jne_imm(&mut self, OperandSize::S64, insn.imm, dst, target_pc),
                ebpf::JNE64_REG  => emit_jne_reg(&mut self, OperandSize::S64, src, dst, target_pc),
                ebpf::JSGT64_IMM => emit_jsgt_imm(&mut self, OperandSize::S64, insn.imm, dst, target_pc),
                ebpf::JSGT64_REG => emit_jsgt_reg(&mut self, OperandSize::S64, src, dst, target_pc),
                ebpf::JSGE64_IMM => emit_jsge_imm(&mut self, OperandSize::S64, insn.imm, dst, target_pc),
                ebpf::JSGE64_REG => emit_jsge_reg(&mut self, OperandSize::S64, src, dst, target_pc),
                ebpf::JSLT64_IMM => emit_jslt_imm(&mut self, OperandSize::S64, insn.imm, dst, target_pc),
                ebpf::JSLT64_REG => emit_jslt_reg(&mut self, OperandSize::S64, src, dst, target_pc),
                ebpf::JSLE64_IMM => emit_jsle_imm(&mut self, OperandSize::S64, insn.imm, dst, target_pc),
                ebpf::JSLE64_REG => emit_jsle_reg(&mut self, OperandSize::S64, src, dst, target_pc),

                ebpf::CALL_IMM     => {
                    emit_call_imm(&mut self, insn);
                },
                ebpf::CALL_REG  => {
                    let target_pc = if self.executable.get_sbpf_version().callx_uses_src_reg() {
                        src
                    } else if self.executable.get_sbpf_version().callx_uses_dst_reg() {
                        dst
                    } else {
                        REGISTER_MAP[insn.imm as usize]
                    };
                    emit_internal_call(&mut self, Value::Register(target_pc));
                },
                ebpf::EXIT      =>{
                    self.emit_validate_and_profile_instruction_count(Some(0));
                    emit_exit(&mut self);
                },

                _ => return Err(EbpfError::UnsupportedInstruction),
            }

            self.pc += 1;
        }

        // Bumper in case there was no final exit 
        if self.offset_in_text_section + MAX_MACHINE_CODE_LENGTH_PER_INSTRUCTION * 2 >= self.result.text_section.len() {
            return Err(EbpfError::ExhaustedTextSegment(self.pc));
        }   
        self.emit_validate_and_profile_instruction_count(Some(self.pc + 1));
        let current_pc = self.pc as i64;
        load_immediate(&mut self, REGISTER_SCRATCH, current_pc); // Save pc
        emit_set_exception_kind(&mut self,EbpfError::ExecutionOverrun);
        emit_throw_exception(&mut self);

        resolve_jumps(&mut self);
        self.result.seal(self.offset_in_text_section)?;
        Ok(self.result)
    }

    #[inline]
    pub fn should_sanitize_constant(&mut self,value: i64) -> bool {
        if !self.config.sanitize_user_provided_values {
            return false;
        }

        match value as u64 {
            0xFFFF | 0xFFFFFF | 0xFFFFFFFF | 0xFFFFFFFFFF | 0xFFFFFFFFFFFF | 0xFFFFFFFFFFFFFF
            | 0xFFFFFFFFFFFFFFFF => false,
            v if v <= 0xFF => false,
            v if !v <= 0xFF => false,
            _ => true,
        }
    }

    #[inline]
    pub fn slot_in_vm(&self, slot: RuntimeEnvironmentSlot) -> i32 {
        8 * (slot as i32 - self.runtime_environment_key)
    }

    #[inline]
    pub(crate) fn emit<T: std::fmt::Debug>(&mut self, data: T) {
        unsafe {
            let ptr = self.result.text_section.as_ptr().add(self.offset_in_text_section);
            #[allow(clippy::cast_ptr_alignment)]
            ptr::write_unaligned(ptr as *mut T, data as T);
        }
        self.offset_in_text_section += mem::size_of::<T>();
    }
    
    pub fn emit_validate_and_profile_instruction_count(&mut self, target_pc: Option<usize>) {
        emit_validate_instruction_count(self,Some(self.pc));
        emit_profile_instruction_count(self, target_pc);
    }

    pub fn set_anchor(&mut self, anchor: usize) {
        self.anchors[anchor] = unsafe { self.result.text_section.as_ptr().add(self.offset_in_text_section) };
    }

    // instruction_length = 5 (Unconditional jump / call) for x86
    // instruction_length = 6 (Conditional jump) for x86
    // instruction_length = 4 bits for RISC-V
    #[inline]
    pub fn relative_to_anchor(&self, anchor: usize, instruction_length: usize) -> i32 {
        let instruction_end = unsafe { self.result.text_section.as_ptr().add(self.offset_in_text_section).add(instruction_length) };
        let destination = self.anchors[anchor];
        debug_assert!(!destination.is_null());
        (unsafe { destination.offset_from(instruction_end) } as i32) // Relative jump
    }

    #[inline]
    pub fn relative_to_target_pc(&mut self, target_pc: usize, instruction_length: usize) -> i32 {
        let instruction_end = unsafe { self.result.text_section.as_ptr().add(self.offset_in_text_section).add(instruction_length) };
        let destination = if self.result.pc_section[target_pc] != 0 {
            // Backward jump
            &self.result.text_section[self.result.pc_section[target_pc] as usize & (i32::MAX as u32 as usize)] as *const u8
        } else {
            // Forward jump, needs relocation
            #[cfg(target_arch = "x86_64")]
            self.text_section_jumps.push(Jump { location: unsafe { instruction_end.sub(4) }, target_pc });
            #[cfg(target_arch = "riscv64")]
            self.text_section_jumps.push(Jump { location: unsafe { instruction_end.sub(0) }, target_pc });
            return 0;
        };
        debug_assert!(!destination.is_null());
        (unsafe { destination.offset_from(instruction_end) } as i32) // Relative jump
    }
}
