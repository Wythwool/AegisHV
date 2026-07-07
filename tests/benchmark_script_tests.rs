use std::fs;
use std::path::Path;

fn repo_file(rel: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn assert_contains_all(text: &str, required: &[&str]) {
    for item in required {
        assert!(text.contains(item), "missing required text: {item}");
    }
}

#[test]
fn benchmark_scripts_run_existing_paths_and_write_raw_outputs() {
    let trace = repo_file("scripts/bench-trace-ingest.sh");
    let wx = repo_file("scripts/bench-wx-state.sh");
    let vmi = repo_file("scripts/bench-vmi-translate.sh");
    let trap = repo_file("scripts/bench-trap-synthetic.sh");
    let performance = repo_file("docs/PERFORMANCE.md");
    let testing = repo_file("docs/TESTING.md");

    assert_contains_all(
        &trace,
        &[
            "examples/traces/kvm_exit_sample.log",
            "validate-jsonl-schema.py",
            "trace-ingest.csv",
            "cargo run --locked -- run --replay",
        ],
    );
    assert_contains_all(
        &wx,
        &[
            "corpus/malicious/wx_same_vm_same_as.log",
            "config.example.toml",
            "wx-state.csv",
            "\"category\":\"wx\"",
        ],
    );
    assert_contains_all(
        &vmi,
        &[
            "tests/fixtures/vmi/x86_64_basic.vmi",
            "x86_64-4level",
            "vmi translate",
            "vmi-translate.csv",
        ],
    );
    assert_contains_all(
        &trap,
        &["trap_synthetic_bench", "--iterations", "trap-synthetic.log"],
    );
    assert_contains_all(
        &performance,
        &[
            "Do not commit invented numbers",
            "scripts/bench-trace-ingest.sh",
            "scripts/bench-wx-state.sh",
            "scripts/bench-vmi-translate.sh",
            "scripts/bench-trap-synthetic.sh",
        ],
    );
    assert!(testing.contains("Benchmark helpers do not write checked-in result numbers"));
}

#[test]
fn live_kvm_script_is_opt_in_and_refuses_missing_host_gate() {
    let script = repo_file("scripts/live-kvm-integration.sh");
    let matrix = repo_file("docs/HARDWARE_TEST_MATRIX.md");

    assert_contains_all(
        &script,
        &[
            "AEGISHV_RUN_LIVE_KVM",
            "exit 77",
            "/dev/kvm is required",
            "scripts/live-tracefs-smoke.sh",
            "cargo metadata --locked",
        ],
    );
    assert_contains_all(
        &matrix,
        &[
            "Live Linux tracefs KVM smoke",
            "planned",
            "Manual runner evidence required before broad live claims",
            "Bare-metal type-1 boot",
            "unsupported",
        ],
    );
}

#[test]
fn synthetic_vmi_fixture_corpus_is_documented() {
    let linux_readme = repo_file("tests/fixtures/vmi/linux/README.md");
    let linux_profile = repo_file("tests/fixtures/vmi/linux/synthetic_task_module.profile");
    let windows_readme = repo_file("tests/fixtures/vmi/windows/README.md");
    let windows_cache = repo_file("tests/fixtures/vmi/windows/synthetic_callbacks.cache");

    assert_contains_all(
        &linux_profile,
        &[
            "aegishv-linux-profile-v1",
            "symbol=init_task",
            "symbol=modules",
            "syscall=59,execve",
        ],
    );
    assert_contains_all(
        &windows_cache,
        &[
            "aegishv-windows-symbol-cache-v1",
            "PspCreateProcessNotifyRoutine",
            "PspCreateThreadNotifyRoutine",
            "PspLoadImageNotifyRoutine",
        ],
    );
    assert!(linux_readme.contains("synthetic_task_module.profile"));
    assert!(windows_readme.contains("synthetic_callbacks.cache"));
}
