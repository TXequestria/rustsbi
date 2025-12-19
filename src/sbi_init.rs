use super::{AtomicU32, Ordering};
static COLD_HART_BOOT_DONE: AtomicU32 = AtomicU32::new(1);


#[inline]
pub(super) unsafe fn init_cold_boot() -> ! {
    todo!()
}

#[inline]
pub(super) unsafe fn init_warm_boot() -> ! {
    loop {
        let done = COLD_HART_BOOT_DONE.load(Ordering::Acquire);
        if done == 0 {
            break;
        }
        core::hint::spin_loop();
    }

    todo!()
}
