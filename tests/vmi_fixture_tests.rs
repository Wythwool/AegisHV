use std::path::{Path, PathBuf};

use aegishv::vmi::{
    GuestMemoryReader, GuestPhysical, GuestVirtual, RegisterReadError, VmId, VmiErrorKind,
};
use aegishv::vmi_cache::{Arm64CacheGranule, TranslationMode};
use aegishv::vmi_fixture::{
    load_vmi_fixture, load_vmi_fixture_set, load_vmi_fixture_with_profiles,
    parse_vmi_fixture_manifest_text, VmiFixtureError, VMI_FIXTURE_VERSION,
};
use aegishv::vmi_profiles::{
    OsProfileRegistry, ProfileArchitecture, ProfileIdentity, StaticOsProfile,
};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vmi")
}

fn x86_fixture() -> PathBuf {
    fixture_dir().join("x86_64_basic.vmi")
}

fn arm64_fixture() -> PathBuf {
    fixture_dir().join("arm64_basic.vmi")
}

fn duplicate_id_fixture() -> PathBuf {
    fixture_dir().join("x86_64_duplicate_id.vmi")
}

fn fixture_text(body: &str) -> String {
    format!("{VMI_FIXTURE_VERSION}\n{body}")
}

fn x86_registry() -> OsProfileRegistry {
    let mut registry = OsProfileRegistry::new();
    let identity = ProfileIdentity::new(
        aegishv::vmi_profiles::OsKind::Linux,
        ProfileArchitecture::X86_64,
        "6.8.0-aegishv-test",
        Some("tiny"),
    )
    .expect("synthetic profile identity");
    registry
        .register(
            StaticOsProfile::synthetic(identity, "synthetic linux x86_64 profile")
                .expect("synthetic profile"),
        )
        .expect("register synthetic profile");
    registry
}

#[test]
fn loads_valid_x86_64_vmi_fixture_with_memory_registers_profile_and_expected_translations() {
    let fixture = load_vmi_fixture_with_profiles(x86_fixture(), &x86_registry())
        .expect("load x86_64 VMI fixture");

    assert_eq!(fixture.id, "x86_64-basic");
    assert_eq!(fixture.architecture, ProfileArchitecture::X86_64);
    assert_eq!(
        fixture.profile_identity.as_ref().unwrap().kernel_or_build,
        "6.8.0-aegishv-test"
    );
    assert_eq!(fixture.registers.x86_cr3().unwrap(), 0x1234_5000);

    let mut bytes = [0u8; 4];
    let read = fixture
        .memory
        .read_physical(VmId(1), GuestPhysical(0x1000), &mut bytes)
        .expect("read fixture memory");
    assert_eq!(read, 4);
    assert_eq!(&bytes, b"abcd");

    assert_eq!(fixture.expected_translations.len(), 2);
    let zero = &fixture.expected_translations[0];
    assert_eq!(zero.name, "zero");
    assert_eq!(zero.gva, GuestVirtual(0));
    assert_eq!(zero.gpa, GuestPhysical(0));
    assert_eq!(zero.mode, TranslationMode::X86_64FourLevel);
    assert_eq!(zero.result().page_size, 0x1000);
}

#[test]
fn loads_valid_arm64_vmi_fixture_with_explicit_no_profile() {
    let fixture = load_vmi_fixture(arm64_fixture()).expect("load arm64 VMI fixture");

    assert_eq!(fixture.id, "arm64-basic");
    assert_eq!(fixture.architecture, ProfileArchitecture::Arm64);
    assert!(fixture.profile_identity.is_none());
    assert_eq!(fixture.registers.arm64_ttbr0_el1().unwrap(), 0x1000);
    assert_eq!(
        fixture.expected_translations[1].mode,
        TranslationMode::Arm64Stage1 {
            granule: Arm64CacheGranule::Size16K
        }
    );
}

#[test]
fn missing_memory_and_register_references_return_typed_errors() {
    let missing_memory = fixture_text(
        "id=missing-memory\nname=missing memory\narch=x86_64\nmemory=memory/missing.map\nregisters=registers/x86_64_basic.regs\nprofile=none\n",
    );
    let err = parse_vmi_fixture_manifest_text(&missing_memory, fixture_dir())
        .expect_err("missing memory manifest must fail");
    assert_eq!(err.kind(), VmiErrorKind::TemporarilyUnavailable);
    assert!(err.to_string().contains("memory manifest"));

    let missing_registers = fixture_text(
        "id=missing-registers\nname=missing registers\narch=x86_64\nmemory=memory/snapshot.map\nregisters=registers/missing.regs\nprofile=none\n",
    );
    let err = parse_vmi_fixture_manifest_text(&missing_registers, fixture_dir())
        .expect_err("missing register fixture must fail");
    assert_eq!(err.kind(), VmiErrorKind::TemporarilyUnavailable);
    assert!(err.to_string().contains("register fixture"));
}

