use crate::{nb, ptr_with_priv::*, traits};
use core::num::NonZeroUsize;
use core::ptr::NonNull;
pub struct NS16550 {
    base_addr: NonNull<u8>,
    buffer: [u8; 16],
    buf_len: u8,
}

impl NS16550 {
    #[inline(always)]
    pub(super) const unsafe fn new(addr: NonZeroUsize) -> Self {
        Self {
            base_addr: NonNull::without_provenance(addr),
            buffer: [0u8; _],
            buf_len: 0,
        }
    }
    #[inline(always)]
    const fn thr(&mut self) -> ReadWritePtr<u8> {
        ReadWritePtr::new(self.base_addr)
    }
    #[inline(always)]
    pub(super) const fn ier(&mut self) -> ReadWritePtr<u8> {
        ReadWritePtr::new(unsafe { self.base_addr.byte_add(1) })
    }
    #[inline(always)]
    pub(super) const fn iir(&self) -> ReadOnlyPtr<u8> {
        ReadOnlyPtr::new(unsafe { self.base_addr.byte_add(2) })
    }
    #[inline(always)]
    pub(super) const fn fcr(&mut self) -> WriteOnlyPtr<u8> {
        WriteOnlyPtr::new(unsafe { self.base_addr.byte_add(2) })
    }
    #[inline(always)]
    pub(super) const fn lcr(&mut self) -> ReadWritePtr<u8> {
        ReadWritePtr::new(unsafe { self.base_addr.byte_add(3) })
    }
    #[inline(always)]
    pub(super) const fn mcr(&mut self) -> WriteOnlyPtr<u8> {
        WriteOnlyPtr::new(unsafe { self.base_addr.byte_add(4) })
    }
    #[inline(always)]
    const fn lsr(&mut self) -> ReadOnlyPtr<u8> {
        ReadOnlyPtr::new(unsafe { self.base_addr.byte_add(5) })
    }
    #[inline(always)]
    pub(super) const fn msr(&mut self) -> ReadOnlyPtr<u8> {
        ReadOnlyPtr::new(unsafe { self.base_addr.byte_add(6) })
    }
    #[inline(always)]
    pub(super) unsafe fn set_divisor(&mut self, divisor: u16) {
        let lsb = divisor as u8;
        let msb = (divisor >> 8) as u8;
        unsafe {
            self.base_addr.write_volatile(lsb);
            self.base_addr.byte_add(1).write_volatile(msb);
        }
    }
    // #[inline(always)]
    // pub(super) unsafe fn get_divisor(&self) -> u16 {
    //     let lsb = unsafe {
    //         self.base_addr.read_volatile()
    //     };
    //     let msb = unsafe {
    //         self.base_addr.byte_add(1).read()
    //     };
    //     (lsb as u16) | ((msb as u16) << 8)
    // }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror_no_std::Error)]
pub enum SerialError {
    #[error("Overrun")]
    Overrun,
    #[error(
        "Frame Error{}", 
        if *overrun {
            " with Overrun"
        } else {
            ""
        }
    )]
    FrameErr { overrun: bool },
    #[error(
        "Parity Error{}", 
        if *overrun {
            " with Overrun"
        } else {
            ""
        }
    )]
    ParityErr { overrun: bool },
    #[error(
        "Parity Error{}", 
        if *overrun {
            " with Overrun"
        } else {
            ""
        }
    )]
    BreakInt { overrun: bool },
}

impl From<SerialError> for core::fmt::Error {
    #[inline]
    fn from(_: SerialError) -> Self {
        Self
    }
}

impl SerialError {
    #[inline(always)]
    const fn from_lsr(lsr: u8) -> Option<Self> {
        if (lsr >> 7) == 0 {
            return None;
        }
        let overrun = (lsr & 2) > 0;
        if ((lsr >> 4) & 1) > 0 {
            return Some(Self::BreakInt { overrun });
        }
        if ((lsr >> 3) & 1) > 0 {
            return Some(Self::FrameErr { overrun });
        }
        if ((lsr >> 2) & 1) > 0 {
            return Some(Self::ParityErr { overrun });
        }
        // if neither happened above, then it's gotta be overrun error
        Some(Self::Overrun)
    }
    #[inline(always)]
    const fn needs_discard(&self) -> bool {
        match self {
            Self::Overrun => false,
            _ => true,
        }
    }
}

