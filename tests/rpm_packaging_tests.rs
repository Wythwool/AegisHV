use std::fs;
use std::path::{Path, PathBuf};

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn rpm_packaging_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![root.join("scripts/package-rpm.sh")];
    for entry in fs::read_dir(root.join("packaging/rpm")).expect("read packaging/rpm") {
        let entry = entry.expect("read rpm packaging file");
        if entry.file_type().expect("file type").is_file() {
            files.push(entry.path());
        }
    }
    files
}

#[test]
fn rpm_spec_declares_honest_package_scope() {
    let spec = read_repo_file("packaging/rpm/aegishv.spec");

    for required in [
        "Name:           aegishv",
        "Version:        0.4.0",
        "License:        MIT",
        "Requires(pre):  shadow-utils",
        "host-side KVM telemetry sensor",
        "It does not provide type-1 hypervisor support, full VMI",
        "EPT/NPT enforcement",
        "syscall-path integrity, live libvirt integration, or hardware PMU sampling",
    ] {
        assert!(
            spec.contains(required),
            "RPM spec metadata is missing required text: {required}"
        );
    }
}

#[test]
fn rpm_spec_installs_operator_layout() {
    let spec = read_repo_file("packaging/rpm/aegishv.spec");

    for required in [
        "cargo build --locked --release --target %{aegishv_cargo_target}",
        "install -D -m 0755 target/%{aegishv_cargo_target}/release/aegishv %{buildroot}%{_bindir}/aegishv",
        "install -D -m 0755 target/release/aegishv %{buildroot}%{_bindir}/aegishv",
        "install -D -m 0644 config.example.toml %{buildroot}%{_sysconfdir}/aegishv/config.toml",
        "install -D -m 0644 schema/event.schema.json %{buildroot}%{_datadir}/aegishv/schema/event.schema.json",
        "install -D -m 0644 schema/snapshot.schema.json %{buildroot}%{_datadir}/aegishv/schema/snapshot.schema.json",
        "install -D -m 0644 packaging/seccomp/aegishv-seccomp.json %{buildroot}%{_datadir}/aegishv/seccomp/aegishv-seccomp.json",
        "install -D -m 0644 packaging/apparmor/usr.bin.aegishv %{buildroot}%{_datadir}/aegishv/apparmor/usr.bin.aegishv",
        "install -D -m 0644 packaging/selinux/aegishv.te %{buildroot}%{_datadir}/aegishv/selinux/aegishv.te",
        "install -D -m 0644 packaging/selinux/aegishv.fc %{buildroot}%{_datadir}/aegishv/selinux/aegishv.fc",
        "install -D -m 0644 packaging/selinux/aegishv.if %{buildroot}%{_datadir}/aegishv/selinux/aegishv.if",
        "install -D -m 0644 packaging/selinux/README.md %{buildroot}%{_datadir}/aegishv/selinux/README.md",
        "install -D -m 0644 packaging/rpm/aegishv.service %{buildroot}/usr/lib/systemd/system/aegishv.service",
        "install -D -m 0644 packaging/rpm/aegishv.tmpfiles %{buildroot}/usr/lib/tmpfiles.d/aegishv.conf",
        "install -D -m 0755 scripts/live-tracefs-smoke.sh %{buildroot}%{_datadir}/aegishv/scripts/live-tracefs-smoke.sh",
        "install -D -m 0755 scripts/smoke-replay.sh %{buildroot}%{_datadir}/aegishv/scripts/smoke-replay.sh",
        "install -D -m 0755 scripts/validate-jsonl-schema.py %{buildroot}%{_datadir}/aegishv/scripts/validate-jsonl-schema.py",
        "install -D -m 0644 packaging/rpm/README.md %{buildroot}%{_docdir}/aegishv/RPM_PACKAGING.md",
        "%config(noreplace) %{_sysconfdir}/aegishv/config.toml",
        "%{_datadir}/aegishv/seccomp/aegishv-seccomp.json",
        "%{_datadir}/aegishv/apparmor/usr.bin.aegishv",
        "%{_datadir}/aegishv/selinux/aegishv.te",
        "%{_datadir}/aegishv/selinux/aegishv.fc",
        "%{_datadir}/aegishv/selinux/aegishv.if",
        "%{_datadir}/aegishv/selinux/README.md",
        "%{_docdir}/aegishv",
    ] {
        assert!(
            spec.contains(required),
            "RPM spec is missing required layout entry: {required}"
        );
    }
}

