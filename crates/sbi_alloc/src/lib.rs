#![no_std]
pub mod layout;
pub mod traits;
mod global;

pub use global::set_global_allocator;