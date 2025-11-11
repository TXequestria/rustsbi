use core::{
    sync::atomic::{AtomicUsize,Ordering},
    cell::UnsafeCell,
    convert::Infallible,
    fmt::{Debug, Display},
    hint::spin_loop,
    marker::PhantomData,
    mem::forget,
    ops::{Deref, DerefMut},
    write,
};
use super::nb::Error;
type LockResult<T> = super::nb::Result<T, Infallible>;

pub struct RwLock<T: ?Sized> {
    counter: AtomicUsize,
    data: UnsafeCell<T>,
}

impl<T: Sized> RwLock<T> {
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            counter: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
        }
    }
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: ?Sized> RwLock<T> {
    #[inline]
    pub const fn clear_counter(&mut self) {
        self.counter = AtomicUsize::new(0);
    }
    #[inline]
    pub fn try_read_lock(&self) -> LockResult<ReadLockGuard<'_, T>> {
        loop {
            let old_counter = self.counter.load(Ordering::Acquire);
            let Some(new_counter) = old_counter.checked_add(1) else {
                return Err(Error::WouldBlock)
            };
            let result = self.counter.compare_exchange_weak(
                old_counter, 
                new_counter, 
                Ordering::AcqRel, 
                Ordering::Acquire
            );
            if result.is_ok() {
                unsafe {
                    return Ok(
                        ReadLockGuard::from_lock(self)
                    )
                }
            }
        }
    }
    #[inline]
    pub fn read_lock(&self) -> ReadLockGuard<'_, T> {
        loop {
            match self.try_read_lock() {
                Ok(lock) => return lock,
                _ => {
                    spin_loop();
                }
            }
        }
    }
    #[inline]
    pub fn try_rw_lock(&self) -> LockResult<RwLockGuard<'_, T>> {
        let result = self.counter.compare_exchange(
            0, 
            usize::MAX, 
            Ordering::AcqRel, 
            Ordering::Acquire
        );
        if result.is_ok() {
            unsafe {
                Ok(
                    RwLockGuard::from_lock(self)
                )
            }
        }else{
            Err(Error::WouldBlock)
        }
    }
    #[inline]
    pub fn rw_lock(&self) -> RwLockGuard<'_, T> {
        loop {
            match self.try_rw_lock() {
                Ok(lock) => return lock,
                _ => {
                    spin_loop();
                }
            }
        }
    }
}

unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}
unsafe impl<T: ?Sized + Sync> Sync for RwLock<T> {}

#[repr(transparent)]
pub struct ReadLockGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
    // make it !Send, we copied this from std::sync::Mutex
    _marker: PhantomData<(&'a T, *const ())>,
}

impl<'a, T: ?Sized + 'a> ReadLockGuard<'a, T> {
    // safety: caller must ensure to update the lock counter
    #[inline(always)]
    const unsafe fn from_lock(lock: &'a RwLock<T>) -> Self {
        Self {
            lock,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: ?Sized + 'a> Deref for ReadLockGuard<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized + 'a> Drop for ReadLockGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        // we are a read lock, so counter must > 0
        self.lock.counter.fetch_sub(1, Ordering::AcqRel);
    }
}

impl<'a, T: ?Sized + Debug + 'a> Debug for ReadLockGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", *self)
    }
}

impl<'a, T: ?Sized + Display + 'a> Display for ReadLockGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", *self)
    }
}

unsafe impl<'a, T: ?Sized + Sync + 'a> Sync for ReadLockGuard<'a, T> {}

#[repr(transparent)]
pub struct RwLockGuard<'a, T: ?Sized + 'a> {
    lock: &'a RwLock<T>,
    // make it !Send, we copied this from std::sync::Mutex
    _marker: PhantomData<(&'a mut T, *mut ())>,
}

impl<'a, T: ?Sized + 'a> RwLockGuard<'a, T> {
    // safety: caller must ensure that counter == 0 and set it to usize::MAX
    #[inline]
    const unsafe fn from_lock(lock: &'a RwLock<T>) -> Self {
        Self {
            _marker: PhantomData,
            lock,
        }
    }
    #[inline]
    pub fn downgrade(self) -> ReadLockGuard<'a, T> {
        let lock = self.lock;
        // we forget self to avoid calling drop on self,
        // which will set counter to 0
        forget(self);
        // we are the only rw lock, after downgrading, there will only be one read lock
        lock.counter.store(1, Ordering::Release);
        unsafe {
            ReadLockGuard::from_lock(lock)
        }
    }
}

impl<'a, T: ?Sized + 'a> Deref for RwLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized + 'a> DerefMut for RwLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: ?Sized + 'a> Drop for RwLockGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.counter.store(0, Ordering::Release);
    }
}

impl<'a, T: ?Sized + Debug + 'a> Debug for RwLockGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", *self)
    }
}

impl<'a, T: ?Sized + Display + 'a> Display for RwLockGuard<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", *self)
    }
}

unsafe impl<'a, T: ?Sized + Sync + 'a> Sync for RwLockGuard<'a, T> {}
