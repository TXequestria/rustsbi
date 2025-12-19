#![no_std]

#[macro_use]
mod macros;

use core::marker::PhantomPinned;
use core::pin::Pin;
use core::ptr::NonNull;

/// a snapshot of caller-saved registers and some csrs
/// 
/// callee-saved registers are saved by calling extern "C" trap_handler function
/// 
/// this struct is pinned since it's validity depends heavily on it's address relative to sp
/// 
/// and nested trap is chained together by a linked list
#[repr(align(16))]
pub struct TrapFrame {
    ra: usize,
    t0: usize,
    t1: usize,
    t2: usize,
    a0: usize,
    a1: usize,
    a2: usize,
    a3: usize,
    a4: usize,
    a5: usize,
    a6: usize,
    a7: usize,
    t3: usize,
    t4: usize,
    t5: usize,
    t6: usize,

    mepc: usize,
    mstatus: usize,
    #[cfg(target_arch = "riscv32")]
    mstatush: usize,

    mcause: usize,
    mtval: usize,
    mtval2: usize,
    mtinst: usize,
    prev_context: Option<NonNull<Self>>,
    #[cfg(target_arch = "riscv64")]
    mstatush: (),
    _pin: PhantomPinned,
}

impl core::fmt::Debug for TrapFrame {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[derive(Debug)]
        #[allow(unused)]
        struct SbiArgs {
            a0: usize,
            a1: usize,
            a2: usize,
            a3: usize,
            a4: usize,
            a5: usize,
            a6: usize,
            a7: usize,
        }
        #[allow(unused)]
        #[derive(Debug)]
        struct Csrs {
            mepc: usize,
            mstatus: u64,
            mcause: usize,
            mtval: usize,
            mtval2: usize,
            mtinst: usize,
        }
        let mut next = Some(NonNull::from_ref(self));
        let mut depth = 0;
        while let Some(frame) = next {
            let frame = unsafe {frame.as_ref()};
            let args = SbiArgs {
                a0: frame.a0(),
                a1: frame.a1(),
                a2: frame.a2(),
                a3: frame.a3(),
                a4: frame.a4(),
                a5: frame.a5(),
                a6: frame.a6(),
                a7: frame.a7(),
            };
            let csrs = Csrs {
                mepc: frame.mepc(),
                mstatus: frame.mstatus(),
                mcause: frame.mcause(),
                mtinst: frame.mtinst(),
                mtval: frame.mtval(),
                mtval2: frame.mtval2(),
            };
            write!(
                f,
                concat!(stringify!(TrapFrame {{ depth:{}, args: {:?}, csrs:{:?} }}),"\n"),
                depth, args, csrs
            )?;
            depth += 1;
            next = frame.prev_context;
        }
        Ok(())
    }
}

impl TrapFrame {
    #[inline(always)]
    pub const fn set_a0(self: Pin<&mut Self>, a0:usize) {
        unsafe { self.get_unchecked_mut().a0 = a0}
    }
    #[inline(always)]
    pub const fn set_a1(self: Pin<&mut Self>, a1:usize) {
        unsafe { self.get_unchecked_mut().a1 = a1}
    }
    #[inline(always)]
    pub const fn set_mepc(self: Pin<&mut Self>, mepc:usize) {
        unsafe { self.get_unchecked_mut().mepc = mepc}
    }
    #[inline(always)]
    pub const fn set_mstatus(self: Pin<&mut Self>, mstatus:u64) {
        let this = unsafe {self.get_unchecked_mut()};
        // we need to set mdt because when returning
        // if a trap happened during restoring csr, 
        // mepc will be clobbered
        // so mdt flag must always be set
        #[cfg(target_arch = "riscv64")]
        {
            const MDT_MASK:usize = 1<<42;
            this.mstatus = MDT_MASK | (mstatus as usize);
        }
        #[cfg(target_arch = "riscv32")]
        {
            const MDT_MASK:usize = 1<<10;
            let mstatush = MDT_MASK | (mstatus >> 32) as usize;
            let mstatus = mstatus as usize;
            this.mstatus = mstatus;
            this.mstatush = mstatush;
        }
    }
}

// read-only fields
impl TrapFrame {
    #[inline(always)]
    pub const fn a0(&self) -> usize {
        self.a0
    }
    #[inline(always)]
    pub const fn a1(&self) -> usize {
        self.a1
    }
    #[inline(always)]
    pub const fn a2(&self) -> usize {
        self.a2
    }
    #[inline(always)]
    pub const fn a3(&self) -> usize {
        self.a3
    }
    #[inline(always)]
    pub const fn a4(&self) -> usize {
        self.a4
    }
    #[inline(always)]
    pub const fn a5(&self) -> usize {
        self.a5
    }
    #[inline(always)]
    pub const fn a6(&self) -> usize {
        self.a6
    }
    #[inline(always)]
    pub const fn a7(&self) -> usize {
        self.a7
    }
    #[inline(always)]
    pub const fn mepc(&self) -> usize {
        self.mepc
    }
    #[inline(always)]
    pub const fn mcause(&self) -> usize {
        self.mcause
    }
    #[inline(always)]
    pub const fn mtinst(&self) -> usize {
        self.mtinst
    }
    #[inline(always)]
    pub const fn mtval(&self) -> usize {
        self.mtval
    }
    #[inline(always)]
    pub const fn mtval2(&self) -> usize {
        self.mtval2
    }
    #[inline(always)]
    pub const fn mstatus(&self) -> u64 {
        #[cfg(target_arch = "riscv64")]
        return self.mstatus as u64;
        #[cfg(target_arch = "riscv32")]
        {
            let mstatus = self.mstatus as u64;
            let mstatush = self.mstatush as u64;

            return (mstatush << 32) | mstatus;
        }
    }
}

pub type TrapHandler = unsafe extern "C" fn(trap_frame: Pin<&mut TrapFrame>);

/// the trap entry function which will be loaded to mtvec
/// 
/// this trap entry will perfrom the following:
/// 
/// save general registers and mepc,mcause,mstatus(H),mtval,mtval2 and mtinst
/// 
/// it will detect nested trap and save prev trap context in an Option<NonNull<TrapFrame>>
/// 
/// it will call trap_handler saved in mscratch
/// 
/// if the trap come from U/S/H mode, it will swtch to exception stack
/// 
/// if the trap is M mode nested trap, it will not swicth stack
pub type TrapEntry = unsafe extern "C" fn() -> !;

#[derive(Clone, Copy)]
pub struct SbiScratch {
    spill_t0: usize,

    // save gp,sp,tp from U/S mode
    saved_gp: usize,
    saved_tp: usize,
    saved_sp: usize,

    m_mode_gp: usize,
    m_mode_tp: usize,
    stack_top: usize,
    // if we have nested trap, this pointer points to the last trap
    context: Option<NonNull<TrapFrame>>,
    trap_handler: TrapHandler,
}

mod no_vector;
pub use no_vector::setup_scratch_and_trap_handler;