#[test]
fn path_traversal_and_absolute_references_are_rejected() {
    let traversal = fixture_text(
        "id=path-traversal\nname=bad path\narch=x86_64\nmemory=../memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\n",
    );
    let err = parse_vmi_fixture_manifest_text(&traversal, fixture_dir())
        .expect_err("parent traversal must fail");
    assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);
    assert!(err.to_string().contains("manifest directory"));

    let absolute_memory = fixture_dir().join("memory").join("snapshot.map");
    let absolute = fixture_text(&format!(
        "id=absolute-path\nname=bad path\narch=x86_64\nmemory={}\nregisters=registers/x86_64_basic.regs\nprofile=none\n",
        absolute_memory.display()
    ));
    let err = parse_vmi_fixture_manifest_text(&absolute, fixture_dir())
        .expect_err("absolute fixture path must fail");
    assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);
    assert!(err.to_string().contains("relative path"));
}

#[test]
fn duplicate_fixture_id_is_rejected_when_loading_sets() {
    let err = load_vmi_fixture_set([x86_fixture(), duplicate_id_fixture()])
        .expect_err("duplicate fixture ids must fail");

    assert!(matches!(
        err,
        VmiFixtureError::DuplicateFixtureId { ref id } if id == "x86_64-basic"
    ));
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
}

#[test]
fn duplicate_expected_translation_name_is_rejected() {
    let manifest = fixture_text(
        "id=dup-translation\nname=duplicate translation\narch=x86_64\nmemory=memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\ntranslation=name=same gva=0x0 gpa=0x0 page_size=0x1000 mode=x86_64-4level readable=true writable=false executable=true user=false\ntranslation=name=same gva=0x1000 gpa=0x1000 page_size=0x1000 mode=x86_64-4level readable=true writable=false executable=true user=false\n",
    );
    let err = parse_vmi_fixture_manifest_text(&manifest, fixture_dir())
        .expect_err("duplicate expected translation names must fail");

    assert!(matches!(
        err,
        VmiFixtureError::DuplicateExpectedTranslation { ref name } if name == "same"
    ));
}

#[test]
fn unsupported_architecture_and_profile_identity_return_typed_errors() {
    let unknown_arch = fixture_text(
        "id=bad-arch\nname=bad arch\narch=mips64\nmemory=memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\n",
    );
    let err = parse_vmi_fixture_manifest_text(&unknown_arch, fixture_dir())
        .expect_err("unknown architecture must fail");
    assert_eq!(err.kind(), VmiErrorKind::UnsupportedArchitecture);

    let unknown_os = fixture_text(
        "id=bad-os\nname=bad os\narch=x86_64\nmemory=memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nos=plan9\nkernel_or_build=synthetic\n",
    );
    let err = parse_vmi_fixture_manifest_text(&unknown_os, fixture_dir())
        .expect_err("unknown profile OS must fail");
    assert_eq!(err.kind(), VmiErrorKind::Unsupported);

    let err = load_vmi_fixture_with_profiles(x86_fixture(), &OsProfileRegistry::new())
        .expect_err("missing registered profile must fail");
    assert_eq!(err.kind(), VmiErrorKind::MissingProfile);
}

