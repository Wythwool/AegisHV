use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VALID_EVENT: &str = r#"{"version":1,"schema_version":2,"ts":"2026-01-01T00:00:00Z","monotonic_ms":0,"sequence":1,"event_id":"evt-test","category":"sensor","severity":"info","vm":"host","data_loss":false}"#;

const VALID_ACTION_AUDIT_EVENT: &str = r#"{"version":1,"schema_version":2,"ts":"2026-01-01T00:00:00Z","monotonic_ms":0,"sequence":1,"event_id":"evt-action-audit-test","category":"policy","severity":"high","vm":"vm-a","vm_id":"libvirt:111","reason":"policy_action","rule_id":"rule-1","decision":"refused","action_id":"act-test","action_status":"refused","data_loss":false,"action":{"rule":"rule-1","kind":"pause_vm","ok":false,"status":"refused","decision":"refused","result":"refused","detail":"identity.require_stable_qmp_match=true refused QMP VM-name fallback","latency_ms":1,"target_vm_id":"libvirt:111","attempt":1,"max_attempts":1,"retry_count":0,"timeout_ms":2000,"timed_out":false,"refused":true,"failure_class":"stable_identity_required"}}"#;

const VALID_LOSS_RANGE_EVENT: &str = r#"{"version":1,"schema_version":2,"ts":"2026-01-01T00:00:00Z","monotonic_ms":0,"sequence":4,"event_id":"evt-loss-range-test","category":"sensor","severity":"high","vm":"host","reason":"telemetry_loss","data_loss":true,"loss":{"dropped_since_last_event":0,"dropped_total":0,"reason":"sequence_gap","range_kind":"sequence_gap","sequence_gap_start":2,"sequence_gap_end":3}}"#;

const VALID_SNAPSHOT: &str = r#"{"schema_version":2,"ts":"2026-01-01T00:00:00Z","kvm":false,"tracefs_root":"/sys/kernel/tracing","trace_pipe":null,"trace_pipe_readable":false,"tracepoints_ok":false,"tracepoints":[{"system":"kvm","name":"kvm_exit","status":"missing","missing_fields":["vcpu_id","exit_reason","instruction_pointer"],"message":"tracepoint kvm/kvm_exit format metadata is missing"}],"vm_inventory":{"status":"ok","source":"none","freshness":"none","vm_count":0,"degraded":false,"vms":[]},"mode":"unavailable"}"#;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn temp_file(label: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "aegishv-validator-{label}-{}-{}.json",
        std::process::id(),
        aegishv::util::next_sequence()
    ));
    std::fs::write(&path, contents).expect("write validator fixture");
    path
}

fn run_validator(args: &[OsString]) -> Output {
    let root = repo_root();
    let script = root.join("scripts/validate-jsonl-schema.py");
    let candidates: &[(&str, &[&str])] = if cfg!(windows) {
        &[("py", &["-3"]), ("python3", &[]), ("python", &[])]
    } else {
        &[("python3", &[]), ("python", &[])]
    };
    let mut errors = Vec::new();
    for (program, prefix_args) in candidates {
        let mut command = Command::new(program);
        command.current_dir(root);
        command.args(*prefix_args);
        command.arg(&script);
        command.args(args);
        match command.output() {
            Ok(output) => return output,
            Err(err) => errors.push(format!("{program}: {err}")),
        }
    }
    panic!("no usable Python interpreter found: {}", errors.join("; "));
}

fn schema_arg(name: &str) -> OsString {
    repo_root().join("schema").join(name).into_os_string()
}

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "validator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: Output, expected: &str) {
    assert!(
        !output.status.success(),
        "validator unexpectedly passed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "stderr did not contain {expected:?}\nstderr:\n{stderr}"
    );
}

#[test]
fn validator_accepts_event_jsonl_against_schema() {
    let jsonl = temp_file("valid-event", &format!("{VALID_EVENT}\n"));
    assert_success(run_validator(&[
        "--schema".into(),
        schema_arg("event.schema.json"),
        "--jsonl".into(),
        jsonl.clone().into_os_string(),
    ]));
    let _ = std::fs::remove_file(jsonl);
}

#[test]
fn validator_accepts_action_audit_event_against_schema() {
    let jsonl = temp_file(
        "valid-action-audit-event",
        &format!("{VALID_ACTION_AUDIT_EVENT}\n"),
    );
    assert_success(run_validator(&[
        "--schema".into(),
        schema_arg("event.schema.json"),
        "--jsonl".into(),
        jsonl.clone().into_os_string(),
    ]));
    let _ = std::fs::remove_file(jsonl);
}

#[test]
fn validator_accepts_loss_range_event_against_schema() {
    let jsonl = temp_file(
        "valid-loss-range-event",
        &format!("{VALID_LOSS_RANGE_EVENT}\n"),
    );
    assert_success(run_validator(&[
        "--schema".into(),
        schema_arg("event.schema.json"),
        "--jsonl".into(),
        jsonl.clone().into_os_string(),
    ]));
    let _ = std::fs::remove_file(jsonl);
}

#[test]
fn validator_rejects_empty_jsonl() {
    let jsonl = temp_file("empty-jsonl", "\n");
    assert_failure(
        run_validator(&[
            "--schema".into(),
            schema_arg("event.schema.json"),
            "--jsonl".into(),
            jsonl.clone().into_os_string(),
        ]),
        "no JSON documents",
    );
    let _ = std::fs::remove_file(jsonl);
}

