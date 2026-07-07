use std::fs;
use std::path::{Path, PathBuf};

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn apparmor_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for entry in fs::read_dir(root.join("packaging/apparmor")).expect("read packaging/apparmor") {
        let entry = entry.expect("read AppArmor file");
        if entry.file_type().expect("file type").is_file() {
            files.push(entry.path());
        }
    }
    files
}

#[test]
fn apparmor_profile_covers_current_sensor_path_categories() {
    let profile = read_repo_file("packaging/apparmor/usr.bin.aegishv");

    for required in [
        "profile aegishv /usr/bin/aegishv",
        "/usr/bin/aegishv mr,",
        "/usr/local/bin/aegishv mr,",
        "/etc/aegishv/** r,",
        "/usr/share/aegishv/** r,",
        "/usr/share/doc/aegishv/** r,",
        "/sys/kernel/tracing/** r,",
        "/sys/kernel/debug/tracing/** r,",
        "/proc/[0-9]*/stat r,",
        "/proc/[0-9]*/cmdline r,",
        "/proc/[0-9]*/cgroup r,",
        "/proc/[0-9]*/task/[0-9]*/stat r,",
        "/var/log/aegishv/** rwk,",
        "/var/lib/aegishv/** rwk,",
        "/run/aegishv/** rwk,",
        "/run/libvirt/qemu/** rw,",
        "/var/run/libvirt/qemu/** rw,",
        "/run/systemd/journal/socket w,",
        "/run/systemd/journal/dev-log w,",
        "/dev/log w,",
        "network inet stream,",
        "network inet6 stream,",
        "network inet dgram,",
        "network inet6 dgram,",
        "unix (create, connect, send, receive, getattr, getopt, setopt) type=stream,",
        "unix (create, connect, send, receive, getattr, getopt, setopt) type=dgram,",
    ] {
        assert!(
            profile.contains(required),
            "AppArmor profile is missing expected rule: {required}"
        );
    }
}

#[test]
fn apparmor_profile_denies_sensitive_paths_and_avoids_broad_write_rules() {
    let profile = read_repo_file("packaging/apparmor/usr.bin.aegishv");

    for required in [
        "deny /etc/shadow r,",
        "deny /root/** rwklx,",
        "deny /home/** w,",
        "deny /tmp/** x,",
    ] {
        assert!(
            profile.contains(required),
            "AppArmor profile is missing expected denial: {required}"
        );
    }

    for forbidden_line in ["/** rw,", "/** rwk,"] {
        assert!(
            !profile.lines().any(|line| line.trim() == forbidden_line),
            "AppArmor profile must not contain broad root write rule: {forbidden_line}"
        );
    }

    for forbidden in [
        "/etc/** rw,",
        "/home/** rw,",
        "/tmp/** rw,",
        "capability sys_admin",
        "capability dac_override",
        "capability dac_read_search",
        "ptrace",
        "mount,",
        "umount",
    ] {
        assert!(
            !profile.contains(forbidden),
            "AppArmor profile must not contain broad or unsafe rule: {forbidden}"
        );
    }
}

