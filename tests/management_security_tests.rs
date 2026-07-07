use std::fs;
use std::path::Path;
use std::process::Command;

fn repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn run_aegishv(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_aegishv"))
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("run aegishv {args:?}: {err}"));
    assert!(
        output.status.success(),
        "aegishv {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout must be UTF-8")
}

fn assert_contains_all(text: &str, required: &[&str]) {
    for item in required {
        assert!(text.contains(item), "missing required text: {item}");
    }
}

#[test]
fn admin_cli_exposes_local_health_and_policy_review() {
    let version = run_aegishv(&["version", "--json"]);
    assert_contains_all(&version, &["\"version\"", "\"target_arch\"", "\"git_rev\""]);

    let health = run_aegishv(&["admin", "health", "--json"]);
    assert_contains_all(&health, &["\"status\":\"ok\"", "\"runtime\":\"local_cli\""]);

    let config = Path::new(env!("CARGO_MANIFEST_DIR")).join("config.example.toml");
    let explain = run_aegishv(&[
        "admin",
        "policy-explain",
        "--config",
        config.to_str().unwrap(),
        "--json",
    ]);
    assert_contains_all(
        &explain,
        &[
            "\"version\"",
            "\"enabled_rules\"",
            "\"stable_qmp_required\"",
        ],
    );
}

#[test]
fn admin_cli_supports_policy_and_action_dry_runs() {
    let config = Path::new(env!("CARGO_MANIFEST_DIR")).join("config.example.toml");
    let cfg = config.to_str().unwrap();
    let policy = run_aegishv(&[
        "admin",
        "policy-test",
        "--config",
        cfg,
        "--category",
        "wx",
        "--severity",
        "high",
        "--reason",
        "sample W^X transition",
        "--vm",
        "vm-a",
        "--json",
    ]);
    assert_contains_all(&policy, &["\"matched\"", "\"events\""]);

    let action = run_aegishv(&[
        "admin",
        "action-dry-run",
        "--config",
        cfg,
        "--kind",
        "pause_vm",
        "--vm",
        "vm-a",
        "--json",
    ]);
    assert_contains_all(
        &action,
        &[
            "\"category\":\"policy\"",
            "\"decision\":\"dry_run\"",
            "\"action\"",
            "\"kind\":\"pause_vm\"",
        ],
    );
}

#[test]
fn management_security_docs_cover_boundaries() {
    let management = repo_file("docs/MANAGEMENT_API.md");
    let bundles = repo_file("docs/POLICY_BUNDLES.md");
    let update = repo_file("docs/UPDATE.md");
    let attestation = repo_file("docs/ATTESTATION.md");
    let incident = repo_file("docs/INCIDENT_RESPONSE.md");
    let status = repo_file("docs/STATUS.md");
    let testing = repo_file("docs/TESTING.md");

    assert_contains_all(
        &management,
        &[
            "does not start a management daemon",
            "does not expose an HTTP listener",
            "policy-test",
            "action-dry-run",
            "AppendOnlyAuditLog",
            "ApprovalStore",
            "not a cryptographic signature",
        ],
    );
    assert_contains_all(
        &bundles,
        &[
            "Policy bundle verification",
            "rejects",
            "versions older than the currently applied policy version",
            "does not fetch keys from the network",
        ],
    );
    assert_contains_all(
        &update,
        &[
            "aegishv validate-config --config FILE",
            "Reject rollback versions",
            "There is no self-update mechanism",
        ],
    );
    assert_contains_all(
        &attestation,
        &[
            "does not implement hardware remote attestation",
            "compile-time Git revision",
            "not a signature",
            "Hardware attestation and measured type-1 launch evidence are not implemented",
        ],
    );
    assert_contains_all(
        &incident,
        &[
            "policy audit events",
            "missing stable VM identity",
            "Do not edit raw event files during triage",
            "does not acquire guest memory from a live VMI backend",
        ],
    );
    assert!(status.contains("Local management CLI"));
    assert!(testing.contains("management_security_tests"));
}
