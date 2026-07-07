use std::process::Command;

#[test]
fn trap_synthetic_bench_runs_small_iteration_count() {
    let output = Command::new(env!("CARGO_BIN_EXE_trap_synthetic_bench"))
        .args(["--iterations", "8"])
        .output()
        .expect("run trap synthetic bench");

    assert!(
        output.status.success(),
        "bench failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("trap_synthetic_bench iterations=8"));
    assert!(stdout.contains("transitions=16"));
}