impl core::error::Error for SerialError {}

use embedded_hal_nb::serial::{self as hal_serial, Write};
impl hal_serial::Error for SerialError {
    fn kind(&self) -> hal_serial::ErrorKind {
        use hal_serial::ErrorKind;
        match self {
            Self::Overrun => ErrorKind::Overrun,
            Self::BreakInt { overrun: _ } => ErrorKind::Other,
            Self::FrameErr { overrun: _ } => ErrorKind::FrameFormat,
            Self::ParityErr { overrun: _ } => ErrorKind::Parity,
        }
    }
}

impl hal_serial::ErrorType for NS16550 {
    type Error = SerialError;
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Interrupts {
    ReceiverLineStatus,
    ReceiverDataAvailable,
    TimeOut,
    THREmpty,
    ModemStatus,
}

impl NS16550 {
    #[inline(always)]
    pub fn get_interrupts(&self) -> Option<Interrupts> {
        let iir = self.iir();
        let byte = unsafe { iir.read_volatile() };
        match byte & 0b111 {
            3 => Some(Interrupts::ReceiverLineStatus),
            2 => Some(Interrupts::ReceiverDataAvailable),
            6 => Some(Interrupts::TimeOut),
            1 => Some(Interrupts::THREmpty),
            0 => Some(Interrupts::ModemStatus),
            _ => None,
        }
    }
    #[inline(always)]
    pub fn get_modem_status(&mut self) -> u8 {
        unsafe { self.msr().read_volatile() }
    }
}

impl Drop for NS16550 {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // reset value
            self.mcr().write_volatile(0);
            // enable divisor latch， force break
            self.lcr().write_volatile(3 | 3 << 6);
            // set divisor to 0, effectively disables the uart
            self.set_divisor(0);
            // disable divisor latch
            self.lcr().write_volatile(3);
            // reset to default value and clear contents
            self.fcr().write_volatile((3 << 6) | (3 << 1));
            self.ier().write_volatile(0);
        }
    }
}

impl traits::Read for NS16550 {
    #[inline]
    fn read(&mut self) -> nb::Result<u8, SerialError> {
        use nb::Error;
        let lsr = unsafe { self.lsr().read_volatile() };
        // no data available
        if (lsr & 1) == 0 {
            return Err(Error::WouldBlock);
        }
        // has data, but errored
        if let Some(error) = SerialError::from_lsr(lsr) {
            if error.needs_discard() {
                unsafe {
                    self.thr().read_volatile();
                }
            }
            Err(Error::Other(error))
        } else {
            let byte = unsafe { self.thr().read_volatile() };
            Ok(byte)
        }
    }
}

impl traits::Write for NS16550 {
    #[inline]
    fn write(&mut self, word: u8) -> nb::Result<(), SerialError> {
        let len = self.buf_len as usize;
        if len < 16 {
            self.buffer[len] = word;
            self.buf_len += 1;
        } else {
            self.flush()?;
            self.buffer[0] = word;
            self.buf_len = 1;
        }
        Ok(())
    }
    #[inline]
    fn flush(&mut self) -> nb::Result<(), SerialError> {
        use nb::Error;
        if self.buf_len == 0 {
            return Ok(());
        }
        let lsr = unsafe { self.lsr().read_volatile() };
        // has received data
        if (lsr & 1) > 0 {
            // that data has errored
            if let Some(error) = SerialError::from_lsr(lsr) {
                if error.needs_discard() {
                    // discards errored byte
                    unsafe {
                        self.thr().read_volatile();
                    }
                }
                return Err(Error::Other(error));
            }
        }
        if ((lsr >> 5) & 3) == 0 {
            return Err(Error::WouldBlock);
        }
        let thr = self.thr();
        let len = self.buf_len as usize;
        for (index, byte) in self.buffer.iter().enumerate() {
            if index >= len {
                break;
            }
            unsafe {
                thr.write_volatile(*byte);
            }
        }
        self.buf_len = 0;
        Ok(())
    }
}

unsafe impl Sync for NS16550 {}
unsafe impl Send for NS16550 {}

impl core::fmt::Write for NS16550 {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.as_bytes() {
            nb::block!(self.write(*byte))?;
        }
        nb::block!(self.flush())?;
        Ok(())
    }
}
