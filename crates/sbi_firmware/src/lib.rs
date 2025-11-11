#![no_std]

#[cfg(any(
    all(feature = "dynamic",feature = "jump"),
    all(feature = "dynamic",feature = "payload"),
    all(feature = "payload",feature = "jump")
))]
compile_error!{
    "Cannot enable more than one of feature dynamic,payload and jump"
}

#[cfg(not(any(
    feature = "dynamic",feature = "jump",feature = "payload"
)))]
compile_error!{
    "None of dynamic,payload,jump is selected, you should enable one and only one of them"
}

#[cfg(all(
    feature = "dynamic",
    not(any(feature = "payload",feature = "jump"))
))]
mod dynamic;
#[cfg(all(
    feature = "dynamic",
    not(any(feature = "payload",feature = "jump"))
))]
pub use dynamic::{is_boot_hart,get_fw_boot_info};

static mut BOOT_LOTTERY:u32 = true as u32;

#[cfg(not(feature = "dynamic"))]
#[unsafe(naked)]
pub unsafe extern "C" fn is_boot_hart(hart_id:usize,opaque:usize) -> bool {
    ::core::arch::naked_asm!(
        ".option push",
        ".option arch,+a",
        "lla t0,{lottery}",
    "#ifdef __riscv_atomic",
        "amoswap.w a0,zero,(t0)",
    "#elif __riscv_zalrsc",
        "0:",
        "lr.w a0,(t0)",
        "sc.w t1,zero,(t0)",
        "bnez t1,0b",
    "#else",
    "#error \"need a or zalrsc\"",
    "#endif",
        "ret",
        ".option pop",
        lottery = sym BOOT_LOTTERY
    )
}

#[cfg(all(
    feature = "payload",
    not(any(feature = "dynamic",feature = "jump"))
))]
mod payload;
#[cfg(all(
    feature = "payload",
    not(any(feature = "dynamic",feature = "jump"))
))]
pub use payload::{get_fw_boot_info};

#[derive(Clone, Copy,PartialEq, Eq, PartialOrd, Ord,Debug)]
pub enum NextMode {
    PrivS = 2,
    PrivM = 3,
    PrivU = 0,
}

#[derive(Clone, Copy,Debug)]
pub struct FwBootInfo {
    pub next_addr:usize,
    pub next_mode:NextMode,
    pub option:usize
}

impl FwBootInfo {
    pub const fn new() -> Self {
        Self {
            next_addr:0,
            next_mode:NextMode::PrivS,
            option:0
        }
    }
}

mod constants;