use aegishv::vmi::{ProfileError, VmiErrorKind};
use aegishv::vmi_profiles::{
    OsKind, OsProfileRegistry, ProfileArchitecture, ProfileIdentity, StaticOsProfile,
};

fn synthetic_profile(
    identity: ProfileIdentity,
    name: &str,
) -> Result<StaticOsProfile, ProfileError> {
    StaticOsProfile::synthetic(identity, name)
}

#[test]
fn registry_starts_empty_and_does_not_ship_fake_profiles() {
    let registry = OsProfileRegistry::new();

    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn registers_and_looks_up_synthetic_linux_x86_64_profile_by_exact_key() {
    let key = ProfileIdentity::linux_x86_64("6.6.1-aegishv-test").expect("valid linux key");
    let mut registry = OsProfileRegistry::new();
    registry
        .register(synthetic_profile(key.clone(), "synthetic-linux-x86_64").expect("profile"))
        .expect("register synthetic linux profile");

    let profile = registry.lookup(&key).expect("exact linux profile lookup");

    assert_eq!(profile.identity(), &key);
    assert_eq!(profile.profile_name(), "synthetic-linux-x86_64");
}

#[test]
fn registers_and_looks_up_synthetic_windows_x86_64_profile_by_exact_key() {
    let key =
        ProfileIdentity::windows_x86_64("10.0.26100-aegishv-test").expect("valid windows key");
    let mut registry = OsProfileRegistry::new();
    registry
        .register(synthetic_profile(key.clone(), "synthetic-windows-x86_64").expect("profile"))
        .expect("register synthetic windows profile");

    let profile = registry.lookup(&key).expect("exact windows profile lookup");

    assert_eq!(profile.identity(), &key);
    assert_eq!(profile.profile_name(), "synthetic-windows-x86_64");
}

#[test]
fn profile_lookup_misses_on_different_os_arch_build_and_variant() {
    let key = ProfileIdentity::new(
        OsKind::Linux,
        ProfileArchitecture::X86_64,
        "6.6.1-aegishv-test",
        Some("generic"),
    )
    .expect("valid linux variant key");
    let mut registry = OsProfileRegistry::new();
    registry
        .register(synthetic_profile(key.clone(), "synthetic-linux-generic").expect("profile"))
        .expect("register profile");

    let different_os = ProfileIdentity::new(
        OsKind::Windows,
        ProfileArchitecture::X86_64,
        "6.6.1-aegishv-test",
        Some("generic"),
    )
    .expect("valid different OS key");
    let different_arch = ProfileIdentity::new(
        OsKind::Linux,
        ProfileArchitecture::Arm64,
        "6.6.1-aegishv-test",
        Some("generic"),
    )
    .expect("valid different arch key");
    let different_build = ProfileIdentity::new(
        OsKind::Linux,
        ProfileArchitecture::X86_64,
        "6.6.2-aegishv-test",
        Some("generic"),
    )
    .expect("valid different build key");
    let different_variant = ProfileIdentity::new(
        OsKind::Linux,
        ProfileArchitecture::X86_64,
        "6.6.1-aegishv-test",
        Some("debug"),
    )
    .expect("valid different variant key");

    for missing in [
        different_os,
        different_arch,
        different_build,
        different_variant,
    ] {
        let err = registry
            .lookup(&missing)
            .expect_err("registry must require an exact profile key");
        assert_eq!(err.kind(), VmiErrorKind::MissingProfile);
        assert!(matches!(err, ProfileError::MissingProfileIdentity { .. }));
    }
}

#[test]
fn duplicate_profile_key_is_rejected() {
    let key = ProfileIdentity::linux_x86_64("6.6.1-aegishv-test").expect("valid linux key");
    let mut registry = OsProfileRegistry::new();
    registry
        .register(synthetic_profile(key.clone(), "first").expect("first profile"))
        .expect("first register succeeds");

    let err = registry
        .register(synthetic_profile(key, "second").expect("second profile"))
        .expect_err("duplicate profile key must fail");

    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(err, ProfileError::MalformedProfile { .. }));
    assert!(err.to_string().contains("duplicate profile key"));
}

