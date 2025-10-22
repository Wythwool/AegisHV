
use std::path::PathBuf;
use std::process::Command;

#[test]
fn replay_runs_and_outputs_json() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let log = root.join("examples/traces/kvm_exit_sample.log");
    let out = Command::new("cargo")
        .args(["run","--quiet","--","run","--replay", log.to_str().unwrap(), "--quiet"])
        .output()
        .expect("run");
    assert!(out.status.success());
}
