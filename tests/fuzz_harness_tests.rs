use std::fs;
use std::path::{Path, PathBuf};

use aegishv::config::Config;

const QMP_REFUSAL_SEEDS: &[(&str, u8, &str)] = &[
    ("missing_identity.seed", 0, "missing_identity"),
    ("pid_only_identity.seed", 1, "pid_only_identity"),
    ("ambiguous_identity.seed", 2, "ambiguous_identity"),
    ("unverified_identity.seed", 3, "unverified_identity"),
    ("stale_identity.seed", 4, "stale_identity"),
    ("conflicting_identity.seed", 5, "conflicting_identity"),
    ("low_confidence.seed", 6, "low_confidence"),
];

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read_repo_file(rel: &str) -> String {
    fs::read_to_string(repo_root().join(rel)).unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

fn list_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        for entry in
            fs::read_dir(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
        {
            let entry =
                entry.unwrap_or_else(|err| panic!("read entry under {}: {err}", path.display()));
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
}

fn is_generated_fuzz_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("target" | "artifacts" | "coverage")
        )
    })
}

fn qmp_seed_reason(selector: u8) -> &'static str {
    match selector % 7 {
        0 => "missing_identity",
        1 => "pid_only_identity",
        2 => "ambiguous_identity",
        3 => "unverified_identity",
        4 => "stale_identity",
        5 => "conflicting_identity",
        6 => "low_confidence",
        _ => unreachable!("selector reduced by modulo 7"),
    }
}

fn qmp_seed_execute_true(data: &[u8]) -> bool {
    data.get(3).map_or(true, |byte| byte & 1 == 0)
}

#[test]
fn fuzz_manifest_declares_cargo_fuzz_targets_without_main_crate_dependency_churn() {
    let root_manifest = read_repo_file("Cargo.toml");
    let fuzz_manifest = read_repo_file("fuzz/Cargo.toml");

    for required in [
        "name = \"aegishv-fuzz\"",
        "publish = false",
        "edition = \"2021\"",
        "cargo-fuzz = true",
        "aegishv = { path = \"..\" }",
        "libfuzzer-sys = \"0.4\"",
        "name = \"trace_parser_line\"",
        "path = \"fuzz_targets/trace_parser_line.rs\"",
        "name = \"config_input\"",
        "path = \"fuzz_targets/config_input.rs\"",
        "name = \"trace_format_metadata\"",
        "path = \"fuzz_targets/trace_format_metadata.rs\"",
        "name = \"qmp_action_safety\"",
        "path = \"fuzz_targets/qmp_action_safety.rs\"",
        "test = false",
        "doc = false",
        "bench = false",
    ] {
        assert!(
            fuzz_manifest.contains(required),
            "fuzz/Cargo.toml is missing cargo-fuzz wiring: {required}"
        );
    }

    assert!(
        !root_manifest.contains("libfuzzer-sys"),
        "libfuzzer-sys must stay scoped to fuzz/Cargo.toml"
    );
}

#[test]
fn fuzz_targets_exercise_parser_config_and_trace_format_surfaces() {
    let trace_parser = read_repo_file("fuzz/fuzz_targets/trace_parser_line.rs");
    let config = read_repo_file("fuzz/fuzz_targets/config_input.rs");
    let trace_format = read_repo_file("fuzz/fuzz_targets/trace_format_metadata.rs");
    let qmp_action_safety = read_repo_file("fuzz/fuzz_targets/qmp_action_safety.rs");

    for required in [
        "#![no_main]",
        "fuzz_target!",
        "parse_line(line)",
        "classify_exit(&parsed)",
        "is_parser_degraded(&parsed)",
        "parsed_gpa_page(&parsed, 4096)",
        "MAX_LINE_BYTES",
    ] {
        assert!(
            trace_parser.contains(required),
            "trace parser fuzz target is missing expected parser coverage: {required}"
        );
    }

    for required in [
        "#![no_main]",
        "fuzz_target!",
        "Config::load(Some(&path))",
        "std::env::temp_dir()",
        "std::fs::remove_file(path)",
        "MAX_CONFIG_BYTES",
    ] {
        assert!(
            config.contains(required),
            "config fuzz target is missing expected config coverage: {required}"
        );
    }

    for required in [
        "#![no_main]",
        "fuzz_target!",
        "parse_tracepoint_format(\"kvm\", \"kvm_exit\", text)",
        "format.has_field(\"vcpu_id\")",
        "format.has_field(\"exit_reason\")",
        "format.has_field(\"guest_rip\")",
        "MAX_FORMAT_BYTES",
    ] {
        assert!(
            trace_format.contains(required),
            "trace format fuzz target is missing expected trace metadata coverage: {required}"
        );
    }

    for required in [
        "#![no_main]",
        "fuzz_target!",
        "ActionDispatcher::new(&cfg)",
        "QMP_REFUSAL_CASES: u8 = 7",
        "QmpMapping",
        "qmp-fuzz-socket",
        "run_action(",
        "\"missing_identity\"",
        "\"pid_only_identity\"",
        "\"ambiguous_identity\"",
        "\"unverified_identity\"",
        "\"stale_identity\"",
        "\"conflicting_identity\"",
        "\"low_confidence\"",
        "identity_conflict:stale_cache",
        "identity_conflict:qmp_socket_mismatch",
        "Metrics::new()",
        "event.to_json()",
        "metrics.encode()",
    ] {
        assert!(
            qmp_action_safety.contains(required),
            "QMP action-safety fuzz target is missing expected refusal coverage: {required}"
        );
    }
}

