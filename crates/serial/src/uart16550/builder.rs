use super::serial_impl::NS16550;
use core::marker::PhantomData;
use core::num::NonZeroUsize;

type Result<T> = core::result::Result<T, ZeroError>;

pub struct Builder {
    addr: NonZeroUsize,
    clk: NonZeroUsize,
    baud_rate: NonZeroUsize,
    parity: Parity,
    int_rda: bool,
    int_thr_empty: bool,
    int_rls: bool,
    int_modem_status: bool,
    // None means disable
    fifo_trig: FifoTrigLevel,
    stop_bits: StopBits,
    _p: PhantomData<fn(Self) -> NS16550>,
}

impl Builder {
    // default config for qemu
    #[inline]
    pub const fn new() -> Self {
        Self {
            addr: unsafe { NonZeroUsize::new_unchecked(0x10000000) },
            clk: unsafe { NonZeroUsize::new_unchecked(0x384000) },
            baud_rate: unsafe { NonZeroUsize::new_unchecked(115200) },
            parity: Parity::None,
            int_modem_status: true,
            int_rda: true,
            int_rls: true,
            int_thr_empty: false,
            fifo_trig:FifoTrigLevel::Fourteen,
            stop_bits: StopBits::One,
            _p: PhantomData,
        }
    }
    #[inline]
    pub unsafe fn build(self) -> NS16550 {
        unsafe {
            let mut ns16550 = NS16550::new(self.addr);
            let divisor = (self.clk.get() / 16) / self.baud_rate.get();
            let divisor = divisor.max(u16::MAX as usize);
            let divisor = divisor as u16;
            let lcr = ns16550.lcr();
            // divisor latch
            lcr.write_volatile(3 | 1 << 7);
            ns16550.set_divisor(divisor);
            lcr.write_volatile(3 | self.parity.as_u8() | self.stop_bits as u8);
            // always enable fifo, and clear its contents
            let fifo = 0b111 | self.fifo_trig as u8;
            let fcr = ns16550.fcr();
            fcr.write_volatile(fifo);
            let ier = ns16550.ier();
            let enabled_int = {
                (self.int_rda as u8) | 
                ((self.int_thr_empty as u8) << 1) |
                ((self.int_rls as u8) << 2) |
                ((self.int_modem_status as u8) << 3)
            };
            ier.write_volatile(enabled_int);
            let mcr = ns16550.mcr();
            mcr.write_volatile(3);
            ns16550
        }
    }
    #[inline]
    pub const fn enable_int_rda(&mut self) -> &mut Self {
        self.int_rda = true;
        self
    }
    #[inline]
    pub const fn disable_int_rda(&mut self) -> &mut Self {
        self.int_rda = false;
        self
    }
    #[inline]
    pub const fn enable_int_thr_empty(&mut self) -> &mut Self {
        self.int_thr_empty = true;
        self
    }
    #[inline]
    pub const fn disable_int_thr_empty(&mut self) -> &mut Self {
        self.int_thr_empty = false;
        self
    }
    #[inline]
    pub const fn enable_int_rls(&mut self) -> &mut Self {
        self.int_rls = true;
        self
    }
    #[inline]
    pub const fn disable_int_rls(&mut self) -> &mut Self {
        self.int_rls = false;
        self
    }
    #[inline]
    pub const fn enable_int_modem_status(&mut self) -> &mut Self {
        self.int_modem_status = true;
        self
    }
    #[inline]
    pub const fn disable_int_modem_status(&mut self) -> &mut Self {
        self.int_modem_status = false;
        self
    }
    #[inline]
    pub const fn disable_parity(&mut self) -> &mut Self {
        self.parity = Parity::None;
        self
    }
    #[inline]
    pub const fn set_even_parity(&mut self, stick: bool) -> &mut Self {
        self.parity = Parity::Even { stick };
        self
    }
    #[inline]
    pub const fn set_odd_parity(&mut self, stick: bool) -> &mut Self {
        self.parity = Parity::Odd { stick };
        self
    }
    #[inline]
    pub const fn set_fifo_trig(&mut self, trig_level: FifoTrigLevel) -> &mut Self {
        self.fifo_trig = trig_level;
        self
    }
    #[inline]
    pub const fn set_stop_bits(&mut self, stopbits: StopBits) -> &mut Self {
        self.stop_bits = stopbits;
        self
    }
    #[inline]
    pub const fn set_addr(&mut self, addr: usize) -> Result<&mut Self> {
        let Some(addr) = NonZeroUsize::new(addr) else {
            return Err(ZeroError);
        };
        self.addr = addr;
        Ok(self)
    }
    #[inline]
    pub const fn set_clk(&mut self, clk: usize) -> Result<&mut Self> {
        let Some(clk) = NonZeroUsize::new(clk) else {
            return Err(ZeroError);
        };
        self.clk = clk;
        Ok(self)
    }
    #[inline]
    pub const fn set_baudrate(&mut self, baudrate: usize) -> Result<&mut Self> {
        let Some(baudrate) = NonZeroUsize::new(baudrate) else {
            return Err(ZeroError);
        };
        self.baud_rate = baudrate;
        Ok(self)
    }
}

#[derive(Clone, Copy)]
enum Parity {
    None,
    Even { stick: bool },
    Odd { stick: bool },
}

impl Parity {
    #[inline]
    const fn as_u8(self) -> u8 {
        let enable = 1u8 << 3;
        match self {
            Self::None => 0,
            Self::Odd { stick } => enable | ((stick as u8) << 5),
            Self::Even { stick } => enable | (1 << 4) | ((stick as u8) << 5),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FifoTrigLevel {
    One = 0,
    Four = 1 << 6,
    Eight = 2 << 6,
    Fourteen = 3 << 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopBits {
    One = 0,
    Two = 1 << 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror_no_std::Error)]
#[error("The supplied value/address is zero!")]
pub struct ZeroError;

impl core::error::Error for ZeroError {}
