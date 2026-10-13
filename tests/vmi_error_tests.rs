use std::error::Error;

use aegishv::vmi::{
    BackendError, GuestPhysical, GuestVirtual, MemoryReadError, ProfileError, RegisterReadError,
    TranslationError, VmiErrorKind,
};

#[test]
fn typed_error_formatting_covers_address_memory_snapshot_and_limits() {
    let invalid = MemoryReadError::InvalidAddress {
        gpa: GuestPhysical(0),
        len: 8,
    };
    assert_eq!(invalid.kind(), VmiErrorKind::InvalidAddress);
    assert_eq!(
        invalid.to_string(),
        "invalid guest physical address range gpa=0x0 len=8"
    );

    let missing = MemoryReadError::MissingMemory {
        gpa: GuestPhysical(0xdead_0000),
        len: 4096,
    };
    assert_eq!(missing.kind(), VmiErrorKind::MissingMemory);
    assert!(missing.to_string().contains("gpa=0xdead0000"));

    let translation = TranslationError::TranslationFailed {
        gva: GuestVirtual(0xffff_8000_0000_1000),
        detail: "page-table root is stale".to_string(),
    };
    assert_eq!(translation.kind(), VmiErrorKind::TranslationFailure);
    assert!(translation.to_string().contains("gva=0xffff800000001000"));
    assert!(translation.to_string().contains("page-table root is stale"));

    let not_present = TranslationError::NotPresent {
        level: "l0",
        gva: GuestVirtual(0x4000),
    };
    assert_eq!(not_present.kind(), VmiErrorKind::TranslationFailure);
    assert_eq!(
        not_present.to_string(),
        "translation entry is not present at l0 for gva=0x4000"
    );
    assert!(!not_present.to_string().contains("x86_64"));

    let snapshot = RegisterReadError::Backend(BackendError::InconsistentSnapshot {
        detail: "registers and memory were captured from different pause epochs".to_string(),
    });
    assert_eq!(snapshot.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(snapshot.to_string().contains("different pause epochs"));

    let permission = MemoryReadError::Backend(BackendError::PermissionDenied {
        operation: "resolve_symbol",
        detail: "profile store is not readable by this process".to_string(),
    });
    assert_eq!(permission.kind(), VmiErrorKind::PermissionDenied);
    assert!(permission.to_string().contains("resolve_symbol"));

    let unavailable = ProfileError::Backend(BackendError::TemporarilyUnavailable {
        resource: "profile-cache",
        detail: "reload is in progress".to_string(),
    });
    assert_eq!(unavailable.kind(), VmiErrorKind::TemporarilyUnavailable);
    assert!(unavailable.to_string().contains("profile-cache"));
}

#[test]
fn degraded_and_unsupported_architecture_errors_are_branchable() {
    let degraded = MemoryReadError::Backend(BackendError::Degraded {
        reason: "memory snapshot lacks executable page permissions".to_string(),
    });
    assert_eq!(degraded.kind(), VmiErrorKind::Degraded);
    assert!(degraded.is_degraded());
    assert!(degraded.to_string().contains("snapshot lacks"));

    let profile = ProfileError::UnsupportedArchitecture {
        arch: "mips64".to_string(),
    };
    assert_eq!(profile.kind(), VmiErrorKind::UnsupportedArchitecture);
    assert!(profile.is_unsupported());
    assert!(profile.to_string().contains("mips64"));

    let translation = TranslationError::MalformedPageTables {
        level: "pud",
        detail: "reserved bit set".to_string(),
    };
    assert_eq!(translation.kind(), VmiErrorKind::Malformed);
    assert!(translation.to_string().contains("reserved bit set"));
}

#[test]
fn backend_error_retains_source_and_kind() {
    let cause = BackendError::TemporarilyUnavailable {
        resource: "vcpu-state",
        detail: "vCPU is running".to_string(),
    };
    let error = RegisterReadError::Backend(cause.clone());
    assert_eq!(error.kind(), VmiErrorKind::TemporarilyUnavailable);
    assert_eq!(
        error.source().map(ToString::to_string),
        Some(cause.to_string())
    );
}
