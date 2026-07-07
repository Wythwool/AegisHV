use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VmId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VcpuId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GuestPhysical(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GuestVirtual(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestRegisters {
    pub pc: u64,
    pub sp: u64,
    pub cr3_or_ttbr: Option<u64>,
    pub privilege: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationResult {
    pub gpa: GuestPhysical,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    pub user: bool,
    pub page_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyscallPathReport {
    pub ok: bool,
    pub os: String,
    pub entry: Option<u64>,
    pub table: Option<u64>,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestProfile {
    pub os: String,
    pub arch: String,
    pub pointer_width: u8,
    pub syscall_entry: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestAttribution {
    pub vm: VmId,
    pub vcpu: Option<VcpuId>,
    pub process: Option<String>,
    pub thread: Option<String>,
    pub module: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrapDecision {
    AllowOnce,
    AllowAndDisarm,
    Deny,
    Escalate(String),
}

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
    MissingIdentity,
    AmbiguousIdentity,
    StaleIdentity,
    SourceConflict,
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
            Self::MissingIdentity => "missing_identity",
            Self::AmbiguousIdentity => "ambiguous_identity",
            Self::StaleIdentity => "stale_identity",
            Self::SourceConflict => "source_conflict",
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
pub enum MemoryReadError {
    Unsupported {
        backend: &'static str,
        operation: &'static str,
    },
    Degraded {
        reason: String,
    },
    InvalidAddress {
        gpa: GuestPhysical,
        len: usize,
    },
    InvalidRange {
        gpa: GuestPhysical,
        len: usize,
    },
    MissingMemory {
        gpa: GuestPhysical,
        len: usize,
    },
    Unmapped {
        gpa: GuestPhysical,
        len: usize,
    },
    Malformed {
        detail: String,
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
    Backend {
        detail: String,
    },
}

impl MemoryReadError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Unsupported { .. } => VmiErrorKind::UnsupportedBackend,
            Self::Degraded { .. } => VmiErrorKind::Degraded,
            Self::InvalidAddress { .. } | Self::InvalidRange { .. } => VmiErrorKind::InvalidAddress,
            Self::MissingMemory { .. } => VmiErrorKind::MissingMemory,
            Self::Unmapped { .. } => VmiErrorKind::Unmapped,
            Self::Malformed { .. } => VmiErrorKind::Malformed,
            Self::InconsistentSnapshot { .. } => VmiErrorKind::InconsistentSnapshot,
            Self::PermissionDenied { .. } => VmiErrorKind::PermissionDenied,
            Self::TemporarilyUnavailable { .. } => VmiErrorKind::TemporarilyUnavailable,
            Self::Backend { .. } => VmiErrorKind::Backend,
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
            Self::Unsupported { backend, operation } => {
                write!(
                    f,
                    "memory read operation '{operation}' is unsupported by backend '{backend}'"
                )
            }
            Self::Degraded { reason } => write!(f, "memory read is degraded: {reason}"),
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
            Self::InconsistentSnapshot { detail } => {
                write!(f, "memory snapshot is inconsistent: {detail}")
            }
            Self::PermissionDenied { operation, detail } => {
                write!(
                    f,
                    "memory read operation '{operation}' is not permitted: {detail}"
                )
            }
            Self::TemporarilyUnavailable { resource, detail } => {
                write!(
                    f,
                    "memory read resource '{resource}' is unavailable: {detail}"
                )
            }
            Self::Backend { detail } => write!(f, "memory read backend error: {detail}"),
        }
    }
}

impl Error for MemoryReadError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterReadError {
    Unsupported {
        backend: &'static str,
        operation: &'static str,
    },
    Degraded {
        reason: String,
    },
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
    Backend {
        detail: String,
    },
}

impl RegisterReadError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Unsupported { .. } => VmiErrorKind::UnsupportedBackend,
            Self::Degraded { .. } => VmiErrorKind::Degraded,
            Self::UnsupportedArchitecture { .. } => VmiErrorKind::UnsupportedArchitecture,
            Self::WrongArchitecture { .. } | Self::MissingRegister { .. } => {
                VmiErrorKind::InvalidInput
            }
            Self::Malformed { .. } => VmiErrorKind::Malformed,
            Self::UnknownVcpu { .. } => VmiErrorKind::UnknownVcpu,
            Self::InconsistentSnapshot { .. } => VmiErrorKind::InconsistentSnapshot,
            Self::PermissionDenied { .. } => VmiErrorKind::PermissionDenied,
            Self::TemporarilyUnavailable { .. } => VmiErrorKind::TemporarilyUnavailable,
            Self::Backend { .. } => VmiErrorKind::Backend,
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
            Self::Unsupported { backend, operation } => {
                write!(
                    f,
                    "register read operation '{operation}' is unsupported by backend '{backend}'"
                )
            }
            Self::Degraded { reason } => write!(f, "register read is degraded: {reason}"),
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
            Self::InconsistentSnapshot { detail } => {
                write!(f, "register snapshot is inconsistent: {detail}")
            }
            Self::PermissionDenied { operation, detail } => {
                write!(
                    f,
                    "register read operation '{operation}' is not permitted: {detail}"
                )
            }
            Self::TemporarilyUnavailable { resource, detail } => {
                write!(f, "register resource '{resource}' is unavailable: {detail}")
            }
            Self::Backend { detail } => write!(f, "register read backend error: {detail}"),
        }
    }
}

impl Error for RegisterReadError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationError {
    Unsupported {
        backend: &'static str,
        operation: &'static str,
    },
    Degraded {
        reason: String,
    },
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
    Backend {
        detail: String,
    },
}

impl TranslationError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Unsupported { .. } => VmiErrorKind::UnsupportedBackend,
            Self::Degraded { .. } => VmiErrorKind::Degraded,
            Self::UnsupportedArchitecture { .. } => VmiErrorKind::UnsupportedArchitecture,
            Self::MissingContext { .. } => VmiErrorKind::InvalidInput,
            Self::InvalidAddress { .. } => VmiErrorKind::InvalidAddress,
            Self::NotPresent { .. } => VmiErrorKind::TranslationFailure,
            Self::MissingMemory { .. } => VmiErrorKind::MissingMemory,
            Self::Unmapped { .. } => VmiErrorKind::Unmapped,
            Self::TranslationFailed { .. } => VmiErrorKind::TranslationFailure,
            Self::MalformedPageTables { .. } => VmiErrorKind::Malformed,
            Self::InconsistentSnapshot { .. } => VmiErrorKind::InconsistentSnapshot,
            Self::PermissionDenied { .. } => VmiErrorKind::PermissionDenied,
            Self::TemporarilyUnavailable { .. } => VmiErrorKind::TemporarilyUnavailable,
            Self::Backend { .. } => VmiErrorKind::Backend,
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
            Self::Unsupported { backend, operation } => {
                write!(f, "address translation operation '{operation}' is unsupported by backend '{backend}'")
            }
            Self::Degraded { reason } => write!(f, "address translation is degraded: {reason}"),
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
            Self::InconsistentSnapshot { detail } => {
                write!(f, "translation snapshot is inconsistent: {detail}")
            }
            Self::PermissionDenied { operation, detail } => {
                write!(
                    f,
                    "translation operation '{operation}' is not permitted: {detail}"
                )
            }
            Self::TemporarilyUnavailable { resource, detail } => {
                write!(
                    f,
                    "translation resource '{resource}' is unavailable: {detail}"
                )
            }
            Self::Backend { detail } => write!(f, "address translation backend error: {detail}"),
        }
    }
}

