use std::fs;
use std::path::{Path, PathBuf};

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn packaging_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![root.join("scripts/package-debian.sh")];
    for entry in fs::read_dir(root.join("packaging/debian")).expect("read packaging/debian") {
        let entry = entry.expect("read packaging file");
        if entry.file_type().expect("file type").is_file() {
            files.push(entry.path());
        }
    }
    files
}

#[test]
fn debian_control_declares_honest_package_scope() {
    let control = read_repo_file("packaging/debian/control");

    for required in [
        "Source: aegishv",
        "Package: aegishv",
        "Architecture: linux-any",
        "Rules-Requires-Root: no",
        "Depends: adduser, systemd-tmpfiles | systemd",
        "host-side KVM telemetry sensor",
        "It does not provide type-1 hypervisor support, full VMI",
        "EPT/NPT enforcement, syscall-path integrity, or hardware PMU sampling",
    ] {
        assert!(
            control.contains(required),
            "Debian control metadata is missing required text: {required}"
        );
    }
}

#[test]
fn debian_install_manifest_covers_operator_layout() {
    let install = read_repo_file("packaging/debian/install");

    for required in [
        "aegishv usr/bin/aegishv 0755",
        "config.example.toml etc/aegishv/config.toml 0644",
        "schema/event.schema.json usr/share/aegishv/schema/event.schema.json 0644",
        "schema/snapshot.schema.json usr/share/aegishv/schema/snapshot.schema.json 0644",
        "packaging/seccomp/aegishv-seccomp.json usr/share/aegishv/seccomp/aegishv-seccomp.json 0644",
        "packaging/apparmor/usr.bin.aegishv usr/share/aegishv/apparmor/usr.bin.aegishv 0644",
        "packaging/selinux/aegishv.te usr/share/aegishv/selinux/aegishv.te 0644",
        "packaging/selinux/aegishv.fc usr/share/aegishv/selinux/aegishv.fc 0644",
        "packaging/selinux/aegishv.if usr/share/aegishv/selinux/aegishv.if 0644",
        "packaging/selinux/README.md usr/share/aegishv/selinux/README.md 0644",
        "packaging/debian/aegishv.service usr/lib/systemd/system/aegishv.service 0644",
        "packaging/debian/aegishv.tmpfiles usr/lib/tmpfiles.d/aegishv.conf 0644",
        "scripts/live-tracefs-smoke.sh usr/share/aegishv/scripts/live-tracefs-smoke.sh 0755",
        "scripts/smoke-replay.sh usr/share/aegishv/scripts/smoke-replay.sh 0755",
        "scripts/validate-jsonl-schema.py usr/share/aegishv/scripts/validate-jsonl-schema.py 0755",
        "packaging/debian/copyright usr/share/doc/aegishv/copyright 0644",
        "docs/DEPLOYMENT.md usr/share/doc/aegishv/DEPLOYMENT.md 0644",
        "docs/SECURITY.md usr/share/doc/aegishv/SECURITY.md 0644",
        "docs/TESTING.md usr/share/doc/aegishv/TESTING.md 0644",
    ] {
        assert!(
            install.contains(required),
            "Debian install manifest is missing required layout entry: {required}"
        );
    }

    assert!(
        read_repo_file("packaging/debian/conffiles").contains("/etc/aegishv/config.toml"),
        "Debian package must mark the operator config as a conffile"
    );
}

