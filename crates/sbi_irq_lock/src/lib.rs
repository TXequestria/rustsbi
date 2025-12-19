#![no_std]

#[cfg(not(any(target_arch = "riscv32", target_arch = "riscv64")))]
compile_error! {"This crate only compiles on riscv targets"}

#[cfg(target_arch = "riscv32")]
macro_rules! ptr {
    () => {
        "w"
    };
}

#[cfg(target_arch = "riscv64")]
macro_rules! ptr {
    () => {
        "d"
    };
}

pub use nb;
pub mod rwlock;
pub mod spin_lock;