impl Error for TranslationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileError {
    Unsupported {
        backend: &'static str,
        operation: &'static str,
    },
    Degraded {
        reason: String,
    },
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
    Backend {
        detail: String,
    },
}

impl ProfileError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Unsupported { .. } => VmiErrorKind::UnsupportedBackend,
            Self::Degraded { .. } => VmiErrorKind::Degraded,
            Self::MissingProfile { .. } | Self::MissingProfileIdentity { .. } => {
                VmiErrorKind::MissingProfile
            }
            Self::UnsupportedGuest { .. } => VmiErrorKind::Unsupported,
            Self::UnsupportedArchitecture { .. } => VmiErrorKind::UnsupportedArchitecture,
            Self::MalformedProfile { .. } => VmiErrorKind::Malformed,
            Self::InconsistentSnapshot { .. } => VmiErrorKind::InconsistentSnapshot,
            Self::PermissionDenied { .. } => VmiErrorKind::PermissionDenied,
            Self::TemporarilyUnavailable { .. } => VmiErrorKind::TemporarilyUnavailable,
            Self::Backend { .. } => VmiErrorKind::Backend,
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
            Self::Unsupported { backend, operation } => {
                write!(
                    f,
                    "guest profile operation '{operation}' is unsupported by backend '{backend}'"
                )
            }
            Self::Degraded { reason } => write!(f, "guest profile handling is degraded: {reason}"),
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
            Self::InconsistentSnapshot { detail } => {
                write!(f, "guest profile snapshot is inconsistent: {detail}")
            }
            Self::PermissionDenied { operation, detail } => {
                write!(
                    f,
                    "guest profile operation '{operation}' is not permitted: {detail}"
                )
            }
            Self::TemporarilyUnavailable { resource, detail } => {
                write!(
                    f,
                    "guest profile resource '{resource}' is unavailable: {detail}"
                )
            }
            Self::Backend { detail } => write!(f, "guest profile backend error: {detail}"),
        }
    }
}

