#![no_std]

pub mod uart16550;
pub use embedded_hal_nb::{
    nb::{Error},
    serial::{Read, Write},
};
mod ptr_with_priv;
#[cfg(not(any(target_arch = "riscv64", target_arch = "riscv32")))]
compile_error! {"This crate only compiles on riscv targets!"}
