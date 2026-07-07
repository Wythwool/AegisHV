use std::fs;
use std::path::{Path, PathBuf};

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn selinux_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for entry in fs::read_dir(root.join("packaging/selinux")).expect("read packaging/selinux") {
        let entry = entry.expect("read SELinux file");
        if entry.file_type().expect("file type").is_file() {
            files.push(entry.path());
        }
    }
    files
}

#[test]
fn selinux_policy_skeleton_declares_current_sensor_domain_and_types() {
    let te = read_repo_file("packaging/selinux/aegishv.te");
    let fc = read_repo_file("packaging/selinux/aegishv.fc");
    let iface = read_repo_file("packaging/selinux/aegishv.if");

    for required in [
        "policy_module(aegishv, 0.1.0)",
        "type aegishv_t;",
        "type aegishv_exec_t;",
        "init_daemon_domain(aegishv_t, aegishv_exec_t)",
        "type aegishv_etc_t;",
        "type aegishv_share_t;",
        "type aegishv_doc_t;",
        "type aegishv_log_t;",
        "type aegishv_var_lib_t;",
        "type aegishv_runtime_t;",
    ] {
        assert!(
            te.contains(required),
            "SELinux .te skeleton is missing expected domain/type text: {required}"
        );
    }

    for required in [
        "/usr/bin/aegishv",
        "/usr/local/bin/aegishv",
        "/etc/aegishv(/.*)?",
        "/usr/share/aegishv(/.*)?",
        "/usr/share/doc/aegishv(/.*)?",
        "/var/log/aegishv(/.*)?",
        "/var/lib/aegishv(/.*)?",
        "/run/aegishv(/.*)?",
    ] {
        assert!(
            fc.contains(required),
            "SELinux .fc skeleton is missing expected file context: {required}"
        );
    }

    for required in [
        "interface(`aegishv_read_config'",
        "interface(`aegishv_read_shared_files'",
        "interface(`aegishv_manage_state'",
    ] {
        assert!(
            iface.contains(required),
            "SELinux .if skeleton is missing expected interface: {required}"
        );
    }
}

#[test]
fn selinux_policy_skeleton_covers_current_sensor_access_categories() {
    let te = read_repo_file("packaging/selinux/aegishv.te");

    for required in [
        "read_files_pattern(aegishv_t, aegishv_etc_t, aegishv_etc_t)",
        "read_files_pattern(aegishv_t, aegishv_share_t, aegishv_share_t)",
        "read_files_pattern(aegishv_t, aegishv_doc_t, aegishv_doc_t)",
        "manage_files_pattern(aegishv_t, aegishv_log_t, aegishv_log_t)",
        "manage_files_pattern(aegishv_t, aegishv_var_lib_t, aegishv_var_lib_t)",
        "manage_files_pattern(aegishv_t, aegishv_runtime_t, aegishv_runtime_t)",
        "allow aegishv_t proc_t:file { getattr open read };",
        "allow aegishv_t sysfs_t:file { getattr open read };",
        "allow aegishv_t configfs_t:file { getattr open read };",
        "type tracefs_t;",
        "type debugfs_t;",
        "allow aegishv_t tracefs_t:dir { getattr open read search };",
        "allow aegishv_t tracefs_t:file { getattr open read };",
        "allow aegishv_t tracefs_t:lnk_file { getattr read };",
        "allow aegishv_t debugfs_t:dir { getattr open read search };",
        "allow aegishv_t debugfs_t:file { getattr open read };",
        "allow aegishv_t debugfs_t:lnk_file { getattr read };",
        "allow aegishv_t virtd_var_run_t:sock_file { getattr read write };",
        "allow aegishv_t virtd_var_run_t:unix_stream_socket connect;",
        "allow aegishv_t devlog_t:sock_file { getattr write };",
        "allow aegishv_t syslogd_t:unix_dgram_socket sendto;",
        "allow aegishv_t self:tcp_socket { accept bind create getattr listen name_bind read setopt write };",
        "allow aegishv_t self:udp_socket { connect create getattr sendto setopt write };",
    ] {
        assert!(
            te.contains(required),
            "SELinux .te skeleton is missing expected access category: {required}"
        );
    }
}

