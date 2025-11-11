use core::{marker::PhantomData, ptr::NonNull};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ReadOnlyPtr<T> {
    ptr: NonNull<T>,
    _p: PhantomData<*const T>,
}

impl<T> ReadOnlyPtr<T> {
    #[inline(always)]
    pub(crate) const fn new(ptr: NonNull<T>) -> Self {
        Self {
            ptr,
            _p: PhantomData,
        }
    }
    #[inline(always)]
    pub(crate) unsafe fn read_volatile(self) -> T {
        unsafe { self.ptr.read_volatile() }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct WriteOnlyPtr<T> {
    ptr: NonNull<T>,
    _p: PhantomData<*mut T>,
}

impl<T> WriteOnlyPtr<T> {
    #[inline(always)]
    pub(crate) const fn new(ptr: NonNull<T>) -> Self {
        Self {
            ptr,
            _p: PhantomData,
        }
    }
    #[inline(always)]
    pub(crate) unsafe fn write_volatile(self, val: T) {
        unsafe {
            self.ptr.write_volatile(val);
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ReadWritePtr<T> {
    ptr: NonNull<T>,
    _p: PhantomData<*mut T>,
}

impl<T> ReadWritePtr<T> {
    #[inline(always)]
    pub(crate) const fn new(ptr: NonNull<T>) -> Self {
        Self {
            ptr,
            _p: PhantomData,
        }
    }
    #[inline(always)]
    pub(crate) unsafe fn read_volatile(self) -> T {
        unsafe { self.ptr.read_volatile() }
    }
    #[inline(always)]
    pub(crate) unsafe fn write_volatile(self, val: T) {
        unsafe {
            self.ptr.write_volatile(val);
        }
    }
}
