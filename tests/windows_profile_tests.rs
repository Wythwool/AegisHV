use aegishv::vmi::{ProfileError, VmiErrorKind};
use aegishv::vmi_profiles::{OsKind, OsProfileRegistry, ProfileArchitecture, ProfileIdentity};
use aegishv::windows_profile::{
    parse_windows_profile, windows_registry_build, WindowsProtectionKind, WindowsProtectionState,
    WindowsStructFieldKey,
};
use aegishv::windows_symbols::parse_windows_symbol_cache;

fn profile_text(extra: &str) -> String {
    format!(
        r#"
aegishv-windows-profile-v1
os=windows
arch=x86_64
build=10.0.22631.3155
variant=synthetic
pdb_file=ntkrnlmp.pdb
pdb_guid=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pdb_age=2
symbol=ntoskrnl.exe,0x0,0x400000
symbol=KiSystemCall64,0x1000,0x80
symbol=KeServiceDescriptorTable,0x2000,0x20
offset=EPROCESS,ActiveProcessLinks,0x448,0x10
offset=EPROCESS,UniqueProcessId,0x440,0x8
offset=EPROCESS,ImageFileName,0x5a8,0xf
offset=ETHREAD,Cid,0x478,0x10
syscall=0,NtAcceptConnectPort,NtAcceptConnectPort
limit=vbs,not_present,synthetic fixture does not enable VBS
limit=hvci,degraded,HVCI state is represented as profile metadata only
{extra}
"#
    )
}

#[test]
fn windows_profile_preserves_identity_symbols_offsets_syscalls_and_limits() {
    let profile = parse_windows_profile(&profile_text("")).expect("parse profile");

    assert_eq!(profile.windows_identity().build, "10.0.22631.3155");
    assert_eq!(profile.windows_identity().pdb_age, 2);
    assert_eq!(
        profile.registry_identity().kernel_or_build,
        windows_registry_build("10.0.22631.3155", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 2)
    );
    assert_eq!(
        profile.registry_identity().variant.as_deref(),
        Some("synthetic")
    );
    assert_eq!(profile.symbols()["KiSystemCall64"].rva, 0x1000);
    let image_name = WindowsStructFieldKey {
        struct_name: "EPROCESS".to_string(),
        field_name: "ImageFileName".to_string(),
    };
    assert_eq!(profile.struct_offsets()[&image_name].offset, 0x5a8);
    assert_eq!(
        profile.syscalls_by_number()[&0].symbol_name.as_deref(),
        Some("NtAcceptConnectPort")
    );
    assert_eq!(profile.limitations()[0].kind, WindowsProtectionKind::Vbs);
    assert_eq!(
        profile.limitations()[1].state,
        WindowsProtectionState::Degraded
    );
}

#[test]
fn windows_profile_registers_by_exact_build_and_pdb_identity() {
    let profile = parse_windows_profile(&profile_text("")).expect("parse profile");
    let exact = profile.registry_identity().clone();
    let missing = ProfileIdentity::new(
        OsKind::Windows,
        ProfileArchitecture::X86_64,
        windows_registry_build("10.0.22631.3155", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", 2),
        Some("synthetic"),
    )
    .expect("identity");
    let mut registry = OsProfileRegistry::new();

    registry.register(profile).expect("register profile");

    assert_eq!(
        registry.lookup(&exact).expect("exact lookup").identity(),
        &exact
    );
    let err = registry
        .lookup(&missing)
        .expect_err("nearest match must fail");
    assert_eq!(err.kind(), VmiErrorKind::MissingProfile);
}

#[test]
fn windows_profile_rejects_unsupported_guest_and_architecture() {
    let err = parse_windows_profile(
        r#"
aegishv-windows-profile-v1
os=linux
arch=x86_64
build=10.0.22631.3155
pdb_file=ntkrnlmp.pdb
pdb_guid=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pdb_age=2
symbol=ntoskrnl.exe,0x0
"#,
    )
    .expect_err("wrong OS must fail");
    assert_eq!(err.kind(), VmiErrorKind::Unsupported);
    assert!(matches!(err, ProfileError::UnsupportedGuest { .. }));

    let err = parse_windows_profile(
        r#"
aegishv-windows-profile-v1
os=windows
arch=arm64
build=10.0.22631.3155
pdb_file=ntkrnlmp.pdb
pdb_guid=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pdb_age=2
symbol=ntoskrnl.exe,0x0
"#,
    )
    .expect_err("unsupported architecture must fail");
    assert_eq!(err.kind(), VmiErrorKind::UnsupportedArchitecture);
}

#[test]
fn windows_profile_rejects_malformed_and_duplicate_records() {
    let err = parse_windows_profile("bad-version\n").expect_err("bad version must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err
        .to_string()
        .contains("expected aegishv-windows-profile-v1"));

    let err = parse_windows_profile(&profile_text("symbol=ntoskrnl.exe,0x10\n"))
        .expect_err("duplicate symbol must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("duplicate symbol"));

    let err = parse_windows_profile(&profile_text("limit=secure,kinda,nope\n"))
        .expect_err("unknown protection limit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err
        .to_string()
        .contains("unsupported Windows protection kind"));
}

#[test]
fn windows_symbol_cache_parses_pre_extracted_symbols_without_network_claims() {
    let cache = parse_windows_symbol_cache(
        r#"
aegishv-windows-symbol-cache-v1
profile_version=aegishv-windows-profile-v1
pdb_file=ntkrnlmp.pdb
pdb_guid=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pdb_age=2
source=pre-extracted synthetic fixture
symbol=KiSystemCall64,0x1000,0x80
symbol=PsLoadedModuleList,0x3000,0x10
"#,
    )
    .expect("parse symbol cache");

    assert_eq!(cache.pdb_file, "ntkrnlmp.pdb");
    assert_eq!(cache.symbols["PsLoadedModuleList"].rva, 0x3000);
    assert_eq!(cache.symbols["KiSystemCall64"].size, Some(0x80));
}

#[test]
fn windows_symbol_cache_rejects_empty_and_duplicate_symbols() {
    let err = parse_windows_symbol_cache(
        r#"
aegishv-windows-symbol-cache-v1
pdb_file=ntkrnlmp.pdb
pdb_guid=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pdb_age=2
source=synthetic
"#,
    )
    .expect_err("empty symbol cache must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);

    let err = parse_windows_symbol_cache(
        r#"
aegishv-windows-symbol-cache-v1
pdb_file=ntkrnlmp.pdb
pdb_guid=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pdb_age=2
source=synthetic
symbol=KiSystemCall64,0x1000
symbol=KiSystemCall64,0x2000
"#,
    )
    .expect_err("duplicate symbol must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("duplicate symbol"));
}