#[test]
fn qmp_action_safety_selector_table_covers_all_refusal_reasons() {
    let target = read_repo_file("fuzz/fuzz_targets/qmp_action_safety.rs");

    for (_, selector, reason) in QMP_REFUSAL_SEEDS {
        assert!(
            target.contains(&format!("{reason}\"")),
            "QMP action-safety target is missing refusal reason {reason}"
        );
        assert_eq!(
            qmp_seed_reason(*selector),
            *reason,
            "test selector mapping must match harness refusal reason"
        );
    }

    for required in [
        "selector % QMP_REFUSAL_CASES",
        "MISSING_IDENTITY_SELECTOR",
        "PID_ONLY_IDENTITY_SELECTOR",
        "AMBIGUOUS_IDENTITY_SELECTOR",
        "UNVERIFIED_IDENTITY_SELECTOR",
        "STALE_IDENTITY_SELECTOR",
        "CONFLICTING_IDENTITY_SELECTOR",
        "LOW_CONFIDENCE_SELECTOR",
    ] {
        assert!(
            target.contains(required),
            "QMP action-safety selector table is missing readable mapping entry: {required}"
        );
    }

    let low_idx = target
        .find("\"low_confidence\"")
        .expect("low_confidence case must be present");
    let low_end = target.len().min(low_idx + 700);
    let low_case = &target[low_idx..low_end];
    for required in [
        "IDENTITY_SOURCE_LIBVIRT_XML",
        "IDENTITY_SOURCE_START_TIME_VERIFIED",
        "confidence: IdentityConfidence::Medium",
        "start_time_verified: true",
        "ambiguous: false",
    ] {
        assert!(
            low_case.contains(required),
            "low_confidence fuzz case must use stable verified identity and medium confidence: {required}"
        );
    }
    assert!(
        !low_case.contains("IDENTITY_SOURCE_FALLBACK_PID"),
        "low_confidence fuzz case must not be represented by PID-only fallback"
    );
}

#[test]
fn fuzz_seed_corpus_is_present_and_small() {
    let corpus_root = repo_root().join("fuzz/corpus");
    let expected_dirs = [
        "trace_parser_line",
        "config_input",
        "trace_format_metadata",
        "qmp_action_safety",
    ];

    for dir in expected_dirs {
        let path = corpus_root.join(dir);
        assert!(
            path.is_dir(),
            "missing fuzz corpus directory {}",
            path.display()
        );
        let seeds = fs::read_dir(&path)
            .unwrap_or_else(|err| panic!("read corpus {}: {err}", path.display()))
            .count();
        assert!(
            seeds > 0,
            "fuzz corpus {} has no seed files",
            path.display()
        );
    }

    for file in list_files(&corpus_root) {
        let metadata =
            fs::metadata(&file).unwrap_or_else(|err| panic!("stat {}: {err}", file.display()));
        assert!(
            metadata.len() <= 4096,
            "fuzz seed {} is too large for a committed seed corpus",
            file.display()
        );
    }
}

