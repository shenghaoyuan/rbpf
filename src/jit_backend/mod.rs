#[cfg(target_arch = "x86_64")]
pub mod x86_jit;

#[cfg(target_arch = "riscv64")]
pub mod riscv_jit;

#[cfg(target_arch = "x86_64")]
pub use x86_jit as arch_backend;

#[cfg(target_arch = "riscv64")]
pub use riscv_jit as arch_backend;
