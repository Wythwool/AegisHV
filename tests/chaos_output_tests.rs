use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "aegishv-chaos-{label}-{}-{}",
        std::process::id(),
        aegishv::util::next_sequence()
    ));
    std::fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn run_aegishv(args: &[OsString]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_aegishv"));
    command.current_dir(repo_root());
    command.args(args);
    command.output().expect("run aegishv")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn replay_run_fails_when_jsonl_output_is_a_directory() {
    let dir = temp_dir("jsonl-directory");
    let replay = repo_root().join("examples/traces/kvm_exit_sample.log");
    let output = run_aegishv(&[
        "run".into(),
        "--replay".into(),
        replay.into_os_string(),
        "--jsonl".into(),
        dir.clone().into_os_string(),
        "--listen".into(),
        "".into(),
        "--quiet".into(),
    ]);

    assert!(
        !output.status.success(),
        "run unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let err = stderr(&output);
    assert!(err.contains("aegishv:"));
    assert!(err.contains("jsonl"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn action_dry_run_reports_dump_without_executing_qmp() {
    let config = repo_root().join("config.example.toml");
    let output = run_aegishv(&[
        "admin".into(),
        "action-dry-run".into(),
        "--config".into(),
        config.into_os_string(),
        "--kind".into(),
        "dump_guest_memory".into(),
        "--vm".into(),
        "vm-a".into(),
        "--json".into(),
    ]);

    assert!(
        output.status.success(),
        "dry-run failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let out = stdout(&output);
    assert!(out.contains("\"kind\":\"dump_guest_memory\""));
    assert!(out.contains("\"status\":\"dry_run\""));
    assert!(out.contains("\"decision\":\"dry_run\""));
    assert!(out.contains("action not executed"));
    assert!(out.contains("\"refused\":false"));
}
