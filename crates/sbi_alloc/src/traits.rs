use crate::layout::NonZeroLayout;
use core::{num::NonZeroUsize, ptr::NonNull};
type Ptr<T> = Option<NonNull<T>>;
pub unsafe trait SbiAlloc: Sync {
    unsafe fn alloc(&self, layout: NonZeroLayout) -> Ptr<u8>;
    unsafe fn dealloc(&self, ptr: NonNull<u8>, layout: NonZeroLayout);
    unsafe fn alloc_zeroed(&self, layout: NonZeroLayout) -> Ptr<u8> {
        unsafe {
            let ptr = self.alloc(layout)?;
            ptr.write_bytes(0, layout.size());
            Some(ptr)
        }
    }
    /// # Safety:
    /// - `new_size` must not overflow `isize` when rounded to the nearest multiple of `align`
    unsafe fn realloc(
        &self,
        ptr: NonNull<u8>,
        layout: NonZeroLayout,
        new_size: NonZeroUsize,
    ) -> Ptr<u8> {
        unsafe {
            let new_layout = layout.with_size_unchecked(new_size);
            let new_ptr = self.alloc(new_layout)?;
            // ptr is not deallocated, so not overlapping!
            new_ptr.copy_from_nonoverlapping(ptr, layout.size().min(new_layout.size()));
            self.dealloc(ptr, layout);
            Some(new_ptr)
        }
    }
}