#[test]
fn missing_profile_lookup_returns_typed_error() {
    let key =
        ProfileIdentity::windows_x86_64("10.0.26100-aegishv-test").expect("valid windows key");
    let registry = OsProfileRegistry::new();

    let err = registry
        .lookup(&key)
        .expect_err("missing profile must fail");

    assert_eq!(err.kind(), VmiErrorKind::MissingProfile);
    assert!(matches!(
        err,
        ProfileError::MissingProfileIdentity {
            ref os,
            ref arch,
            ref kernel_or_build
        } if os == "windows" && arch == "x86_64" && kernel_or_build == "10.0.26100-aegishv-test"
    ));
}

#[test]
fn unsupported_os_kind_and_architecture_return_typed_errors() {
    let err = ProfileIdentity::new(
        OsKind::other("solaris"),
        ProfileArchitecture::X86_64,
        "11.4-test",
        None::<String>,
    )
    .expect_err("unsupported OS kind must fail");

    assert_eq!(err.kind(), VmiErrorKind::Unsupported);
    assert!(err.is_unsupported());
    assert!(matches!(
        err,
        ProfileError::UnsupportedGuest { ref os, .. } if os == "solaris"
    ));

    let err = ProfileIdentity::new(
        OsKind::Linux,
        ProfileArchitecture::other("mips64"),
        "6.6.1-aegishv-test",
        None::<String>,
    )
    .expect_err("unsupported architecture must fail");

    assert_eq!(err.kind(), VmiErrorKind::UnsupportedArchitecture);
    assert!(err.is_unsupported());
    assert!(matches!(
        err,
        ProfileError::UnsupportedArchitecture { ref arch } if arch == "mips64"
    ));
}

#[test]
fn registry_rejects_unsupported_identity_constructed_by_hand() {
    let key = ProfileIdentity {
        os: OsKind::Linux,
        arch: ProfileArchitecture::other("sparc64"),
        kernel_or_build: "6.6.1-aegishv-test".to_string(),
        variant: None,
    };
    let profile = synthetic_profile(key.clone(), "synthetic-unsupported-arch").expect("profile");
    let mut registry = OsProfileRegistry::new();

    let err = registry
        .register(profile)
        .expect_err("registry must validate profile identity");

    assert_eq!(err.kind(), VmiErrorKind::UnsupportedArchitecture);
    assert_eq!(
        registry.lookup(&key).unwrap_err().kind(),
        VmiErrorKind::UnsupportedArchitecture
    );
}

#[test]
fn malformed_or_incomplete_profile_identity_is_rejected() {
    let err = ProfileIdentity::new(
        OsKind::Linux,
        ProfileArchitecture::X86_64,
        "   ",
        None::<String>,
    )
    .expect_err("empty kernel/build must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("kernel_or_build"));

    let err = ProfileIdentity::new(
        OsKind::Linux,
        ProfileArchitecture::X86_64,
        "6.6.1-aegishv-test",
        Some(" "),
    )
    .expect_err("empty variant must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("variant"));

    let key = ProfileIdentity::linux_x86_64("6.6.1-aegishv-test").expect("valid key");
    let err =
        StaticOsProfile::synthetic(key, " ").expect_err("empty profile name must be rejected");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(err.to_string().contains("profile_name"));
}

#[test]
fn registry_does_not_do_nearest_match_fallback() {
    let registered = ProfileIdentity::linux_x86_64("6.6.1-aegishv-test").expect("valid key");
    let requested =
        ProfileIdentity::linux_x86_64("6.6.1-aegishv-test+local").expect("valid requested key");
    let mut registry = OsProfileRegistry::new();
    registry
        .register(synthetic_profile(registered, "synthetic-linux").expect("profile"))
        .expect("register profile");

    let err = registry
        .lookup(&requested)
        .expect_err("nearby build strings must not match");

    assert_eq!(err.kind(), VmiErrorKind::MissingProfile);
    assert!(err.to_string().contains("6.6.1-aegishv-test+local"));
}
