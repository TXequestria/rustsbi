#![no_std]
use core::arch::asm;

#[inline(always)]
pub fn hartid() -> usize {
    let id;
    unsafe {
        asm!(
            "csrr {id},mhartid",
            id = out(reg) id,
            options(nomem,nostack,preserves_flags)
        );
    }
    id
}

#[inline(always)]
pub fn wfi() {
    unsafe { asm!("wfi", options(nomem, nostack, preserves_flags)) };
}