impl Error for ProfileError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributionError {
    Unsupported {
        backend: &'static str,
        operation: &'static str,
    },
    Degraded {
        reason: String,
    },
    MissingIdentity {
        vm: VmId,
    },
    AmbiguousIdentity {
        vm: VmId,
        reason: String,
    },
    StaleIdentity {
        vm: VmId,
    },
    SourceConflict {
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
    Backend {
        detail: String,
    },
}

impl AttributionError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Unsupported { .. } => VmiErrorKind::UnsupportedBackend,
            Self::Degraded { .. } => VmiErrorKind::Degraded,
            Self::MissingIdentity { .. } => VmiErrorKind::MissingIdentity,
            Self::AmbiguousIdentity { .. } => VmiErrorKind::AmbiguousIdentity,
            Self::StaleIdentity { .. } => VmiErrorKind::StaleIdentity,
            Self::SourceConflict { .. } => VmiErrorKind::SourceConflict,
            Self::InconsistentSnapshot { .. } => VmiErrorKind::InconsistentSnapshot,
            Self::PermissionDenied { .. } => VmiErrorKind::PermissionDenied,
            Self::TemporarilyUnavailable { .. } => VmiErrorKind::TemporarilyUnavailable,
            Self::Backend { .. } => VmiErrorKind::Backend,
        }
    }

    pub fn is_unsupported(&self) -> bool {
        self.kind().is_unsupported()
    }

    pub fn is_degraded(&self) -> bool {
        self.kind().is_degraded()
    }
}

impl fmt::Display for AttributionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { backend, operation } => {
                write!(f, "guest attribution operation '{operation}' is unsupported by backend '{backend}'")
            }
            Self::Degraded { reason } => write!(f, "guest attribution is degraded: {reason}"),
            Self::MissingIdentity { vm } => {
                write!(f, "guest attribution has no identity for VM {}", vm.0)
            }
            Self::AmbiguousIdentity { vm, reason } => {
                write!(
                    f,
                    "guest attribution identity is ambiguous for VM {}: {reason}",
                    vm.0
                )
            }
            Self::StaleIdentity { vm } => {
                write!(f, "guest attribution identity is stale for VM {}", vm.0)
            }
            Self::SourceConflict { reason } => {
                write!(f, "guest attribution source conflict: {reason}")
            }
            Self::InconsistentSnapshot { detail } => {
                write!(f, "guest attribution snapshot is inconsistent: {detail}")
            }
            Self::PermissionDenied { operation, detail } => {
                write!(
                    f,
                    "guest attribution operation '{operation}' is not permitted: {detail}"
                )
            }
            Self::TemporarilyUnavailable { resource, detail } => {
                write!(
                    f,
                    "guest attribution resource '{resource}' is unavailable: {detail}"
                )
            }
            Self::Backend { detail } => write!(f, "guest attribution backend error: {detail}"),
        }
    }
}