#[test]
fn apparmor_packaging_ships_profile_without_enforcing_service_defaults() {
    let debian_install = read_repo_file("packaging/debian/install");
    let debian_script = read_repo_file("scripts/package-debian.sh");
    let debian_service = read_repo_file("packaging/debian/aegishv.service");
    let debian_postinst = read_repo_file("packaging/debian/postinst");
    let rpm_spec = read_repo_file("packaging/rpm/aegishv.spec");
    let rpm_service = read_repo_file("packaging/rpm/aegishv.service");
    let ci = read_repo_file(".github/workflows/ci.yml");

    for required in [
        "packaging/apparmor/usr.bin.aegishv usr/share/aegishv/apparmor/usr.bin.aegishv 0644",
        "install -D -m 0644 packaging/apparmor/usr.bin.aegishv \"$PKGROOT/usr/share/aegishv/apparmor/usr.bin.aegishv\"",
        "install -D -m 0644 packaging/apparmor/usr.bin.aegishv %{buildroot}%{_datadir}/aegishv/apparmor/usr.bin.aegishv",
        "%{_datadir}/aegishv/apparmor/usr.bin.aegishv",
    ] {
        assert!(
            debian_install.contains(required)
                || debian_script.contains(required)
                || rpm_spec.contains(required),
            "packaging does not ship the optional AppArmor profile: {required}"
        );
    }

    for service in [debian_service, rpm_service] {
        assert!(
            !service.contains("AppArmorProfile="),
            "packaged service defaults must not enforce AppArmor without operator review"
        );
    }

    for text in [debian_script, debian_postinst, rpm_spec] {
        for forbidden in [
            "apparmor_parser",
            "aa-enforce",
            "aa-complain",
            "/etc/apparmor.d/usr.bin.aegishv",
        ] {
            assert!(
                !text.contains(forbidden),
                "package scripts must not load or enforce AppArmor profiles by default: {forbidden}"
            );
        }
    }

    assert!(
        !ci.contains("apparmor_parser") && !ci.contains("aa-enforce") && !ci.contains("aa-status"),
        "normal CI must not require AppArmor tooling or host policy enforcement"
    );
}

#[test]
fn apparmor_docs_are_operator_reviewed_and_honest() {
    let deployment = read_repo_file("docs/DEPLOYMENT.md");
    let testing = read_repo_file("docs/TESTING.md");
    let security = read_repo_file("docs/SECURITY.md");

    for required in [
        "packaging/apparmor/usr.bin.aegishv",
        "Debian and RPM packages ship it",
        "do not install it into `/etc/apparmor.d`",
        "the packaged systemd units do not enable it",
        "Operators must test the profile",
        "The profile permits",
        "Everything else is denied by AppArmor",
        "explicitly denies `/root` access",
        "does not grant broad writes outside the AegisHV log, state, and runtime directories",
        "sudo apparmor_parser -r /etc/apparmor.d/usr.bin.aegishv",
        "sudo aa-complain aegishv",
        "sudo aa-enforce aegishv",
        "AppArmorProfile=aegishv",
        "That override is not installed by the package",
        "can break deployments",
        "must be adjusted per deployment",
        "does not prove complete sandboxing",
    ] {
        assert!(
            deployment.contains(required),
            "docs/DEPLOYMENT.md is missing AppArmor operator guidance: {required}"
        );
    }

    for required in [
        "AppArmor Profile Checks",
        "Normal PR tests do not require root",
        "expected path categories",
        "package installation of the profile as an optional file",
        "without enabling it in service defaults",
        "no fake sandbox",
        "The tests do not prove AppArmor enforcement",
    ] {
        assert!(
            testing.contains(required),
            "docs/TESTING.md is missing AppArmor test guidance: {required}"
        );
    }

    for required in [
        "Optional AppArmor profile at `packaging/apparmor/usr.bin.aegishv`",
        "not enabled by default",
        "Enforced AppArmor policy and deployment-specific seccomp tuning",
    ] {
        assert!(
            security.contains(required),
            "docs/SECURITY.md is missing AppArmor security posture text: {required}"
        );
    }
}

#[test]
fn apparmor_files_do_not_embed_forbidden_artifacts_or_fake_claims() {
    for path in apparmor_files() {
        let text = fs::read_to_string(&path).expect("read AppArmor file");
        let rel = path
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(&path)
            .display()
            .to_string()
            .replace('\\', "/");

        for forbidden in [
            "C:\\Users",
            "/Users/",
            ".git/",
            "target/",
            "dist/",
            ".zip",
            "__pycache__",
            "node_modules",
            "PRIVATE KEY",
            "BEGIN RSA",
            "complete sandboxing is provided",
            "exploit prevention is provided",
            "type-1 safety is provided",
            "full VMI support",
            "EPT/NPT enforcement is provided",
            "hardware PMU support is provided",
        ] {
            assert!(
                !text.contains(forbidden),
                "{rel} contains forbidden AppArmor text: {forbidden}"
            );
        }
    }
}
