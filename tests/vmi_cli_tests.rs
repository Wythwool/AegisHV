use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn fixture(name: &str) -> PathBuf {
    repo_root().join("tests/fixtures/vmi").join(name)
}

fn run_aegishv(args: &[String]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_aegishv"));
    command.current_dir(repo_root());
    command.args(args);
    command.output().expect("run aegishv")
}

fn translate_args(fixture: PathBuf, gva: &str, mode: &str) -> Vec<String> {
    vec![
        "vmi".to_string(),
        "translate".to_string(),
        "--fixture".to_string(),
        fixture.display().to_string(),
        "--gva".to_string(),
        gva.to_string(),
        "--mode".to_string(),
        mode.to_string(),
        "--json".to_string(),
    ]
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn assert_success(output: Output) -> String {
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    stdout(&output)
}

fn assert_failure(output: Output) -> String {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    stdout(&output)
}

#[test]
fn vmi_translate_cli_translates_x86_64_four_level_fixture() {
    let out = assert_success(run_aegishv(&translate_args(
        fixture("x86_64_basic.vmi"),
        "0x0",
        "x86_64-4level",
    )));

    assert!(out.contains("\"ok\":true"));
    assert!(out.contains("\"architecture\":\"x86_64\""));
    assert!(out.contains("\"mode\":\"x86_64-4level\""));
    assert!(out.contains("\"gva\":\"0x0\""));
    assert!(out.contains("\"gpa\":\"0x0\""));
    assert!(out.contains("\"page_size\":4096"));
    assert!(out.contains("\"readable\":true"));
    assert!(out.contains("\"writable\":true"));
    assert!(out.contains("\"executable\":true"));
    assert!(out.contains("\"user\":true"));
    assert!(out.contains("\"fixture_id\":\"x86_64-basic\""));
}

#[test]
fn vmi_translate_cli_translates_x86_64_la57_fixture() {
    let out = assert_success(run_aegishv(&translate_args(
        fixture("x86_64_basic.vmi"),
        "0xffff800000001000",
        "x86_64-la57",
    )));

    assert!(out.contains("\"ok\":true"));
    assert!(out.contains("\"mode\":\"x86_64-la57\""));
    assert!(out.contains("\"gva\":\"0xffff800000001000\""));
    assert!(out.contains("\"gpa\":\"0x1000\""));
    assert!(out.contains("\"page_size\":4096"));
}

#[test]
fn vmi_translate_cli_translates_arm64_stage1_fixture() {
    let out = assert_success(run_aegishv(&translate_args(
        fixture("arm64_basic.vmi"),
        "0x0",
        "arm64-stage1-4k",
    )));

    assert!(out.contains("\"ok\":true"));
    assert!(out.contains("\"architecture\":\"arm64\""));
    assert!(out.contains("\"mode\":\"arm64-stage1-4k\""));
    assert!(out.contains("\"gva\":\"0x0\""));
    assert!(out.contains("\"gpa\":\"0x0\""));
    assert!(out.contains("\"page_size\":4096"));
    assert!(out.contains("\"readable\":true"));
    assert!(out.contains("\"writable\":true"));
    assert!(out.contains("\"executable\":true"));
    assert!(out.contains("\"user\":true"));
}

#[test]
fn vmi_translate_cli_rejects_bad_arguments_and_malformed_inputs() {
    let bad_gva = assert_failure(run_aegishv(&translate_args(
        fixture("x86_64_basic.vmi"),
        "not-an-address",
        "x86_64-4level",
    )));
    assert!(bad_gva.contains("\"ok\":false"));
    assert!(bad_gva.contains("\"kind\":\"invalid_input\""));
    assert!(bad_gva.contains("invalid gva"));

    let missing_fixture = assert_failure(run_aegishv(&translate_args(
        fixture("missing.vmi"),
        "0x0",
        "x86_64-4level",
    )));
    assert!(missing_fixture.contains("\"kind\":\"temporarily_unavailable\""));

    let malformed_fixture = assert_failure(run_aegishv(&translate_args(
        fixture("malformed_header.vmi"),
        "0x0",
        "x86_64-4level",
    )));
    assert!(malformed_fixture.contains("\"kind\":\"malformed\""));

    let unsupported_mode = assert_failure(run_aegishv(&[
        "vmi".to_string(),
        "translate".to_string(),
        "--fixture".to_string(),
        fixture("x86_64_basic.vmi").display().to_string(),
        "--gva".to_string(),
        "0x0".to_string(),
        "--mode".to_string(),
        "riscv-stage1".to_string(),
        "--json".to_string(),
    ]));
    assert!(unsupported_mode.contains("\"kind\":\"unsupported_backend\""));

    let no_fixture = assert_failure(run_aegishv(&[
        "vmi".to_string(),
        "translate".to_string(),
        "--gva".to_string(),
        "0x0".to_string(),
        "--mode".to_string(),
        "x86_64-4level".to_string(),
        "--json".to_string(),
    ]));
    assert!(no_fixture.contains("\"kind\":\"invalid_input\""));
    assert!(no_fixture.contains("requires --fixture"));
}

#[test]
fn vmi_translate_cli_rejects_architecture_register_and_memory_mismatches() {
    let arch_mismatch = assert_failure(run_aegishv(&translate_args(
        fixture("arm64_basic.vmi"),
        "0x0",
        "x86_64-4level",
    )));
    assert!(arch_mismatch.contains("\"kind\":\"malformed\""));
    assert!(arch_mismatch.contains("does not match fixture architecture"));

    let missing_cr3 = assert_failure(run_aegishv(&translate_args(
        fixture("x86_64_missing_cr3.vmi"),
        "0x0",
        "x86_64-4level",
    )));
    assert!(missing_cr3.contains("\"kind\":\"invalid_input\""));
    assert!(missing_cr3.contains("cr3"));

    let unmapped = assert_failure(run_aegishv(&translate_args(
        fixture("x86_64_unmapped.vmi"),
        "0x0",
        "x86_64-4level",
    )));
    assert!(unmapped.contains("\"kind\":\"missing_memory\""));
}

#[test]
fn vmi_translate_cli_does_not_start_sensor_runtime_paths() {
    let output = run_aegishv(&translate_args(
        fixture("x86_64_basic.vmi"),
        "0x0",
        "x86_64-4level",
    ));
    let err = stderr(&output);
    let out = assert_success(output);

    assert!(err.is_empty());
    assert!(!out.contains("sensor_startup"));
    assert!(!out.contains("metrics"));
    assert!(!out.contains("tracefs"));
    assert!(!out.contains("qmp"));
}