impl Error for AttributionError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyscallCheckError {
    Unsupported {
        backend: &'static str,
        operation: &'static str,
    },
    Degraded {
        reason: String,
    },
    InvalidTarget {
        vm: VmId,
        detail: String,
    },
    UnsupportedArchitecture {
        arch: String,
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
    Profile(ProfileError),
    Memory(MemoryReadError),
    Registers(RegisterReadError),
    Translation(TranslationError),
    Backend {
        detail: String,
    },
}

impl SyscallCheckError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::Unsupported { .. } => VmiErrorKind::UnsupportedBackend,
            Self::Degraded { .. } => VmiErrorKind::Degraded,
            Self::InvalidTarget { .. } => VmiErrorKind::InvalidInput,
            Self::UnsupportedArchitecture { .. } => VmiErrorKind::UnsupportedArchitecture,
            Self::InconsistentSnapshot { .. } => VmiErrorKind::InconsistentSnapshot,
            Self::PermissionDenied { .. } => VmiErrorKind::PermissionDenied,
            Self::TemporarilyUnavailable { .. } => VmiErrorKind::TemporarilyUnavailable,
            Self::Profile(err) => err.kind(),
            Self::Memory(err) => err.kind(),
            Self::Registers(err) => err.kind(),
            Self::Translation(err) => err.kind(),
            Self::Backend { .. } => VmiErrorKind::Backend,
        }
    }

    pub fn is_unsupported(&self) -> bool {
        self.kind().is_unsupported()
    }

    pub fn is_degraded(&self) -> bool {
        self.kind().is_degraded()
    }
}

impl fmt::Display for SyscallCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { backend, operation } => {
                write!(
                    f,
                    "syscall path operation '{operation}' is unsupported by backend '{backend}'"
                )
            }
            Self::Degraded { reason } => write!(f, "syscall path check is degraded: {reason}"),
            Self::InvalidTarget { vm, detail } => {
                write!(f, "invalid syscall check target VM {}: {detail}", vm.0)
            }
            Self::UnsupportedArchitecture { arch } => {
                write!(
                    f,
                    "syscall path check is unsupported for guest architecture '{arch}'"
                )
            }
            Self::InconsistentSnapshot { detail } => {
                write!(f, "syscall path snapshot is inconsistent: {detail}")
            }
            Self::PermissionDenied { operation, detail } => {
                write!(
                    f,
                    "syscall path operation '{operation}' is not permitted: {detail}"
                )
            }
            Self::TemporarilyUnavailable { resource, detail } => {
                write!(
                    f,
                    "syscall path resource '{resource}' is unavailable: {detail}"
                )
            }
            Self::Profile(err) => write!(f, "syscall profile error: {err}"),
            Self::Memory(err) => write!(f, "syscall memory read error: {err}"),
            Self::Registers(err) => write!(f, "syscall register read error: {err}"),
            Self::Translation(err) => write!(f, "syscall translation error: {err}"),
            Self::Backend { detail } => write!(f, "syscall path backend error: {detail}"),
        }
    }
}

impl Error for SyscallCheckError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(err) => Some(err),
            Self::Memory(err) => Some(err),
            Self::Registers(err) => Some(err),
            Self::Translation(err) => Some(err),
            Self::Unsupported { .. }
            | Self::Degraded { .. }
            | Self::InvalidTarget { .. }
            | Self::UnsupportedArchitecture { .. }
            | Self::InconsistentSnapshot { .. }
            | Self::PermissionDenied { .. }
            | Self::TemporarilyUnavailable { .. }
            | Self::Backend { .. } => None,
        }
    }
}

impl From<ProfileError> for SyscallCheckError {
    fn from(value: ProfileError) -> Self {
        Self::Profile(value)
    }
}

impl From<MemoryReadError> for SyscallCheckError {
    fn from(value: MemoryReadError) -> Self {
        Self::Memory(value)
    }
}

impl From<RegisterReadError> for SyscallCheckError {
    fn from(value: RegisterReadError) -> Self {
        Self::Registers(value)
    }
}

impl From<TranslationError> for SyscallCheckError {
    fn from(value: TranslationError) -> Self {
        Self::Translation(value)
    }
}

pub struct NoVmiBackend;

pub trait GuestMemoryReader: Send + Sync {
    fn read_physical(
        &self,
        vm: VmId,
        gpa: GuestPhysical,
        buf: &mut [u8],
    ) -> Result<usize, MemoryReadError>;
}

#[derive(Debug, Clone, Default)]
pub struct SyntheticGuestPhysicalMemoryReader {
    ranges: Vec<SyntheticMemoryRange>,
    partial_reads: bool,
}

#[derive(Debug, Clone)]
struct SyntheticMemoryRange {
    start: u64,
    end: u64,
    access: SyntheticMemoryAccess,
}

