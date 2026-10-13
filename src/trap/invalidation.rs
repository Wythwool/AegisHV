use super::stage2::{PageSize, Stage2BackendKind};
use super::{TrapError, TrapErrorKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidationScope {
    SinglePage {
        owner_vm: String,
        address_space: String,
        gpa: u64,
        page_size: PageSize,
    },
    AddressSpace {
        owner_vm: String,
        address_space: String,
    },
    Vm {
        owner_vm: String,
    },
    Global,
}

impl InvalidationScope {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SinglePage { .. } => "single_page",
            Self::AddressSpace { .. } => "address_space",
            Self::Vm { .. } => "vm",
            Self::Global => "global",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidationPrimitive {
    SyntheticRecord,
    IntelInveptSingleContext,
    IntelInveptAllContexts,
    AmdInvlpga,
    AmdFlushAsid,
    ArmTlbiVaae2,
    ArmTlbiVmalls12e1,
}

impl InvalidationPrimitive {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SyntheticRecord => "synthetic_record",
            Self::IntelInveptSingleContext => "invept_single_context",
            Self::IntelInveptAllContexts => "invept_all_contexts",
            Self::AmdInvlpga => "invlpga",
            Self::AmdFlushAsid => "amd_flush_asid",
            Self::ArmTlbiVaae2 => "tlbi_vaae2",
            Self::ArmTlbiVmalls12e1 => "tlbi_vmalls12e1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidationStatus {
    Recorded,
    Required,
}

impl InvalidationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::Required => "required",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidationPlan {
    pub backend: Stage2BackendKind,
    pub scope_kind: &'static str,
    pub primitive: InvalidationPrimitive,
    pub status: InvalidationStatus,
}

pub fn plan_invalidation(
    backend: Stage2BackendKind,
    scope: &InvalidationScope,
) -> Result<InvalidationPlan, TrapError> {
    validate_scope(scope)?;
    let primitive = match backend {
        Stage2BackendKind::Synthetic => InvalidationPrimitive::SyntheticRecord,
        Stage2BackendKind::IntelEpt => match scope {
            InvalidationScope::Global | InvalidationScope::Vm { .. } => {
                InvalidationPrimitive::IntelInveptAllContexts
            }
            InvalidationScope::SinglePage { .. } | InvalidationScope::AddressSpace { .. } => {
                InvalidationPrimitive::IntelInveptSingleContext
            }
        },
        Stage2BackendKind::AmdNpt => match scope {
            InvalidationScope::SinglePage { .. } => InvalidationPrimitive::AmdInvlpga,
            InvalidationScope::AddressSpace { .. }
            | InvalidationScope::Vm { .. }
            | InvalidationScope::Global => InvalidationPrimitive::AmdFlushAsid,
        },
        Stage2BackendKind::ArmStage2 => match scope {
            InvalidationScope::SinglePage { .. } => InvalidationPrimitive::ArmTlbiVaae2,
            InvalidationScope::AddressSpace { .. }
            | InvalidationScope::Vm { .. }
            | InvalidationScope::Global => InvalidationPrimitive::ArmTlbiVmalls12e1,
        },
    };
    Ok(InvalidationPlan {
        backend,
        scope_kind: scope.kind(),
        primitive,
        status: if backend == Stage2BackendKind::Synthetic {
            InvalidationStatus::Recorded
        } else {
            InvalidationStatus::Required
        },
    })
}

fn validate_scope(scope: &InvalidationScope) -> Result<(), TrapError> {
    match scope {
        InvalidationScope::SinglePage {
            owner_vm,
            address_space,
            gpa,
            page_size,
        } => {
            if owner_vm.trim().is_empty() || address_space.trim().is_empty() {
                return Err(TrapError::new(
                    TrapErrorKind::MalformedInput,
                    "single-page invalidation requires VM and address-space identifiers",
                ));
            }
            if !page_size.is_aligned(*gpa) {
                return Err(TrapError::new(
                    TrapErrorKind::Misaligned,
                    format!(
                        "single-page invalidation address {gpa:#x} is not aligned to {}",
                        page_size.as_str()
                    ),
                ));
            }
        }
        InvalidationScope::AddressSpace {
            owner_vm,
            address_space,
        } => {
            if owner_vm.trim().is_empty() || address_space.trim().is_empty() {
                return Err(TrapError::new(
                    TrapErrorKind::MalformedInput,
                    "address-space invalidation requires VM and address-space identifiers",
                ));
            }
        }
        InvalidationScope::Vm { owner_vm } => {
            if owner_vm.trim().is_empty() {
                return Err(TrapError::new(
                    TrapErrorKind::MalformedInput,
                    "VM invalidation requires a non-empty VM identifier",
                ));
            }
        }
        InvalidationScope::Global => {}
    }
    Ok(())
}
