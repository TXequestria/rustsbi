#![no_std]

#[derive(Clone,Copy)]
pub struct FwConfig {
    pub jump_address:Option<usize>
}

pub const FW_CONIFG:FwConfig = include!(concat!(env!("OUT_DIR"),"/fw_config.in"));

#[derive(Clone,Copy)]
pub struct LinkConfig {
    pub start_address:usize,
    pub payload_offset:Option<usize>,
    pub payload_align:Option<usize>
}

pub const LINK_CONFIG:LinkConfig = include!(concat!(env!("OUT_DIR"),"/link_config.in"));

#[derive(Clone, Copy)]
pub struct PlatformConfig {
    pub stack_size:usize,
    // if omitted, calculate from actual hart count
    pub heap_size:Option<usize>,
    pub max_harts:usize,
    pub page_size:usize,
}

pub const PLATFORM_CONFIG:PlatformConfig = include!(concat!(env!("OUT_DIR"),"/platform_config.in"));