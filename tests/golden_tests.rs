use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn golden_dir() -> PathBuf {
    repo_root().join("tests/fixtures/golden")
}

fn golden_fixture(name: &str) -> PathBuf {
    golden_dir().join(name)
}

fn temp_jsonl(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "aegishv-golden-{label}-{}-{}.jsonl",
        std::process::id(),
        aegishv::util::next_sequence()
    ))
}

fn run_aegishv(args: &[OsString]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_aegishv"));
    command.current_dir(repo_root());
    command.args(args);
    command.output().expect("run aegishv")
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
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn compare_generated_jsonl_to_fixture(label: &str, args: &[OsString], fixture: &Path) {
    let out = temp_jsonl(label);
    let mut full_args = args.to_vec();
    full_args.push("--jsonl".into());
    full_args.push(out.clone().into_os_string());
    full_args.push("--listen".into());
    full_args.push("".into());
    full_args.push("--quiet".into());

    assert_success(run_aegishv(&full_args));

    let generated = std::fs::read(&out).expect("read generated deterministic JSONL");
    let expected = std::fs::read(fixture).expect("read committed golden JSONL");
    assert_eq!(
        generated,
        expected,
        "deterministic replay output changed for {}",
        fixture.display()
    );
    let _ = std::fs::remove_file(out);
}

fn golden_jsonl_fixtures() -> Vec<PathBuf> {
    let mut fixtures = std::fs::read_dir(golden_dir())
        .expect("read golden fixture dir")
        .map(|entry| entry.expect("read golden fixture entry").path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    fixtures.sort();
    fixtures
}

fn validate_golden_hygiene(name: &str, contents: &str) -> Result<(), String> {
    for (idx, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if !line.contains("\"ts\":\"2026-01-01T00:00:00.000Z\"") {
            return Err(format!(
                "{name}: line {} does not use the frozen timestamp",
                idx + 1
            ));
        }
        if line.contains("C:\\")
            || line.contains("\\Users\\")
            || line.contains("/Users/")
            || line.contains("/home/")
            || line.contains("/tmp/")
            || line.contains("machine-id")
        {
            return Err(format!(
                "{name}: line {} contains host-specific text",
                idx + 1
            ));
        }
        if line.contains("\"host_id\":") && !line.contains("\"host_id\":\"deterministic-host\"") {
            return Err(format!(
                "{name}: line {} contains a non-deterministic host_id",
                idx + 1
            ));
        }
        if line.contains("\"sensor_id\":")
            && !line.contains("\"sensor_id\":\"deterministic-sensor\"")
        {
            return Err(format!(
                "{name}: line {} contains a non-deterministic sensor_id",
                idx + 1
            ));
        }
        if line.contains("\"tenant_id\":")
            && !line.contains("\"tenant_id\":\"deterministic-tenant\"")
        {
            return Err(format!(
                "{name}: line {} contains a non-deterministic tenant_id",
                idx + 1
            ));
        }
    }
    Ok(())
}

#[test]
fn deterministic_replay_matches_kvm_exit_golden_fixture() {
    compare_generated_jsonl_to_fixture(
        "kvm-exit",
        &[
            "run".into(),
            "--replay".into(),
            repo_root()
                .join("examples/traces/kvm_exit_sample.log")
                .into_os_string(),
            "--deterministic-replay".into(),
        ],
        &golden_fixture("replay_kvm_exit_sample.jsonl"),
    );
}

#[test]
fn deterministic_replay_matches_wx_policy_action_golden_fixture() {
    compare_generated_jsonl_to_fixture(
        "wx-policy-action",
        &[
            "run".into(),
            "--replay".into(),
            repo_root()
                .join("corpus/malicious/wx_same_vm_same_as.log")
                .into_os_string(),
            "--deterministic-replay".into(),
            "--config".into(),
            repo_root().join("config.example.toml").into_os_string(),
        ],
        &golden_fixture("replay_wx_policy_action_sample.jsonl"),
    );
}

#[test]
fn committed_golden_jsonl_fixtures_validate_against_event_schema() {
    let fixtures = golden_jsonl_fixtures();
    assert_eq!(fixtures.len(), 3, "unexpected golden fixture count");
    for fixture in fixtures {
        assert_success(run_validator(&[
            "--schema".into(),
            repo_root()
                .join("schema/event.schema.json")
                .into_os_string(),
            "--jsonl".into(),
            fixture.into_os_string(),
        ]));
    }
}

#[test]
fn golden_fixtures_cover_expected_event_contracts_without_host_specific_values() {
    let mut combined = String::new();
    for fixture in golden_jsonl_fixtures() {
        let contents = std::fs::read_to_string(&fixture).expect("read golden fixture");
        validate_golden_hygiene(&fixture.display().to_string(), &contents)
            .expect("golden fixture uses deterministic values");
        combined.push_str(&contents);
    }

    for expected in [
        "\"category\":\"exit\"",
        "\"category\":\"wx\"",
        "\"category\":\"pmu\"",
        "\"category\":\"policy\"",
        "\"category\":\"sensor\"",
        "\"action\":{",
        "\"identity\":{",
        "\"loss\":{",
        "\"data_loss\":true",
    ] {
        assert!(
            combined.contains(expected),
            "missing golden coverage for {expected}"
        );
    }
}

#[test]
fn golden_hygiene_guard_rejects_host_specific_paths() {
    let err = validate_golden_hygiene(
        "bad.jsonl",
        r#"{"version":1,"schema_version":2,"ts":"2026-01-01T00:00:00.000Z","monotonic_ms":0,"sequence":1,"event_id":"evt-deterministic-0000000000000001","host_id":"C:\Users\User","category":"sensor","severity":"info","vm":"host","data_loss":false}"#,
    )
    .expect_err("host-specific text must be rejected");

    assert!(err.contains("host-specific") || err.contains("host_id"));
}
