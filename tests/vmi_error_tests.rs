use std::error::Error;

use aegishv::vmi::{
    AddressTranslator, AttributionError, GuestAttributor, GuestMemoryReader, GuestPhysical,
    GuestProfileProvider, GuestRegisters, GuestVirtual, MemoryReadError, NoVmiBackend,
    ProfileError, RegisterReadError, SyscallCheckError, SyscallPathChecker, TranslationError,
    VcpuId, VcpuRegisterReader, VmId, VmiErrorKind,
};

fn empty_registers() -> GuestRegisters {
    GuestRegisters {
        pc: 0,
        sp: 0,
        cr3_or_ttbr: None,
        privilege: None,
    }
}

#[test]
fn no_vmi_backend_refuses_every_operation_with_typed_unsupported_errors() {
    let backend = NoVmiBackend;
    let vm = VmId(9);
    let mut buf = [0u8; 16];
    let regs = empty_registers();

    let memory = backend
        .read_physical(vm, GuestPhysical(0x1000), &mut buf)
        .expect_err("no-backend memory read must not return success");
    assert_eq!(memory.kind(), VmiErrorKind::UnsupportedBackend);
    assert!(memory.is_unsupported());
    assert_eq!(memory.kind().as_str(), "unsupported_backend");
    assert!(memory.to_string().contains("host-side-sensor"));
    assert!(memory.to_string().contains("read_physical"));

    let registers = backend
        .read_registers(vm, VcpuId(0))
        .expect_err("no-backend register read must not return success");
    assert_eq!(registers.kind(), VmiErrorKind::UnsupportedBackend);
    assert!(registers.to_string().contains("read_registers"));

    let translation = backend
        .translate(vm, &regs, GuestVirtual(0x4000))
        .expect_err("no-backend translation must not return success");
    assert_eq!(translation.kind(), VmiErrorKind::UnsupportedBackend);
    assert!(translation.to_string().contains("translate"));

    let profile = backend
        .load_profile(vm)
        .expect_err("no-backend profile load must not return success");
    assert_eq!(profile.kind(), VmiErrorKind::UnsupportedBackend);
    assert!(profile.to_string().contains("load_profile"));

    let attribution = backend
        .attribute_address(vm, Some(VcpuId(0)), GuestVirtual(0x4000))
        .expect_err("no-backend attribution must not return success");
    assert_eq!(attribution.kind(), VmiErrorKind::UnsupportedBackend);
    assert!(attribution.to_string().contains("attribute_address"));

    let syscall = backend
        .check_syscall_path(vm)
        .expect_err("no-backend syscall check must not return success");
    assert_eq!(syscall.kind(), VmiErrorKind::UnsupportedBackend);
    assert!(syscall.to_string().contains("check_syscall_path"));
}

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

    let snapshot = RegisterReadError::InconsistentSnapshot {
        detail: "registers and memory were captured from different pause epochs".to_string(),
    };
    assert_eq!(snapshot.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(snapshot.to_string().contains("different pause epochs"));

    let permission = AttributionError::PermissionDenied {
        operation: "resolve_symbol",
        detail: "profile store is not readable by this process".to_string(),
    };
    assert_eq!(permission.kind(), VmiErrorKind::PermissionDenied);
    assert!(permission.to_string().contains("resolve_symbol"));

    let unavailable = ProfileError::TemporarilyUnavailable {
        resource: "profile-cache",
        detail: "reload is in progress".to_string(),
    };
    assert_eq!(unavailable.kind(), VmiErrorKind::TemporarilyUnavailable);
    assert!(unavailable.to_string().contains("profile-cache"));
}

#[test]
fn degraded_and_unsupported_architecture_errors_are_branchable() {
    let degraded = SyscallCheckError::Degraded {
        reason: "memory snapshot lacks executable page permissions".to_string(),
    };
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
fn syscall_wrappers_preserve_nested_error_kind_and_source() {
    let memory = MemoryReadError::MissingMemory {
        gpa: GuestPhysical(0x2000),
        len: 64,
    };
    let syscall: SyscallCheckError = memory.clone().into();

    assert_eq!(syscall.kind(), VmiErrorKind::MissingMemory);
    assert!(syscall.to_string().contains("syscall memory read error"));
    assert_eq!(
        syscall.source().map(ToString::to_string),
        Some(memory.to_string())
    );

    let registers = RegisterReadError::TemporarilyUnavailable {
        resource: "vcpu-state",
        detail: "vCPU is running".to_string(),
    };
    let syscall: SyscallCheckError = registers.clone().into();
    assert_eq!(syscall.kind(), VmiErrorKind::TemporarilyUnavailable);
    assert_eq!(
        syscall.source().map(ToString::to_string),
        Some(registers.to_string())
    );
}
