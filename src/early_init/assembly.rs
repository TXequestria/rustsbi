use core::arch::naked_asm;
use sbi_config::LINK_CONFIG;
use sbi_firmware::is_boot_hart;
use sbi_platform::platform::MemorySegment;
use sbi_trap::setup_scratch_and_trap_handler;
#[cfg(target_arch = "riscv64")]
macro_rules! save {
    ($src:ident => $ptr:ident [$expr:expr]) => {
        concat!(stringify!(sd $src ,(8*($expr))($ptr)),"\n")
    }
}

#[cfg(target_arch = "riscv32")]
macro_rules! save {
    ($src:ident => $ptr:ident [$expr:expr]) => {
        concat!(stringify!(sw $src ,(4*($expr))($ptr)),"\n")
    }
}

#[cfg(target_arch = "riscv64")]
macro_rules! load {
    ($ptr:ident [$expr:expr] => $dest:ident) => {
        concat!(stringify!(ld $dest ,(8*($expr))($ptr)),"\n")
    }
}

#[cfg(target_arch = "riscv32")]
macro_rules! load {
    ($ptr:ident [$expr:expr] => $dest:ident) => {
        concat!(stringify!(lw $dest ,(4*($expr))($ptr)),"\n")
    }
}

#[cfg(target_arch = "riscv64")]
macro_rules! clear_mdt {
    () => {
        "li t0,1\nslli t0,t0,42\ncsrc mstatus,t0"
    };
}
#[cfg(target_arch = "riscv32")]
macro_rules! clear_mdt {
    () => {
        "li t0,1\nslli t0,t0,10\ncsrc mstatush,t0"
    };
}

static mut MEMORY_INIT_UNDONE: bool = true;

/*
 * partially referenced opensbi/firmware/fw_base.S
 * SPDX-License-Identifier: BSD-2-Clause
 *
 * Copyright (c) 2019 Western Digital Corporation or its affiliates.
 *
 * Authors:
 *   Anup Patel <anup.patel@wdc.com>
 */

#[unsafe(naked)]
#[unsafe(export_name = "_start")]
#[allow(named_asm_labels)]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        ".option push",
        ".option arch, +a",
        ".option arch, +m",
        "csrw mie,zero",
        "mv s1,a0",
        "mv s2,a1",
        "mv s3,a2",
        "mv a1,a2",
        "call {is_boot}",
        "beqz a0,_wait_as_seconday",
        // patch got table
        "li t0,{link_addr}",
        "lla t1,_fw_start",
        "beq t0,t1,_patch_got_done",
        // t0 = t1-t0
        "sub t0,t1,t0",
        "lla t1,__rela_dyn_start",
        "lla t2,__rela_dyn_end",
        "li t3,{rela_dyn}",

        "0:",
        load!(t1[1] => t4),
        "bne t4,t3,1f",
        load!(t1[0] => t4),
        load!(t1[2] => t5),
        "add t5,t5,t0",
        "add t4,t4,t0",
        save!(t5 => t4[0]),
        "1:",
        "addi t1,t1,3*{usize}",
        "blt t1,t2,0b",
        "_patch_got_done:",

        "lla t0,_bss_start",
        "lla t1,_bss_end",

        // zero bss

        "_zero_bss:",
        "bgeu t0,t1,_zero_bss_done",
        save!(zero => t0[0]),
        "addi t0,t0,{usize}",
        "j _zero_bss",
        "_zero_bss_done:",

        // rudementary trap handling
        "lla t0,_early_hang",
        "csrw mtvec,t0",
        clear_mdt!(),

        "lla t0,_fw_end",
        "li t1,{temp_stack}",
        "add sp,t0,t1",
        "mv a0,s2",
        "mv a1,s3",
        "call {memory_init}",
        "fence rw,rw",
        "lla t1,{flag_undone}",
        "sb zero,(t1)",
        "j _start_warm",
        "_wait_as_seconday:",
        "lla t0,{flag_undone}",
        "0:",
        "div t1,t1,t1",
        "div t2,t2,t2",
        "div t3,t3,t3",
        "div t4,t4,t4",
        "div t5,t5,t5",
        "div t6,t6,t6",
        "lbu t1,(t0)",
        "bnez t1,0b",
        "_start_warm:",
        "fence.i",
        "mv x1,x0",
        "mv x2,x0",
        "mv x3,x0",
        "mv x4,x0",
        "mv x5,x0",
        "mv x6,x0",
        "mv x7,x0",
        "mv x8,x0",
        "mv x9,x0",
        "mv x10,x0",
        "mv x11,x0",
        "mv x12,x0",
        "mv x13,x0",
        "mv x14,x0",
        "mv x15,x0",
        "mv x16,x0",
        "mv x17,x0",
        "mv x18,x0",
        "mv x19,x0",
        "mv x20,x0",
        "mv x21,x0",
        "mv x22,x0",
        "mv x23,x0",
        "mv x24,x0",
        "mv x25,x0",
        "mv x26,x0",
        "mv x27,x0",
        "mv x28,x0",
        "mv x29,x0",
        "mv x30,x0",
        "mv x31,x0",
        "csrw mscratch,x0",
        "lla a0,{layout}",
        "csrr a1,mhartid",
        "call {locate_stack_scratch}",
        // a0:stack, a1:scratch
        "mv sp,a0",
        "mv s1,a1",
        "lla a0,{trap_handler}",
        "mv a4,sp",
        "li a2,0",
        "li a3,0",
        "call {setup_scratch}",
        "beqz a0,_early_hang",
        "csrw mscratch,s1",
        "csrw mtvec,a0",
        clear_mdt!(),
        "call {main}",
        // we are not supposed to return. but if it does we hang by default
        ".balign 4",
        "_early_hang:",
        "wfi",
        "j _early_hang",
        ".option pop",
        temp_stack = const 1024*16,
        usize = const size_of::<usize>(),
        rela_dyn = const 3,
        link_addr = const LINK_CONFIG.start_address,
        flag_undone = sym MEMORY_INIT_UNDONE,
        is_boot = sym is_boot_hart,
        memory_init = sym super::early_init,
        layout = sym super::MEMORY_LAYOUT,
        locate_stack_scratch = sym MemorySegment::locate_stack_scratch,
        setup_scratch = sym setup_scratch_and_trap_handler,
        trap_handler = sym crate::trap_handler::trap_entry,
        main = sym crate::main
    )
}