#[test]
fn selinux_packaging_ships_skeleton_without_loading_or_enforcing_it() {
    let debian_install = read_repo_file("packaging/debian/install");
    let debian_script = read_repo_file("scripts/package-debian.sh");
    let debian_service = read_repo_file("packaging/debian/aegishv.service");
    let debian_postinst = read_repo_file("packaging/debian/postinst");
    let rpm_spec = read_repo_file("packaging/rpm/aegishv.spec");
    let rpm_service = read_repo_file("packaging/rpm/aegishv.service");
    let ci = read_repo_file(".github/workflows/ci.yml");

    for required in [
        "packaging/selinux/aegishv.te usr/share/aegishv/selinux/aegishv.te 0644",
        "packaging/selinux/aegishv.fc usr/share/aegishv/selinux/aegishv.fc 0644",
        "packaging/selinux/aegishv.if usr/share/aegishv/selinux/aegishv.if 0644",
        "packaging/selinux/README.md usr/share/aegishv/selinux/README.md 0644",
        "install -D -m 0644 packaging/selinux/aegishv.te \"$PKGROOT/usr/share/aegishv/selinux/aegishv.te\"",
        "install -D -m 0644 packaging/selinux/aegishv.te %{buildroot}%{_datadir}/aegishv/selinux/aegishv.te",
        "%{_datadir}/aegishv/selinux/aegishv.te",
    ] {
        assert!(
            debian_install.contains(required)
                || debian_script.contains(required)
                || rpm_spec.contains(required),
            "packaging does not ship the optional SELinux skeleton: {required}"
        );
    }

    for service in [debian_service, rpm_service] {
        assert!(
            !service.contains("SELinuxContext="),
            "packaged service defaults must not force a SELinux context"
        );
    }

    for text in [debian_script, debian_postinst, rpm_spec] {
        for forbidden in [
            "semodule -i",
            "restorecon",
            "semanage permissive",
            "checkpolicy",
            "/etc/selinux",
        ] {
            assert!(
                !text.contains(forbidden),
                "package scripts must not load or enforce SELinux policy by default: {forbidden}"
            );
        }
    }

    for forbidden in [
        "semodule",
        "checkpolicy",
        "semanage permissive",
        "setenforce",
    ] {
        assert!(
            !ci.contains(forbidden),
            "normal CI must not require SELinux tooling or host enforcement: {forbidden}"
        );
    }
}

#[test]
fn selinux_docs_are_operator_reviewed_and_honest() {
    let deployment = read_repo_file("docs/DEPLOYMENT.md");
    let testing = read_repo_file("docs/TESTING.md");
    let security = read_repo_file("docs/SECURITY.md");
    let readme = read_repo_file("packaging/selinux/README.md");

    for required in [
        "packaging/selinux",
        "Debian and RPM packages ship it",
        "do not load it with `semodule`",
        "Operators must build, review, tune, load, and test the skeleton",
        "an `aegishv_t` process domain",
        "tracefs/debugfs, sysfs, configfs, and procfs reads",
        "common SELinux labels such as `tracefs_t` and `debugfs_t`",
        "Tracefs labeling is distro-specific",
        "operators may still need local file contexts or allow rules",
        "QMP Unix socket access",
        "syslog and journald datagram socket writes",
        "sudo semodule -i aegishv.pp",
        "sudo semanage permissive -a aegishv_t",
        "Review audit denials",
        "can break deployments",
        "must be adjusted per deployment",
        "not complete confinement",
    ] {
        assert!(
            deployment.contains(required),
            "docs/DEPLOYMENT.md is missing SELinux operator guidance: {required}"
        );
    }

    for required in [
        "SELinux Policy Skeleton Checks",
        "Normal PR tests do not require root",
        "`checkpolicy`, `semodule`, or distro-specific SELinux tooling",
        "expected categories for config, schemas, docs, tracefs/debugfs/procfs reads",
        "explicit read coverage for common SELinux trace labels",
        "package installation of the policy skeleton as optional review material",
        "without loading it or enabling enforcement in service defaults",
        "no fake confinement",
        "The tests do not prove that the policy compiles",
    ] {
        assert!(
            testing.contains(required),
            "docs/TESTING.md is missing SELinux test guidance: {required}"
        );
    }

    for required in [
        "Optional SELinux policy skeleton at `packaging/selinux`",
        "includes common tracefs/debugfs read labels",
        "not installed with `semodule` by default",
        "Reviewed and enforced SELinux policy for the target distribution",
    ] {
        assert!(
            security.contains(required),
            "docs/SECURITY.md is missing SELinux security posture text: {required}"
        );
    }

    for required in [
        "optional SELinux policy skeleton",
        "using the common `tracefs_t` and `debugfs_t` labels",
        "Tracefs labels vary by distribution",
        "not loaded by package scripts",
        "Run replay, live tracefs smoke, metrics listener checks",
        "not complete confinement",
    ] {
        assert!(
            readme.contains(required),
            "packaging/selinux/README.md is missing required guidance: {required}"
        );
    }
}

#[test]
fn selinux_files_do_not_embed_forbidden_artifacts_or_fake_claims() {
    for path in selinux_files() {
        let text = fs::read_to_string(&path).expect("read SELinux file");
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
            "complete confinement is provided",
            "exploit prevention is provided",
            "kernel isolation is provided",
            "type-1 safety is provided",
            "full VMI support",
            "EPT/NPT enforcement is provided",
            "hardware PMU support is provided",
        ] {
            assert!(
                !text.contains(forbidden),
                "{rel} contains forbidden SELinux text: {forbidden}"
            );
        }
    }
}