#[derive(Debug, Clone)]
enum SyntheticMemoryAccess {
    Mapped(Vec<u8>),
    PermissionDenied { detail: String },
    TemporarilyUnavailable { detail: String },
}

impl SyntheticGuestPhysicalMemoryReader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_partial_reads(mut self, allow: bool) -> Self {
        self.partial_reads = allow;
        self
    }

    pub fn set_partial_reads(&mut self, allow: bool) {
        self.partial_reads = allow;
    }

    pub fn map_range(
        &mut self,
        gpa: GuestPhysical,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), MemoryReadError> {
        let bytes = bytes.into();
        let end = checked_memory_range(gpa, bytes.len())?;
        self.insert_range(SyntheticMemoryRange {
            start: gpa.0,
            end,
            access: SyntheticMemoryAccess::Mapped(bytes),
        })
    }

    pub fn deny_range(
        &mut self,
        gpa: GuestPhysical,
        len: usize,
        detail: impl Into<String>,
    ) -> Result<(), MemoryReadError> {
        let end = checked_memory_range(gpa, len)?;
        self.insert_range(SyntheticMemoryRange {
            start: gpa.0,
            end,
            access: SyntheticMemoryAccess::PermissionDenied {
                detail: detail.into(),
            },
        })
    }

    pub fn mark_unavailable_range(
        &mut self,
        gpa: GuestPhysical,
        len: usize,
        detail: impl Into<String>,
    ) -> Result<(), MemoryReadError> {
        let end = checked_memory_range(gpa, len)?;
        self.insert_range(SyntheticMemoryRange {
            start: gpa.0,
            end,
            access: SyntheticMemoryAccess::TemporarilyUnavailable {
                detail: detail.into(),
            },
        })
    }

    fn insert_range(&mut self, range: SyntheticMemoryRange) -> Result<(), MemoryReadError> {
        if self
            .ranges
            .iter()
            .any(|existing| ranges_overlap(range.start, range.end, existing.start, existing.end))
        {
            return Err(MemoryReadError::InvalidRange {
                gpa: GuestPhysical(range.start),
                len: range_len(range.start, range.end),
            });
        }

        self.ranges.push(range);
        self.ranges.sort_by_key(|range| range.start);
        Ok(())
    }

    fn validate_full_read(&self, gpa: GuestPhysical, len: usize) -> Result<(), MemoryReadError> {
        let mut cursor = gpa.0;
        let mut remaining = len;
        while remaining > 0 {
            let range = self
                .range_containing(cursor)
                .ok_or(MemoryReadError::Unmapped {
                    gpa: GuestPhysical(cursor),
                    len: remaining,
                })?;

            match &range.access {
                SyntheticMemoryAccess::Mapped(bytes) => {
                    let offset = range_offset(range, cursor);
                    let readable = bytes.len().saturating_sub(offset).min(remaining);
                    cursor = advance_guest_physical(cursor, readable)?;
                    remaining -= readable;
                }
                SyntheticMemoryAccess::PermissionDenied { detail } => {
                    return Err(MemoryReadError::PermissionDenied {
                        operation: "read_physical",
                        detail: detail.clone(),
                    });
                }
                SyntheticMemoryAccess::TemporarilyUnavailable { detail } => {
                    return Err(MemoryReadError::TemporarilyUnavailable {
                        resource: "synthetic-memory-range",
                        detail: detail.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn copy_mapped_prefix(
        &self,
        gpa: GuestPhysical,
        buf: &mut [u8],
    ) -> Result<usize, MemoryReadError> {
        let mut cursor = gpa.0;
        let mut copied = 0;
        while copied < buf.len() {
            let Some(range) = self.range_containing(cursor) else {
                if self.partial_reads && copied > 0 {
                    return Ok(copied);
                }
                return Err(MemoryReadError::Unmapped {
                    gpa: GuestPhysical(cursor),
                    len: buf.len() - copied,
                });
            };

            match &range.access {
                SyntheticMemoryAccess::Mapped(bytes) => {
                    let offset = range_offset(range, cursor);
                    let readable = bytes.len().saturating_sub(offset).min(buf.len() - copied);
                    buf[copied..copied + readable]
                        .copy_from_slice(&bytes[offset..offset + readable]);
                    cursor = advance_guest_physical(cursor, readable)?;
                    copied += readable;
                }
                SyntheticMemoryAccess::PermissionDenied { detail } => {
                    if self.partial_reads && copied > 0 {
                        return Ok(copied);
                    }
                    return Err(MemoryReadError::PermissionDenied {
                        operation: "read_physical",
                        detail: detail.clone(),
                    });
                }
                SyntheticMemoryAccess::TemporarilyUnavailable { detail } => {
                    if self.partial_reads && copied > 0 {
                        return Ok(copied);
                    }
                    return Err(MemoryReadError::TemporarilyUnavailable {
                        resource: "synthetic-memory-range",
                        detail: detail.clone(),
                    });
                }
            }
        }
        Ok(copied)
    }

    fn range_containing(&self, addr: u64) -> Option<&SyntheticMemoryRange> {
        self.ranges
            .iter()
            .find(|range| range.start <= addr && addr < range.end)
    }
}

impl GuestMemoryReader for SyntheticGuestPhysicalMemoryReader {
    fn read_physical(
        &self,
        _vm: VmId,
        gpa: GuestPhysical,
        buf: &mut [u8],
    ) -> Result<usize, MemoryReadError> {
        checked_memory_range(gpa, buf.len())?;
        if !self.partial_reads {
            self.validate_full_read(gpa, buf.len())?;
        }
        self.copy_mapped_prefix(gpa, buf)
    }
}

fn checked_memory_range(gpa: GuestPhysical, len: usize) -> Result<u64, MemoryReadError> {
    if len == 0 {
        return Err(MemoryReadError::InvalidRange { gpa, len });
    }

    let len_u64 = u64::try_from(len).map_err(|_| MemoryReadError::InvalidAddress { gpa, len })?;
    gpa.0
        .checked_add(len_u64)
        .ok_or(MemoryReadError::InvalidAddress { gpa, len })
}

fn advance_guest_physical(gpa: u64, len: usize) -> Result<u64, MemoryReadError> {
    let len_u64 = u64::try_from(len).map_err(|_| MemoryReadError::InvalidAddress {
        gpa: GuestPhysical(gpa),
        len,
    })?;
    gpa.checked_add(len_u64)
        .ok_or(MemoryReadError::InvalidAddress {
            gpa: GuestPhysical(gpa),
            len,
        })
}

fn range_offset(range: &SyntheticMemoryRange, addr: u64) -> usize {
    usize::try_from(addr - range.start).expect("synthetic memory range offset fits usize")
}

fn range_len(start: u64, end: u64) -> usize {
    usize::try_from(end - start).expect("synthetic memory range length came from usize")
}

fn ranges_overlap(left_start: u64, left_end: u64, right_start: u64, right_end: u64) -> bool {
    left_start < right_end && right_start < left_end
}

pub trait VcpuRegisterReader: Send + Sync {
    fn read_registers(&self, vm: VmId, vcpu: VcpuId) -> Result<GuestRegisters, RegisterReadError>;
}

pub trait AddressTranslator: Send + Sync {
    fn translate(
        &self,
        vm: VmId,
        regs: &GuestRegisters,
        gva: GuestVirtual,
    ) -> Result<TranslationResult, TranslationError>;
}

pub trait GuestProfileProvider: Send + Sync {
    fn load_profile(&self, vm: VmId) -> Result<GuestProfile, ProfileError>;
}

pub trait GuestAttributor: Send + Sync {
    fn attribute_address(
        &self,
        vm: VmId,
        vcpu: Option<VcpuId>,
        gva: GuestVirtual,
    ) -> Result<GuestAttribution, AttributionError>;
}

pub trait SyscallPathChecker: Send + Sync {
    fn check_syscall_path(&self, vm: VmId) -> Result<SyscallPathReport, SyscallCheckError>;
}

pub trait TrapController: Send + Sync {
    fn arm_execute_trap(&self, vm: VmId, gpa: GuestPhysical, page_size: u64) -> Result<(), String>;
    fn arm_write_trap(&self, vm: VmId, gpa: GuestPhysical, page_size: u64) -> Result<(), String>;
    fn decide_trap(
        &self,
        vm: VmId,
        vcpu: VcpuId,
        gpa: GuestPhysical,
    ) -> Result<TrapDecision, String>;
}

impl GuestMemoryReader for NoVmiBackend {
    fn read_physical(
        &self,
        _vm: VmId,
        _gpa: GuestPhysical,
        _buf: &mut [u8],
    ) -> Result<usize, MemoryReadError> {
        Err(MemoryReadError::Unsupported {
            backend: "host-side-sensor",
            operation: "read_physical",
        })
    }
}

impl VcpuRegisterReader for NoVmiBackend {
    fn read_registers(
        &self,
        _vm: VmId,
        _vcpu: VcpuId,
    ) -> Result<GuestRegisters, RegisterReadError> {
        Err(RegisterReadError::Unsupported {
            backend: "host-side-sensor",
            operation: "read_registers",
        })
    }
}

impl AddressTranslator for NoVmiBackend {
    fn translate(
        &self,
        _vm: VmId,
        _regs: &GuestRegisters,
        _gva: GuestVirtual,
    ) -> Result<TranslationResult, TranslationError> {
        Err(TranslationError::Unsupported {
            backend: "host-side-sensor",
            operation: "translate",
        })
    }
}

impl GuestProfileProvider for NoVmiBackend {
    fn load_profile(&self, _vm: VmId) -> Result<GuestProfile, ProfileError> {
        Err(ProfileError::Unsupported {
            backend: "host-side-sensor",
            operation: "load_profile",
        })
    }
}

impl GuestAttributor for NoVmiBackend {
    fn attribute_address(
        &self,
        _vm: VmId,
        _vcpu: Option<VcpuId>,
        _gva: GuestVirtual,
    ) -> Result<GuestAttribution, AttributionError> {
        Err(AttributionError::Unsupported {
            backend: "host-side-sensor",
            operation: "attribute_address",
        })
    }
}

impl SyscallPathChecker for NoVmiBackend {
    fn check_syscall_path(&self, _vm: VmId) -> Result<SyscallPathReport, SyscallCheckError> {
        Err(SyscallCheckError::Unsupported {
            backend: "host-side-sensor",
            operation: "check_syscall_path",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SyntheticBackend;

    impl GuestMemoryReader for SyntheticBackend {
        fn read_physical(
            &self,
            _vm: VmId,
            _gpa: GuestPhysical,
            buf: &mut [u8],
        ) -> Result<usize, MemoryReadError> {
            if buf.is_empty() {
                return Err(MemoryReadError::InvalidRange {
                    gpa: GuestPhysical(0x1000),
                    len: 0,
                });
            }
            buf[0] = 0xcc;
            Ok(1)
        }
    }

    impl VcpuRegisterReader for SyntheticBackend {
        fn read_registers(
            &self,
            _vm: VmId,
            _vcpu: VcpuId,
        ) -> Result<GuestRegisters, RegisterReadError> {
            Ok(GuestRegisters {
                pc: 0x401000,
                sp: 0x7fff_ffff,
                cr3_or_ttbr: Some(0x100000),
                privilege: Some("kernel".to_string()),
            })
        }
    }

    impl AddressTranslator for SyntheticBackend {
        fn translate(
            &self,
            _vm: VmId,
            regs: &GuestRegisters,
            gva: GuestVirtual,
        ) -> Result<TranslationResult, TranslationError> {
            if regs.cr3_or_ttbr.is_none() {
                return Err(TranslationError::MissingContext {
                    field: "cr3_or_ttbr",
                });
            }
            Ok(TranslationResult {
                gpa: GuestPhysical(gva.0 & 0x000f_ffff),
                readable: true,
                writable: false,
                executable: true,
                user: false,
                page_size: 4096,
            })
        }
    }

    impl GuestProfileProvider for SyntheticBackend {
        fn load_profile(&self, _vm: VmId) -> Result<GuestProfile, ProfileError> {
            Ok(GuestProfile {
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                pointer_width: 64,
                syscall_entry: Some(0xffff_ffff_8100_0000),
            })
        }
    }

    impl GuestAttributor for SyntheticBackend {
        fn attribute_address(
            &self,
            vm: VmId,
            vcpu: Option<VcpuId>,
            _gva: GuestVirtual,
        ) -> Result<GuestAttribution, AttributionError> {
            Ok(GuestAttribution {
                vm,
                vcpu,
                process: Some("kernel".to_string()),
                thread: None,
                module: Some("vmlinux".to_string()),
                symbol: Some("entry_SYSCALL_64".to_string()),
            })
        }
    }

    impl SyscallPathChecker for SyntheticBackend {
        fn check_syscall_path(&self, _vm: VmId) -> Result<SyscallPathReport, SyscallCheckError> {
            Ok(SyscallPathReport {
                ok: true,
                os: "linux".to_string(),
                entry: Some(0xffff_ffff_8100_0000),
                table: Some(0xffff_ffff_8200_0000),
                findings: Vec::new(),
            })
        }
    }

    #[test]
    fn no_vmi_backend_returns_typed_unsupported_errors() {
        let backend = NoVmiBackend;
        let vm = VmId(7);
        let regs = GuestRegisters {
            pc: 0,
            sp: 0,
            cr3_or_ttbr: None,
            privilege: None,
        };
        let mut buf = [0u8; 8];

        assert!(backend
            .read_physical(vm, GuestPhysical(0), &mut buf)
            .unwrap_err()
            .is_unsupported());
        assert!(backend
            .read_registers(vm, VcpuId(0))
            .unwrap_err()
            .is_unsupported());
        assert!(backend
            .translate(vm, &regs, GuestVirtual(0))
            .unwrap_err()
            .is_unsupported());
        assert!(backend.load_profile(vm).unwrap_err().is_unsupported());
        assert!(backend
            .attribute_address(vm, Some(VcpuId(0)), GuestVirtual(0))
            .unwrap_err()
            .is_unsupported());
        assert!(backend.check_syscall_path(vm).unwrap_err().is_unsupported());
    }

    #[test]
    fn synthetic_backend_can_return_success_without_claiming_live_vmi() {
        let backend = SyntheticBackend;
        let vm = VmId(1);
        let mut buf = [0u8; 4];

        assert_eq!(
            backend
                .read_physical(vm, GuestPhysical(0x1000), &mut buf)
                .expect("synthetic memory read"),
            1
        );
        assert_eq!(buf[0], 0xcc);

        let regs = backend
            .read_registers(vm, VcpuId(0))
            .expect("synthetic register read");
        assert_eq!(regs.pc, 0x401000);

        let translation = backend
            .translate(vm, &regs, GuestVirtual(0xffff_8000_0000_3000))
            .expect("synthetic translation");
        assert_eq!(translation.gpa, GuestPhysical(0x3000));

        let profile = backend.load_profile(vm).expect("synthetic profile");
        assert_eq!(profile.pointer_width, 64);

        let attribution = backend
            .attribute_address(vm, Some(VcpuId(0)), GuestVirtual(0))
            .expect("synthetic attribution");
        assert_eq!(attribution.symbol.as_deref(), Some("entry_SYSCALL_64"));

        assert!(
            backend
                .check_syscall_path(vm)
                .expect("synthetic syscall")
                .ok
        );
    }

    #[test]
    fn typed_errors_cover_malformed_unmapped_and_degraded_inputs() {
        let unmapped = MemoryReadError::Unmapped {
            gpa: GuestPhysical(0xdead_0000),
            len: 4096,
        };
        assert_eq!(unmapped.kind(), VmiErrorKind::Unmapped);
        assert!(unmapped.to_string().contains("unmapped"));

        let malformed = TranslationError::MalformedPageTables {
            level: "pte",
            detail: "reserved bit set".to_string(),
        };
        assert_eq!(malformed.kind(), VmiErrorKind::Malformed);
        let syscall: SyscallCheckError = malformed.into();
        assert_eq!(syscall.kind(), VmiErrorKind::Malformed);

        let degraded = ProfileError::Degraded {
            reason: "profile cache unavailable".to_string(),
        };
        assert!(degraded.is_degraded());

        let missing = AttributionError::MissingIdentity { vm: VmId(42) };
        assert_eq!(missing.kind(), VmiErrorKind::MissingIdentity);
    }

    #[test]
    fn synthetic_backend_rejects_malformed_translation_context() {
        let backend = SyntheticBackend;
        let regs = GuestRegisters {
            pc: 0,
            sp: 0,
            cr3_or_ttbr: None,
            privilege: None,
        };
        let err = backend
            .translate(VmId(1), &regs, GuestVirtual(0x1000))
            .expect_err("missing translation root must be typed");

        assert_eq!(err.kind(), VmiErrorKind::InvalidInput);
        assert!(err.to_string().contains("cr3_or_ttbr"));
    }
}
