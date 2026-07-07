use std::path::PathBuf;

use aegishv::vmi::{RegisterReadError, VmiErrorKind};
use aegishv::vmi_register_fixtures::{
    load_register_snapshot_fixture, parse_register_snapshot_fixture,
};
use aegishv::vmi_registers::{DescriptorTableRegister, RegisterSnapshot};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("vmi")
        .join("registers")
        .join(name)
}

#[test]
fn loads_valid_x86_64_register_fixture() {
    let snapshot = load_register_snapshot_fixture(fixture_path("x86_64_basic.regs"))
        .expect("load x86 register fixture");

    assert_eq!(snapshot.architecture(), "x86_64");
    assert_eq!(snapshot.x86_cr3().expect("cr3"), 0x1234_5000);
    assert!(snapshot.x86_la57_enabled().expect("la57"));
    assert!(snapshot.x86_long_mode_enabled().expect("lme"));
    assert!(snapshot.x86_long_mode_active().expect("lma"));
    assert!(snapshot.x86_nx_enabled().expect("nxe"));
    assert_eq!(
        snapshot.x86_idtr().expect("idtr"),
        DescriptorTableRegister::new(0xffff_8000_0000_1000, 0x0fff)
    );
    assert_eq!(
        snapshot.x86_gdtr().expect("gdtr"),
        DescriptorTableRegister::new(0xffff_8000_0000_2000, 127)
    );
}

#[test]
fn loads_valid_arm64_register_fixture() {
    let snapshot = load_register_snapshot_fixture(fixture_path("arm64_basic.regs"))
        .expect("load ARM64 register fixture");

    assert_eq!(snapshot.architecture(), "arm64");
    assert_eq!(
        snapshot.arm64_ttbr0_el1().expect("ttbr0"),
        0x0000_0000_1000_0001
    );
    assert_eq!(
        snapshot.arm64_ttbr1_el1().expect("ttbr1"),
        0xffff_0000_2000_0002
    );
    assert_eq!(
        snapshot.arm64_tcr_el1().expect("tcr"),
        0x0000_0000_8080_3520
    );
    assert_eq!(snapshot.arm64_sctlr_el1().expect("sctlr"), 0x1);
    assert_eq!(
        snapshot.arm64_vbar_el1().expect("vbar"),
        0xffff_0000_0000_8000
    );
}

#[test]
fn zero_valued_fixture_registers_are_preserved_when_present() {
    let snapshot = load_register_snapshot_fixture(fixture_path("x86_64_zero.regs"))
        .expect("load zero-valued x86 fixture");

    assert_eq!(snapshot.x86_cr3().expect("zero cr3"), 0);
    assert_eq!(
        snapshot.x86_idtr().expect("zero idtr"),
        DescriptorTableRegister::new(0, 0)
    );
    assert!(!snapshot.x86_la57_enabled().expect("zero cr4"));
    assert!(!snapshot.x86_long_mode_enabled().expect("zero efer"));
    assert!(!snapshot.x86_long_mode_active().expect("zero efer"));
    assert!(!snapshot.x86_nx_enabled().expect("zero efer"));
}

#[test]
fn missing_required_x86_register_returns_typed_missing_register() {
    let err = parse_register_snapshot_fixture(
        r#"
aegishv-registers-v1
arch=x86_64
cr0=0x1
cr2=0x0
cr4=0x0
efer=0x0
idtr.base=0x0
idtr.limit=0x0
gdtr.base=0x0
gdtr.limit=0x0
"#,
    )
    .expect_err("missing CR3 must fail");

    assert_eq!(err.kind(), VmiErrorKind::InvalidInput);
    assert!(matches!(
        err,
        RegisterReadError::MissingRegister {
            arch: "x86_64",
            register: "cr3"
        }
    ));
}

