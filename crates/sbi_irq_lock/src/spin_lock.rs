use core::arch::asm;
use core::cell::UnsafeCell;
use core::fmt::{Debug, Display};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
type Result<T> = crate::nb::Result<T, core::convert::Infallible>;

const NOT_LOCKED: u32 = 0x114514;
const LOCKED: u32 = 0x1919810;
pub struct SpinLock<T: ?Sized> {
    lock: UnsafeCell<u32>,
    data: UnsafeCell<T>,
}

impl<T> SpinLock<T> {
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            lock: UnsafeCell::new(NOT_LOCKED),
            data: UnsafeCell::new(data),
        }
    }
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: ?Sized> SpinLock<T> {
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
    #[inline]
    pub const fn clear_lock(&mut self) {
        *self.lock.get_mut() = NOT_LOCKED;
    }
    #[inline]
    pub fn try_lock(&self) -> Result<SpinLockGuard<'_, T>> {
        let mie:usize;
        let zero_on_success:usize;
        unsafe {
            asm!(
                "csrrw {saved_mie},mie,zero",
                "amoswap.w.aqrl {scratch},{locked},({lock})",
                "xor {scratch},{scratch},{not_locked}",
                lock = in(reg) self.lock.get(),
                locked = in(reg) LOCKED,
                not_locked = in(reg) NOT_LOCKED,
                saved_mie = out(reg) mie,
                scratch = out(reg) zero_on_success,
                options(nostack,preserves_flags)
            );
            if zero_on_success != 0 {
                asm!(
                    "csrrw zero,mie,{mie}",
                    mie = in(reg) mie,
                    options(nomem, nostack,preserves_flags)
                );
                Err(nb::Error::WouldBlock)
            }else {
                Ok(SpinLockGuard::new(self,mie))
            }
        }
    }
}

unsafe impl<T:?Sized + Sync> Sync for SpinLock<T> {}
unsafe impl<T:?Sized + Send> Send for SpinLock<T> {}

pub struct SpinLockGuard<'a, T: ?Sized + 'a> {
    mie: usize,
    lock: &'a SpinLock<T>,
    // make guard !Send
    _marker: PhantomData<(&'a mut T, *mut ())>,
}

impl<'a, T: ?Sized + 'a> SpinLockGuard<'a, T> {
    #[inline]
    const unsafe fn new(lock: &'a SpinLock<T>, mie: usize) -> Self {
        Self {
            mie,
            lock,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: ?Sized + 'a> Drop for SpinLockGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            asm!(
                "amoswap.w.rl zero,{not_locked},({lock})",
                "csrw mie,{saved_mie}",
                saved_mie = in(reg) self.mie,
                lock = in(reg) self.lock.lock.get(),
                not_locked = in(reg) NOT_LOCKED,
                options(nostack,preserves_flags)
            )
        }
    }
}

impl<'a, T: ?Sized + 'a> Deref for SpinLockGuard<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized + 'a> DerefMut for SpinLockGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        let this = &*self;
        unsafe { &mut *this.lock.data.get() }
    }
}

impl<'a, T: ?Sized + Debug + 'a> Debug for SpinLockGuard<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", &**self)
    }
}

impl<'a, T: ?Sized + Display + 'a> Display for SpinLockGuard<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", &**self)
    }
}

unsafe impl<'a, T: ?Sized + Sync + 'a> Sync for SpinLockGuard<'a, T> {}
