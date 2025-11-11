#![no_std]

#[cfg(not(any(
    target_arch = "riscv64",
    target_arch = "riscv32"
)))]
compile_error!{"This crate only compiles on riscv targets"}

pub use nb;
pub mod rw_lock;