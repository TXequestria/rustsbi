type Result<T> = crate::nb::Result<T, core::convert::Infallible>;
use crate::nb::Error;
use core::arch::asm;
use core::cell::UnsafeCell;
use core::concat;
use core::fmt::{Debug, Display};
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
pub struct RwLock<T: ?Sized> {
    counter: UnsafeCell<usize>,
    data: UnsafeCell<T>,
}

impl<T> RwLock<T> {
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            counter: UnsafeCell::new(0),
            data: UnsafeCell::new(data),
        }
    }
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: ?Sized> RwLock<T> {
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
    #[inline]
    pub const fn reset_counter(&mut self) {
        *self.counter.get_mut() = 0;
    }
    #[inline]
    pub fn try_read_lock(&self) -> Result<ReadGuard<'_, T>> {
        let counter = self.counter.get();
        let zero_on_success: usize;
        let saved_mie: usize;
        unsafe {
            asm!(
                // disables interrupts
                "csrrw {saved_mie},mie,zero",
                "li {result},1",
                "1:",
                concat!("lr.", ptr!(), ".aq {scratch},({counter})"),
                "addi {scratch},{scratch},1",
                // scratch wrapped around, previous value must be -1, which is RwGuard
                // jump to end while result != 0
                "beqz {scratch},2f",
                concat!("sc.", ptr!(), ".rl {result},{scratch},({counter})"),
                // on success, jump to 3:
                "beqz {result},3f",
                "j 1b",
                // the failed path, we re-enable interrupts
                "2:",
                "csrrw zero,mie,{saved_mie}",
                // the success path, do nothing
                "3:",
                result = out(reg) zero_on_success,
                saved_mie = out(reg) saved_mie,
                counter = in(reg) counter,
                scratch = out(reg) _,
                options(nostack, preserves_flags)
            );
        }
        if zero_on_success != 0 {
            Err(Error::WouldBlock)
        } else {
            unsafe { Ok(ReadGuard::new(self, saved_mie)) }
        }
    }
    #[inline]
    pub fn try_write_lock(&self) -> Result<WriteGuard<'_, T>> {
        let counter = self.counter.get();
        let zero_on_success: usize;
        let saved_mie: usize;
        unsafe {
            asm!(
                // disables interrupts
                "csrrw {saved_mie},mie,zero",
                "li {result},1",
                "1:",
                concat!("lr.", ptr!(), ".aq {scratch},({counter})"),
                // jump to failure while result != 0, because we are exclusive lock
                "bnez {scratch},2f",
                // write guard:-1
                "li {scratch},-1",
                concat!("sc.", ptr!(), ".rl {result},{scratch},({counter})"),
                // on success, jump to 3:
                "beqz {result},3f",
                "j 1b",
                // the failed path, we re-enable interrupts
                "2:",
                "csrrw zero,mie,{saved_mie}",
                // the success path, do nothing
                "3:",
                result = out(reg) zero_on_success,
                saved_mie = out(reg) saved_mie,
                counter = in(reg) counter,
                scratch = out(reg) _,
                options(nostack, preserves_flags)
            );
        }
        if zero_on_success != 0 {
            Err(Error::WouldBlock)
        } else {
            unsafe { Ok(WriteGuard::new(self, saved_mie)) }
        }
    }
}

unsafe impl<T: ?Sized + Sync> Sync for RwLock<T> {}
unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}

pub struct ReadGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
    mie: usize,
    // make it !Send +
    _marker: PhantomData<(&'a T, *mut ())>,
}

impl<'a, T: ?Sized + 'a> ReadGuard<'a, T> {
    #[inline]
    const unsafe fn new(lock: &'a RwLock<T>, mie: usize) -> Self {
        Self {
            mie,
            lock,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: ?Sized + 'a> Deref for ReadGuard<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized + Debug + 'a> Debug for ReadGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", &**self)
    }
}

impl<'a, T: ?Sized + Display + 'a> Display for ReadGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", &**self)
    }
}

impl<'a, T: ?Sized + 'a> Drop for ReadGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        let counter = self.lock.counter.get();
        let saved_mie = self.mie;
        unsafe {
            // safety: we are a read guard, we can assure that counter >= 1 and subtract by 1
            asm!(
                concat!("amoadd.", ptr!(), ".aqrl zero,{neg1},({counter})"),
                // re-enable interrupts
                "csrw mie,{saved_mie}",
                counter = in(reg) counter,
                saved_mie = in(reg) saved_mie,
                neg1 = in(reg) -1isize,
                options(nostack,preserves_flags)
            )
        }
    }
}

unsafe impl<'a, T: ?Sized + Sync + 'a> Sync for ReadGuard<'a, T> {}

pub struct WriteGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
    mie: usize,
    // make it !Send
    _marker: PhantomData<(&'a mut T, *mut ())>,
}

impl<'a, T: ?Sized + 'a> WriteGuard<'a, T> {
    #[inline]
    const unsafe fn new(lock: &'a RwLock<T>, mie: usize) -> Self {
        Self {
            lock,
            mie,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: ?Sized + Display + 'a> Display for WriteGuard<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", &**self)
    }
}

impl<'a, T: ?Sized + Debug + 'a> Debug for WriteGuard<'a, T> {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", &**self)
    }
}

impl<'a, T: ?Sized + 'a> Deref for WriteGuard<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized + 'a> DerefMut for WriteGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: ?Sized + 'a> Drop for WriteGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        let saved_mie = self.mie;
        let counter = self.lock.counter.get();
        unsafe {
            asm!(
                concat!("s", ptr!(), " zero,({counter})"),
                "csrrw zero,mie,{saved_mie}",
                "fence w,rw",
                counter = in(reg) counter,
                saved_mie = in(reg) saved_mie,
                options(nostack,preserves_flags)
            )
        }
    }
}

unsafe impl<'a, T: ?Sized + Sync + 'a> Sync for WriteGuard<'a, T> {}
