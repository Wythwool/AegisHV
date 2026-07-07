use aegishv::vmi::{RegisterReadError, VmiErrorKind};
use aegishv::vmi_registers::{
    Arm64RegisterSnapshot, DescriptorTableRegister, RegisterSnapshot, X86_64RegisterSnapshot,
};

const X86_CR4_LA57: u64 = 1 << 12;
const X86_EFER_LME: u64 = 1 << 8;
const X86_EFER_LMA: u64 = 1 << 10;
const X86_EFER_NXE: u64 = 1 << 11;
const ARM64_SCTLR_M: u64 = 1 << 0;

fn x86_snapshot() -> X86_64RegisterSnapshot {
    X86_64RegisterSnapshot::new(
        0x8005_0033,
        0xdead_beef,
        0x1234_5000,
        X86_CR4_LA57,
        X86_EFER_LME | X86_EFER_LMA | X86_EFER_NXE,
        DescriptorTableRegister::new(0xffff_8000_0000_1000, 0x0fff),
        DescriptorTableRegister::new(0xffff_8000_0000_2000, 0x007f),
    )
}

fn arm64_snapshot() -> Arm64RegisterSnapshot {
    Arm64RegisterSnapshot::new(
        0x0000_0000_1000_0001,
        0xffff_0000_2000_0002,
        0x0000_0000_8080_3520,
        ARM64_SCTLR_M,
        0xffff_0000_0000_8000,
    )
}

#[test]
fn x86_64_register_snapshot_preserves_control_msrs_and_tables() {
    let snapshot = x86_snapshot();
    let top = RegisterSnapshot::x86_64(snapshot.clone());

    assert_eq!(top.architecture(), "x86_64");
    assert_eq!(snapshot.cr0().expect("cr0"), 0x8005_0033);
    assert_eq!(snapshot.cr2().expect("cr2"), 0xdead_beef);
    assert_eq!(top.x86_cr3().expect("cr3"), 0x1234_5000);
    assert_eq!(snapshot.cr4().expect("cr4"), X86_CR4_LA57);
    assert_eq!(
        snapshot.efer().expect("efer"),
        X86_EFER_LME | X86_EFER_LMA | X86_EFER_NXE
    );
    assert_eq!(
        top.x86_idtr().expect("idtr"),
        DescriptorTableRegister::new(0xffff_8000_0000_1000, 0x0fff)
    );
    assert_eq!(
        top.x86_gdtr().expect("gdtr"),
        DescriptorTableRegister::new(0xffff_8000_0000_2000, 0x007f)
    );
}

#[test]
fn x86_64_register_accessors_derive_paging_and_efer_state() {
    let snapshot = RegisterSnapshot::x86_64(x86_snapshot());

    assert!(snapshot.x86_la57_enabled().expect("la57"));
    assert!(snapshot.x86_long_mode_enabled().expect("lme"));
    assert!(snapshot.x86_long_mode_active().expect("lma"));
    assert!(snapshot.x86_nx_enabled().expect("nxe"));
}

#[test]
fn arm64_register_snapshot_preserves_translation_and_vector_state() {
    let snapshot = arm64_snapshot();
    let top = RegisterSnapshot::arm64(snapshot.clone());

    assert_eq!(top.architecture(), "arm64");
    assert_eq!(
        top.arm64_ttbr0_el1().expect("ttbr0_el1"),
        0x0000_0000_1000_0001
    );
    assert_eq!(
        top.arm64_ttbr1_el1().expect("ttbr1_el1"),
        0xffff_0000_2000_0002
    );
    assert_eq!(top.arm64_tcr_el1().expect("tcr_el1"), 0x0000_0000_8080_3520);
    assert_eq!(top.arm64_sctlr_el1().expect("sctlr_el1"), ARM64_SCTLR_M);
    assert_eq!(
        top.arm64_vbar_el1().expect("vbar_el1"),
        0xffff_0000_0000_8000
    );
    assert!(snapshot.mmu_enabled().expect("sctlr.m"));
}

