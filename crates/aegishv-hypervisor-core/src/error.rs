use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreErrorKind {
    InvalidId,
    InvalidAddress,
    InvalidArgument,
    InvalidMemoryMap,
    CapacityExceeded,
    Overlap,
    OutOfMemory,
    DoubleFree,
    ZeroingFailed,
    InvalidTransition,
    RingFull,
    UnknownCommand,
    Unsupported,
    PermissionViolation,
    SerialTimeout,
    InvalidState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoreError {
    pub kind: CoreErrorKind,
    pub message: &'static str,
}

impl CoreError {
    pub const fn new(kind: CoreErrorKind, message: &'static str) -> Self {
        Self { kind, message }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CoreError {}
