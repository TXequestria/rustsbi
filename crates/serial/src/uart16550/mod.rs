mod builder;
mod serial_impl;
pub use serial_impl::{NS16550, SerialError};
pub use builder::{Builder, FifoTrigLevel, StopBits};