#[test]
fn malformed_headers_unknown_keys_and_incomplete_profiles_are_rejected() {
    let malformed_header = "aegishv-vmi-fixture-v2\nid=bad\n";
    let err = parse_vmi_fixture_manifest_text(malformed_header, fixture_dir())
        .expect_err("bad header must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains(VMI_FIXTURE_VERSION));

    let unknown_key = fixture_text(
        "id=unknown-key\nname=unknown key\narch=x86_64\nmemory=memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\nsurprise=value\n",
    );
    let err = parse_vmi_fixture_manifest_text(&unknown_key, fixture_dir())
        .expect_err("unknown fixture key must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("surprise"));

    let incomplete_profile = fixture_text(
        "id=incomplete-profile\nname=incomplete profile\narch=x86_64\nmemory=memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nos=linux\n",
    );
    let err = parse_vmi_fixture_manifest_text(&incomplete_profile, fixture_dir())
        .expect_err("profile without kernel_or_build must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("kernel_or_build"));
}

#[test]
fn malformed_expected_translation_fields_are_rejected_with_line_detail() {
    let invalid_gva = fixture_text(
        "id=bad-gva\nname=bad gva\narch=x86_64\nmemory=memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\ntranslation=name=bad gva=nope gpa=0x0 page_size=0x1000 mode=x86_64-4level readable=true writable=false executable=true user=false\n",
    );
    let err = parse_vmi_fixture_manifest_text(&invalid_gva, fixture_dir())
        .expect_err("bad address must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("line"));
    assert!(err.to_string().contains("gva"));

    let bad_page_size = fixture_text(
        "id=bad-page-size\nname=bad page size\narch=x86_64\nmemory=memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\ntranslation=name=bad gva=0x0 gpa=0x0 page_size=3 mode=x86_64-4level readable=true writable=false executable=true user=false\n",
    );
    let err = parse_vmi_fixture_manifest_text(&bad_page_size, fixture_dir())
        .expect_err("bad page size must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("page_size"));

    let unsupported_mode = fixture_text(
        "id=bad-mode\nname=bad mode\narch=x86_64\nmemory=memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\ntranslation=name=bad gva=0x0 gpa=0x0 page_size=0x1000 mode=riscv-stage1 readable=true writable=false executable=true user=false\n",
    );
    let err = parse_vmi_fixture_manifest_text(&unsupported_mode, fixture_dir())
        .expect_err("unsupported translation mode must fail");
    assert_eq!(err.kind(), VmiErrorKind::UnsupportedBackend);
}

#[test]
fn expected_translations_preserve_zero_values_when_explicit() {
    let manifest = fixture_text(
        "id=zero-translation\nname=zero translation\narch=x86_64\nmemory=memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\ntranslation=name=zero gva=0x0 gpa=0x0 page_size=0x1000 mode=x86_64-4level readable=true writable=false executable=true user=false\n",
    );
    let fixture = parse_vmi_fixture_manifest_text(&manifest, fixture_dir())
        .expect("load zero-valued expected translation");
    let zero = fixture
        .expected_translations
        .iter()
        .find(|case| case.name == "zero")
        .expect("zero expected translation");

    assert_eq!(zero.gva, GuestVirtual(0));
    assert_eq!(zero.gpa, GuestPhysical(0));
    assert!(zero.readable);
    assert!(!zero.writable);
}

#[test]
fn fixture_architecture_must_match_loaded_register_snapshot_architecture() {
    let x86_with_arm64_regs = fixture_text(
        "id=x86-with-arm64-regs\nname=bad registers\narch=x86_64\nmemory=memory/snapshot.map\nregisters=registers/arm64_basic.regs\nprofile=none\n",
    );
    let err = parse_vmi_fixture_manifest_text(&x86_with_arm64_regs, fixture_dir())
        .expect_err("x86 fixture must reject arm64 register snapshot");
    assert!(matches!(
        err,
        VmiFixtureError::Registers(RegisterReadError::WrongArchitecture {
            expected: "x86_64",
            actual: "arm64",
        })
    ));

    let arm64_with_x86_regs = fixture_text(
        "id=arm64-with-x86-regs\nname=bad registers\narch=arm64\nmemory=memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\n",
    );
    let err = parse_vmi_fixture_manifest_text(&arm64_with_x86_regs, fixture_dir())
        .expect_err("arm64 fixture must reject x86 register snapshot");
    assert!(matches!(
        err,
        VmiFixtureError::Registers(RegisterReadError::WrongArchitecture {
            expected: "arm64",
            actual: "x86_64",
        })
    ));
}

#[test]
fn expected_translation_modes_must_match_fixture_architecture() {
    let x86_with_arm64_mode = fixture_text(
        "id=x86-with-arm64-mode\nname=bad mode\narch=x86_64\nmemory=memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\ntranslation=name=bad gva=0x0 gpa=0x0 page_size=0x1000 mode=arm64-stage1-4k readable=true writable=false executable=true user=false\n",
    );
    let err = parse_vmi_fixture_manifest_text(&x86_with_arm64_mode, fixture_dir())
        .expect_err("x86 fixture must reject arm64 translation mode");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("translation mode"));
    assert!(err.to_string().contains("x86_64"));

    let arm64_with_x86_mode = fixture_text(
        "id=arm64-with-x86-mode\nname=bad mode\narch=arm64\nmemory=memory/snapshot.map\nregisters=registers/arm64_basic.regs\nprofile=none\ntranslation=name=bad gva=0x0 gpa=0x0 page_size=0x1000 mode=x86_64-4level readable=true writable=false executable=true user=false\n",
    );
    let err = parse_vmi_fixture_manifest_text(&arm64_with_x86_mode, fixture_dir())
        .expect_err("arm64 fixture must reject x86 translation mode");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("translation mode"));
    assert!(err.to_string().contains("arm64"));
}

#[test]
fn expected_translation_page_sizes_are_checked_against_translation_mode() {
    let x86_bad_power_of_two = fixture_text(
        "id=x86-bad-page-size\nname=bad page size\narch=x86_64\nmemory=memory/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\ntranslation=name=bad gva=0x0 gpa=0x0 page_size=0x8000 mode=x86_64-4level readable=true writable=false executable=true user=false\n",
    );
    let err = parse_vmi_fixture_manifest_text(&x86_bad_power_of_two, fixture_dir())
        .expect_err("x86 mode must reject unsupported power-of-two page size");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("page_size"));
    assert!(err.to_string().contains("x86_64-4level"));

    let arm64_4k_bad_power_of_two = fixture_text(
        "id=arm64-4k-bad-page-size\nname=bad page size\narch=arm64\nmemory=memory/snapshot.map\nregisters=registers/arm64_basic.regs\nprofile=none\ntranslation=name=bad gva=0x0 gpa=0x0 page_size=0x4000 mode=arm64-stage1-4k readable=true writable=false executable=true user=false\n",
    );
    let err = parse_vmi_fixture_manifest_text(&arm64_4k_bad_power_of_two, fixture_dir())
        .expect_err("arm64 4k mode must reject 16k page size");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("arm64-stage1-4k"));

    let valid_large_granule_sizes = fixture_text(
        "id=arm64-valid-large-granules\nname=valid page sizes\narch=arm64\nmemory=memory/snapshot.map\nregisters=registers/arm64_basic.regs\nprofile=none\ntranslation=name=page16k gva=0x0 gpa=0x0 page_size=0x4000 mode=arm64-stage1-16k readable=true writable=false executable=true user=false\ntranslation=name=block16k gva=0x0 gpa=0x0 page_size=0x2000000 mode=arm64-stage1-16k readable=true writable=false executable=true user=false\ntranslation=name=block16k-l1 gva=0x0 gpa=0x0 page_size=0x1000000000 mode=arm64-stage1-16k readable=true writable=false executable=true user=false\ntranslation=name=page64k gva=0x0 gpa=0x0 page_size=0x10000 mode=arm64-stage1-64k readable=true writable=false executable=true user=false\ntranslation=name=block64k gva=0x0 gpa=0x0 page_size=0x20000000 mode=arm64-stage1-64k readable=true writable=false executable=true user=false\n",
    );
    let fixture = parse_vmi_fixture_manifest_text(&valid_large_granule_sizes, fixture_dir())
        .expect("valid arm64 16k and 64k page sizes must load");
    assert_eq!(fixture.expected_translations.len(), 5);
}

#[test]
fn windows_drive_unc_and_backslash_paths_are_rejected_portably() {
    let windows_drive = fixture_text(
        "id=windows-drive-path\nname=bad path\narch=x86_64\nmemory=C:/fixtures/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\n",
    );
    let err = parse_vmi_fixture_manifest_text(&windows_drive, fixture_dir())
        .expect_err("Windows drive-style path must fail");
    assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);
    assert!(err.to_string().contains("portable relative path"));

    let unc_like = fixture_text(
        "id=unc-path\nname=bad path\narch=x86_64\nmemory=//server/share/snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\n",
    );
    let err = parse_vmi_fixture_manifest_text(&unc_like, fixture_dir())
        .expect_err("UNC-like path must fail");
    assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);

    let backslash = fixture_text(
        "id=backslash-path\nname=bad path\narch=x86_64\nmemory=memory\\snapshot.map\nregisters=registers/x86_64_basic.regs\nprofile=none\n",
    );
    let err = parse_vmi_fixture_manifest_text(&backslash, fixture_dir())
        .expect_err("backslash-separated path must fail");
    assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);
    assert!(err.to_string().contains("portable relative path"));
}
