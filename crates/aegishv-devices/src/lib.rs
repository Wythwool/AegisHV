#![cfg_attr(not(any(test, feature = "std")), no_std)]

pub mod block;
pub mod console;
pub mod error;
pub mod net;
pub mod virtio_mmio;

pub use error::{DeviceError, DeviceErrorKind};
