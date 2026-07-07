use aegishv::vmi::{ProfileError, VmiErrorKind};
use aegishv::vmi_linux_profile::{
    load_linux_profile, parse_linux_profile, resolve_linux_kaslr, LinuxKaslrMode,
    LinuxKaslrResolutionSource, LinuxStructFieldKey,
};
use aegishv::vmi_profiles::{OsKind, OsProfileRegistry, ProfileArchitecture, ProfileIdentity};

fn fixture_path(name: &str) -> String {
    format!(
        "{}/tests/fixtures/profiles/linux/{name}",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn valid_profile_text() -> &'static str {
    r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
variant=test
kaslr=slide-known
kaslr_slide=0x100000
symbol=start_kernel,0xffffffff81000000,0x120
symbol=sys_call_table,0xffffffff81200000
kaslr_anchor=start_kernel,554889e5,0x200000,0x100000
offset=task_struct,pid,0x430,0x4
offset=task_struct,comm,0x738,0x10
syscall=0,read,__x64_sys_read
syscall=1,write,__x64_sys_write
"#
}

#[test]
fn loads_valid_synthetic_linux_x86_64_profile_fixture() {
    let profile =
        load_linux_profile(fixture_path("synthetic_x86_64.profile")).expect("load fixture");

    assert_eq!(
        profile.linux_identity().kernel_release,
        "6.8.0-aegishv-synthetic"
    );
    assert_eq!(
        profile.linux_identity().kernel_build,
        "synthetic-test-build"
    );
    assert_eq!(profile.linux_identity().variant.as_deref(), Some("test"));
    assert_eq!(profile.registry_identity().os, OsKind::Linux);
    assert_eq!(
        profile.registry_identity().arch,
        ProfileArchitecture::X86_64
    );
    assert_eq!(
        profile.registry_identity().kernel_or_build,
        "6.8.0-aegishv-synthetic#synthetic-test-build"
    );
}

#[test]
fn linux_profile_preserves_kaslr_symbols_offsets_and_syscalls() {
    let profile = parse_linux_profile(valid_profile_text()).expect("parse profile");

    assert_eq!(
        profile.kaslr(),
        LinuxKaslrMode::SlideKnown { slide: 0x100000 }
    );
    assert_eq!(profile.kaslr().slide(), Some(0x100000));
    assert_eq!(
        profile.symbols()["start_kernel"].virtual_address,
        0xffff_ffff_8100_0000
    );
    assert_eq!(profile.symbols()["start_kernel"].size, Some(0x120));
    assert_eq!(profile.kaslr_anchors().len(), 1);
    assert_eq!(profile.kaslr_anchors()[0].symbol_name, "start_kernel");
    assert_eq!(
        profile.kaslr_anchors()[0].bytes,
        vec![0x55, 0x48, 0x89, 0xe5]
    );
    assert_eq!(
        profile.symbols()["sys_call_table"].virtual_address,
        0xffff_ffff_8120_0000
    );
    assert_eq!(profile.symbols()["sys_call_table"].size, None);

    let pid = LinuxStructFieldKey {
        struct_name: "task_struct".to_string(),
        field_name: "pid".to_string(),
    };
    let comm = LinuxStructFieldKey {
        struct_name: "task_struct".to_string(),
        field_name: "comm".to_string(),
    };
    assert_eq!(profile.struct_offsets()[&pid].offset, 0x430);
    assert_eq!(profile.struct_offsets()[&pid].size, Some(0x4));
    assert_eq!(profile.struct_offsets()[&comm].offset, 0x738);
    assert_eq!(profile.struct_offsets()[&comm].size, Some(0x10));

    assert_eq!(profile.syscalls_by_number()[&0].name, "read");
    assert_eq!(
        profile.syscalls_by_number()[&0].symbol_name.as_deref(),
        Some("__x64_sys_read")
    );
    assert_eq!(profile.syscalls_by_number()[&1].name, "write");
}

#[test]
fn linux_kaslr_resolver_uses_fixed_and_known_profile_modes_without_memory_reads() {
    let fixed = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
"#,
    )
    .expect("parse fixed profile");
    let known = parse_linux_profile(valid_profile_text()).expect("parse known profile");

    let fixed_resolution =
        resolve_linux_kaslr(&fixed, |_, _| panic!("fixed KASLR must not read memory"))
            .expect("resolve fixed");
    let known_resolution =
        resolve_linux_kaslr(&known, |_, _| panic!("known KASLR must not read memory"))
            .expect("resolve known");

    assert_eq!(fixed_resolution.slide, 0);
    assert_eq!(
        fixed_resolution.source,
        LinuxKaslrResolutionSource::FixedProfile
    );
    assert_eq!(known_resolution.slide, 0x100000);
    assert_eq!(
        known_resolution.source,
        LinuxKaslrResolutionSource::KnownProfileSlide
    );
}

#[test]
fn linux_kaslr_resolver_finds_unique_anchor_slide() {
    let profile = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=unknown-unsupported
symbol=start_kernel,0xffffffff81000000,0x120
kaslr_anchor=start_kernel,554889e5,0x300000,0x100000
"#,
    )
    .expect("parse scan profile");

    let resolution = resolve_linux_kaslr(&profile, |addr, buf| {
        if addr == 0xffff_ffff_8120_0000 {
            buf.copy_from_slice(&[0x55, 0x48, 0x89, 0xe5]);
        } else {
            buf.fill(0xcc);
        }
        Ok(())
    })
    .expect("resolve by anchor");

    assert_eq!(resolution.slide, 0x200000);
    assert_eq!(resolution.source, LinuxKaslrResolutionSource::AnchorScan);
    assert!(resolution.anchors_checked >= 3);
}

