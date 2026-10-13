use std::error::Error;
use std::fmt;

use super::{GuestPhysical, GuestVirtual, VcpuId, VmId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmiErrorKind {
    Unsupported,
    UnsupportedBackend,
    Degraded,
    InvalidInput,
    InvalidAddress,
    MissingMemory,
    TranslationFailure,
    InconsistentSnapshot,
    UnsupportedArchitecture,
    PermissionDenied,
    TemporarilyUnavailable,
    Unmapped,
    UnknownVcpu,
    Malformed,
    MissingProfile,
    Backend,
}

impl VmiErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::UnsupportedBackend => "unsupported_backend",
            Self::Degraded => "degraded",
            Self::InvalidInput => "invalid_input",
            Self::InvalidAddress => "invalid_address",
            Self::MissingMemory => "missing_memory",
            Self::TranslationFailure => "translation_failure",
            Self::InconsistentSnapshot => "inconsistent_snapshot",
            Self::UnsupportedArchitecture => "unsupported_architecture",
            Self::PermissionDenied => "permission_denied",
            Self::TemporarilyUnavailable => "temporarily_unavailable",
            Self::Unmapped => "unmapped",
            Self::UnknownVcpu => "unknown_vcpu",
            Self::Malformed => "malformed",
            Self::MissingProfile => "missing_profile",
            Self::Backend => "backend",
        }
    }

    pub fn is_unsupported(self) -> bool {
        matches!(
            self,
            Self::Unsupported | Self::UnsupportedBackend | Self::UnsupportedArchitecture
        )
    }

    pub fn is_degraded(self) -> bool {
        self == Self::Degraded
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    Unsupported {
        backend: &'static str,
        operation: &'static str,
    },
    Degraded {
        reason: String,
    },
    InconsistentSnapshot {
        detail: String,
    },
    PermissionDenied {
        operation: &'static str,
        detail: String,
    },
    TemporarilyUnavailable {
        resource: &'static str,
        detail: String,
    },
    Failure {
        detail: String,
    },
}

impl BackendError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Unsupported { .. } => VmiErrorKind::UnsupportedBackend,
            Self::Degraded { .. } => VmiErrorKind::Degraded,
            Self::InconsistentSnapshot { .. } => VmiErrorKind::InconsistentSnapshot,
            Self::PermissionDenied { .. } => VmiErrorKind::PermissionDenied,
            Self::TemporarilyUnavailable { .. } => VmiErrorKind::TemporarilyUnavailable,
            Self::Failure { .. } => VmiErrorKind::Backend,
        }
    }

    pub(crate) fn with_context(mut self, context: &str) -> Self {
        let detail = match &mut self {
            Self::Unsupported { .. } => return self,
            Self::Degraded { reason } => reason,
            Self::InconsistentSnapshot { detail }
            | Self::PermissionDenied { detail, .. }
            | Self::TemporarilyUnavailable { detail, .. }
            | Self::Failure { detail } => detail,
        };
        *detail = format!("{context}: {detail}");
        self
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { backend, operation } => write!(
                f,
                "operation '{operation}' is unsupported by backend '{backend}'"
            ),
            Self::Degraded { reason } => write!(f, "degraded: {reason}"),
            Self::InconsistentSnapshot { detail } => write!(f, "inconsistent snapshot: {detail}"),
            Self::PermissionDenied { operation, detail } => {
                write!(f, "operation '{operation}' is not permitted: {detail}")
            }
            Self::TemporarilyUnavailable { resource, detail } => {
                write!(f, "resource '{resource}' is unavailable: {detail}")
            }
            Self::Failure { detail } => write!(f, "backend error: {detail}"),
        }
    }
}

impl Error for BackendError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryReadError {
    Backend(BackendError),
    InvalidAddress { gpa: GuestPhysical, len: usize },
    InvalidRange { gpa: GuestPhysical, len: usize },
    MissingMemory { gpa: GuestPhysical, len: usize },
    Unmapped { gpa: GuestPhysical, len: usize },
    Malformed { detail: String },
}

