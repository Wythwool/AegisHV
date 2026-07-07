use std::fs;
use std::path::{Path, PathBuf};

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn seccomp_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for entry in fs::read_dir(root.join("packaging/seccomp")).expect("read packaging/seccomp") {
        let entry = entry.expect("read seccomp file");
        if entry.file_type().expect("file type").is_file() {
            files.push(entry.path());
        }
    }
    files
}

fn assert_profile_allows(profile: &str, syscall: &str) {
    let needle = format!("\"{syscall}\"");
    assert!(
        profile.contains(&needle),
        "seccomp profile is missing expected syscall: {syscall}"
    );
}

#[test]
fn seccomp_profile_is_default_deny_for_current_linux_targets() {
    let profile = read_repo_file("packaging/seccomp/aegishv-seccomp.json");

    for required in [
        "\"defaultAction\": \"SCMP_ACT_ERRNO\"",
        "\"defaultErrnoRet\": 38",
        "\"SCMP_ARCH_X86_64\"",
        "\"SCMP_ARCH_AARCH64\"",
        "\"action\": \"SCMP_ACT_ALLOW\"",
    ] {
        assert!(
            profile.contains(required),
            "seccomp profile is missing required default-deny metadata: {required}"
        );
    }
}

#[test]
fn seccomp_profile_covers_current_sensor_syscall_categories() {
    let profile = read_repo_file("packaging/seccomp/aegishv-seccomp.json");

    for syscall in [
        "execve",
        "exit_group",
        "clone",
        "clone3",
        "futex",
        "rt_sigaction",
        "rt_sigreturn",
        "mmap",
        "mprotect",
        "munmap",
        "getrandom",
        "clock_gettime",
        "openat",
        "newfstatat",
        "read",
        "write",
        "fsync",
        "fdatasync",
        "mkdirat",
        "renameat",
        "unlinkat",
        "epoll_wait",
        "epoll_pwait",
        "poll",
        "socket",
        "bind",
        "listen",
        "accept4",
        "connect",
        "sendto",
        "sendmsg",
        "recvmsg",
        "setsockopt",
        "getsockname",
        "shutdown",
    ] {
        assert_profile_allows(&profile, syscall);
    }
}

#[test]
fn seccomp_profile_does_not_allow_high_risk_kernel_or_privilege_syscalls() {
    let profile = read_repo_file("packaging/seccomp/aegishv-seccomp.json");

    for forbidden in [
        "bpf",
        "ptrace",
        "perf_event_open",
        "mount",
        "umount2",
        "init_module",
        "finit_module",
        "delete_module",
        "kexec_load",
        "reboot",
        "keyctl",
        "add_key",
        "request_key",
        "unshare",
        "setns",
    ] {
        let needle = format!("\"{forbidden}\"");
        assert!(
            !profile.contains(&needle),
            "seccomp profile must not allow high-risk syscall: {forbidden}"
        );
    }
}

#[test]
fn seccomp_packaging_ships_profile_without_enforcing_service_defaults() {
    let debian_install = read_repo_file("packaging/debian/install");
    let debian_script = read_repo_file("scripts/package-debian.sh");
    let debian_service = read_repo_file("packaging/debian/aegishv.service");
    let rpm_spec = read_repo_file("packaging/rpm/aegishv.spec");
    let rpm_service = read_repo_file("packaging/rpm/aegishv.service");
    let dockerfile = read_repo_file("Dockerfile");
    let ci = read_repo_file(".github/workflows/ci.yml");

    for required in [
        "packaging/seccomp/aegishv-seccomp.json usr/share/aegishv/seccomp/aegishv-seccomp.json 0644",
        "install -D -m 0644 packaging/seccomp/aegishv-seccomp.json \"$PKGROOT/usr/share/aegishv/seccomp/aegishv-seccomp.json\"",
        "install -D -m 0644 packaging/seccomp/aegishv-seccomp.json %{buildroot}%{_datadir}/aegishv/seccomp/aegishv-seccomp.json",
        "%{_datadir}/aegishv/seccomp/aegishv-seccomp.json",
    ] {
        assert!(
            debian_install.contains(required)
                || debian_script.contains(required)
                || rpm_spec.contains(required),
            "packaging does not ship the optional seccomp profile: {required}"
        );
    }

    for service in [debian_service, rpm_service] {
        assert!(
            !service.contains("SystemCallFilter")
                && !service.contains("SystemCallErrorNumber")
                && !service.contains("aegishv-seccomp.json"),
            "packaged service defaults must not enforce seccomp without operator review"
        );
    }

    assert!(
        !dockerfile.contains("security-opt") && !dockerfile.contains("seccomp="),
        "Dockerfile must not claim seccomp enforcement by itself"
    );
    assert!(
        !ci.contains("seccomp-tools") && !ci.contains("docker run --security-opt"),
        "normal CI must not require host seccomp enforcement"
    );
}

#[test]
fn seccomp_docs_are_operator_reviewed_and_honest() {
    let deployment = read_repo_file("docs/DEPLOYMENT.md");
    let testing = read_repo_file("docs/TESTING.md");
    let security = read_repo_file("docs/SECURITY.md");

    for required in [
        "packaging/seccomp/aegishv-seccomp.json",
        "Debian and RPM packages ship it",
        "the packaged systemd units do not enable it",
        "Operators must test the profile",
        "defaultAction = SCMP_ACT_ERRNO",
        "QMP Unix sockets",
        "UDP syslog",
        "journald datagram writes",
        "Everything not listed is denied by default",
        "systemd does not consume OCI seccomp JSON directly",
        "The profile reduces syscall surface where practical",
        "does not prove complete sandboxing",
        "exploit prevention",
    ] {
        assert!(
            deployment.contains(required),
            "docs/DEPLOYMENT.md is missing seccomp operator guidance: {required}"
        );
    }

    for required in [
        "Seccomp Profile Checks",
        "Normal PR tests do not require root",
        "default-deny OCI seccomp profile",
        "package installation of the profile as an optional file",
        "no fake sandbox",
        "Operators still need to run replay",
    ] {
        assert!(
            testing.contains(required),
            "docs/TESTING.md is missing seccomp test guidance: {required}"
        );
    }

    for required in [
        "Optional seccomp profile at `packaging/seccomp/aegishv-seccomp.json`",
        "not enabled by default",
        "Enforced AppArmor policy and deployment-specific seccomp tuning",
    ] {
        assert!(
            security.contains(required),
            "docs/SECURITY.md is missing seccomp security posture text: {required}"
        );
    }
}

#[test]
fn seccomp_files_do_not_embed_forbidden_artifacts_or_fake_claims() {
    for path in seccomp_files() {
        let text = fs::read_to_string(&path).expect("read seccomp file");
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
                "{rel} contains forbidden seccomp text: {forbidden}"
            );
        }
    }
}
