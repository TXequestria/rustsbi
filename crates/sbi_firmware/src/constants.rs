#[cfg(feature = "jump")]
use sbi_config::FW_CONIFG;

#[cfg(feature = "jump")]
pub(crate) const JUMP_ADDRESS:usize = FW_CONIFG.jump_address.unwrap();

