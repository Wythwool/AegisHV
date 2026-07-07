use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceErrorKind {
    InvalidRegister,
    InvalidQueue,
    CapacityExceeded,
    Unsupported,
    PermissionDenied,
    IsolationMissing,
    OutOfBounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceError {
    pub kind: DeviceErrorKind,
    pub message: &'static str,
}

impl DeviceError {
    pub const fn new(kind: DeviceErrorKind, message: &'static str) -> Self {
        Self { kind, message }
    }
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DeviceError {}