#[test]
fn debian_maintainer_scripts_create_user_dirs_without_autostart() {
    let postinst = read_repo_file("packaging/debian/postinst");
    let postrm = read_repo_file("packaging/debian/postrm");

    for required in [
        "getent group aegishv",
        "addgroup --system aegishv",
        "getent passwd aegishv",
        "adduser --system",
        "--ingroup aegishv",
        "--home /var/lib/aegishv",
        "--disabled-login",
        "install -d -o aegishv -g aegishv -m 0750 /var/lib/aegishv",
        "install -d -o aegishv -g aegishv -m 0750 /var/lib/aegishv/dumps",
        "install -d -o aegishv -g aegishv -m 0750 /var/lib/aegishv/spool",
        "install -d -o aegishv -g aegishv -m 0750 /var/log/aegishv",
        "install -d -o aegishv -g aegishv -m 0750 /run/aegishv",
        "systemd-tmpfiles --create /usr/lib/tmpfiles.d/aegishv.conf",
    ] {
        assert!(
            postinst.contains(required),
            "postinst is missing safe setup command: {required}"
        );
    }

    for forbidden in [
        "systemctl enable",
        "systemctl start",
        "systemctl restart",
        "rm -rf /var/lib/aegishv",
        "rm -rf /var/log/aegishv",
    ] {
        assert!(
            !postinst.contains(forbidden) && !postrm.contains(forbidden),
            "Debian maintainer scripts must not surprise operators: {forbidden}"
        );
    }
}

#[test]
fn debian_service_and_tmpfiles_use_tight_package_paths() {
    let service = read_repo_file("packaging/debian/aegishv.service");
    let tmpfiles = read_repo_file("packaging/debian/aegishv.tmpfiles");

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
            "Debian service is missing required package-safe setting: {required}"
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
            "Debian tmpfiles config is missing tight directory rule: {required}"
        );
    }
}

#[test]
fn debian_package_script_is_non_root_ci_friendly() {
    let script = read_repo_file("scripts/package-debian.sh");
    let ci = read_repo_file(".github/workflows/ci.yml");

    for required in [
        "command -v dpkg-deb",
        "dpkg-deb --build --root-owner-group",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "Debian packaging supports GNU Linux targets only",
        "install -D -m 0755 \"$BIN\" \"$PKGROOT/usr/bin/aegishv\"",
        "install -D -m 0644 config.example.toml \"$PKGROOT/etc/aegishv/config.toml\"",
        "install -D -m 0644 packaging/seccomp/aegishv-seccomp.json \"$PKGROOT/usr/share/aegishv/seccomp/aegishv-seccomp.json\"",
        "install -D -m 0644 packaging/apparmor/usr.bin.aegishv \"$PKGROOT/usr/share/aegishv/apparmor/usr.bin.aegishv\"",
        "install -D -m 0644 packaging/selinux/aegishv.te \"$PKGROOT/usr/share/aegishv/selinux/aegishv.te\"",
        "install -D -m 0644 packaging/debian/copyright \"$PKGROOT/usr/share/doc/aegishv/copyright\"",
        "chmod 0750 \"$PKGROOT/var/lib/aegishv\"",
    ] {
        assert!(
            script.contains(required),
            "Debian package script is missing required non-root packaging guard: {required}"
        );
    }

    assert!(
        !ci.contains("package-debian.sh") && !ci.contains("dpkg-deb"),
        "normal CI must not require Debian package builds, root installs, or dpkg-deb"
    );
}

#[test]
fn debian_packaging_files_do_not_embed_forbidden_artifacts_or_secrets() {
    for path in packaging_files() {
        let text = fs::read_to_string(&path).expect("read packaging file");
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
            ".zip",
            "__pycache__",
            "node_modules",
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

#[test]
fn release_upload_patterns_do_not_duplicate_signature_or_provenance_assets() {
    let workflow = read_repo_file(".github/workflows/release.yml");

    for required in [
        "dist/*.tar.gz.sigstore.json",
        "dist/*.sbom.cdx.json.sigstore.json",
        "dist/SHA256SUMS-*.txt.sigstore.json",
        "dist/*.slsa-provenance.sigstore.json",
    ] {
        assert!(
            workflow.contains(required),
            "release upload list is missing explicit artifact pattern: {required}"
        );
    }

    assert!(
        !workflow.contains("dist/*.sigstore.json\n"),
        "release upload list must not use an overlapping generic signature glob"
    );
}
