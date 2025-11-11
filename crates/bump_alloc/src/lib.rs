#![no_std]

use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    mem::MaybeUninit,
    num::NonZeroUsize,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

use sbi_alloc::{layout::NonZeroLayout, traits::SbiAlloc};

type Ptr<T> = Option<NonNull<T>>;
const CURSOR_MASK: usize = (1 << COUNTER_SHIFT) - 1;
const COUNTER_SHIFT: usize = (usize::BITS / 2 + 1) as _;
const MAX_COUNTER: usize = (1 << (usize::BITS / 2 - 1)) - 1;

pub struct BumpAlloc {
    start: NonNull<()>,
    end: NonNull<()>,
    pos_and_count: AtomicUsize,
    _marker: PhantomData<UnsafeCell<[MaybeUninit<u8>]>>,
}

unsafe impl Sync for BumpAlloc {}

unsafe impl SbiAlloc for BumpAlloc {
    #[inline]
    unsafe fn alloc(&self, layout: NonZeroLayout) -> Ptr<u8> {
        let Ok(layout) = layout.align_to(size_of::<usize>()) else {
            return None;
        };
        let layout = layout.pad_to_align();
        if self.size() < layout.size() {
            return None;
        }
        loop {
            let old_val = self.pos_and_count.load(Ordering::Acquire);
            let mut offset_in_usize = old_val & CURSOR_MASK;
            let counter = old_val >> COUNTER_SHIFT;
            if counter >= MAX_COUNTER {
                return None;
            } else if counter == 0 {
                offset_in_usize = 0;
            }
            let current_addr = self
                .start()
                .saturating_add(offset_in_usize * size_of::<usize>());
            debug_assert!(current_addr <= self.end());
            let mut this_alloc_start = current_addr;
            this_alloc_start = this_alloc_start.saturating_add(2 * size_of::<usize>());
            this_alloc_start = this_alloc_start.saturating_add(layout.align() - 1);
            // this means we overflowed
            if this_alloc_start == usize::MAX {
                return None;
            }
            this_alloc_start &= !(layout.align() - 1);
            let this_alloc_end = this_alloc_start.saturating_add(layout.size());
            if this_alloc_end > self.end() || this_alloc_end == usize::MAX {
                return None;
            }
            offset_in_usize = (this_alloc_end - self.start()) / size_of::<usize>();
            offset_in_usize &= CURSOR_MASK;
            let new_val = ((counter + 1) << COUNTER_SHIFT) | offset_in_usize;
            let result = self.pos_and_count.compare_exchange_weak(
                old_val,
                new_val,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            if result.is_err() {
                continue;
            }
            let this_alloc_ptr = this_alloc_start as *mut usize;
            unsafe {
                // logically prefix is a
                // struct prefix {
                //   usize last_addr
                //   usize this_block_size
                // }
                this_alloc_ptr.sub(2).write(current_addr);
                this_alloc_ptr.sub(1).write(layout.size());
            }
            return NonNull::new(this_alloc_ptr as _);
        }
    }
    #[inline]
    unsafe fn dealloc(&self, ptr: NonNull<u8>, layout: NonZeroLayout) {
        assert!(ptr.cast() >= self.start);
        assert!(ptr.cast() <= self.end);
        assert!(ptr.cast::<usize>().is_aligned());
        assert!(ptr.addr().get() % layout.align() == 0);
        let (last_addr, block_size) = unsafe {
            let current_ptr = ptr.cast::<usize>();
            let a = current_ptr.sub(2).read();
            let s = current_ptr.sub(1).read();
            (a, s)
        };
        let block_end_addr = ptr.addr().get().saturating_add(block_size);
        assert!(layout.size() <= block_size);
        assert_ne!(block_end_addr, usize::MAX);
        assert!(block_end_addr <= self.end());
        let cursor_this_block = (block_end_addr - self.start()) / size_of::<usize>();
        loop {
            let old_val = self.pos_and_count.load(Ordering::Acquire);
            let current_cursor = old_val & CURSOR_MASK;
            let counter = old_val >> COUNTER_SHIFT;
            let counter = counter - 1;
            // when we are the last one, reset counter
            let new_cursor = if counter == 0 {
                0
            // if no others allocated after us, reset to saved last state
            } else if current_cursor == cursor_this_block {
                (last_addr - self.start()) / size_of::<usize>()
            // others allocated, do nothing
            } else {
                current_cursor
            };
            let new_val = (counter << COUNTER_SHIFT) | (new_cursor & CURSOR_MASK);
            let result = self.pos_and_count.compare_exchange_weak(
                old_val,
                new_val,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            if result.is_ok() {
                return;
            }
        }
    }
    unsafe fn alloc_zeroed(&self, layout: NonZeroLayout) -> Ptr<u8> {
        unsafe {
            // safety: self.alloc is guaranteed to be usize-aligned
            let this_block: NonNull<usize> = self.alloc(layout)?.cast();
            let block_size = this_block.sub(1).read();
            let mut total_written = 0;
            let mut zero_cursor = this_block;
            while total_written < block_size {
                zero_cursor.write(0);
                total_written += size_of::<usize>();
                zero_cursor = zero_cursor.add(1);
            }
            Some(this_block.cast())
        }
    }
    unsafe fn realloc(
        &self,
        ptr: NonNull<u8>,
        layout: NonZeroLayout,
        new_size: NonZeroUsize,
    ) -> Ptr<u8> {
        let new_size = new_size.get();
        let old_size = layout.size();
        if new_size <= old_size {
            return Some(ptr);
        }
        let block_size = unsafe { ptr.cast::<usize>().sub(1).read() };
        if new_size <= block_size {
            return Some(ptr);
        }
        let new_size = (new_size + size_of::<usize>() - 1) & !(size_of::<usize>() - 1);
        // the new end address if we just bump up the cursor inplace
        let new_end_inplace = (ptr.as_ptr() as usize).saturating_add(new_size);
        // overflow check
        if new_end_inplace == usize::MAX {
            return None;
        }
        let cursor_this_block = self.cursor_of((ptr.as_ptr() as usize) + block_size)?;
        let cursor_inplace_bump = self.cursor_of(new_end_inplace)?;
        // try to inplace bump
        loop {
            let old_val = self.pos_and_count.load(Ordering::Acquire);
            let counter = old_val >> COUNTER_SHIFT;
            let current_cursor = old_val & CURSOR_MASK;
            if current_cursor != cursor_this_block {
                break;
            }
            // counter remains intact, because realloc in place doesn't alloc or free
            let new_val = (counter << COUNTER_SHIFT) | cursor_inplace_bump;
            let result = self.pos_and_count.compare_exchange_weak(
                old_val,
                new_val,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            if result.is_err() {
                continue;
            }
            // update the stored size
            unsafe {
                ptr.cast::<usize>().sub(1).write(new_size);
            }
            return Some(ptr);
        }
        // wecannot realloc in place, alloc new and memcpy
        let Ok(new_layout) = NonZeroLayout::new(new_size, layout.align()) else {
            return None;
        };
        unsafe {
            let new_ptr = self.alloc(new_layout)?;
            new_ptr.copy_from_nonoverlapping(ptr, layout.size());
            self.dealloc(ptr, layout);
            Some(new_ptr)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DebugInfo {
    pub counter: usize,
    pub cursor: usize,
}

impl BumpAlloc {
    #[inline]
    pub const unsafe fn new(start_addr: usize, end_addr: usize) -> Option<Self> {
        let start_addr = (start_addr + align_of::<usize>() - 1) & !(align_of::<usize>() - 1);
        let mut end_addr = end_addr;
        if start_addr >= end_addr {
            end_addr = start_addr;
        }
        let Some(start) = NonNull::new(start_addr as _) else {
            return None;
        };
        // safety: end >= start, so cannot be null
        let end = unsafe { NonNull::new_unchecked(end_addr as _) };
        Some(Self {
            start,
            end,
            pos_and_count: AtomicUsize::new(0),
            _marker: PhantomData,
        })
    }
    #[inline]
    pub fn debug_info(&self) -> DebugInfo {
        let val = self.pos_and_count.load(Ordering::Acquire);
        DebugInfo {
            counter: val >> COUNTER_SHIFT,
            cursor: val & CURSOR_MASK,
        }
    }
    #[inline(always)]
    fn start(&self) -> usize {
        self.start.as_ptr() as _
    }
    #[inline(always)]
    fn end(&self) -> usize {
        let end = self
            .start()
            .saturating_add(CURSOR_MASK * size_of::<usize>());
        end.min(self.end.as_ptr() as _)
    }
    #[inline(always)]
    fn size(&self) -> usize {
        self.end().saturating_sub(self.start())
    }
    #[inline(always)]
    fn cursor_of(&self, addr: usize) -> Option<usize> {
        if addr < self.start() || addr > self.end() {
            return None;
        }
        let cursor = (addr - self.start()) / size_of::<usize>();
        Some(cursor & CURSOR_MASK)
    }
}