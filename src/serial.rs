use core::hint::spin_loop;

use sbi_irq_lock::spin_lock::SpinLock;
use serial::uart16550::{Builder, NS16550};

pub(crate) struct Serial {
    serial: Option<NS16550>,
}

impl core::fmt::Write for Serial {
    #[inline]
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let Some(serial) = self.serial.as_mut() else {
            return Err(core::fmt::Error);
        };
        serial.write_str(s)
    }
}

pub(crate) static SERIAL: SpinLock<Serial> = SpinLock::new(Serial { serial: None });

pub(crate) unsafe fn init_serial(builder: Builder) {
    loop {
        match SERIAL.try_lock() {
            Ok(mut guard) => {
                (*guard).serial = unsafe { Some(builder.build()) };
                return;
            }
            _ => spin_loop(),
        }
    }
}