#[test]
fn qmp_action_safety_seed_corpus_maps_to_named_refusal_reasons() {
    let qmp_corpus = repo_root().join("fuzz/corpus/qmp_action_safety");
    let mut names = fs::read_dir(&qmp_corpus)
        .unwrap_or_else(|err| panic!("read QMP corpus {}: {err}", qmp_corpus.display()))
        .map(|entry| {
            entry
                .unwrap_or_else(|err| panic!("read QMP corpus entry: {err}"))
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    names.sort();

    let mut expected_names = QMP_REFUSAL_SEEDS
        .iter()
        .map(|(name, _, _)| (*name).to_string())
        .collect::<Vec<_>>();
    expected_names.sort();

    assert_eq!(
        names, expected_names,
        "QMP action-safety corpus must have exactly one .seed file per refusal reason"
    );

    for (name, expected_selector, expected_reason) in QMP_REFUSAL_SEEDS {
        let path = qmp_corpus.join(name);
        let data =
            fs::read(&path).unwrap_or_else(|err| panic!("read QMP seed {}: {err}", path.display()));
        assert!(
            data.len() >= 4,
            "QMP seed {} must include selector, action selector, VM bytes, and execute byte",
            path.display()
        );
        assert_eq!(
            data[0] % 7,
            *expected_selector,
            "QMP seed {} selects the wrong refusal case",
            name
        );
        assert_eq!(
            qmp_seed_reason(data[0]),
            *expected_reason,
            "QMP seed {} maps to the wrong refusal reason",
            name
        );
        assert!(
            qmp_seed_execute_true(&data),
            "QMP seed {} must execute the action path, not dry-run",
            name
        );
        assert_eq!(
            path.extension().and_then(|value| value.to_str()),
            Some("seed"),
            "QMP seed {} must use .seed to avoid the root *.bin ignore rule",
            name
        );
    }

    let gitignore = read_repo_file(".gitignore");
    let bin_unignored = gitignore.contains("!fuzz/corpus/qmp_action_safety/*.bin")
        || gitignore.contains("!fuzz/corpus/qmp_action_safety/**/*.bin");
    for file in list_files(&qmp_corpus) {
        if file.extension().and_then(|value| value.to_str()) == Some("bin") {
            assert!(
                bin_unignored,
                "QMP .bin corpus file {} would be ignored by the root *.bin rule",
                file.display()
            );
        }
    }
}

#[test]
fn config_minimal_seed_is_valid_aegishv_config() {
    let path = repo_root().join("fuzz/corpus/config_input/minimal.toml");
    Config::load(Some(&path)).unwrap_or_else(|err| {
        panic!(
            "fuzz config minimal seed must be accepted by Config::load: {}",
            err
        )
    });
}

#[test]
fn fuzz_docs_explain_local_runs_and_do_not_claim_results() {
    let fuzz_readme = read_repo_file("fuzz/README.md");
    let testing = read_repo_file("docs/TESTING.md");
    let combined = format!("{fuzz_readme}\n{testing}");

    for required in [
        "cargo install cargo-fuzz",
        "cargo fuzz run trace_parser_line -- -max_total_time=60",
        "cargo fuzz run config_input -- -max_total_time=60",
        "cargo fuzz run trace_format_metadata -- -max_total_time=60",
        "cargo fuzz run qmp_action_safety -- -max_total_time=60",
        "Normal PR tests do not run cargo-fuzz campaigns",
        "cargo check --manifest-path fuzz/Cargo.toml --bins",
        "The Python JSON Schema validator is not fuzzed",
        "do not require live KVM",
        "It does not connect to a QMP socket",
        "do not prove vulnerability absence",
        "type-1 support",
        "hardware PMU support",
    ] {
        assert!(
            combined.contains(required),
            "fuzz docs are missing required scope or run guidance: {required}"
        );
    }

    for forbidden in [
        "fuzzing found",
        "bugs found by fuzzing",
        "proved safe",
        "proves safety",
        "complete fuzz coverage",
        "vulnerability-free",
        "all parser bugs",
        "CI runs cargo-fuzz campaigns",
        "normal PR CI runs cargo fuzz",
    ] {
        assert!(
            !combined.contains(forbidden),
            "fuzz docs contain fake or unsupported fuzzing claim: {forbidden}"
        );
    }
}

#[test]
fn normal_ci_does_not_run_cargo_fuzz_campaigns() {
    let workflows = repo_root().join(".github/workflows");
    let ci = read_repo_file(".github/workflows/ci.yml");

    for required in [
        "name: fuzz harness compile check",
        "cargo metadata --manifest-path fuzz/Cargo.toml --no-deps --format-version 1",
        "cargo check --manifest-path fuzz/Cargo.toml --bins",
        "Compile fuzz harnesses without running campaigns",
    ] {
        assert!(
            ci.contains(required),
            "CI is missing bounded fuzz harness smoke compile/check: {required}"
        );
    }

    for file in list_files(&workflows) {
        let contents = fs::read_to_string(&file)
            .unwrap_or_else(|err| panic!("read {}: {err}", file.display()));
        for forbidden in [
            "cargo fuzz run",
            "cargo-fuzz",
            "cargo install cargo-fuzz",
            "-max_total_time",
            "-runs=",
        ] {
            assert!(
                !contents.contains(forbidden),
                "workflow {} must not run cargo-fuzz campaigns in normal CI: {forbidden}",
                file.display()
            );
        }
    }

    for forbidden in ["live-tracefs-smoke.sh", "secrets."] {
        assert!(
            !ci.contains(forbidden),
            "normal CI fuzz harness smoke must not require host-only checks or secrets: {forbidden}"
        );
    }
}

#[test]
fn fuzz_files_avoid_local_paths_secrets_and_unsupported_security_claims() {
    let fuzz_root = repo_root().join("fuzz");
    for file in list_files(&fuzz_root) {
        if is_generated_fuzz_path(&file) {
            continue;
        }
        let contents = fs::read_to_string(&file)
            .unwrap_or_else(|err| panic!("read {}: {err}", file.display()));
        for forbidden in [
            "C:\\Users",
            "\\Users\\",
            "/Users/",
            "/home/",
            "BEGIN PRIVATE KEY",
            "PRIVATE KEY",
            "password=",
            "token=",
            "type-1 fuzz coverage",
            "VMI fuzz coverage",
            "EPT/NPT enforcement fuzz coverage",
            "syscall integrity fuzz coverage",
            "hardware PMU fuzz coverage",
            "production fuzz coverage",
        ] {
            assert!(
                !contents.contains(forbidden),
                "{} contains a forbidden local artifact, secret marker, or fake claim: {forbidden}",
                file.display()
            );
        }
    }
}
