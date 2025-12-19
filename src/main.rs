#![no_std]
#![no_main]

extern crate alloc;

mod early_init;
mod sbi_init;
mod trap_handler;
mod serial;
use core::sync::atomic::{AtomicU32, Ordering};
#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {
        asm_wrapper::wfi();
    }
}

unsafe extern "C" fn main() -> ! {

    let hartid = asm_wrapper::hartid();

    let mut guard = loop {
        if let Ok(g) = serial::SERIAL.try_lock() {
            break g;
        }
    };
    use core::fmt::Write;
    let _ = write!(guard,"Hello from hart {hartid}\n");
    drop(guard);
    loop {}
}