impl MemoryReadError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Backend(err) => err.kind(),
            Self::InvalidAddress { .. } | Self::InvalidRange { .. } => VmiErrorKind::InvalidAddress,
            Self::MissingMemory { .. } => VmiErrorKind::MissingMemory,
            Self::Unmapped { .. } => VmiErrorKind::Unmapped,
            Self::Malformed { .. } => VmiErrorKind::Malformed,
        }
    }

    pub fn is_unsupported(&self) -> bool {
        self.kind().is_unsupported()
    }

    pub fn is_degraded(&self) -> bool {
        self.kind().is_degraded()
    }
}

impl fmt::Display for MemoryReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(err) => err.fmt(f),
            Self::InvalidAddress { gpa, len } => {
                write!(
                    f,
                    "invalid guest physical address range gpa=0x{:x} len={len}",
                    gpa.0
                )
            }
            Self::InvalidRange { gpa, len } => {
                write!(
                    f,
                    "invalid guest physical memory range gpa=0x{:x} len={len}",
                    gpa.0
                )
            }
            Self::MissingMemory { gpa, len } => {
                write!(
                    f,
                    "guest physical memory is unavailable gpa=0x{:x} len={len}",
                    gpa.0
                )
            }
            Self::Unmapped { gpa, len } => {
                write!(
                    f,
                    "guest physical memory range is unmapped gpa=0x{:x} len={len}",
                    gpa.0
                )
            }
            Self::Malformed { detail } => {
                write!(f, "memory snapshot manifest is malformed: {detail}")
            }
        }
    }
}

impl Error for MemoryReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Backend(err) => Some(err),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterReadError {
    Backend(BackendError),
    UnsupportedArchitecture {
        arch: String,
    },
    WrongArchitecture {
        expected: &'static str,
        actual: &'static str,
    },
    MissingRegister {
        arch: &'static str,
        register: &'static str,
    },
    Malformed {
        detail: String,
    },
    UnknownVcpu {
        vm: VmId,
        vcpu: VcpuId,
    },
}

impl RegisterReadError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Backend(err) => err.kind(),
            Self::UnsupportedArchitecture { .. } => VmiErrorKind::UnsupportedArchitecture,
            Self::WrongArchitecture { .. } | Self::MissingRegister { .. } => {
                VmiErrorKind::InvalidInput
            }
            Self::Malformed { .. } => VmiErrorKind::Malformed,
            Self::UnknownVcpu { .. } => VmiErrorKind::UnknownVcpu,
        }
    }

    pub fn is_unsupported(&self) -> bool {
        self.kind().is_unsupported()
    }

    pub fn is_degraded(&self) -> bool {
        self.kind().is_degraded()
    }
}

impl fmt::Display for RegisterReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(err) => err.fmt(f),
            Self::UnsupportedArchitecture { arch } => {
                write!(
                    f,
                    "register read is unsupported for guest architecture '{arch}'"
                )
            }
            Self::WrongArchitecture { expected, actual } => {
                write!(
                    f,
                    "register snapshot architecture mismatch: expected '{expected}', got '{actual}'"
                )
            }
            Self::MissingRegister { arch, register } => {
                write!(
                    f,
                    "register snapshot for architecture '{arch}' is missing required register '{register}'"
                )
            }
            Self::Malformed { detail } => write!(f, "malformed register snapshot: {detail}"),
            Self::UnknownVcpu { vm, vcpu } => {
                write!(f, "unknown vCPU {} for VM {}", vcpu.0, vm.0)
            }
        }
    }
}

impl Error for RegisterReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Backend(err) => Some(err),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationError {
    Backend(BackendError),
    UnsupportedArchitecture {
        arch: String,
    },
    MissingContext {
        field: &'static str,
    },
    InvalidAddress {
        gva: GuestVirtual,
    },
    NotPresent {
        level: &'static str,
        gva: GuestVirtual,
    },
    MissingMemory {
        gpa: GuestPhysical,
        detail: String,
    },
    Unmapped {
        gva: GuestVirtual,
    },
    TranslationFailed {
        gva: GuestVirtual,
        detail: String,
    },
    MalformedPageTables {
        level: &'static str,
        detail: String,
    },
}