#[test]
fn validator_rejects_malformed_json_line() {
    let jsonl = temp_file("malformed-jsonl", "{\n");
    assert_failure(
        run_validator(&[
            "--schema".into(),
            schema_arg("event.schema.json"),
            "--jsonl".into(),
            jsonl.clone().into_os_string(),
        ]),
        "invalid JSON",
    );
    let _ = std::fs::remove_file(jsonl);
}

#[test]
fn validator_rejects_missing_required_field() {
    let jsonl = temp_file(
        "missing-required",
        r#"{"version":1,"schema_version":2,"ts":"2026-01-01T00:00:00Z","monotonic_ms":0,"sequence":1,"category":"sensor","severity":"info","vm":"host","data_loss":false}"#,
    );
    assert_failure(
        run_validator(&[
            "--schema".into(),
            schema_arg("event.schema.json"),
            "--jsonl".into(),
            jsonl.clone().into_os_string(),
        ]),
        "$.event_id: required property is missing",
    );
    let _ = std::fs::remove_file(jsonl);
}

#[test]
fn validator_rejects_const_enum_and_type_errors() {
    for (label, document, expected) in [
        (
            "bad-const",
            r#"{"version":1,"schema_version":3,"ts":"2026-01-01T00:00:00Z","monotonic_ms":0,"sequence":1,"event_id":"evt-test","category":"sensor","severity":"info","vm":"host","data_loss":false}"#,
            "expected const 2",
        ),
        (
            "bad-enum",
            r#"{"version":1,"schema_version":2,"ts":"2026-01-01T00:00:00Z","monotonic_ms":0,"sequence":1,"event_id":"evt-test","category":"bogus","severity":"info","vm":"host","data_loss":false}"#,
            "not in enum",
        ),
        (
            "bad-type",
            r#"{"version":1,"schema_version":2,"ts":"2026-01-01T00:00:00Z","monotonic_ms":"0","sequence":1,"event_id":"evt-test","category":"sensor","severity":"info","vm":"host","data_loss":false}"#,
            "expected type",
        ),
    ] {
        let jsonl = temp_file(label, document);
        assert_failure(
            run_validator(&[
                "--schema".into(),
                schema_arg("event.schema.json"),
                "--jsonl".into(),
                jsonl.clone().into_os_string(),
            ]),
            expected,
        );
        let _ = std::fs::remove_file(jsonl);
    }
}

#[test]
fn validator_rejects_nested_schema_violation() {
    let jsonl = temp_file(
        "nested-violation",
        r#"{"version":1,"schema_version":2,"ts":"2026-01-01T00:00:00Z","monotonic_ms":0,"sequence":1,"event_id":"evt-test","category":"sensor","severity":"info","vm":"host","data_loss":false,"addr":{"gpa":7}}"#,
    );
    assert_failure(
        run_validator(&[
            "--schema".into(),
            schema_arg("event.schema.json"),
            "--jsonl".into(),
            jsonl.clone().into_os_string(),
        ]),
        "$.addr.gpa: expected type",
    );
    let _ = std::fs::remove_file(jsonl);
}

#[test]
fn validator_rejects_invalid_identity_confidence() {
    let jsonl = temp_file(
        "bad-identity-confidence",
        r#"{"version":1,"schema_version":2,"ts":"2026-01-01T00:00:00Z","monotonic_ms":0,"sequence":1,"event_id":"evt-test","category":"exit","severity":"info","vm":"vm-a","data_loss":false,"identity":{"sources":["trace_comm"],"confidence":"certain","start_time_verified":false,"ambiguous":false}}"#,
    );
    assert_failure(
        run_validator(&[
            "--schema".into(),
            schema_arg("event.schema.json"),
            "--jsonl".into(),
            jsonl.clone().into_os_string(),
        ]),
        "$.identity.confidence: value 'certain' is not in enum",
    );
    let _ = std::fs::remove_file(jsonl);
}

#[test]
fn validator_accepts_snapshot_json_against_schema() {
    let json = temp_file("valid-snapshot", VALID_SNAPSHOT);
    assert_success(run_validator(&[
        "--schema".into(),
        schema_arg("snapshot.schema.json"),
        "--json".into(),
        json.clone().into_os_string(),
    ]));
    let _ = std::fs::remove_file(json);
}

#[test]
fn validator_rejects_snapshot_additional_property() {
    let json = temp_file(
        "snapshot-extra",
        r#"{"schema_version":2,"ts":"2026-01-01T00:00:00Z","kvm":false,"tracefs_root":"/sys/kernel/tracing","trace_pipe":null,"trace_pipe_readable":false,"tracepoints_ok":true,"tracepoints":[],"vm_inventory":{"status":"ok","source":"none","freshness":"none","vm_count":0,"degraded":false,"vms":[]},"mode":"unavailable","extra":true}"#,
    );
    assert_failure(
        run_validator(&[
            "--schema".into(),
            schema_arg("snapshot.schema.json"),
            "--json".into(),
            json.clone().into_os_string(),
        ]),
        "$.extra: additional property is not allowed",
    );
    let _ = std::fs::remove_file(json);
}
