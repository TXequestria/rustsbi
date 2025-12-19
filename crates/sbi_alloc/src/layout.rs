extern crate alloc;
use core::num::NonZeroUsize;
use alloc::alloc::Layout;

/// equvalent to `alloc::alloc::Layout`, except that size and alignment are nonzero
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonZeroLayout {
    size: NonZeroUsize,
    align: NonZeroUsize,
}

impl NonZeroLayout {
    #[inline]
    pub const fn new(size: usize, align: usize) -> Result<Self, LayoutError> {
        if size == 0 {
            return Err(LayoutError::ZeroSize);
        }
        if align == 0 || !align.is_power_of_two() {
            return Err(LayoutError::AlignNotPowerOf2);
        }
        // safety: align is checked non zero
        let mask = unsafe { align.unchecked_sub(1) };
        let roundup = size.saturating_add(mask) & !mask;
        if roundup > isize::MAX as usize {
            return Err(LayoutError::SizeOverflow);
        }
        // safety: we checked for nonzero and power-of-2 aligned
        unsafe { Ok(Self::new_unckecked(size, align)) }
    }
    /// safety: size > 0, align > 0 and is power of 2
    #[inline(always)]
    pub(crate) const unsafe fn new_unckecked(size: usize, align: usize) -> Self {
        unsafe {
            Self {
                size: NonZeroUsize::new_unchecked(size),
                align: NonZeroUsize::new_unchecked(align),
            }
        }
    }
    #[inline(always)]
    pub const fn size(&self) -> usize {
        self.size.get()
    }
    #[inline(always)]
    pub const fn align(&self) -> usize {
        self.align.get()
    }
    #[inline(always)]
    pub const fn from_layout(layout: Layout) -> Option<Self> {
        if layout.size() == 0 {
            return None;
        }
        unsafe { Some(Self::new_unckecked(layout.size(), layout.align())) }
    }
    #[inline(always)]
    pub const fn of_type<T>() -> Option<Self> {
        Self::from_layout(Layout::new::<T>())
    }
    #[inline(always)]
    pub const fn of_value<T: ?Sized>(val: &T) -> Option<Self> {
        let layout = Layout::for_value(val);
        Self::from_layout(layout)
    }
    #[inline(always)]
    pub const fn pad_to_align(&self) -> Self {
        // safety: align > 0, not overflowing
        let mask = unsafe { self.align.get().unchecked_sub(1) };
        let size = self.size.get().saturating_add(mask);
        let size = size & !mask;
        let align = self.align.get();
        // safety: self is guaranteed not to overflow isize
        // and the alignment come from existing NonZeroLayout
        // so is valid power_of_2
        unsafe { Self::new_unckecked(size, align) }
    }
    #[inline]
    pub const fn align_to(&self, align: usize) -> Result<Self, LayoutError> {
        if align == 0 || !align.is_power_of_two() {
            return Err(LayoutError::AlignNotPowerOf2);
        }
        let old_align = self.align.get();
        let align = if old_align > align { old_align } else { align };
        let size = self.size();
        Self::new(size, align)
    }
    #[inline(always)]
    pub const fn padding_size(&self) -> usize {
        let size = self.size();
        // safety: align is NonZero, so sub 1 will not overflow
        let mask = unsafe {
            self.align().unchecked_sub(1)
        };
        let padded = size.saturating_add(mask) & !mask;
        // safety: round up so sub will not overflow
        unsafe {
            padded.unchecked_sub(size)
        }
    }
    #[inline(always)]
    pub(crate) const unsafe fn with_size_unchecked(&self,size:NonZeroUsize) -> Self {
        let align = self.align;
        Self {
            size,align
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror_no_std::Error)]
pub enum LayoutError {
    #[error("the given size is zero")]
    ZeroSize,
    #[error("the given alignment is zero or is not a power of 2")]
    AlignNotPowerOf2,
    #[error(
        "the given size would overflow isize when rounded up to the nearest multiple of the given align"
    )]
    SizeOverflow,
}

impl core::error::Error for LayoutError {}
