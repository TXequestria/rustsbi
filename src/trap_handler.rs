use core::pin::Pin;
use sbi_trap::TrapFrame;
pub(crate) unsafe extern "C" fn trap_entry(frame: Pin<&mut TrapFrame>) {
    let mut frame = frame;
    let mepc = frame.as_mut().mepc();
    frame.as_mut().set_mepc(mepc+4);
}