#[test]
fn linux_kaslr_resolver_refuses_ambiguous_anchor_matches() {
    let profile = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=unknown-unsupported
symbol=start_kernel,0xffffffff81000000,0x120
kaslr_anchor=start_kernel,554889e5,0x100000,0x100000
"#,
    )
    .expect("parse scan profile");

    let err = resolve_linux_kaslr(&profile, |_, buf| {
        buf.copy_from_slice(&[0x55, 0x48, 0x89, 0xe5]);
        Ok(())
    })
    .expect_err("ambiguous KASLR scan must fail");

    assert_eq!(err.kind(), VmiErrorKind::InconsistentSnapshot);
    assert!(err.to_string().contains("more than one slide"));
}

#[test]
fn linux_profile_registers_with_existing_profile_registry_by_exact_key() {
    let profile = parse_linux_profile(valid_profile_text()).expect("parse profile");
    let key = profile.registry_identity().clone();
    let different_release = ProfileIdentity::new(
        OsKind::Linux,
        ProfileArchitecture::X86_64,
        "6.8.1-aegishv-synthetic#synthetic-test-build",
        Some("test"),
    )
    .expect("different release key");
    let different_build = ProfileIdentity::new(
        OsKind::Linux,
        ProfileArchitecture::X86_64,
        "6.8.0-aegishv-synthetic#different-build",
        Some("test"),
    )
    .expect("different build key");
    let different_variant = ProfileIdentity::new(
        OsKind::Linux,
        ProfileArchitecture::X86_64,
        "6.8.0-aegishv-synthetic#synthetic-test-build",
        Some("debug"),
    )
    .expect("different variant key");
    let mut registry = OsProfileRegistry::new();

    registry.register(profile).expect("register linux profile");

    assert_eq!(
        registry.lookup(&key).expect("exact lookup").identity(),
        &key
    );
    for missing in [different_release, different_build, different_variant] {
        let err = registry
            .lookup(&missing)
            .expect_err("registry must not use nearest-match fallback");
        assert_eq!(err.kind(), VmiErrorKind::MissingProfile);
    }
}

#[test]
fn linux_profile_rejects_missing_required_identity_fields() {
    let err = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kaslr=fixed
"#,
    )
    .expect_err("missing kernel_build must fail");

    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("kernel_build"));
}

#[test]
fn linux_profile_rejects_unsupported_os_and_architecture() {
    let err = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=windows
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
"#,
    )
    .expect_err("non-Linux OS must fail");
    assert_eq!(err.kind(), VmiErrorKind::Unsupported);
    assert!(matches!(err, ProfileError::UnsupportedGuest { .. }));

    let err = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=arm64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
"#,
    )
    .expect_err("non-x86_64 arch must fail");
    assert_eq!(err.kind(), VmiErrorKind::UnsupportedArchitecture);
    assert!(matches!(
        err,
        ProfileError::UnsupportedArchitecture { ref arch } if arch == "arm64"
    ));
}

#[test]
fn linux_profile_rejects_malformed_header_unknown_keys_and_bad_integers() {
    let err =
        parse_linux_profile("wrong-version\nos=linux\n").expect_err("malformed version must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err
        .to_string()
        .contains("expected aegishv-linux-profile-v1"));

    let err = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
unknown=value
"#,
    )
    .expect_err("unknown key must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("unknown Linux profile key"));

    let err = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
symbol=start_kernel,not-a-number
"#,
    )
    .expect_err("bad address must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("line 8"));
    assert!(err.to_string().contains("symbol.virtual_address"));
}

#[test]
fn linux_profile_rejects_duplicate_symbols_offsets_and_syscalls() {
    let err = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
symbol=start_kernel,0xffffffff81000000
symbol=start_kernel,0xffffffff81000100
"#,
    )
    .expect_err("duplicate symbol must fail");
    assert!(err.to_string().contains("duplicate symbol"));

    let err = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
offset=task_struct,pid,0x430
offset=task_struct,pid,0x438
"#,
    )
    .expect_err("duplicate struct field must fail");
    assert!(err.to_string().contains("duplicate struct offset"));

    let err = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
syscall=0,read,__x64_sys_read
syscall=0,write,__x64_sys_write
"#,
    )
    .expect_err("duplicate syscall number must fail");
    assert!(err.to_string().contains("duplicate syscall number"));

    let err = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=fixed
syscall=0,read,__x64_sys_read
syscall=1,read,__x64_sys_read
"#,
    )
    .expect_err("duplicate syscall name must fail");
    assert!(err.to_string().contains("duplicate syscall name"));
}

#[test]
fn linux_profile_kaslr_unknown_is_explicit_and_does_not_fake_a_slide() {
    let profile = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=unknown-unsupported
"#,
    )
    .expect("unknown KASLR mode is representable");

    assert_eq!(profile.kaslr(), LinuxKaslrMode::UnknownUnsupported);
    assert_eq!(profile.kaslr().slide(), None);

    let err = parse_linux_profile(
        r#"
aegishv-linux-profile-v1
os=linux
arch=x86_64
kernel_release=6.8.0-aegishv-synthetic
kernel_build=synthetic-test-build
kaslr=unknown-unsupported
kaslr_slide=0x100000
"#,
    )
    .expect_err("unknown KASLR mode must not accept a fake slide");

    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("must not set kaslr_slide"));
}