#[test]
fn wrong_architecture_access_returns_typed_error() {
    let arm64 = RegisterSnapshot::arm64(arm64_snapshot());
    let err = arm64
        .x86_cr3()
        .expect_err("ARM64 snapshot must not expose x86 CR3");

    assert_eq!(err.kind(), VmiErrorKind::InvalidInput);
    assert!(matches!(
        err,
        RegisterReadError::WrongArchitecture {
            expected: "x86_64",
            actual: "arm64"
        }
    ));

    let x86 = RegisterSnapshot::x86_64(x86_snapshot());
    let err = x86
        .arm64_ttbr0_el1()
        .expect_err("x86 snapshot must not expose ARM64 TTBR0");

    assert_eq!(err.kind(), VmiErrorKind::InvalidInput);
    assert!(matches!(
        err,
        RegisterReadError::WrongArchitecture {
            expected: "arm64",
            actual: "x86_64"
        }
    ));
}

#[test]
fn missing_required_register_state_returns_typed_error() {
    let mut x86 = x86_snapshot();
    x86.cr3 = None;
    let err = RegisterSnapshot::x86_64(x86)
        .x86_cr3()
        .expect_err("missing CR3 must fail");

    assert_eq!(err.kind(), VmiErrorKind::InvalidInput);
    assert!(matches!(
        err,
        RegisterReadError::MissingRegister {
            arch: "x86_64",
            register: "cr3"
        }
    ));
    assert!(err.to_string().contains("cr3"));

    let mut arm64 = arm64_snapshot();
    arm64.ttbr1_el1 = None;
    let err = RegisterSnapshot::arm64(arm64)
        .arm64_ttbr1_el1()
        .expect_err("missing TTBR1_EL1 must fail");

    assert_eq!(err.kind(), VmiErrorKind::InvalidInput);
    assert!(matches!(
        err,
        RegisterReadError::MissingRegister {
            arch: "arm64",
            register: "ttbr1_el1"
        }
    ));
}

#[test]
fn zero_valued_registers_are_valid_when_present() {
    let x86 = X86_64RegisterSnapshot::new(
        0,
        0,
        0,
        0,
        0,
        DescriptorTableRegister::new(0, 0),
        DescriptorTableRegister::new(0, 0),
    );
    let x86 = RegisterSnapshot::x86_64(x86);

    assert_eq!(x86.x86_cr3().expect("zero cr3"), 0);
    assert_eq!(
        x86.x86_idtr().expect("zero idtr"),
        DescriptorTableRegister::new(0, 0)
    );
    assert!(!x86.x86_la57_enabled().expect("zero cr4"));
    assert!(!x86.x86_long_mode_enabled().expect("zero efer"));
    assert!(!x86.x86_long_mode_active().expect("zero efer"));
    assert!(!x86.x86_nx_enabled().expect("zero efer"));

    let arm64 = Arm64RegisterSnapshot::new(0, 0, 0, 0, 0);
    assert_eq!(arm64.ttbr0_el1().expect("zero ttbr0"), 0);
    assert_eq!(arm64.ttbr1_el1().expect("zero ttbr1"), 0);
    assert_eq!(arm64.tcr_el1().expect("zero tcr"), 0);
    assert_eq!(arm64.vbar_el1().expect("zero vbar"), 0);
    assert!(!arm64.mmu_enabled().expect("zero sctlr"));
}

#[test]
fn unsupported_architecture_returns_typed_unsupported_error() {
    let snapshot = RegisterSnapshot::unsupported_architecture("riscv64");
    let err = snapshot
        .x86_cr3()
        .expect_err("unsupported architecture must not expose x86 registers");

    assert_eq!(err.kind(), VmiErrorKind::UnsupportedArchitecture);
    assert!(err.is_unsupported());
    assert!(matches!(
        err,
        RegisterReadError::UnsupportedArchitecture { ref arch } if arch == "riscv64"
    ));
}
