#![no_std]
#![no_main]

use bump_alloc::BumpAlloc;
use core::arch::asm;
extern crate alloc;
use core::mem::MaybeUninit;
mod early_init;
mod trap_handler;
use core::usize;

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { asm!("wfi", options(nomem, nostack, preserves_flags)) };
    }
}

static mut BUMP_ALLOCATOR: MaybeUninit<BumpAlloc> = MaybeUninit::zeroed();

unsafe extern "C" fn main() {
    let hartid = unsafe {
        let hartid: usize;
        asm!(
            "csrr {hartid},mhartid",
            hartid = out(reg) hartid,
            options(nomem, nostack, preserves_flags)
        );
        hartid
    };
    if hartid != 0 {
        loop {
            unsafe { asm!("wfi", options(nomem, nostack, preserves_flags)) }
        }
    }
    use serial::{Write, uart16550::Builder};
    let mut serial = unsafe { Builder::new().build() };
    for byte in "Hello World!".bytes() {
        let _ = serial.write(byte);
        let _ = serial.flush();
    }
    loop {
        unsafe { asm!("wfi", options(nomem, nostack, preserves_flags)) }
    }
}
