#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![deny(unsafe_code)]

pub mod abi;
pub mod allocator;
pub mod crash;
pub mod error;
pub mod ids;
pub mod memory;
pub mod percpu;
pub mod scheduler;
pub mod vm;

pub use error::{CoreError, CoreErrorKind};

pub const CORE_ABI_VERSION: u16 = 1;
