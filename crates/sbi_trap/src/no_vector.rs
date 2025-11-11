use super::{SbiScratch,TrapFrame,TrapHandler,TrapEntry};
use macros::mangled_symbol;

unsafe extern "C" {
    #[link_name = mangled_symbol!(_trap_entry_no_vector)]
    unsafe fn entry_no_vector() -> !;
}

const fn assert(_:TrapEntry) {}

const _:() = assert(entry_no_vector);

#[cfg(target_arch = "riscv32")]
macro_rules! save_mstatush {
    () => {
        "csrr t0,mstatush\n sw t0,{off_mstatush}(sp)\n"
    };
}

#[cfg(target_arch = "riscv64")]
macro_rules! save_mstatush {
    () => {
        "/*{off_mstatush}*/"
    };
}

#[cfg(target_arch = "riscv32")]
macro_rules! restore_mstatush {
    () => {
        "lw t0,{off_mstatush}(sp)\n csrw mstatush,t0\n"
    };
}

#[cfg(target_arch = "riscv64")]
macro_rules! restore_mstatush {
    () => {
        "/*{off_mstatush}*/"
    };
}

#[cfg(target_arch = "riscv64")]
macro_rules! clear_mdt {
    ($scratch:ident) => {
        concat!(
            stringify!(li $scratch,1),"\n",
            stringify!(slli $scratch,$scratch,42),"\n",
            stringify!(csrc mstatus,$scratch)
        )
    };
}

#[cfg(target_arch = "riscv32")]
macro_rules! clear_mdt {
    ($scratch:ident) => {
        concat!(
            stringify!(li $scratch,1),"\n",
            stringify!(slli $scratch,$scratch,10),"\n",
            stringify!(csrc mstatush,$scratch)
        )
    };
}

/// function to initialized scratch space and return trap entry.  
/// 
/// you should write the returned TrapEntry to mtvec  
/// 
/// and write the passed in *mut SbiScratch to mscratch  
/// 
/// if the returned value is not None.
/// 
/// an error is returned via returning None, which is zero  
#[unsafe(naked)]
pub unsafe extern "C" fn setup_scratch_and_trap_handler(
    // a0
    trap_handler:Option<TrapHandler>,
    // a1
    scratch:*mut SbiScratch,
    // a2
    m_mode_gp:usize,
    // a3
    m_mode_tp:usize,
    // a4
    stack_top:usize,
) -> Option<TrapEntry> {

    naked_asm!(
        // trap_handler = None
        "beqz a0,114f",
        // *mut SbiScratch = null
        "beqz a1,114f",
        "andi t0,a1,{scratch_align_mask}",
        // if scratch is not aligned
        "bne a1,t0,114f",
        // check stack alignment
        "andi t0,a4,{stack_align_mask}",
        "bne a4,t0,114f",
        
        save!(a0 => a1[b:{off_trap_handler}]),
        save!(a2 => a1[b:{off_m_gp}]),
        save!(a3 => a1[b:{off_m_tp}]),
        save!(a4 => a1[b:{off_stack_top}]),

        save!(zero => a1[
            b:{off_prev_context},
            b:{off_saved_gp},
            b:{off_saved_sp},
            b:{off_saved_tp},
            b:{off_spill_t0}
        ]),
        
        "la a0,{entry}",
        "ret",
        // the failed path
        "114:",
        "mv a0,zero",
        "ret",

        off_prev_context = const offset_of!(SbiScratch,context),
        off_saved_gp = const offset_of!(SbiScratch,saved_gp),
        off_saved_tp = const offset_of!(SbiScratch,saved_tp),
        off_saved_sp = const offset_of!(SbiScratch,saved_sp),
        off_spill_t0 = const offset_of!(SbiScratch,spill_t0),

        off_stack_top = const offset_of!(SbiScratch,stack_top),
        off_m_tp = const offset_of!(SbiScratch,m_mode_tp),
        off_m_gp = const offset_of!(SbiScratch,m_mode_gp),
        off_trap_handler = const offset_of!(SbiScratch,trap_handler),
        stack_align_mask = const !(16usize-1),
        scratch_align_mask = const !(align_of::<SbiScratch>() - 1),
        entry = sym entry_no_vector,
    )
}

use core::arch::global_asm;
use core::arch::naked_asm;
use core::mem::offset_of;