impl TranslationError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Backend(err) => err.kind(),
            Self::UnsupportedArchitecture { .. } => VmiErrorKind::UnsupportedArchitecture,
            Self::MissingContext { .. } => VmiErrorKind::InvalidInput,
            Self::InvalidAddress { .. } => VmiErrorKind::InvalidAddress,
            Self::NotPresent { .. } => VmiErrorKind::TranslationFailure,
            Self::MissingMemory { .. } => VmiErrorKind::MissingMemory,
            Self::Unmapped { .. } => VmiErrorKind::Unmapped,
            Self::TranslationFailed { .. } => VmiErrorKind::TranslationFailure,
            Self::MalformedPageTables { .. } => VmiErrorKind::Malformed,
        }
    }

    pub fn is_unsupported(&self) -> bool {
        self.kind().is_unsupported()
    }

    pub fn is_degraded(&self) -> bool {
        self.kind().is_degraded()
    }
}

impl fmt::Display for TranslationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(err) => err.fmt(f),
            Self::UnsupportedArchitecture { arch } => {
                write!(
                    f,
                    "address translation is unsupported for guest architecture '{arch}'"
                )
            }
            Self::MissingContext { field } => {
                write!(
                    f,
                    "address translation is missing required context field '{field}'"
                )
            }
            Self::InvalidAddress { gva } => {
                write!(f, "invalid guest virtual address gva=0x{:x}", gva.0)
            }
            Self::NotPresent { level, gva } => {
                write!(
                    f,
                    "translation entry is not present at {level} for gva=0x{:x}",
                    gva.0
                )
            }
            Self::MissingMemory { gpa, detail } => {
                write!(
                    f,
                    "address translation cannot read guest memory gpa=0x{:x}: {detail}",
                    gpa.0
                )
            }
            Self::Unmapped { gva } => {
                write!(f, "guest virtual address is unmapped gva=0x{:x}", gva.0)
            }
            Self::TranslationFailed { gva, detail } => {
                write!(
                    f,
                    "guest virtual address translation failed gva=0x{:x}: {detail}",
                    gva.0
                )
            }
            Self::MalformedPageTables { level, detail } => {
                write!(f, "malformed guest page table at {level}: {detail}")
            }
        }
    }
}

impl Error for TranslationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Backend(err) => Some(err),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    Backend(BackendError),
    MissingProfile {
        vm: VmId,
    },
    MissingProfileIdentity {
        os: String,
        arch: String,
        kernel_or_build: String,
    },
    UnsupportedGuest {
        os: String,
        arch: String,
    },
    UnsupportedArchitecture {
        arch: String,
    },
    MalformedProfile {
        detail: String,
    },
}

impl ProfileError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Backend(err) => err.kind(),
            Self::MissingProfile { .. } | Self::MissingProfileIdentity { .. } => {
                VmiErrorKind::MissingProfile
            }
            Self::UnsupportedGuest { .. } => VmiErrorKind::Unsupported,
            Self::UnsupportedArchitecture { .. } => VmiErrorKind::UnsupportedArchitecture,
            Self::MalformedProfile { .. } => VmiErrorKind::Malformed,
        }
    }

    pub fn is_unsupported(&self) -> bool {
        self.kind().is_unsupported()
    }

    pub fn is_degraded(&self) -> bool {
        self.kind().is_degraded()
    }
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backend(err) => err.fmt(f),
            Self::MissingProfile { vm } => write!(f, "guest profile is missing for VM {}", vm.0),
            Self::MissingProfileIdentity {
                os,
                arch,
                kernel_or_build,
            } => {
                write!(
                    f,
                    "guest profile is missing for os='{os}' arch='{arch}' kernel_or_build='{kernel_or_build}'"
                )
            }
            Self::UnsupportedGuest { os, arch } => {
                write!(
                    f,
                    "guest profile is unsupported for os='{os}' arch='{arch}'"
                )
            }
            Self::UnsupportedArchitecture { arch } => {
                write!(f, "guest profile is unsupported for architecture '{arch}'")
            }
            Self::MalformedProfile { detail } => write!(f, "guest profile is malformed: {detail}"),
        }
    }
}

impl Error for ProfileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Backend(err) => Some(err),
            _ => None,
        }
    }
}
