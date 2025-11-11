use core::arch::naked_asm;

pub unsafe fn get_fw_boot_info(opaque:usize) -> Option<FwBootInfo> {
    let fw_dynamic = opaque as *const usize;
    if fw_dynamic.is_null() {return None;}
    if !fw_dynamic.is_aligned() {return None;}
    unsafe {
        if fw_dynamic.read() != FW_DYNAMIC_INFO_MAGIC_VALUE {
            return None;
        }
        let version = fw_dynamic.add(1).read();
        if version > 2 {
            panic!("unsupported fw_dynamic version {version}")
        }
        let next_addr = fw_dynamic.add(2).read();
        let next_mode = match fw_dynamic.add(3).read() {
            0 => NextMode::PrivU,
            1 => NextMode::PrivS,
            3 => NextMode::PrivM,
            unsupported => panic!("invalid next mode {unsupported}")
        };
        let option = fw_dynamic.add(4).read();
        Some(FwBootInfo { next_addr, next_mode,option })
    }
}

use core::mem::{size_of,align_of};

use crate::{FwBootInfo, NextMode};

#[cfg(target_arch = "riscv64")]
const _:() = {
    if size_of::<usize>() != 8 {
        panic!("usize on 64 bit target != 8 bytes")
    }
    if align_of::<usize>() != 8 {
        panic!("usize on 64 bit target != 8 bytes")
    }
};

#[cfg(target_arch = "riscv32")]
const _:() = {
    if size_of::<usize>() != 4 {
        panic!("usize on 32 bit target != 4bytes")
    }
    if align_of::<usize>() != 4 {
        panic!("usize on 32 bit target != 4bytes")
    }
};

#[cfg(target_arch = "riscv64")]
macro_rules! load {
    ($ptr:ident [$offset:expr] => $dest:ident) => {
        ::core::stringify!(ld $dest,(8*($offset))($ptr))
    };
}

#[cfg(target_arch = "riscv32")]
macro_rules! load {
    ($ptr:ident [$offset:expr] => $dest:ident) => {
        ::core::stringify!(lw $dest,(4*($offset))($ptr))
    };
}

const FW_DYNAMIC_INFO_MAGIC_VALUE:usize = 0x4942534f;

#[unsafe(naked)]
pub unsafe extern "C" fn is_boot_hart(
    // a0
    hart_id:usize,
    // a1
    opaque:usize) -> bool {
    naked_asm!(
        ".option push",
        ".option arch,+a",
        "li t1,{magic}",
        // offset magic
        load!(a1[0] => t0),
        "bne t0,t1,114f",
        // offset version
        load!(a1[1] => t0),
        // max version 2
        "li t1,2",
        "bltu t1,t0,114f",
        "bne t1,t0,1f",
        // boot hart
        load!(a1[5] => t0),
        // boothart = -1, use lotttery
        "xori t1,t0,-1",
        "beqz t1,1f",
        "xor a0,a0,t0",
        "sltiu a0,a0,1",
        "ret",
        // the lottery path
        "1:",
        "lla t0,{lottery}",
    "#ifdef __riscv_atomic",
        "amoswap.w a0,zero,(t0)",
    "#elif __riscv_zalrsc",
        "2:",
        "lr.w a0,(t0)",
        "sc.w t1,zero,(t0)",
        "bnez t1,2b",
    "#else",
    "#error \"need a or zalrsc\"",
    "#endif",
        "ret",
        // the failed path
        "114:",
        "ebreak",
        "0:",
        "wfi",
        "j 0b",
        ".option pop",
        lottery = sym super::BOOT_LOTTERY,
        magic = const FW_DYNAMIC_INFO_MAGIC_VALUE
    )
}