global_asm!{
    ".section push",
    ".section .text",
    ".option push",
    ".option arch,+zicsr",
    ".globl {entry}",
    ".type {entry},@function",
    ".balign 4",
    "{entry}:",
    "csrrw sp,mscratch,sp",
    save!(t0 => sp[b:{off_spill_t0}]),
    "csrr t0,mstatus",
    "srli t0,t0,{mpp}",
    "andi t0,t0,{m_mode_mask}",
    "sltiu t0,t0,{m_mode_mask}", 
    // t0 = 0, come from M
    // t0 = 1, come from U/S/H
    "beqz t0,1f",
    // come from U/S mode, save U/S mode tp,sp,gp
    // load M mode tp,gp
    // zero prev_context,
    save!(zero => sp[b:{off_prev_context}]),
    save!(tp => sp[b:{off_saved_tp}]),
    save!(gp => sp[b:{off_saved_gp}]),
    load!(tp <= sp[b:{off_m_tp}]),
    load!(gp <= sp[b:{off_m_gp}]),
    // load stack top to t0, and swap t0 with mscratch, which is now S/U mode sp
    load!(t0 <= sp[b:{off_stack_top}]),
    "csrrw t0,mscratch,t0",
    save!(t0 => sp[b:{off_saved_sp}]),
    // currently: mscratch: stack top (if come from U/S), old sp (if come from M)
    "1:",
    load!(t0 <= sp[b:{off_spill_t0}]),
    "csrrw sp,mscratch,sp",
    "addi sp,sp,-{context_size}",
    // save caller-saved regs
    save!(ra => sp[b:{off_ra}]),
    save!(t0 => sp[b:{off_t0}]),
    save!(t1 => sp[b:{off_t1}]),
    save!(t2 => sp[b:{off_t2}]),
    save!(a0 => sp[b:{off_a0}]),
    save!(a1 => sp[b:{off_a1}]),
    save!(a2 => sp[b:{off_a2}]),
    save!(a3 => sp[b:{off_a3}]),
    save!(a4 => sp[b:{off_a4}]),
    save!(a5 => sp[b:{off_a5}]),
    save!(a6 => sp[b:{off_a6}]),
    save!(a7 => sp[b:{off_a7}]),
    save!(t3 => sp[b:{off_t3}]),
    save!(t4 => sp[b:{off_t4}]),
    save!(t5 => sp[b:{off_t5}]),
    save!(t6 => sp[b:{off_t6}]),

    // update prev_context

    "csrr t0,mscratch",
    // scratch.context
    load!(t1 <= t0[b:{off_prev_context}]),
    save!(t1 => sp[b:{off_frame_prev_context}]),
    save!(sp => t0[b:{off_prev_context}]),

    // load function pointer to a7
    load!(a7 <= t0[b:{off_trap_handler}]),

    // DO NOT TOUCH a7!!!!
    // save csrs
    "csrr t0,mepc",
    save!(t0 => sp[b:{off_mepc}]),
    "csrr t0,mstatus",
    save!(t0 => sp[b:{off_mstatus}]),
    save_mstatush!(),
    "csrr t0,mcause",
    save!(t0 => sp[b:{off_mcause}]),
    "csrr t0,mtval",
    save!(t0 => sp[b:{off_mtval}]),
    "csrr t0,misa",
    "srli t0,t0,{hype_bit}",
    "andi t0,t0,1",
    "bnez t0,2f",
    // t0 = 0: no hype, write zero
    save!(zero => sp[b:{off_mtval2}]),
    save!(zero => sp[b:{off_mtinst}]),
    "mv a0,sp",
    // csr saved, volatile regs saved, trap_context linked list ready
    // we can take nested trap
    clear_mdt!(t0),
    // return to 3:, skip the save csr branch
    // a7: function pointer
    "la ra,3f",
    "jalr zero,a7,0",
    // have hype extension
    "2:",
    "csrr t0,mtval2",
    save!(t0 => sp[b:{off_mtval2}]),
    "csrr t0,mtinst",
    save!(t0 => sp[b:{off_mtinst}]),
    "mv a0,sp",
    clear_mdt!(t0),
    "jalr ra,a7,0",
    //restore regs except sp and t0
    "3:",
    load!(ra <= sp[b:{off_ra}]),
    load!(t1 <= sp[b:{off_t1}]),
    load!(t2 <= sp[b:{off_t2}]),
    load!(a0 <= sp[b:{off_a0}]),
    load!(a1 <= sp[b:{off_a1}]),
    load!(a2 <= sp[b:{off_a2}]),
    load!(a3 <= sp[b:{off_a3}]),
    load!(a4 <= sp[b:{off_a4}]),
    load!(a5 <= sp[b:{off_a5}]),
    load!(a6 <= sp[b:{off_a6}]),
    load!(a7 <= sp[b:{off_a7}]),
    load!(t3 <= sp[b:{off_t3}]),
    load!(t4 <= sp[b:{off_t4}]),
    load!(t5 <= sp[b:{off_t5}]),
    load!(t6 <= sp[b:{off_t6}]),
    // determin which mode to return to
    load!(t0 <= sp[b:{off_mstatus}]),
    "srli t0,t0,{mpp}",
    "andi t0,t0,{m_mode_mask}",
    "sltiu t0,t0,{m_mode_mask}",
    "beqz t0,4f",
    // t0 = 1, return to non-m mode
    // restore mstatush,mstatus and mepc
    // start with mstatush and mstatus because we need to disable trap
    restore_mstatush!(),
    load!(t0 <= sp[b:{off_mstatus}]),
    "csrw mstatus,t0",
    load!(t0 <= sp[b:{off_mepc}]),
    "csrw mepc,t0",
    // load saved t0
    load!(t0 <= sp[b:{off_t0}]),
    // return t0 U/S mode
    // restore U/S mode sp,tp,gp
    // since we are returning to U/S mode it's fine to discard current sp
    "csrr sp,mscratch",
    load!(tp <= sp[b:{off_saved_tp}]),
    load!(gp <= sp[b:{off_saved_gp}]),
    load!(sp <= sp[b:{off_saved_sp}]),
    "mret",
    // t0 = 0
    "4:",// this is m_mode_nested trap
    // no need to restore tp,gp,sp
    // just add sp by context size
    // but we need to restore last_trap
    //restore mstatush,mstatus and mepc
    restore_mstatush!(),
    load!(t0 <= sp[b:{off_mstatus}]),
    "csrw mstatus,t0",
    load!(t0 <= sp[b:{off_mepc}]),
    "csrw mepc,t0",
    // load prev context
    load!(t0 <= sp[b:{off_frame_prev_context}]),
    "csrrw sp,mscratch,sp",
    // save prev context on scratch
    save!(t0 => sp[b:{off_prev_context}]),
    "csrrw sp,mscratch,sp",
    load!(t0 <= sp[b:{off_t0}]),
    "addi sp,sp,{context_size}",
    "mret",
    ".section pop",
    ".option pop",
    off_trap_handler = const offset_of!(SbiScratch,trap_handler),
    hype_bit = const b'H' - b'A',
    off_mtval2 = const offset_of!(TrapFrame,mtval2),
    off_mtinst = const offset_of!(TrapFrame,mtinst),
    off_mtval = const offset_of!(TrapFrame,mtval),
    off_mcause = const offset_of!(TrapFrame,mcause),
    off_mstatush = const offset_of!(TrapFrame,mstatush),
    off_mstatus = const offset_of!(TrapFrame,mstatus),
    off_mepc = const offset_of!(TrapFrame,mepc),
    off_ra = const offset_of!(TrapFrame,ra),
    off_t0 = const offset_of!(TrapFrame,t0),
    off_t1 = const offset_of!(TrapFrame,t1),
    off_t2 = const offset_of!(TrapFrame,t2),
    off_a0 = const offset_of!(TrapFrame,a0),
    off_a1 = const offset_of!(TrapFrame,a1),
    off_a2 = const offset_of!(TrapFrame,a2),
    off_a3 = const offset_of!(TrapFrame,a3),
    off_a4 = const offset_of!(TrapFrame,a4),
    off_a5 = const offset_of!(TrapFrame,a5),
    off_a6 = const offset_of!(TrapFrame,a6),
    off_a7 = const offset_of!(TrapFrame,a7),
    off_t3 = const offset_of!(TrapFrame,t3),
    off_t4 = const offset_of!(TrapFrame,t4),
    off_t5 = const offset_of!(TrapFrame,t5),
    off_t6 = const offset_of!(TrapFrame,t6),
    context_size = const size_of::<TrapFrame>(),
    off_saved_sp = const offset_of!(SbiScratch,saved_sp),
    off_stack_top = const offset_of!(SbiScratch,stack_top),
    off_prev_context = const offset_of!(SbiScratch,context),
    off_frame_prev_context = const offset_of!(TrapFrame,prev_context),
    off_saved_tp = const offset_of!(SbiScratch,saved_tp),
    off_saved_gp = const offset_of!(SbiScratch,saved_gp),
    off_m_tp = const offset_of!(SbiScratch,m_mode_tp),
    off_m_gp = const offset_of!(SbiScratch,m_mode_gp),
    mpp = const 11,
    m_mode_mask = const 3,
    off_spill_t0 = const offset_of!(SbiScratch,spill_t0),
    entry = sym entry_no_vector,
}
