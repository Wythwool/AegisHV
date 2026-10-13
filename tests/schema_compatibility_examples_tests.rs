use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn schema_examples_dir() -> PathBuf {
    repo_root().join("schema/examples")
}

fn run_validator(args: &[OsString]) -> Output {
    let script = repo_root().join("scripts/validate-jsonl-schema.py");
    let candidates: &[(&str, &[&str])] = if cfg!(windows) {
        &[("py", &["-3"]), ("python3", &[]), ("python", &[])]
    } else {
        &[("python3", &[]), ("python", &[])]
    };
    let mut errors = Vec::new();
    for (program, prefix_args) in candidates {
        let mut command = Command::new(program);
        command.current_dir(repo_root());
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

fn assert_success(output: Output) {
    assert!(
        output.status.success(),
        "validator failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn schema_compatibility_examples_validate_against_current_schemas() {
    let event_example = schema_examples_dir().join("event-v2-compatibility.jsonl");
    let snapshot_example = schema_examples_dir().join("snapshot-v2-inventory.json");

    assert_success(run_validator(&[
        "--schema".into(),
        repo_root()
            .join("schema/event.schema.json")
            .into_os_string(),
        "--jsonl".into(),
        event_example.into_os_string(),
    ]));

    assert_success(run_validator(&[
        "--schema".into(),
        repo_root()
            .join("schema/snapshot.schema.json")
            .into_os_string(),
        "--json".into(),
        snapshot_example.into_os_string(),
    ]));
}

#[test]
fn schema_compatibility_examples_cover_current_contract_topics() {
    let event_contents =
        std::fs::read_to_string(schema_examples_dir().join("event-v2-compatibility.jsonl"))
            .expect("read event compatibility example");
    let snapshot_contents =
        std::fs::read_to_string(schema_examples_dir().join("snapshot-v2-inventory.json"))
            .expect("read snapshot compatibility example");

    for expected in [
        "\"category\":\"exit\"",
        "\"category\":\"wx\"",
        "\"category\":\"pmu\"",
        "\"category\":\"policy\"",
        "\"category\":\"snapshot\"",
        "\"reason\":\"sensor_startup\"",
        "\"reason\":\"tracefs_format_diagnostic\"",
        "\"action\":{",
        "\"trap\":{",
        "\"identity\":{",
        "\"loss\":{",
        "\"range_kind\":\"sequence_gap\"",
    ] {
        assert!(
            event_contents.contains(expected),
            "event compatibility example does not cover {expected}"
        );
    }

    for expected in [
        "\"vm_inventory\"",
        "\"known_host_tasks\"",
        "\"vcpu_mappings\"",
        "\"qmp\"",
        "\"confidence\": \"high\"",
        "\"tracepoints\"",
    ] {
        assert!(
            snapshot_contents.contains(expected),
            "snapshot compatibility example does not cover {expected}"
        );
    }
}

#[test]
fn schema_compatibility_examples_avoid_host_specific_values() {
    let forbidden = [
        "C:\\",
        "\\Users\\",
        "/Users/",
        "/home/",
        "/tmp/",
        "/run/",
        "/var/",
        "/sys/",
        "BEGIN PRIVATE KEY",
        "PRIVATE KEY",
        "<uuid>",
        "<path>",
        "<repo>",
    ];

    for entry in std::fs::read_dir(schema_examples_dir()).expect("read schema examples dir") {
        let path = entry.expect("read schema example entry").path();
        if !matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("json") | Some("jsonl") | Some("md")
        ) {
            continue;
        }
        let contents = std::fs::read_to_string(&path).expect("read schema example");
        for needle in forbidden {
            assert!(
                !contents.contains(needle),
                "{} contains forbidden host-specific or placeholder value {needle:?}",
                path.display()
            );
        }
    }
}