#[test]
fn rpm_scriptlets_create_user_dirs_without_autostart() {
    let spec = read_repo_file("packaging/rpm/aegishv.spec");

    for required in [
        "getent group aegishv",
        "groupadd -r aegishv",
        "getent passwd aegishv",
        "useradd -r -g aegishv",
        "-d /var/lib/aegishv",
        "-s /sbin/nologin",
        "install -d -o aegishv -g aegishv -m 0750 /var/lib/aegishv",
        "install -d -o aegishv -g aegishv -m 0750 /var/lib/aegishv/dumps",
        "install -d -o aegishv -g aegishv -m 0750 /var/lib/aegishv/spool",
        "install -d -o aegishv -g aegishv -m 0750 /var/log/aegishv",
        "install -d -o aegishv -g aegishv -m 0750 /run/aegishv",
        "systemd-tmpfiles --create /usr/lib/tmpfiles.d/aegishv.conf",
    ] {
        assert!(
            spec.contains(required),
            "RPM scriptlets are missing safe setup command: {required}"
        );
    }

    for forbidden in [
        "systemctl enable",
        "systemctl start",
        "systemctl restart",
        "%systemd_post",
        "%systemd_preun",
        "rm -rf /var/lib/aegishv",
        "rm -rf /var/log/aegishv",
    ] {
        assert!(
            !spec.contains(forbidden),
            "RPM scriptlets must not surprise operators: {forbidden}"
        );
    }
}

#[test]
fn rpm_service_and_tmpfiles_use_tight_package_paths() {
    let service = read_repo_file("packaging/rpm/aegishv.service");
    let tmpfiles = read_repo_file("packaging/rpm/aegishv.tmpfiles");

    for required in [
        "User=aegishv",
        "Group=aegishv",
        "ExecStart=/usr/bin/aegishv run --tracefs /sys/kernel/tracing --config /etc/aegishv/config.toml --jsonl /var/log/aegishv/events.jsonl",
        "ReadWritePaths=/var/lib/aegishv /var/log/aegishv /run/aegishv",
        "UMask=0077",
        "NoNewPrivileges=yes",
        "ProtectSystem=strict",
    ] {
        assert!(
            service.contains(required),
            "RPM service is missing required package-safe setting: {required}"
        );
    }

    for required in [
        "d /var/lib/aegishv 0750 aegishv aegishv -",
        "d /var/lib/aegishv/dumps 0750 aegishv aegishv -",
        "d /var/lib/aegishv/spool 0750 aegishv aegishv -",
        "d /var/log/aegishv 0750 aegishv aegishv -",
        "d /run/aegishv 0750 aegishv aegishv -",
    ] {
        assert!(
            tmpfiles.contains(required),
            "RPM tmpfiles config is missing tight directory rule: {required}"
        );
    }
}

#[test]
fn rpm_package_script_is_non_root_ci_friendly() {
    let script = read_repo_file("scripts/package-rpm.sh");
    let ci = read_repo_file(".github/workflows/ci.yml");

    for required in [
        "command -v rpmbuild",
        "rpmbuild -bb packaging/rpm/aegishv.spec",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "RPM packaging supports GNU Linux targets only",
        "--define \"aegishv_cargo_target $target\"",
        "--exclude='./target'",
        "--exclude='./.git'",
        "--exclude='./dist'",
        "--exclude='./__pycache__'",
        "--exclude='./node_modules'",
        "--exclude='*.zip'",
        "--exclude='*.tar.gz'",
    ] {
        assert!(
            script.contains(required),
            "RPM package script is missing required non-root packaging guard: {required}"
        );
    }

    assert!(
        !ci.contains("package-rpm.sh") && !ci.contains("rpmbuild"),
        "normal CI must not require RPM package builds, root installs, or rpmbuild"
    );
}

#[test]
fn rpm_packaging_files_do_not_embed_forbidden_artifacts_or_secrets() {
    for path in rpm_packaging_files() {
        let text = fs::read_to_string(&path).expect("read RPM packaging file");
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
            "PRIVATE KEY",
            "BEGIN RSA",
            "fake type-1",
            "full VMI support",
            "hardware PMU support",
        ] {
            assert!(
                !text.contains(forbidden),
                "{rel} contains forbidden packaging text: {forbidden}"
            );
        }
    }
}