#[test]
fn missing_required_arm64_register_returns_typed_missing_register() {
    let err = parse_register_snapshot_fixture(
        r#"
aegishv-registers-v1
arch=arm64
ttbr0_el1=0x0
ttbr1_el1=0x0
sctlr_el1=0x0
vbar_el1=0x0
"#,
    )
    .expect_err("missing TCR_EL1 must fail");

    assert_eq!(err.kind(), VmiErrorKind::InvalidInput);
    assert!(matches!(
        err,
        RegisterReadError::MissingRegister {
            arch: "arm64",
            register: "tcr_el1"
        }
    ));
}

#[test]
fn unknown_architecture_returns_typed_unsupported_architecture() {
    let err = parse_register_snapshot_fixture(
        r#"
aegishv-registers-v1
arch=riscv64
x0=0
"#,
    )
    .expect_err("unknown architecture must fail");

    assert_eq!(err.kind(), VmiErrorKind::UnsupportedArchitecture);
    assert!(err.is_unsupported());
    assert!(matches!(
        err,
        RegisterReadError::UnsupportedArchitecture { ref arch } if arch == "riscv64"
    ));
}

#[test]
fn malformed_version_and_header_are_rejected() {
    let err = parse_register_snapshot_fixture(
        r#"
aegishv-registers-v2
arch=x86_64
"#,
    )
    .expect_err("bad version must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("line 2"));
    assert!(err.to_string().contains("aegishv-registers-v1"));

    let err = parse_register_snapshot_fixture(
        r#"
aegishv-registers-v1
x86_64
"#,
    )
    .expect_err("malformed architecture header must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("line 3"));
}

#[test]
fn duplicate_register_key_is_rejected() {
    let err = parse_register_snapshot_fixture(
        r#"
aegishv-registers-v1
arch=x86_64
cr0=0x1
cr0=0x2
"#,
    )
    .expect_err("duplicate register key must fail");

    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("line 5"));
    assert!(err.to_string().contains("duplicate register key 'cr0'"));
}

#[test]
fn unknown_register_key_is_rejected() {
    let err = parse_register_snapshot_fixture(
        r#"
aegishv-registers-v1
arch=arm64
ttbr0_el1=0
sp_el0=0
"#,
    )
    .expect_err("unknown register key must fail");

    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("line 5"));
    assert!(err.to_string().contains("sp_el0"));
}

#[test]
fn invalid_integer_value_reports_field_and_line() {
    let err = parse_register_snapshot_fixture(
        r#"
aegishv-registers-v1
arch=arm64
ttbr0_el1=0xnot_hex
"#,
    )
    .expect_err("invalid integer must fail");

    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("line 4"));
    assert!(err.to_string().contains("ttbr0_el1"));
}

#[test]
fn descriptor_limit_out_of_range_is_rejected() {
    let err = parse_register_snapshot_fixture(
        r#"
aegishv-registers-v1
arch=x86_64
cr0=0
cr2=0
cr3=0
cr4=0
efer=0
idtr.base=0
idtr.limit=65536
gdtr.base=0
gdtr.limit=0
"#,
    )
    .expect_err("descriptor limit outside u16 must fail");

    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("line 10"));
    assert!(err.to_string().contains("idtr.limit"));
}

#[test]
fn wrong_architecture_access_after_loading_remains_typed() {
    let snapshot = load_register_snapshot_fixture(fixture_path("arm64_basic.regs"))
        .expect("load ARM64 register fixture");

    let err = snapshot
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
}

#[test]
fn loaded_snapshot_can_still_use_architecture_specific_accessors() {
    let x86 = load_register_snapshot_fixture(fixture_path("x86_64_basic.regs"))
        .expect("load x86 register fixture");
    let arm64 = load_register_snapshot_fixture(fixture_path("arm64_basic.regs"))
        .expect("load ARM64 register fixture");

    assert!(matches!(x86, RegisterSnapshot::X86_64(_)));
    assert!(matches!(arm64, RegisterSnapshot::Arm64(_)));
}
