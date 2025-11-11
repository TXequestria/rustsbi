extern crate alloc;
use alloc::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::mem::needs_drop;
use core::num::NonZeroUsize;
use core::ptr::{NonNull, null_mut};
type Ptr<T> = Option<NonNull<T>>;
use crate::layout::NonZeroLayout;
use crate::traits::SbiAlloc;
type FnAlloc = unsafe fn(this: NonNull<()>, layout: NonZeroLayout) -> Ptr<u8>;
type FnDealloc = unsafe fn(this: NonNull<()>, ptr: NonNull<u8>, layout: NonZeroLayout);
type FnDrop = unsafe fn(this: NonNull<()>);
type FnRealloc = unsafe fn(
    this: NonNull<()>,
    ptr: NonNull<u8>,
    layout: NonZeroLayout,
    new_size: NonZeroUsize,
) -> Ptr<u8>;

#[inline(always)]
const fn map_ptr<T>(ptr: Ptr<T>) -> *mut T {
    match ptr {
        None => null_mut(),
        Some(p) => p.as_ptr(),
    }
}

#[inline]
pub unsafe fn set_global_allocator<A: SbiAlloc + Sized + 'static>(allocator: *mut A) {
    if let Some(alloc) = NonNull::new(allocator) {
        unsafe {
            let vtable = Vtable::from_raw(alloc);
            GLOBAL_ALLOCATOR.replace(vtable);
        }
    } else {
        unsafe {
            GLOBAL_ALLOCATOR.drop_old();
        }
    }
}

struct Vtable {
    this: NonNull<()>,
    alloc: FnAlloc,
    dealloc: FnDealloc,
    alloc_zeroed: FnAlloc,
    realloc: FnRealloc,
    drop: Option<FnDrop>,
}

impl Vtable {
    // like box::from_raw(), it creates an owning, unique vtable
    // that may drop the value behind the pointer
    #[inline(always)]
    const unsafe fn from_raw<A: SbiAlloc + Sized + 'static>(alloc: NonNull<A>) -> Self {
        let this = alloc.cast::<()>();
        let alloc = |this: NonNull<()>, layout| {
            let this = this.cast::<A>();
            unsafe { this.as_ref().alloc(layout) }
        };
        let dealloc = |this: NonNull<()>, ptr, layout| {
            let this = this.cast::<A>();
            unsafe {
                this.as_ref().dealloc(ptr, layout);
            }
        };
        let alloc_zeroed = |this: NonNull<()>, layout| {
            let this = this.cast::<A>();
            unsafe { this.as_ref().alloc_zeroed(layout) }
        };
        let realloc = |this: NonNull<()>, ptr, layout, new_size| {
            let this = this.cast::<A>();
            unsafe { this.as_ref().realloc(ptr, layout, new_size) }
        };
        let mut drop = None;
        if needs_drop::<A>() {
            let func = |this: NonNull<()>| unsafe {
                this.cast::<A>().drop_in_place();
            };
            drop = Some(func as FnDrop);
        }
        Self {
            this,
            alloc,
            dealloc,
            alloc_zeroed,
            realloc,
            drop,
        }
    }
}

impl Drop for Vtable {
    #[inline(always)]
    fn drop(&mut self) {
        if let Some(drop) = self.drop {
            unsafe { drop(self.this) }
        }
    }
}

#[repr(transparent)]
struct SbiGlobal {
    vtable: UnsafeCell<Option<Vtable>>,
}

unsafe impl Sync for SbiGlobal {}

impl SbiGlobal {
    const fn new() -> Self {
        Self {
            vtable: UnsafeCell::new(None),
        }
    }
    #[inline(always)]
    unsafe fn replace(&self, vtable: Vtable) {
        unsafe {
            (*self.vtable.get()) = Some(vtable);
        }
    }
    // not thread safe!
    #[inline(always)]
    unsafe fn drop_old(&self) {
        unsafe {
            let _ = (*self.vtable.get()).take();
        }
    }
}

unsafe impl GlobalAlloc for SbiGlobal {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let vtable = unsafe { (&*self.vtable.get()).as_ref() };
        let Some(vtable) = vtable else {
            return null_mut();
        };
        let Some(layout) = NonZeroLayout::from_layout(layout) else {
            return layout.align() as _;
        };
        unsafe { map_ptr((vtable.alloc)(vtable.this, layout)) }
    }
    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let vtable = unsafe { (&*self.vtable.get()).as_ref() };
        let Some(vtable) = vtable else { return };
        let Some(layout) = NonZeroLayout::from_layout(layout) else {
            return;
        };
        let Some(ptr) = NonNull::new(ptr) else { return };
        unsafe {
            (vtable.dealloc)(vtable.this, ptr, layout);
        }
    }
    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let vtable = unsafe { (&*self.vtable.get()).as_ref() };
        let Some(vtable) = vtable else {
            return null_mut();
        };
        let Some(layout) = NonZeroLayout::from_layout(layout) else {
            return layout.align() as _;
        };
        unsafe { map_ptr((vtable.alloc_zeroed)(vtable.this, layout)) }
    }
    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let vtable = unsafe { (&*self.vtable.get()).as_ref() };
        let Some(vtable) = vtable else {
            return null_mut();
        };
        let ptr = NonNull::new(ptr);
        let align = layout.align();
        let new_size = NonZeroUsize::new(new_size);
        let layout = NonZeroLayout::from_layout(layout);
        match (ptr, layout, new_size) {
            // null to ZST, allocate a dangling pointer
            (None, _, None) | (_, None, None) => align as _,
            // null to nonzero size, alloc new
            (None, _, Some(size)) | (_, None, Some(size)) => unsafe {
                // safety: align comes from existing Layout, so is valid
                // new size is nonzero
                let layout = NonZeroLayout::new_unckecked(size.get(), align);
                map_ptr((vtable.alloc)(vtable.this, layout))
            },
            // nonzero nonnull to ZST, free and allocate dangling pointer
            (Some(ptr), Some(layout), None) => unsafe {
                (vtable.dealloc)(vtable.this, ptr, layout);
                align as _
            },
            (Some(ptr), Some(layout), Some(new_size)) => unsafe {
                map_ptr((vtable.realloc)(vtable.this, ptr, layout, new_size))
            },
        }
    }
}

impl Drop for SbiGlobal {
    #[inline(always)]
    fn drop(&mut self) {
        let _ = self.vtable.get_mut().take();
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: SbiGlobal = SbiGlobal::new();