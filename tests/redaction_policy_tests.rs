use std::fs;
use std::path::Path;

fn redaction_policy() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/EVENT_REDACTION.md"))
        .expect("read event redaction policy")
}

#[test]
fn redaction_policy_documents_current_non_implementation() {
    let text = redaction_policy();

    for required in [
        "Runtime redaction is not implemented",
        "No `[redaction]` config section exists",
        "No schema fields declare redaction state",
        "No sink rewrites or strips event fields today",
        "AegisHV does not provide a privacy guarantee",
    ] {
        assert!(
            text.contains(required),
            "redaction policy is missing required current-status text: {required}"
        );
    }
}

#[test]
fn redaction_policy_covers_required_sinks_and_event_types() {
    let text = redaction_policy();

    for required in [
        "### JSONL",
        "### Syslog",
        "### Journald",
        "### Disk Spool",
        "### OTLP Design",
        "### OCSF/ECS Mapping Docs",
        "### Action Audit Details",
        "### Identity Metadata",
        "### VM Inventory",
        "### Tracefs Diagnostics",
        "### Lifecycle Events",
        "### Loss Events",
    ] {
        assert!(
            text.contains(required),
            "redaction policy is missing required section: {required}"
        );
    }
}

#[test]
fn redaction_policy_rejects_sensitive_metric_label_sources() {
    let text = redaction_policy();

    for required in [
        "VM names",
        "UUIDs",
        "PIDs or TIDs",
        "socket paths",
        "host paths",
        "command lines",
        "XML",
        "event messages",
        "`action.detail`",
        "arbitrary error strings",
    ] {
        assert!(
            text.contains(required),
            "redaction policy is missing forbidden metric label source: {required}"
        );
    }
}
