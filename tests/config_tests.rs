use aegishv::config::{Config, SpoolCompression};
use aegishv::event::IdentityConfidence;
use std::io::Write;

fn temp_config(contents: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "aegishv-config-test-{}-{}.toml",
        std::process::id(),
        aegishv::util::next_sequence()
    ));
    let mut f = std::fs::File::create(&path).unwrap();
    write!(f, "{}", contents).unwrap();
    path
}

fn slash_path(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[test]
fn rejects_invalid_policy_regex() {
    let path = temp_config(
        r#"
[[policy.rules]]
name = "bad"
action = { kind = "pause_vm" }
match = { category = "wx", reason_regex = "(" }
"#,
    );
    let err = Config::load(Some(&path)).expect_err("invalid regex must fail startup");
    assert!(err.contains("invalid reason_regex"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn clamps_general_values() {
    let path = temp_config(
        r#"
[general]
wx_window_ms = 0
wx_cooldown_ms = 999999999
wx_max_pages = 1
page_size = 1234
flush_every = 0
"#,
    );
    let cfg = Config::load(Some(&path)).unwrap();
    assert_eq!(cfg.general.wx_window_ms, 1);
    assert_eq!(cfg.general.page_size, 4096);
    assert!(cfg.general.wx_max_pages >= 1024);
    assert_eq!(cfg.general.flush_every, 1);
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_invalid_policy_severity() {
    let path = temp_config(
        r#"
[[policy.rules]]
name = "bad-severity"
action = { kind = "pause_vm" }
match = { category = "wx", severity_at_least = "urgent" }
"#,
    );
    let err = Config::load(Some(&path)).expect_err("invalid severity must fail startup");
    assert!(err.contains("invalid severity"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_dump_action_without_output_path() {
    let path = temp_config(
        r#"
[[policy.rules]]
name = "bad-dump"
action = { kind = "dump_guest_memory" }
match = { category = "wx", severity_at_least = "high" }
"#,
    );
    let err = Config::load(Some(&path)).expect_err("bad dump action must fail startup");
    assert!(err.contains("dump_guest_memory requires action.output_path"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_invalid_stable_qmp_match_bool() {
    let path = temp_config(
        r#"
[identity]
require_stable_qmp_match = maybe
"#,
    );
    let err =
        Config::load(Some(&path)).expect_err("invalid stable QMP match flag must fail startup");
    assert!(err.contains("invalid identity.require_stable_qmp_match"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn accepts_medium_action_identity_confidence_threshold() {
    let path = temp_config(
        r#"
[identity]
min_action_confidence = "medium"
"#,
    );
    let cfg = Config::load(Some(&path)).expect("medium action confidence threshold must load");
    assert_eq!(
        cfg.identity.min_action_confidence,
        IdentityConfidence::Medium
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_low_action_identity_confidence_threshold() {
    let path = temp_config(
        r#"
[identity]
min_action_confidence = "low"
"#,
    );
    let err = Config::load(Some(&path))
        .expect_err("low confidence threshold must not authorize QMP actions");
    assert!(err.contains("low confidence cannot authorize QMP actions"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn accepts_configured_pmu_rediscover_interval() {
    let path = temp_config(
        r#"
[pmu]
rediscover_ms = 2500
"#,
    );
    let cfg = Config::load(Some(&path)).expect("valid PMU rediscovery interval must load");
    assert_eq!(cfg.pmu.rediscover_ms, 2500);
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_pmu_rediscover_interval_below_minimum() {
    let path = temp_config(
        r#"
[pmu]
rediscover_ms = 999
"#,
    );
    let err = Config::load(Some(&path)).expect_err("too-small PMU rediscovery interval must fail");
    assert!(err.contains("invalid pmu.rediscover_ms"));
    assert!(err.contains("expected 1000..=3600000 milliseconds"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_pmu_rediscover_interval_above_maximum() {
    let path = temp_config(
        r#"
[pmu]
rediscover_ms = 3600001
"#,
    );
    let err = Config::load(Some(&path)).expect_err("too-large PMU rediscovery interval must fail");
    assert!(err.contains("invalid pmu.rediscover_ms"));
    assert!(err.contains("expected 1000..=3600000 milliseconds"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_malformed_pmu_rediscover_interval() {
    let path = temp_config(
        r#"
[pmu]
rediscover_ms = soon
"#,
    );
    let err = Config::load(Some(&path)).expect_err("malformed PMU rediscovery interval must fail");
    assert!(err.contains("invalid pmu.rediscover_ms"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_malformed_wx_cooldown_interval() {
    let path = temp_config(
        r#"
[general]
wx_cooldown_ms = soon
"#,
    );
    let err = Config::load(Some(&path)).expect_err("malformed W^X cooldown interval must fail");
    assert!(err.contains("invalid wx_cooldown_ms"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn accepts_explicit_metrics_bind_failure_degraded_mode() {
    let path = temp_config(
        r#"
[metrics]
allow_bind_failure = true
"#,
    );
    let cfg = Config::load(Some(&path)).expect("valid metrics bind failure policy must load");
    assert!(cfg.metrics.allow_bind_failure);
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_malformed_metrics_bind_failure_policy() {
    let path = temp_config(
        r#"
[metrics]
allow_bind_failure = maybe
"#,
    );
    let err = Config::load(Some(&path))
        .expect_err("malformed metrics bind failure policy must fail startup");
    assert!(err.contains("invalid metrics.allow_bind_failure"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn accepts_detector_scheduler_policy() {
    let path = temp_config(
        r#"
[detectors]
enable = true
default_budget_ms = 75
default_max_findings = 256
state_file = "/var/lib/aegishv/detectors.state"

[[detectors.rules]]
id = "kernel_text_tamper"
enabled = true
budget_ms = 40
max_findings = 32

[[detectors.rules]]
id = "hidden_process"
enabled = false
max_findings = 16
"#,
    );
    let cfg = Config::load(Some(&path)).expect("valid detector scheduler policy must load");

    assert!(cfg.detectors.enable);
    assert_eq!(cfg.detectors.default_budget_ms, 75);
    assert_eq!(cfg.detectors.default_max_findings, 256);
    assert_eq!(cfg.detectors.rules.len(), 2);
    assert_eq!(cfg.detectors.rules[0].id, "kernel_text_tamper");
    assert_eq!(cfg.detectors.rules[0].budget_ms, Some(40));
    assert_eq!(cfg.detectors.rules[0].max_findings, Some(32));
    assert_eq!(cfg.detectors.rules[1].id, "hidden_process");
    assert!(!cfg.detectors.rules[1].enabled);
    assert_eq!(cfg.detectors.rules[1].budget_ms, None);
    assert_eq!(cfg.detectors.rules[1].max_findings, Some(16));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_detector_duplicate_rule_id() {
    let path = temp_config(
        r#"
[[detectors.rules]]
id = "syscall_hook"

[[detectors.rules]]
id = "syscall_hook"
"#,
    );
    let err = Config::load(Some(&path)).expect_err("duplicate detector ids must fail");
    assert!(err.contains("duplicate detectors.rules id 'syscall_hook'"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_detector_rule_missing_id() {
    let path = temp_config(
        r#"
[[detectors.rules]]
enabled = true
"#,
    );
    let err = Config::load(Some(&path)).expect_err("detector rules need stable ids");
    assert!(err.contains("invalid detectors.rules id"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_detector_budget_out_of_range() {
    let path = temp_config(
        r#"
[detectors]
default_budget_ms = 60001
"#,
    );
    let err = Config::load(Some(&path)).expect_err("oversized detector budget must fail");
    assert!(err.contains("invalid detectors.default_budget_ms"));
    assert!(err.contains("expected 1..=60000 milliseconds"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_detector_rule_budget_out_of_range() {
    let path = temp_config(
        r#"
[[detectors.rules]]
id = "rwx_mapping"
budget_ms = 0
"#,
    );
    let err = Config::load(Some(&path)).expect_err("zero detector budget must fail");
    assert!(err.contains("invalid detectors.rules 'rwx_mapping' budget_ms"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn accepts_synthetic_trap_engine_policy() {
    let path = temp_config(
        r#"
[trap]
enable = true
backend = "synthetic"
storm_window_ms = 250
storm_max_hits = 32
storm_mode = "fail_open"
jit_window_ms = 7
jit_max_pages = 4
"#,
    );
    let cfg = Config::load(Some(&path)).expect("valid trap policy must load");

    assert!(cfg.trap.enable);
    assert_eq!(cfg.trap.backend, "synthetic");
    assert_eq!(cfg.trap.storm_window_ms, 250);
    assert_eq!(cfg.trap.storm_max_hits, 32);
    assert_eq!(cfg.trap.storm_mode, "fail_open");
    assert_eq!(cfg.trap.jit_window_ms, 7);
    assert_eq!(cfg.trap.jit_max_pages, 4);
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_trap_hardware_backend_until_runtime_exists() {
    let path = temp_config(
        r#"
[trap]
backend = "intel_ept"
"#,
    );
    let err = Config::load(Some(&path)).expect_err("unsupported trap backend must fail");
    assert!(err.contains("invalid trap.backend"));
    assert!(err.contains("only \"synthetic\" is supported"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_trap_storm_mode_outside_fail_policy() {
    let path = temp_config(
        r#"
[trap]
storm_mode = "ignore"
"#,
    );
    let err = Config::load(Some(&path)).expect_err("bad trap storm mode must fail");
    assert!(err.contains("invalid trap.storm_mode"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_trap_jit_window_out_of_range() {
    let path = temp_config(
        r#"
[trap]
jit_window_ms = 0
"#,
    );
    let err = Config::load(Some(&path)).expect_err("zero trap JIT window must fail");
    assert!(err.contains("invalid trap.jit_window_ms"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn accepts_disabled_spool_defaults_and_explicit_limits() {
    let path = temp_config(
        r#"
[spool]
enable = false
dir = "/var/lib/aegishv/spool"
max_bytes = 1048576
segment_bytes = 65536
"#,
    );
    let cfg = Config::load(Some(&path)).expect("valid spool config must load");
    assert!(!cfg.spool.enable);
    assert_eq!(cfg.spool.dir, "/var/lib/aegishv/spool");
    assert_eq!(cfg.spool.max_bytes, 1_048_576);
    assert_eq!(cfg.spool.segment_bytes, 65_536);
    assert_eq!(cfg.spool.compression, SpoolCompression::None);
    let _ = std::fs::remove_file(path);
}

#[test]
fn accepts_spool_rle_compression() {
    let path = temp_config(
        r#"
[spool]
enable = true
dir = "/var/lib/aegishv/spool"
max_bytes = 1048576
segment_bytes = 65536
compression = "rle"
"#,
    );
    let cfg = Config::load(Some(&path)).expect("valid compressed spool config must load");
    assert_eq!(cfg.spool.compression, SpoolCompression::Rle);
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_enabled_spool_without_directory() {
    let path = temp_config(
        r#"
[spool]
enable = true
dir = ""
"#,
    );
    let err = Config::load(Some(&path)).expect_err("enabled spool needs a directory");
    assert!(err.contains("invalid spool.dir"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_spool_segment_larger_than_limit() {
    let path = temp_config(
        r#"
[spool]
max_bytes = 8192
segment_bytes = 16384
"#,
    );
    let err = Config::load(Some(&path)).expect_err("oversized spool segment must fail");
    assert!(err.contains("invalid spool.segment_bytes"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_malformed_spool_enable_flag() {
    let path = temp_config(
        r#"
[spool]
enable = maybe
"#,
    );
    let err = Config::load(Some(&path)).expect_err("malformed spool enable flag must fail");
    assert!(err.contains("invalid spool.enable"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_unknown_spool_compression() {
    let path = temp_config(
        r#"
[spool]
compression = "zstd"
"#,
    );
    let err = Config::load(Some(&path)).expect_err("unknown spool compression must fail");
    assert!(err.contains("invalid spool.compression"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn accepts_configured_syslog_udp_sink() {
    let path = temp_config(
        r#"
[syslog]
enable = true
address = "127.0.0.1:5514"
facility = "local4"
max_message_bytes = 4096
"#,
    );
    let cfg = Config::load(Some(&path)).expect("valid syslog config must load");
    assert!(cfg.syslog.enable);
    assert_eq!(cfg.syslog.address, "127.0.0.1:5514");
    assert_eq!(cfg.syslog.facility, "local4");
    assert_eq!(cfg.syslog.max_message_bytes, 4096);
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_enabled_syslog_without_address() {
    let path = temp_config(
        r#"
[syslog]
enable = true
address = ""
"#,
    );
    let err = Config::load(Some(&path)).expect_err("enabled syslog needs a target");
    assert!(err.contains("invalid syslog.address"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_malformed_syslog_address() {
    let path = temp_config(
        r#"
[syslog]
enable = true
address = "localhost"
"#,
    );
    let err = Config::load(Some(&path)).expect_err("syslog address must be numeric ip:port");
    assert!(err.contains("invalid syslog.address"));
    assert!(err.contains("expected numeric ip:port"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_unknown_syslog_facility() {
    let path = temp_config(
        r#"
[syslog]
facility = "mail"
"#,
    );
    let err = Config::load(Some(&path)).expect_err("unsupported syslog facility must fail");
    assert!(err.contains("invalid syslog.facility"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_malformed_syslog_enable_flag() {
    let path = temp_config(
        r#"
[syslog]
enable = maybe
"#,
    );
    let err = Config::load(Some(&path)).expect_err("malformed syslog enable flag must fail");
    assert!(err.contains("invalid syslog.enable"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn accepts_configured_journald_sink() {
    let path = temp_config(
        r#"
[journald]
enable = true
socket = "/run/systemd/journal/socket"
identifier = "aegishv-test"
max_message_bytes = 4096
"#,
    );
    let cfg = Config::load(Some(&path)).expect("valid journald config must load");
    assert!(cfg.journald.enable);
    assert_eq!(cfg.journald.socket, "/run/systemd/journal/socket");
    assert_eq!(cfg.journald.identifier, "aegishv-test");
    assert_eq!(cfg.journald.max_message_bytes, 4096);
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_enabled_journald_without_socket() {
    let path = temp_config(
        r#"
[journald]
enable = true
socket = ""
"#,
    );
    let err = Config::load(Some(&path)).expect_err("enabled journald needs a socket path");
    assert!(err.contains("invalid journald.socket"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_invalid_journald_identifier() {
    let path = temp_config(
        r#"
[journald]
identifier = "aegishv test"
"#,
    );
    let err = Config::load(Some(&path)).expect_err("unsafe journald identifier must fail");
    assert!(err.contains("invalid journald.identifier"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_journald_message_limit_below_minimum() {
    let path = temp_config(
        r#"
[journald]
max_message_bytes = 511
"#,
    );
    let err = Config::load(Some(&path)).expect_err("too-small journald message limit must fail");
    assert!(err.contains("invalid journald.max_message_bytes"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_malformed_journald_enable_flag() {
    let path = temp_config(
        r#"
[journald]
enable = maybe
"#,
    );
    let err = Config::load(Some(&path)).expect_err("malformed journald enable flag must fail");
    assert!(err.contains("invalid journald.enable"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn accepts_strict_config_subset_for_arrays_and_action_tables() {
    let path = temp_config(
        r#"
[allow]
ignore_vm = ["builder", "ci-runner"]

[identity]
qmp_socket_dirs = ["/run/libvirt/qemu", "/run/qemu"]

[[policy.rules]]
name = "strict-valid"
id = "strict-valid"
mode = "dry_run"
actions = [{ kind = "noop" }, { kind = "manual_approval" }]
match = { category = "wx", reason_regex = "W\\^X" }
"#,
    );
    let cfg = Config::load(Some(&path)).expect("valid strict config subset must load");
    assert_eq!(cfg.allow.ignore_vm, ["builder", "ci-runner"]);
    assert_eq!(
        cfg.identity.qmp_socket_dirs,
        ["/run/libvirt/qemu", "/run/qemu"]
    );
    assert_eq!(cfg.policy.rules[0].actions.len(), 2);
    let _ = std::fs::remove_file(path);
}

#[test]
fn accepts_existing_libvirt_xml_discovery_dir() {
    let dir = slash_path(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libvirt"),
    );
    let path = temp_config(&format!(
        r#"
[identity]
libvirt_xml_dir = "{dir}"
"#
    ));

    let cfg = Config::load(Some(&path)).expect("existing libvirt XML fixture dir must load");

    assert_eq!(cfg.identity.libvirt_xml_dir, dir);
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_missing_libvirt_xml_discovery_dir() {
    let missing = slash_path(
        &std::env::temp_dir().join(format!("aegishv-missing-libvirt-{}", std::process::id())),
    );
    let path = temp_config(&format!(
        r#"
[identity]
libvirt_xml_dir = "{missing}"
"#
    ));

    let err = Config::load(Some(&path)).expect_err("missing libvirt XML dir must fail");

    assert!(err.contains("invalid identity.libvirt_xml_dir"));
    assert!(err.contains("expected an existing directory"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_malformed_string_array_with_line_number() {
    let path = temp_config(
        r#"
[allow]
ignore_vm = "builder"
"#,
    );
    let err = Config::load(Some(&path)).expect_err("missing array brackets must fail");
    assert!(err.contains("line 3"));
    assert!(err.contains("expected string array"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_malformed_actions_array_with_line_number() {
    let path = temp_config(
        r#"
[[policy.rules]]
name = "bad-actions"
actions = [{ kind = "pause_vm" } { kind = "noop" }]
"#,
    );
    let err = Config::load(Some(&path)).expect_err("missing action-array comma must fail");
    assert!(err.contains("line 4"));
    assert!(err.contains("expected comma between actions array entries"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_malformed_action_table_with_line_number() {
    let path = temp_config(
        r#"
[[policy.rules]]
name = "bad-action"
action = { kind = "pause_vm", }
"#,
    );
    let err = Config::load(Some(&path)).expect_err("trailing inline table comma must fail");
    assert!(err.contains("line 4"));
    assert!(err.contains("trailing comma in inline table"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_duplicate_config_key_with_line_number() {
    let path = temp_config(
        r#"
[identity]
require_stable_qmp_match = true
require_stable_qmp_match = false
"#,
    );
    let err = Config::load(Some(&path)).expect_err("duplicate identity key must fail");
    assert!(err.contains("line 4"));
    assert!(err.contains("duplicate key 'require_stable_qmp_match'"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_duplicate_inline_action_key_with_line_number() {
    let path = temp_config(
        r#"
[[policy.rules]]
name = "bad-inline-duplicate"
action = { kind = "pause_vm", kind = "noop" }
"#,
    );
    let err = Config::load(Some(&path)).expect_err("duplicate inline action key must fail");
    assert!(err.contains("line 4"));
    assert!(err.contains("duplicate inline key 'kind'"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn rejects_unsupported_section_syntax_with_line_number() {
    let path = temp_config(
        r#"
[general]]
quiet = true
"#,
    );
    let err = Config::load(Some(&path)).expect_err("malformed section header must fail");
    assert!(err.contains("line 2"));
    assert!(err.contains("malformed section header"));
    let _ = std::fs::remove_file(path);
}
