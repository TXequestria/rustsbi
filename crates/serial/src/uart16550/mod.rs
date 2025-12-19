mod builder;
mod serial_impl;
pub use builder::{Builder, FifoTrigLevel, StopBits, ZeroError};
pub use serial_impl::{NS16550, SerialError};
