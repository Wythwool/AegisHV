use aegishv::actions::ActionDispatcher;
use aegishv::admin::{validate_policy_test_input, AdminHealth, PolicyExplain, PolicyTestInput};
use aegishv::build_info::BuildInfo;
use aegishv::collector::{spawn_collector, ControlMessage, IngestItem, Source};
use aegishv::config::{
    syslog_facility_code, Config, Journald as JournaldConfig, Spool as SpoolConfig,
    SpoolCompression, Syslog as SyslogConfig,
};
use aegishv::event::{category_from_str, severity_from_str, Category, Event, Severity};
use aegishv::identity::VmIdentityResolver;
use aegishv::metrics::{Metrics, TraceInputReason};
use aegishv::parser::{
    classify_exit, is_parser_degraded, parse_line, ParseOutcome, UnsupportedKind,
};
use aegishv::policy::PolicyEngine;
use aegishv::trace_format::{diagnose_kvm_tracepoints, TracepointDiagnostic};
use aegishv::tracefs;
use aegishv::util::now_rfc3339;
use aegishv::vmi::{
    GuestRegisters, GuestVirtual, RegisterReadError, TranslationError, TranslationResult, VmId,
    VmiErrorKind,
};
use aegishv::vmi_arm64::{translate_arm64_stage1, Arm64Granule, Arm64Stage1Context, Arm64Tcr};
use aegishv::vmi_cache::{Arm64CacheGranule, TranslationMode};
use aegishv::vmi_fixture::{
    load_vmi_fixture, parse_translation_mode, translation_mode_name, VmiFixture, VmiFixtureError,
};
use aegishv::vmi_profiles::ProfileArchitecture;
use aegishv::vmi_x86::{translate_x86_64, X86PagingMode};
use aegishv::wx::WxEngine;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::net::{SocketAddr, TcpListener, UdpSocket};
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, sync_channel, RecvTimeoutError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const SHUTDOWN_NONE: u8 = 0;
const SHUTDOWN_SIGINT: u8 = 1;
const SHUTDOWN_SIGTERM: u8 = 2;
const SHUTDOWN_CONSOLE: u8 = 3;
const JSONL_BUFFER_BYTES: usize = 256 * 1024;
const SPOOL_SEGMENT_HEADER_V1: &[u8] = b"aegishv-spool-v1 len-hex-jsonl\n";
const SPOOL_SEGMENT_HEADER_V2_RLE: &[u8] =
    b"aegishv-spool-v2 compression=rle record=hex-u64-uncompressed-hex-u64-payload\n";
const SPOOL_RECORD_PREFIX_BYTES: u64 = 17;
const SPOOL_COMPRESSED_RECORD_PREFIX_BYTES: u64 = 34;
const SPOOL_RECORD_SUFFIX_BYTES: u64 = 1;

static SHUTDOWN_SIGNAL_RECEIVED: AtomicBool = AtomicBool::new(false);
static SHUTDOWN_SIGNAL_KIND: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(SHUTDOWN_NONE);
static SHUTDOWN_SIGNAL_HANDLERS_INSTALLED: AtomicBool = AtomicBool::new(false);
static RELOAD_SIGNAL_RECEIVED: AtomicBool = AtomicBool::new(false);

fn main() {
    if let Err(e) = real_main() {
        eprintln!("aegishv: {e}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<(), String> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        print_help();
        return Ok(());
    }
    let cmd = args.remove(0);
    match cmd.as_str() {
        "run" => run_cmd(args),
        "snapshot" => snapshot_cmd(args),
        "dump-schemas" => dump_schemas_cmd(args),
        "validate-config" => validate_config_cmd(args),
        "version" => version_cmd(args),
        "admin" => admin_cmd(args),
        "vmi" => vmi_cmd(args),
        other => Err(format!("unknown command '{other}'")),
    }
}

fn print_help() {
    println!("AegisHV {}", env!("CARGO_PKG_VERSION"));
    println!("commands:");
    println!("  run [--tracefs PATH] [--replay FILE] [--deterministic-replay] [--config FILE] [--jsonl FILE|-] [--listen ADDR] [--queue N] [--quiet] [--no-quiet]");
    println!("  snapshot [--tracefs PATH] [--config FILE] [--json FILE]");
    println!("  dump-schemas [--out-dir DIR]");
    println!("  validate-config --config FILE");
    println!("  version [--json]");
    println!("  admin health [--json]");
    println!("  admin policy-explain --config FILE [--json]");
    println!("  admin policy-test --config FILE --category CAT --severity SEV --reason REASON --vm VM [--vm-id ID] [--json]");
    println!("  admin action-dry-run --config FILE --kind KIND --vm VM [--vm-id ID] [--output-path FILE] [--nic NAME] [--json]");
    println!("  vmi translate --fixture FILE --gva ADDR --mode MODE --json");
}

fn run_cmd(args: Vec<String>) -> Result<(), String> {
    let mut tracefs_root = PathBuf::from("/sys/kernel/tracing");
    let mut replay: Option<PathBuf> = None;
    let mut config_path: Option<PathBuf> = None;
    let mut jsonl = "-".to_string();
    let mut listen = "127.0.0.1:9108".to_string();
    let mut queue = 8192usize;
    let mut quiet = false;
    let mut no_quiet = false;
    let mut deterministic_replay = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--tracefs" => {
                i += 1;
                tracefs_root =
                    PathBuf::from(args.get(i).ok_or("--tracefs requires value")?.as_str());
            }
            "--replay" => {
                i += 1;
                replay = Some(PathBuf::from(
                    args.get(i).ok_or("--replay requires value")?.as_str(),
                ));
            }
            "--config" => {
                i += 1;
                config_path = Some(PathBuf::from(
                    args.get(i).ok_or("--config requires value")?.as_str(),
                ));
            }
            "--jsonl" => {
                i += 1;
                jsonl = args.get(i).ok_or("--jsonl requires value")?.clone();
            }
            "--listen" => {
                i += 1;
                listen = args.get(i).ok_or("--listen requires value")?.clone();
            }
            "--queue" => {
                i += 1;
                queue = args
                    .get(i)
                    .ok_or("--queue requires value")?
                    .parse()
                    .map_err(|_| "invalid --queue".to_string())?;
            }
            "--quiet" => quiet = true,
            "--no-quiet" => no_quiet = true,
            "--deterministic-replay" => deterministic_replay = true,
            other => return Err(format!("unknown run option '{other}'")),
        }
        i += 1;
    }
    validate_deterministic_replay_mode(replay.as_ref(), deterministic_replay)
        .map_err(|e| e.to_string())?;
    run(
        tracefs_root,
        replay,
        config_path,
        jsonl,
        listen,
        queue.max(1),
        quiet,
        no_quiet,
        deterministic_replay,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeterministicReplayModeError {
    RequiresReplay,
}

impl fmt::Display for DeterministicReplayModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiresReplay => write!(
                f,
                "--deterministic-replay requires --replay; live tracefs output is not made deterministic"
            ),
        }
    }
}

fn validate_deterministic_replay_mode(
    replay: Option<&PathBuf>,
    deterministic_replay: bool,
) -> Result<(), DeterministicReplayModeError> {
    if deterministic_replay && replay.is_none() {
        Err(DeterministicReplayModeError::RequiresReplay)
    } else {
        Ok(())
    }
}

fn snapshot_cmd(args: Vec<String>) -> Result<(), String> {
    let mut tracefs_root = PathBuf::from("/sys/kernel/tracing");
    let mut json_out: Option<PathBuf> = None;
    let mut config_path: Option<PathBuf> = None;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--tracefs" => {
                i += 1;
                tracefs_root =
                    PathBuf::from(args.get(i).ok_or("--tracefs requires value")?.as_str());
            }
            "--config" => {
                i += 1;
                config_path = Some(PathBuf::from(
                    args.get(i).ok_or("--config requires value")?.as_str(),
                ));
            }
            "--json" => {
                i += 1;
                json_out = Some(PathBuf::from(
                    args.get(i).ok_or("--json requires value")?.as_str(),
                ));
            }
            other => return Err(format!("unknown snapshot option '{other}'")),
        }
        i += 1;
    }
    let cfg = Config::load(config_path.as_deref())?;
    let identity = VmIdentityResolver::new(cfg.identity).map_err(|e| e.to_string())?;
    let snap = tracefs::snapshot_with_inventory(&tracefs_root, identity.inventory_snapshot())?;
    let json = snap.to_json_pretty();
    if let Some(path) = json_out {
        std::fs::write(&path, json).map_err(|e| format!("write snapshot {}: {e}", path.display()))
    } else {
        println!("{json}");
        Ok(())
    }
}

fn dump_schemas_cmd(args: Vec<String>) -> Result<(), String> {
    let mut out_dir: Option<PathBuf> = None;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--out-dir" => {
                i += 1;
                out_dir = Some(PathBuf::from(
                    args.get(i).ok_or("--out-dir requires value")?.as_str(),
                ));
            }
            other => return Err(format!("unknown dump-schemas option '{other}'")),
        }
        i += 1;
    }
    let event_schema = include_str!("../schema/event.schema.json");
    let snapshot_schema = include_str!("../schema/snapshot.schema.json");
    if let Some(dir) = out_dir {
        std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        std::fs::write(dir.join("event.schema.json"), event_schema)
            .map_err(|e| format!("write event schema: {e}"))?;
        std::fs::write(dir.join("snapshot.schema.json"), snapshot_schema)
            .map_err(|e| format!("write snapshot schema: {e}"))?;
    } else {
        println!("{}", event_schema);
        println!();
        println!("{}", snapshot_schema);
    }
    Ok(())
}

fn validate_config_cmd(args: Vec<String>) -> Result<(), String> {
    let mut config: Option<PathBuf> = None;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config = Some(PathBuf::from(
                    args.get(i).ok_or("--config requires value")?.as_str(),
                ));
            }
            other => return Err(format!("unknown validate-config option '{other}'")),
        }
        i += 1;
    }
    let path = config.ok_or("validate-config requires --config")?;
    let cfg = Config::load(Some(&path))?;
    println!(
        "configuration ok: version={} policy_rules={} qmp_mappings={}",
        cfg.version,
        cfg.policy.rules.len(),
        cfg.actions.qmp.len()
    );
    Ok(())
}

fn version_cmd(args: Vec<String>) -> Result<(), String> {
    let json = parse_json_only(args, "version")?;
    let info = BuildInfo::current();
    if json {
        println!("{}", info.to_json());
    } else {
        println!(
            "AegisHV {} target={}/{} git_rev={}",
            info.version, info.target_os, info.target_arch, info.git_rev
        );
    }
    Ok(())
}

fn admin_cmd(mut args: Vec<String>) -> Result<(), String> {
    if args.is_empty() {
        return Err("admin requires a subcommand".to_string());
    }
    let sub = args.remove(0);
    match sub.as_str() {
        "health" => admin_health_cmd(args),
        "policy-explain" => admin_policy_explain_cmd(args),
        "policy-test" => admin_policy_test_cmd(args),
        "action-dry-run" => admin_action_dry_run_cmd(args),
        other => Err(format!("unknown admin subcommand '{other}'")),
    }
}

fn admin_health_cmd(args: Vec<String>) -> Result<(), String> {
    let json = parse_json_only(args, "admin health")?;
    let health = AdminHealth::local();
    if json {
        println!("{}", health.to_json());
    } else {
        println!(
            "admin health: status={} runtime={} version={}",
            health.status, health.runtime, health.version
        );
    }
    Ok(())
}

fn admin_policy_explain_cmd(args: Vec<String>) -> Result<(), String> {
    let (config_path, json) = parse_config_and_json(args, "admin policy-explain")?;
    let cfg = Config::load(Some(&config_path))?;
    let explain = PolicyExplain::from_config(&cfg);
    if json {
        println!("{}", explain.to_json());
    } else {
        println!(
            "policy: version={} enabled_rules={} qmp_mappings={} stable_qmp_required={}",
            explain.version,
            explain.enabled_rules,
            explain.qmp_mappings,
            explain.stable_qmp_required
        );
    }
    Ok(())
}

fn admin_policy_test_cmd(args: Vec<String>) -> Result<(), String> {
    let parsed = parse_policy_test_args(args)?;
    let mut cfg = Config::load(Some(&parsed.config))?;
    for rule in &mut cfg.policy.rules {
        rule.mode = "dry_run".to_string();
    }
    let input = PolicyTestInput {
        category: parsed.category.clone(),
        severity: parsed.severity.clone(),
        reason: parsed.reason.clone(),
        vm: parsed.vm.clone(),
    };
    validate_policy_test_input(&input)?;
    let category = category_from_str(&parsed.category).ok_or("unknown category")?;
    let severity = severity_from_str(&parsed.severity).ok_or("unknown severity")?;
    let mut event = Event::base(category, severity, now_rfc3339(), parsed.vm);
    event.reason = Some(parsed.reason);
    event.vm_id = parsed.vm_id;
    let metrics = Metrics::new()?;
    let engine = PolicyEngine::new(&cfg)?;
    let outputs = engine.apply(&metrics, &event);
    if parsed.json {
        let joined = outputs
            .iter()
            .map(Event::to_json)
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{{\"matched\":{},\"events\":[{}]}}",
            !outputs.is_empty(),
            joined
        );
    } else {
        println!(
            "policy test: matched={} events={}",
            !outputs.is_empty(),
            outputs.len()
        );
    }
    Ok(())
}

fn admin_action_dry_run_cmd(args: Vec<String>) -> Result<(), String> {
    let parsed = parse_action_dry_run_args(args)?;
    let cfg = Config::load(Some(&parsed.config))?;
    let dispatcher = ActionDispatcher::new(&cfg)?;
    let metrics = Metrics::new()?;
    let event = dispatcher.run_action(
        &metrics,
        None,
        &parsed.vm,
        parsed.vm_id.as_deref(),
        &parsed.kind,
        parsed.output_path.as_deref(),
        parsed.nic.as_deref(),
        None,
        &[],
        false,
    );
    if parsed.json {
        println!("{}", event.to_json());
    } else {
        let action = event
            .action
            .as_ref()
            .ok_or("action event missing action body")?;
        println!(
            "action dry-run: kind={} status={} decision={}",
            action.kind, action.status, action.decision
        );
    }
    Ok(())
}

fn parse_json_only(args: Vec<String>, command: &str) -> Result<bool, String> {
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            other => return Err(format!("unknown {command} option '{other}'")),
        }
    }
    Ok(json)
}

fn parse_config_and_json(args: Vec<String>, command: &str) -> Result<(PathBuf, bool), String> {
    let mut config = None;
    let mut json = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config = Some(PathBuf::from(
                    args.get(i).ok_or("--config requires value")?.as_str(),
                ));
            }
            "--json" => json = true,
            other => return Err(format!("unknown {command} option '{other}'")),
        }
        i += 1;
    }
    Ok((config.ok_or(format!("{command} requires --config"))?, json))
}

struct PolicyTestArgs {
    config: PathBuf,
    category: String,
    severity: String,
    reason: String,
    vm: String,
    vm_id: Option<String>,
    json: bool,
}

fn parse_policy_test_args(args: Vec<String>) -> Result<PolicyTestArgs, String> {
    let mut out = PolicyTestArgs {
        config: PathBuf::new(),
        category: String::new(),
        severity: String::new(),
        reason: String::new(),
        vm: String::new(),
        vm_id: None,
        json: false,
    };
    let mut have_config = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                out.config = PathBuf::from(args.get(i).ok_or("--config requires value")?);
                have_config = true;
            }
            "--category" => {
                i += 1;
                out.category = args.get(i).ok_or("--category requires value")?.clone();
            }
            "--severity" => {
                i += 1;
                out.severity = args.get(i).ok_or("--severity requires value")?.clone();
            }
            "--reason" => {
                i += 1;
                out.reason = args.get(i).ok_or("--reason requires value")?.clone();
            }
            "--vm" => {
                i += 1;
                out.vm = args.get(i).ok_or("--vm requires value")?.clone();
            }
            "--vm-id" => {
                i += 1;
                out.vm_id = Some(args.get(i).ok_or("--vm-id requires value")?.clone());
            }
            "--json" => out.json = true,
            other => return Err(format!("unknown admin policy-test option '{other}'")),
        }
        i += 1;
    }
    if !have_config {
        return Err("admin policy-test requires --config".to_string());
    }
    Ok(out)
}

struct ActionDryRunArgs {
    config: PathBuf,
    kind: String,
    vm: String,
    vm_id: Option<String>,
    output_path: Option<String>,
    nic: Option<String>,
    json: bool,
}

fn parse_action_dry_run_args(args: Vec<String>) -> Result<ActionDryRunArgs, String> {
    let mut out = ActionDryRunArgs {
        config: PathBuf::new(),
        kind: String::new(),
        vm: String::new(),
        vm_id: None,
        output_path: None,
        nic: None,
        json: false,
    };
    let mut have_config = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                out.config = PathBuf::from(args.get(i).ok_or("--config requires value")?);
                have_config = true;
            }
            "--kind" => {
                i += 1;
                out.kind = args.get(i).ok_or("--kind requires value")?.clone();
            }
            "--vm" => {
                i += 1;
                out.vm = args.get(i).ok_or("--vm requires value")?.clone();
            }
            "--vm-id" => {
                i += 1;
                out.vm_id = Some(args.get(i).ok_or("--vm-id requires value")?.clone());
            }
            "--output-path" => {
                i += 1;
                out.output_path = Some(args.get(i).ok_or("--output-path requires value")?.clone());
            }
            "--nic" => {
                i += 1;
                out.nic = Some(args.get(i).ok_or("--nic requires value")?.clone());
            }
            "--json" => out.json = true,
            other => return Err(format!("unknown admin action-dry-run option '{other}'")),
        }
        i += 1;
    }
    if !have_config {
        return Err("admin action-dry-run requires --config".to_string());
    }
    if out.kind.trim().is_empty() {
        return Err("admin action-dry-run requires --kind".to_string());
    }
    if out.vm.trim().is_empty() {
        return Err("admin action-dry-run requires --vm".to_string());
    }
    Ok(out)
}

fn vmi_cmd(mut args: Vec<String>) -> Result<(), String> {
    if args.is_empty() {
        return Err("vmi requires a subcommand".to_string());
    }

    let subcommand = args.remove(0);
    match subcommand.as_str() {
        "translate" => vmi_translate_cmd(args),
        other => Err(format!("unknown vmi subcommand '{other}'")),
    }
}

#[derive(Debug, Clone)]
struct VmiTranslateArgs {
    fixture: PathBuf,
    gva: u64,
    mode: TranslationMode,
}

#[derive(Debug, Clone)]
struct VmiTranslateCliError {
    kind: VmiErrorKind,
    detail: String,
}

impl VmiTranslateCliError {
    fn new(kind: VmiErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    fn from_fixture(err: VmiFixtureError) -> Self {
        Self::new(err.kind(), err.to_string())
    }

    fn from_register(err: RegisterReadError) -> Self {
        Self::new(err.kind(), err.to_string())
    }

    fn from_translation(err: TranslationError) -> Self {
        Self::new(err.kind(), err.to_string())
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"ok\":false,\"kind\":\"{}\",\"error\":\"{}\"}}",
            self.kind.as_str(),
            json_escape(&self.detail)
        )
    }
}

impl fmt::Display for VmiTranslateCliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind.as_str(), self.detail)
    }
}

fn vmi_translate_cmd(args: Vec<String>) -> Result<(), String> {
    match vmi_translate_cmd_inner(args) {
        Ok(json) => {
            println!("{json}");
            Ok(())
        }
        Err(err) => {
            println!("{}", err.to_json());
            Err(err.to_string())
        }
    }
}

fn vmi_translate_cmd_inner(args: Vec<String>) -> Result<String, VmiTranslateCliError> {
    let parsed = parse_vmi_translate_args(args)?;
    let fixture = load_vmi_fixture(&parsed.fixture).map_err(VmiTranslateCliError::from_fixture)?;
    validate_cli_mode_matches_fixture(&fixture, parsed.mode)?;
    let result = translate_fixture_address(&fixture, parsed.gva, parsed.mode)?;
    Ok(translation_success_json(
        &fixture,
        parsed.gva,
        parsed.mode,
        &result,
    ))
}

fn parse_vmi_translate_args(args: Vec<String>) -> Result<VmiTranslateArgs, VmiTranslateCliError> {
    let mut fixture: Option<PathBuf> = None;
    let mut gva: Option<u64> = None;
    let mut mode: Option<TranslationMode> = None;
    let mut i = 0usize;

    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                let value = args.get(i).ok_or_else(|| {
                    VmiTranslateCliError::new(
                        VmiErrorKind::InvalidInput,
                        "--fixture requires value",
                    )
                })?;
                fixture = Some(PathBuf::from(value));
            }
            "--gva" => {
                i += 1;
                let value = args.get(i).ok_or_else(|| {
                    VmiTranslateCliError::new(VmiErrorKind::InvalidInput, "--gva requires value")
                })?;
                gva = Some(parse_cli_u64("gva", value)?);
            }
            "--mode" => {
                i += 1;
                let value = args.get(i).ok_or_else(|| {
                    VmiTranslateCliError::new(VmiErrorKind::InvalidInput, "--mode requires value")
                })?;
                mode = Some(
                    parse_translation_mode(value).map_err(VmiTranslateCliError::from_fixture)?,
                );
            }
            "--json" => {}
            other => {
                return Err(VmiTranslateCliError::new(
                    VmiErrorKind::InvalidInput,
                    format!("unknown vmi translate option '{other}'"),
                ));
            }
        }
        i += 1;
    }

    Ok(VmiTranslateArgs {
        fixture: fixture.ok_or_else(|| {
            VmiTranslateCliError::new(
                VmiErrorKind::InvalidInput,
                "vmi translate requires --fixture",
            )
        })?,
        gva: gva.ok_or_else(|| {
            VmiTranslateCliError::new(VmiErrorKind::InvalidInput, "vmi translate requires --gva")
        })?,
        mode: mode.ok_or_else(|| {
            VmiTranslateCliError::new(VmiErrorKind::InvalidInput, "vmi translate requires --mode")
        })?,
    })
}

fn parse_cli_u64(field: &'static str, value: &str) -> Result<u64, VmiTranslateCliError> {
    let parsed = if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
    } else {
        value.parse::<u64>()
    };

    parsed.map_err(|_| {
        VmiTranslateCliError::new(
            VmiErrorKind::InvalidInput,
            format!("invalid {field} value '{value}'"),
        )
    })
}

fn validate_cli_mode_matches_fixture(
    fixture: &VmiFixture,
    mode: TranslationMode,
) -> Result<(), VmiTranslateCliError> {
    let matches_architecture = matches!(
        (&fixture.architecture, mode),
        (
            ProfileArchitecture::X86_64,
            TranslationMode::X86_64FourLevel | TranslationMode::X86_64La57,
        ) | (
            ProfileArchitecture::Arm64,
            TranslationMode::Arm64Stage1 { .. }
        )
    );

    if matches_architecture {
        Ok(())
    } else {
        Err(VmiTranslateCliError::new(
            VmiErrorKind::Malformed,
            format!(
                "mode '{}' does not match fixture architecture '{}'",
                translation_mode_name(mode),
                fixture_architecture_name(&fixture.architecture)
            ),
        ))
    }
}

fn translate_fixture_address(
    fixture: &VmiFixture,
    gva: u64,
    mode: TranslationMode,
) -> Result<TranslationResult, VmiTranslateCliError> {
    match mode {
        TranslationMode::X86_64FourLevel => {
            let regs = x86_guest_registers(fixture)?;
            translate_x86_64(
                &fixture.memory,
                VmId(1),
                &regs,
                GuestVirtual(gva),
                X86PagingMode::FourLevel,
            )
            .map_err(VmiTranslateCliError::from_translation)
        }
        TranslationMode::X86_64La57 => {
            let regs = x86_guest_registers(fixture)?;
            translate_x86_64(
                &fixture.memory,
                VmId(1),
                &regs,
                GuestVirtual(gva),
                X86PagingMode::La57,
            )
            .map_err(VmiTranslateCliError::from_translation)
        }
        TranslationMode::Arm64Stage1 { granule } => {
            let context = arm64_stage1_context(fixture, granule)?;
            translate_arm64_stage1(&fixture.memory, VmId(1), &context, GuestVirtual(gva))
                .map_err(VmiTranslateCliError::from_translation)
        }
    }
}

fn x86_guest_registers(fixture: &VmiFixture) -> Result<GuestRegisters, VmiTranslateCliError> {
    Ok(GuestRegisters {
        pc: 0,
        sp: 0,
        cr3_or_ttbr: Some(
            fixture
                .registers
                .x86_cr3()
                .map_err(VmiTranslateCliError::from_register)?,
        ),
        privilege: None,
    })
}

fn arm64_stage1_context(
    fixture: &VmiFixture,
    granule: Arm64CacheGranule,
) -> Result<Arm64Stage1Context, VmiTranslateCliError> {
    let ttbr0 = fixture
        .registers
        .arm64_ttbr0_el1()
        .map_err(VmiTranslateCliError::from_register)?;
    let ttbr1 = fixture
        .registers
        .arm64_ttbr1_el1()
        .map_err(VmiTranslateCliError::from_register)?;
    let tcr_raw = fixture
        .registers
        .arm64_tcr_el1()
        .map_err(VmiTranslateCliError::from_register)?;
    let t0sz = (tcr_raw & 0x3f) as u8;
    let t1sz = ((tcr_raw >> 16) & 0x3f) as u8;
    let granule = match granule {
        Arm64CacheGranule::Size4K => Arm64Granule::Size4K,
        Arm64CacheGranule::Size16K => Arm64Granule::Size16K,
        Arm64CacheGranule::Size64K => Arm64Granule::Size64K,
    };

    Ok(Arm64Stage1Context {
        ttbr0: Some(ttbr0),
        ttbr1: Some(ttbr1),
        tcr: Arm64Tcr {
            t0sz,
            t1sz,
            granule,
        },
    })
}

fn translation_success_json(
    fixture: &VmiFixture,
    gva: u64,
    mode: TranslationMode,
    result: &TranslationResult,
) -> String {
    format!(
        "{{\"ok\":true,\"architecture\":\"{}\",\"mode\":\"{}\",\"gva\":\"0x{:x}\",\"gpa\":\"0x{:x}\",\"page_size\":{},\"readable\":{},\"writable\":{},\"executable\":{},\"user\":{},\"fixture_id\":\"{}\",\"fixture_name\":\"{}\"}}",
        fixture_architecture_name(&fixture.architecture),
        translation_mode_name(mode),
        gva,
        result.gpa.0,
        result.page_size,
        result.readable,
        result.writable,
        result.executable,
        result.user,
        json_escape(&fixture.id),
        json_escape(&fixture.name)
    )
}

fn fixture_architecture_name(architecture: &ProfileArchitecture) -> &str {
    match architecture {
        ProfileArchitecture::X86_64 => "x86_64",
        ProfileArchitecture::Arm64 => "arm64",
        ProfileArchitecture::Other(arch) => arch.as_str(),
    }
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped
}

#[allow(clippy::too_many_arguments)]
fn run(
    tracefs_root: PathBuf,
    replay: Option<PathBuf>,
    config_path: Option<PathBuf>,
    jsonl: String,
    listen: String,
    queue: usize,
    quiet_flag: bool,
    no_quiet_flag: bool,
    deterministic_replay: bool,
) -> Result<(), String> {
    validate_deterministic_replay_mode(replay.as_ref(), deterministic_replay)
        .map_err(|e| e.to_string())?;
    let cfg = Config::load(config_path.as_deref())?;
    let run_mode = if replay.is_some() {
        "replay"
    } else {
        "tracefs"
    };
    let config_source = if config_path.is_some() {
        "file"
    } else {
        "defaults"
    };
    let jsonl_target = if jsonl == "-" { "stdout" } else { "file" };
    let metrics_listener = if listen.trim().is_empty() {
        "disabled"
    } else {
        "enabled"
    };
    let pmu_enable = cfg.pmu.enable && replay.is_none();
    let pmu_sample_ms = cfg.pmu.sample_ms;
    let pmu_rediscover_ms = cfg.pmu.rediscover_ms;
    let pmu_qemu_pid = cfg.pmu.qemu_pid;
    let pmu_vm_regex = cfg.pmu.vm_regex.clone();
    let allow_metrics_bind_failure = cfg.metrics.allow_bind_failure;
    let mut runtime = RuntimeState::new_with_identity_mode(
        &cfg,
        quiet_flag,
        no_quiet_flag,
        deterministic_replay,
    )?;
    let mut replay_determinism = if deterministic_replay {
        ReplayDeterminism::enabled()
    } else {
        ReplayDeterminism::disabled()
    };

    let metrics = Metrics::new()?;
    metrics.set_queue_depth(0, queue);
    metrics.set_wx_pages_tracked(0);
    metrics.set_identity_inventory(&runtime.identity.inventory_snapshot());
    metrics.mark_policy_ok();
    metrics.mark_actions_ok();

    let stop = Arc::new(AtomicBool::new(false));
    reset_shutdown_signal_for_run();
    install_shutdown_signal_handlers()?;
    let metrics_thread = start_metrics_server(
        &listen,
        metrics.clone(),
        stop.clone(),
        allow_metrics_bind_failure,
    )?;
    let tracepoint_diagnostics = if replay.is_none() {
        diagnose_kvm_tracepoints(&tracefs_root)
    } else {
        Vec::new()
    };

    let (tx, rx) = sync_channel::<IngestItem>(queue);
    let (control_tx, control_rx) = channel::<ControlMessage>();
    let source = if let Some(path) = replay.clone() {
        Source::Replay { path }
    } else {
        Source::Tracefs { root: tracefs_root }
    };
    let collector = spawn_collector(
        source,
        tx.clone(),
        control_tx,
        stop.clone(),
        metrics.clone(),
        queue,
    );
    metrics.mark_collector_running();
    let pmu = aegishv::pmu::spawn_pmu_sampler(
        pmu_enable,
        pmu_sample_ms,
        pmu_rediscover_ms,
        pmu_qemu_pid,
        pmu_vm_regex,
        tx.clone(),
        stop.clone(),
        metrics.clone(),
        queue,
    );
    if pmu.is_some() {
        metrics.mark_pmu_running();
    } else {
        metrics.mark_pmu_disabled();
    }

    let mut out = JsonlEventSink::open(&jsonl, &cfg.spool, &cfg.syslog, &cfg.journald)?;
    metrics.mark_output_ok();
    metrics.mark_runtime_running();
    let mut collector_done = false;
    let mut pipeline_error: Option<String> = None;
    let mut loss = LossTracker::default();
    let mut shutdown_event_emitted = false;
    let mut shutdown_reason = ShutdownReason::Clean;

    let mut ev = startup_lifecycle_event(
        &cfg,
        &runtime,
        LifecycleStartup {
            mode: run_mode,
            config_source,
            jsonl_target,
            metrics_listener,
            queue_capacity: queue,
            pmu_enabled: pmu_enable,
            deterministic_replay,
        },
    );
    emit(
        &mut out,
        &metrics,
        &mut loss,
        runtime.quiet,
        &mut ev,
        &mut replay_determinism,
    )?;
    flush_jsonl(
        &mut out,
        &metrics,
        "flush jsonl after startup lifecycle event",
    )?;
    emit_tracepoint_diagnostics(
        &mut out,
        &metrics,
        &mut loss,
        &tracepoint_diagnostics,
        runtime.quiet,
        runtime.flush_every,
        &mut replay_determinism,
    )?;

    loop {
        if let Some(signal_name) =
            observe_shutdown_signal(stop.as_ref(), &mut shutdown_event_emitted)
        {
            metrics.mark_runtime_stopping();
            shutdown_reason = ShutdownReason::Signal(signal_name.to_string());
            let mut ev = shutdown_event(signal_name);
            emit(
                &mut out,
                &metrics,
                &mut loss,
                runtime.quiet,
                &mut ev,
                &mut replay_determinism,
            )?;
            maybe_flush(&mut out, &metrics, runtime.flush_every, ev.severity)?;
        }
        maybe_handle_sighup(
            &mut out,
            &metrics,
            &mut loss,
            config_path.as_deref(),
            &mut runtime,
            quiet_flag,
            no_quiet_flag,
            &mut replay_determinism,
        )?;
        while let Ok(ctrl) = control_rx.try_recv() {
            match ctrl {
                ControlMessage::ReplayEof => {
                    collector_done = true;
                    metrics.mark_collector_stopped();
                }
                ControlMessage::CollectorError(msg) => {
                    collector_done = true;
                    metrics.mark_collector_failed();
                    metrics.mark_runtime_failed();
                    let mut ev = sensor_event(Severity::High, "collector_error", msg.clone());
                    emit(
                        &mut out,
                        &metrics,
                        &mut loss,
                        runtime.quiet,
                        &mut ev,
                        &mut replay_determinism,
                    )?;
                    maybe_flush(&mut out, &metrics, runtime.flush_every, ev.severity)?;
                    pipeline_error = Some(msg);
                    stop.store(true, Ordering::Relaxed);
                }
            }
        }

        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(IngestItem::Line(line)) => {
                metrics.record_queue_receive();
                let t0 = Instant::now();
                match parse_line(&line) {
                    ParseOutcome::Parsed(parsed) => {
                        metrics.inc_parse_ok();
                        if is_parser_degraded(&parsed) {
                            metrics.inc_trace_input(TraceInputReason::ParserDegraded);
                        } else {
                            metrics.inc_trace_input(TraceInputReason::Parsed);
                        }
                        metrics.observe_parse_latency_ms(t0.elapsed().as_secs_f64() * 1000.0);
                        let mut exit_ev = classify_exit(&parsed);
                        let enrichment = runtime.identity.enrich_event(&mut exit_ev);
                        metrics.record_identity_enrichment(&enrichment);
                        if let Some(mut conflict_ev) = enrichment.conflict_event {
                            emit(
                                &mut out,
                                &metrics,
                                &mut loss,
                                runtime.quiet,
                                &mut conflict_ev,
                                &mut replay_determinism,
                            )?;
                            maybe_flush(
                                &mut out,
                                &metrics,
                                runtime.flush_every,
                                conflict_ev.severity,
                            )?;
                        }
                        if runtime.policy.should_ignore_vm(&exit_ev.vm) {
                            metrics.inc_unsupported();
                            loss.account_intentionally_skipped_sequence(exit_ev.sequence);
                            continue;
                        }
                        emit(
                            &mut out,
                            &metrics,
                            &mut loss,
                            runtime.quiet,
                            &mut exit_ev,
                            &mut replay_determinism,
                        )?;
                        maybe_flush(&mut out, &metrics, runtime.flush_every, exit_ev.severity)?;
                        if let Some(mut wx_ev) = runtime.wx.on_exit_event(&exit_ev) {
                            metrics.inc_wx();
                            emit(
                                &mut out,
                                &metrics,
                                &mut loss,
                                runtime.quiet,
                                &mut wx_ev,
                                &mut replay_determinism,
                            )?;
                            maybe_flush(&mut out, &metrics, runtime.flush_every, wx_ev.severity)?;
                            for mut action_ev in runtime.policy.apply(&metrics, &wx_ev) {
                                emit(
                                    &mut out,
                                    &metrics,
                                    &mut loss,
                                    runtime.quiet,
                                    &mut action_ev,
                                    &mut replay_determinism,
                                )?;
                                maybe_flush(
                                    &mut out,
                                    &metrics,
                                    runtime.flush_every,
                                    action_ev.severity,
                                )?;
                            }
                        }
                        metrics.set_wx_pages_tracked(runtime.wx.pages_tracked());
                        metrics.inc_wx_prune(runtime.wx.take_pruned_delta());
                        metrics.inc_wx_cooldown_suppressed(
                            runtime.wx.take_cooldown_suppressed_delta(),
                        );
                        for mut action_ev in runtime.policy.apply(&metrics, &exit_ev) {
                            emit(
                                &mut out,
                                &metrics,
                                &mut loss,
                                runtime.quiet,
                                &mut action_ev,
                                &mut replay_determinism,
                            )?;
                            maybe_flush(
                                &mut out,
                                &metrics,
                                runtime.flush_every,
                                action_ev.severity,
                            )?;
                        }
                    }
                    ParseOutcome::Unsupported { kind, .. } => {
                        metrics.inc_unsupported();
                        metrics.inc_trace_input(unsupported_trace_input_reason(kind));
                        if matches!(kind, UnsupportedKind::UnrelatedTracepoint) {
                            metrics.inc_unrelated_tracepoint();
                        }
                    }
                    ParseOutcome::MalformedKvmExit { detail } => {
                        metrics.inc_parse_error();
                        metrics.inc_trace_input(TraceInputReason::MalformedKvmExit);
                        let mut ev = sensor_event(Severity::Low, "malformed_kvm_exit", detail);
                        emit(
                            &mut out,
                            &metrics,
                            &mut loss,
                            runtime.quiet,
                            &mut ev,
                            &mut replay_determinism,
                        )?;
                        maybe_flush(&mut out, &metrics, runtime.flush_every, ev.severity)?;
                    }
                }
            }
            Ok(IngestItem::Event(mut ev)) => {
                metrics.record_queue_receive();
                emit(
                    &mut out,
                    &metrics,
                    &mut loss,
                    runtime.quiet,
                    &mut ev,
                    &mut replay_determinism,
                )?;
                maybe_flush(&mut out, &metrics, runtime.flush_every, ev.severity)?;
                for mut action_ev in runtime.policy.apply(&metrics, &ev) {
                    emit(
                        &mut out,
                        &metrics,
                        &mut loss,
                        runtime.quiet,
                        &mut action_ev,
                        &mut replay_determinism,
                    )?;
                    maybe_flush(&mut out, &metrics, runtime.flush_every, action_ev.severity)?;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some(signal_name) =
                    observe_shutdown_signal(stop.as_ref(), &mut shutdown_event_emitted)
                {
                    metrics.mark_runtime_stopping();
                    shutdown_reason = ShutdownReason::Signal(signal_name.to_string());
                    let mut ev = shutdown_event(signal_name);
                    emit(
                        &mut out,
                        &metrics,
                        &mut loss,
                        runtime.quiet,
                        &mut ev,
                        &mut replay_determinism,
                    )?;
                    maybe_flush(&mut out, &metrics, runtime.flush_every, ev.severity)?;
                }
                maybe_handle_sighup(
                    &mut out,
                    &metrics,
                    &mut loss,
                    config_path.as_deref(),
                    &mut runtime,
                    quiet_flag,
                    no_quiet_flag,
                    &mut replay_determinism,
                )?;
                if collector_done || stop.load(Ordering::Relaxed) {
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    if loss.has_unreported(&metrics) {
        let dropped = loss.delta(&metrics);
        let mut ev = sensor_event(
            Severity::High,
            "telemetry_loss",
            format!("{} telemetry items were dropped before shutdown", dropped),
        );
        emit(
            &mut out,
            &metrics,
            &mut loss,
            runtime.quiet,
            &mut ev,
            &mut replay_determinism,
        )?;
    }
    stop.store(true, Ordering::Relaxed);

    match collector.join() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            metrics.mark_collector_failed();
            metrics.mark_runtime_failed();
            if pipeline_error.is_none() {
                pipeline_error = Some(e);
            }
        }
        Err(_) => {
            metrics.mark_collector_failed();
            metrics.mark_runtime_failed();
            if pipeline_error.is_none() {
                pipeline_error = Some("collector thread panicked".to_string());
            }
        }
    }
    if let Some(handle) = pmu {
        match handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                metrics.mark_pmu_failed();
                metrics.mark_runtime_failed();
                if pipeline_error.is_none() {
                    pipeline_error = Some(e);
                }
            }
            Err(_) => {
                metrics.mark_pmu_failed();
                metrics.mark_runtime_failed();
                if pipeline_error.is_none() {
                    pipeline_error = Some("pmu thread panicked".to_string());
                }
            }
        }
    }
    if let Some(err) = &pipeline_error {
        shutdown_reason = ShutdownReason::Failure(err.clone());
    }
    let mut ev = shutdown_lifecycle_event(&shutdown_reason, &metrics);
    emit(
        &mut out,
        &metrics,
        &mut loss,
        runtime.quiet,
        &mut ev,
        &mut replay_determinism,
    )?;
    flush_jsonl(
        &mut out,
        &metrics,
        "flush jsonl after shutdown lifecycle event",
    )?;
    if let Some(handle) = metrics_thread {
        let _ = handle.join();
    }
    if let Some(err) = pipeline_error {
        return Err(err);
    }
    metrics.mark_runtime_stopped();
    Ok(())
}

#[derive(Default)]
struct LossTracker {
    last_drop_total: u64,
    last_accounted_sequence: Option<u64>,
}

impl LossTracker {
    fn decorate(&mut self, ev: &mut Event, metrics: &Metrics) {
        let total = metrics.dropped_total();
        let delta = total.saturating_sub(self.last_drop_total);
        let sequence_gap = self.sequence_gap_before(ev.sequence);
        if delta > 0 || sequence_gap.is_some() {
            let reason = if delta > 0 {
                "queue_full_or_output_backpressure"
            } else {
                "sequence_gap"
            };
            ev.with_loss_report(delta, total, reason, sequence_gap);
            self.last_drop_total = total;
        }
    }

    fn finish_emit(&mut self, sequence: u64, status: SinkWriteStatus) {
        if status == SinkWriteStatus::Written {
            self.account_sequence(sequence);
        }
    }

    fn account_intentionally_skipped_sequence(&mut self, sequence: u64) {
        self.account_sequence(sequence);
    }

    fn has_unreported(&self, metrics: &Metrics) -> bool {
        metrics.dropped_total() > self.last_drop_total
    }

    fn delta(&self, metrics: &Metrics) -> u64 {
        metrics.dropped_total().saturating_sub(self.last_drop_total)
    }

    fn sequence_gap_before(&self, sequence: u64) -> Option<(u64, u64)> {
        let last = self.last_accounted_sequence?;
        let start = last.checked_add(1)?;
        if sequence > start {
            Some((start, sequence - 1))
        } else {
            None
        }
    }

    fn account_sequence(&mut self, sequence: u64) {
        self.last_accounted_sequence = Some(
            self.last_accounted_sequence
                .map_or(sequence, |last| last.max(sequence)),
        );
    }
}

struct RuntimeState {
    quiet: bool,
    flush_every: usize,
    wx: WxEngine,
    identity: VmIdentityResolver,
    policy: PolicyEngine,
    policy_rule_count: usize,
    qmp_mapping_count: usize,
}

impl RuntimeState {
    fn new(cfg: &Config, quiet_flag: bool, no_quiet_flag: bool) -> Result<Self, String> {
        Self::new_with_identity_mode(cfg, quiet_flag, no_quiet_flag, false)
    }

    fn new_with_identity_mode(
        cfg: &Config,
        quiet_flag: bool,
        no_quiet_flag: bool,
        deterministic_replay: bool,
    ) -> Result<Self, String> {
        let policy = PolicyEngine::new(cfg)?;
        let identity = if deterministic_replay {
            VmIdentityResolver::deterministic_replay(cfg.identity.clone())
        } else {
            VmIdentityResolver::new(cfg.identity.clone())
        }
        .map_err(|e| e.to_string())?;
        Ok(Self {
            quiet: effective_quiet(cfg, quiet_flag, no_quiet_flag),
            flush_every: cfg.general.flush_every,
            wx: WxEngine::new(cfg),
            identity,
            policy,
            policy_rule_count: cfg.policy.rules.iter().filter(|rule| rule.enabled).count(),
            qmp_mapping_count: cfg.actions.qmp.len(),
        })
    }
}

struct LifecycleStartup<'a> {
    mode: &'a str,
    config_source: &'a str,
    jsonl_target: &'a str,
    metrics_listener: &'a str,
    queue_capacity: usize,
    pmu_enabled: bool,
    deterministic_replay: bool,
}

enum ShutdownReason {
    Clean,
    Signal(String),
    Failure(String),
}

impl ShutdownReason {
    fn label(&self) -> &str {
        match self {
            Self::Clean => "clean",
            Self::Signal(_) => "signal",
            Self::Failure(_) => "failure",
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Clean => "collector stopped without a fatal pipeline error".to_string(),
            Self::Signal(signal) => signal.clone(),
            Self::Failure(err) => failure_shutdown_detail(err),
        }
    }
}

fn failure_shutdown_detail(err: &str) -> String {
    let lower = err.to_ascii_lowercase();
    if lower.contains("collector") || lower.contains("trace") {
        "collector pipeline error".to_string()
    } else if lower.contains("pmu") {
        "PMU pipeline error".to_string()
    } else if lower.contains("jsonl") || lower.contains("output") {
        "output pipeline error".to_string()
    } else {
        "fatal pipeline error".to_string()
    }
}

fn unsupported_trace_input_reason(kind: UnsupportedKind) -> TraceInputReason {
    match kind {
        UnsupportedKind::UnrelatedTracepoint => TraceInputReason::UnrelatedTracepoint,
        UnsupportedKind::UnsupportedTracepoint => TraceInputReason::UnsupportedLine,
    }
}

struct ReplayDeterminism {
    enabled: bool,
    next_event: u64,
}

impl ReplayDeterminism {
    fn disabled() -> Self {
        Self {
            enabled: false,
            next_event: 1,
        }
    }

    fn enabled() -> Self {
        Self {
            enabled: true,
            next_event: 1,
        }
    }

    fn normalize(&mut self, ev: &mut Event) {
        if !self.enabled {
            return;
        }
        let sequence = self.next_event;
        self.next_event = self.next_event.saturating_add(1);
        let event_id = format!("evt-deterministic-{sequence:016x}");

        ev.ts = "2026-01-01T00:00:00.000Z".to_string();
        ev.monotonic_ms = u128::from(sequence.saturating_sub(1));
        ev.sequence = sequence;
        ev.event_id = event_id.clone();
        ev.host_id = Some("deterministic-host".to_string());
        ev.sensor_id = Some("deterministic-sensor".to_string());
        ev.tenant_id = Some("deterministic-tenant".to_string());
        if ev.action_id.is_some() {
            ev.action_id = Some(format!("act-{event_id}"));
        }
        if let Some(wx) = &mut ev.wx {
            wx.delta_ms = 0;
            wx.confidence = 1.0;
        }
        if let Some(action) = &mut ev.action {
            action.latency_ms = Some(0);
        }
    }
}

enum JsonlTarget {
    Stdout,
    File(PathBuf),
}

enum JsonlReopenOutcome {
    Reopened(PathBuf),
    StdoutSkipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SinkWriteStatus {
    Written,
    Spooled,
}

trait EventSink {
    fn write_event_line(
        &mut self,
        metrics: &Metrics,
        category: Category,
        severity: Severity,
        line: &str,
    ) -> Result<SinkWriteStatus, String>;
    fn maybe_flush(
        &mut self,
        metrics: &Metrics,
        flush_every: usize,
        severity: Severity,
    ) -> Result<(), String>;
    fn flush(&mut self, metrics: &Metrics, context: &str) -> Result<(), String>;
    fn reopen_jsonl(&mut self) -> Result<JsonlReopenOutcome, String>;
}

struct JsonlOutput {
    target: JsonlTarget,
    writer: BufWriter<Box<dyn Write + Send>>,
}

impl JsonlOutput {
    fn open(jsonl: &str) -> Result<Self, String> {
        if jsonl == "-" {
            return Ok(Self {
                target: JsonlTarget::Stdout,
                writer: BufWriter::with_capacity(JSONL_BUFFER_BYTES, Box::new(io::stdout())),
            });
        }
        let path = PathBuf::from(jsonl);
        Ok(Self {
            writer: BufWriter::with_capacity(JSONL_BUFFER_BYTES, open_jsonl_file(&path)?),
            target: JsonlTarget::File(path),
        })
    }

    fn reopen(&mut self) -> Result<JsonlReopenOutcome, String> {
        let JsonlTarget::File(path) = &self.target else {
            return Ok(JsonlReopenOutcome::StdoutSkipped);
        };
        self.writer
            .flush()
            .map_err(|e| format!("flush jsonl before reopen {}: {e}", path.display()))?;
        let next = BufWriter::with_capacity(JSONL_BUFFER_BYTES, open_jsonl_file(path)?);
        self.writer = next;
        Ok(JsonlReopenOutcome::Reopened(path.clone()))
    }
}

impl Write for JsonlOutput {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

struct JsonlEventSink {
    output: JsonlOutput,
    spool: EventSpool,
    syslog: SyslogOutput,
    journald: JournaldOutput,
    pending_flush: usize,
}

impl JsonlEventSink {
    fn open(
        jsonl: &str,
        spool: &SpoolConfig,
        syslog: &SyslogConfig,
        journald: &JournaldConfig,
    ) -> Result<Self, String> {
        Ok(Self {
            output: JsonlOutput::open(jsonl)?,
            spool: EventSpool::open(spool)?,
            syslog: SyslogOutput::open(syslog)?,
            journald: JournaldOutput::open(journald)?,
            pending_flush: 0,
        })
    }
}

impl EventSink for JsonlEventSink {
    fn write_event_line(
        &mut self,
        metrics: &Metrics,
        category: Category,
        severity: Severity,
        line: &str,
    ) -> Result<SinkWriteStatus, String> {
        let status = if let Err(e) = self
            .output
            .write_all(line.as_bytes())
            .and_then(|_| self.output.write_all(b"\n"))
        {
            metrics.inc_json_write_failure();
            if !self.spool.is_enabled() {
                metrics.mark_output_failed();
                metrics.mark_runtime_failed();
                return Err(format!("write jsonl: {e}"));
            }
            if let Err(spool_err) = self.spool.append(line) {
                metrics.inc_spool_write_failure();
                if spool_err.drops_event() {
                    metrics.inc_spool_dropped();
                }
                metrics.mark_output_failed();
                metrics.mark_runtime_failed();
                return Err(format!(
                    "write jsonl: {e}; event spool failed: {}",
                    spool_err.detail()
                ));
            }
            metrics.inc_spool_event();
            metrics.mark_output_degraded();
            SinkWriteStatus::Spooled
        } else {
            SinkWriteStatus::Written
        };
        if let Err(e) = self.syslog.write_event_line(severity, line) {
            metrics.inc_syslog_write_failure();
            metrics.mark_output_failed();
            metrics.mark_runtime_failed();
            return Err(e);
        }
        if let Err(e) = self.journald.write_event_line(category, severity, line) {
            metrics.inc_journald_write_failure();
            metrics.mark_output_failed();
            metrics.mark_runtime_failed();
            return Err(e);
        }
        Ok(status)
    }

    fn maybe_flush(
        &mut self,
        metrics: &Metrics,
        flush_every: usize,
        severity: Severity,
    ) -> Result<(), String> {
        self.pending_flush += 1;
        if self.pending_flush >= flush_every || severity.at_least(Severity::High) {
            self.flush(metrics, "flush jsonl")?;
        }
        Ok(())
    }

    fn flush(&mut self, metrics: &Metrics, context: &str) -> Result<(), String> {
        if let Err(e) = self.output.flush() {
            metrics.mark_output_failed();
            metrics.mark_runtime_failed();
            return Err(format!("{context}: {e}"));
        }
        self.pending_flush = 0;
        Ok(())
    }

    fn reopen_jsonl(&mut self) -> Result<JsonlReopenOutcome, String> {
        self.output.reopen()
    }
}

enum SyslogOutput {
    Disabled,
    Udp(SyslogUdpSink),
}

struct SyslogUdpSink {
    socket: UdpSocket,
    facility: u8,
    max_message_bytes: usize,
}

impl SyslogOutput {
    fn open(cfg: &SyslogConfig) -> Result<Self, String> {
        if !cfg.enable {
            return Ok(Self::Disabled);
        }
        let address = cfg
            .address
            .parse::<SocketAddr>()
            .map_err(|_| "open syslog sink: syslog.address must be numeric ip:port".to_string())?;
        let bind_addr = if address.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };
        let socket =
            UdpSocket::bind(bind_addr).map_err(|e| format!("open syslog UDP socket: {e}"))?;
        socket
            .connect(address)
            .map_err(|e| format!("connect syslog UDP socket: {e}"))?;
        let facility = syslog_facility_code(&cfg.facility)
            .ok_or_else(|| "open syslog sink: unsupported syslog.facility".to_string())?;
        Ok(Self::Udp(SyslogUdpSink {
            socket,
            facility,
            max_message_bytes: cfg.max_message_bytes,
        }))
    }

    fn write_event_line(&mut self, severity: Severity, line: &str) -> Result<(), String> {
        match self {
            Self::Disabled => Ok(()),
            Self::Udp(sink) => sink.write_event_line(severity, line),
        }
    }
}

impl SyslogUdpSink {
    fn write_event_line(&self, severity: Severity, line: &str) -> Result<(), String> {
        let priority = u16::from(self.facility) * 8 + u16::from(syslog_severity_code(severity));
        let prefix = format!("<{priority}>1 - aegishv - - - - ");
        let message_len = prefix.len() + line.len();
        if message_len > self.max_message_bytes {
            return Err(format!(
                "syslog message is {message_len} bytes, exceeds syslog.max_message_bytes={}",
                self.max_message_bytes
            ));
        }
        let mut message = String::with_capacity(message_len);
        message.push_str(&prefix);
        message.push_str(line);
        let sent = self
            .socket
            .send(message.as_bytes())
            .map_err(|e| format!("write syslog UDP datagram: {e}"))?;
        if sent != message.len() {
            return Err("write syslog UDP datagram: short send".to_string());
        }
        Ok(())
    }
}

enum JournaldOutput {
    Disabled,
    #[cfg(target_os = "linux")]
    Datagram(JournaldDatagramSink),
    #[cfg(test)]
    Memory(JournaldMemorySink),
}

#[cfg(target_os = "linux")]
struct JournaldDatagramSink {
    socket: UnixDatagram,
    identifier: String,
    max_message_bytes: usize,
}

#[cfg(test)]
struct JournaldMemorySink {
    buffer: Arc<std::sync::Mutex<Vec<u8>>>,
    identifier: String,
    max_message_bytes: usize,
}

impl JournaldOutput {
    fn open(cfg: &JournaldConfig) -> Result<Self, String> {
        if !cfg.enable {
            return Ok(Self::Disabled);
        }
        open_enabled_journald_output(cfg)
    }

    fn write_event_line(
        &mut self,
        category: Category,
        severity: Severity,
        line: &str,
    ) -> Result<(), String> {
        #[cfg(not(any(target_os = "linux", test)))]
        let _ = (category, severity, line);
        match self {
            Self::Disabled => Ok(()),
            #[cfg(target_os = "linux")]
            Self::Datagram(sink) => sink.write_event_line(category, severity, line),
            #[cfg(test)]
            Self::Memory(sink) => sink.write_event_line(category, severity, line),
        }
    }

    #[cfg(test)]
    fn test_memory(
        buffer: Arc<std::sync::Mutex<Vec<u8>>>,
        identifier: &str,
        max_message_bytes: usize,
    ) -> Self {
        Self::Memory(JournaldMemorySink {
            buffer,
            identifier: identifier.to_string(),
            max_message_bytes,
        })
    }
}

#[cfg(target_os = "linux")]
fn open_enabled_journald_output(cfg: &JournaldConfig) -> Result<JournaldOutput, String> {
    let socket =
        UnixDatagram::unbound().map_err(|e| format!("open journald datagram socket: {e}"))?;
    socket
        .connect(&cfg.socket)
        .map_err(|e| format!("connect journald datagram socket {}: {e}", cfg.socket))?;
    Ok(JournaldOutput::Datagram(JournaldDatagramSink {
        socket,
        identifier: cfg.identifier.clone(),
        max_message_bytes: cfg.max_message_bytes,
    }))
}

#[cfg(not(target_os = "linux"))]
fn open_enabled_journald_output(_cfg: &JournaldConfig) -> Result<JournaldOutput, String> {
    Err(
        "open journald sink: journald output requires Linux with systemd journald socket support; disable journald.enable on this host"
            .to_string(),
    )
}

#[cfg(target_os = "linux")]
impl JournaldDatagramSink {
    fn write_event_line(
        &self,
        category: Category,
        severity: Severity,
        line: &str,
    ) -> Result<(), String> {
        let payload = journald_payload(
            category,
            severity,
            &self.identifier,
            line,
            self.max_message_bytes,
        )?;
        let sent = self
            .socket
            .send(&payload)
            .map_err(|e| format!("write journald datagram: {e}"))?;
        if sent != payload.len() {
            return Err("write journald datagram: short send".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
impl JournaldMemorySink {
    fn write_event_line(
        &self,
        category: Category,
        severity: Severity,
        line: &str,
    ) -> Result<(), String> {
        let payload = journald_payload(
            category,
            severity,
            &self.identifier,
            line,
            self.max_message_bytes,
        )?;
        let mut buffer = self.buffer.lock().expect("journald test buffer lock");
        buffer.extend_from_slice(&payload);
        Ok(())
    }
}

#[cfg(any(target_os = "linux", test))]
fn journald_payload(
    category: Category,
    severity: Severity,
    identifier: &str,
    line: &str,
    max_message_bytes: usize,
) -> Result<Vec<u8>, String> {
    let priority = syslog_severity_code(severity).to_string();
    let mut message = String::with_capacity(
        80 + priority.len() + identifier.len() + category.as_str().len() + line.len(),
    );
    message.push_str("PRIORITY=");
    message.push_str(&priority);
    message.push('\n');
    message.push_str("SYSLOG_IDENTIFIER=");
    message.push_str(identifier);
    message.push('\n');
    message.push_str("AEGISHV_CATEGORY=");
    message.push_str(category.as_str());
    message.push('\n');
    message.push_str("AEGISHV_SEVERITY=");
    message.push_str(severity.as_str());
    message.push('\n');
    message.push_str("MESSAGE=");
    message.push_str(line);
    message.push('\n');

    if message.len() > max_message_bytes {
        return Err(format!(
            "journald message is {} bytes, exceeds journald.max_message_bytes={max_message_bytes}",
            message.len()
        ));
    }
    Ok(message.into_bytes())
}

fn syslog_severity_code(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 2,
        Severity::High => 3,
        Severity::Medium => 4,
        Severity::Low => 5,
        Severity::Info => 6,
    }
}

fn open_jsonl_file(path: &Path) -> Result<Box<dyn Write + Send>, String> {
    Ok(Box::new(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| format!("open jsonl {}: {e}", path.display()))?,
    ))
}

enum EventSpool {
    Disabled,
    Enabled(DiskSpool),
}

impl EventSpool {
    fn open(cfg: &SpoolConfig) -> Result<Self, String> {
        if !cfg.enable {
            return Ok(Self::Disabled);
        }
        DiskSpool::open(cfg)
            .map(Self::Enabled)
            .map_err(|e| format!("open event spool: {}", e.detail()))
    }

    fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled(_))
    }

    fn append(&mut self, line: &str) -> Result<(), SpoolError> {
        match self {
            Self::Disabled => Ok(()),
            Self::Enabled(spool) => spool.append(line),
        }
    }
}

enum PendingSpoolRecord<'a> {
    Plain {
        line: &'a str,
    },
    Rle {
        uncompressed_len: usize,
        payload: Vec<u8>,
    },
}

impl<'a> PendingSpoolRecord<'a> {
    fn build(line: &'a str, compression: SpoolCompression) -> Self {
        match compression {
            SpoolCompression::None => Self::Plain { line },
            SpoolCompression::Rle => Self::Rle {
                uncompressed_len: line.len(),
                payload: rle_compress(line.as_bytes()),
            },
        }
    }

    fn total_bytes(&self) -> u64 {
        match self {
            Self::Plain { line } => spool_plain_record_bytes(line),
            Self::Rle { payload, .. } => {
                let payload_bytes = u64::try_from(payload.len()).unwrap_or(u64::MAX);
                SPOOL_COMPRESSED_RECORD_PREFIX_BYTES
                    .saturating_add(payload_bytes)
                    .saturating_add(SPOOL_RECORD_SUFFIX_BYTES)
            }
        }
    }

    fn write_to<W: Write>(&self, writer: &mut W, path: &Path) -> Result<(), SpoolError> {
        match self {
            Self::Plain { line } => {
                write!(writer, "{:016x} ", line.len())
                    .map_err(|e| SpoolError::io("write spool record header", path, e))?;
                writer
                    .write_all(line.as_bytes())
                    .map_err(|e| SpoolError::io("write spool record body", path, e))?;
            }
            Self::Rle {
                uncompressed_len,
                payload,
            } => {
                write!(writer, "{uncompressed_len:016x} {:016x} ", payload.len())
                    .map_err(|e| SpoolError::io("write compressed spool record header", path, e))?;
                writer
                    .write_all(payload)
                    .map_err(|e| SpoolError::io("write compressed spool record body", path, e))?;
            }
        }
        writer
            .write_all(b"\n")
            .map_err(|e| SpoolError::io("write spool record newline", path, e))
    }
}

struct DiskSpool {
    dir: PathBuf,
    max_bytes: u64,
    segment_bytes: u64,
    compression: SpoolCompression,
    total_bytes: u64,
    current_bytes: u64,
    next_index: u64,
    writer: Option<BufWriter<File>>,
}

impl DiskSpool {
    fn open(cfg: &SpoolConfig) -> Result<Self, SpoolError> {
        let dir = PathBuf::from(&cfg.dir);
        std::fs::create_dir_all(&dir).map_err(|e| SpoolError::io("create spool dir", &dir, e))?;
        let (total_bytes, next_index) = scan_spool_dir(&dir)?;
        Ok(Self {
            dir,
            max_bytes: cfg.max_bytes,
            segment_bytes: cfg.segment_bytes,
            compression: cfg.compression,
            total_bytes,
            current_bytes: 0,
            next_index,
            writer: None,
        })
    }

    fn append(&mut self, line: &str) -> Result<(), SpoolError> {
        let record = PendingSpoolRecord::build(line, self.compression);
        let record_bytes = record.total_bytes();
        if record_bytes > self.max_bytes {
            return Err(SpoolError::RecordTooLarge {
                record_bytes,
                max_bytes: self.max_bytes,
            });
        }
        if self.available_bytes() < record_bytes {
            return Err(SpoolError::Full {
                record_bytes,
                available_bytes: self.available_bytes(),
                max_bytes: self.max_bytes,
            });
        }
        self.ensure_segment(record_bytes)?;
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| SpoolError::Internal("spool segment was not opened".to_string()))?;
        record.write_to(writer, &self.dir)?;
        writer
            .flush()
            .map_err(|e| SpoolError::io("flush spool record", &self.dir, e))?;
        writer
            .get_ref()
            .sync_data()
            .map_err(|e| SpoolError::io("sync spool record", &self.dir, e))?;
        self.current_bytes = self.current_bytes.saturating_add(record_bytes);
        self.total_bytes = self.total_bytes.saturating_add(record_bytes);
        Ok(())
    }

    fn ensure_segment(&mut self, record_bytes: u64) -> Result<(), SpoolError> {
        let needs_segment = self.writer.is_none()
            || self.current_bytes > 0
                && self.current_bytes.saturating_add(record_bytes) > self.segment_bytes;
        if !needs_segment {
            return Ok(());
        }
        self.close_current_segment()?;
        let header = spool_segment_header(self.compression);
        let header_bytes = header.len() as u64;
        let needed_bytes = header_bytes.saturating_add(record_bytes);
        if self.available_bytes() < needed_bytes {
            return Err(SpoolError::Full {
                record_bytes,
                available_bytes: self.available_bytes(),
                max_bytes: self.max_bytes,
            });
        }
        let (path, file) = self.create_next_segment()?;
        let mut writer = BufWriter::with_capacity(JSONL_BUFFER_BYTES, file);
        writer
            .write_all(header)
            .map_err(|e| SpoolError::io("write spool segment header", &path, e))?;
        writer
            .flush()
            .map_err(|e| SpoolError::io("flush spool segment header", &path, e))?;
        writer
            .get_ref()
            .sync_data()
            .map_err(|e| SpoolError::io("sync spool segment header", &path, e))?;
        self.writer = Some(writer);
        self.current_bytes = header_bytes;
        self.total_bytes = self.total_bytes.saturating_add(header_bytes);
        Ok(())
    }

    fn close_current_segment(&mut self) -> Result<(), SpoolError> {
        if let Some(mut writer) = self.writer.take() {
            writer
                .flush()
                .map_err(|e| SpoolError::io("flush spool segment", &self.dir, e))?;
            writer
                .get_ref()
                .sync_data()
                .map_err(|e| SpoolError::io("sync spool segment", &self.dir, e))?;
        }
        self.current_bytes = 0;
        Ok(())
    }

    fn create_next_segment(&mut self) -> Result<(PathBuf, File), SpoolError> {
        for _ in 0..1024 {
            let index = self.next_index;
            self.next_index = self
                .next_index
                .checked_add(1)
                .ok_or_else(|| SpoolError::Internal("spool segment index exhausted".to_string()))?;
            let path = spool_segment_path(&self.dir, index);
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok((path, file)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(SpoolError::io("open spool segment", &path, e)),
            }
        }
        Err(SpoolError::Internal(
            "could not allocate an unused spool segment name".to_string(),
        ))
    }

    fn available_bytes(&self) -> u64 {
        self.max_bytes.saturating_sub(self.total_bytes)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum SpoolError {
    Full {
        record_bytes: u64,
        available_bytes: u64,
        max_bytes: u64,
    },
    RecordTooLarge {
        record_bytes: u64,
        max_bytes: u64,
    },
    Io {
        op: &'static str,
        path: PathBuf,
        message: String,
    },
    Internal(String),
}

impl SpoolError {
    fn io(op: &'static str, path: &Path, err: io::Error) -> Self {
        Self::Io {
            op,
            path: path.to_path_buf(),
            message: err.to_string(),
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::Full {
                record_bytes,
                available_bytes,
                max_bytes,
            } => format!(
                "spool full: record requires {record_bytes} bytes, {available_bytes} bytes remain under max_bytes={max_bytes}"
            ),
            Self::RecordTooLarge {
                record_bytes,
                max_bytes,
            } => format!(
                "spool record is too large: record requires {record_bytes} bytes with max_bytes={max_bytes}"
            ),
            Self::Io { op, path, message } => {
                format!("{op} {}: {message}", path.display())
            }
            Self::Internal(message) => message.clone(),
        }
    }

    fn drops_event(&self) -> bool {
        matches!(self, Self::Full { .. } | Self::RecordTooLarge { .. })
    }
}

fn scan_spool_dir(dir: &Path) -> Result<(u64, u64), SpoolError> {
    let mut total_bytes = 0u64;
    let mut max_index = None::<u64>;
    for entry in std::fs::read_dir(dir).map_err(|e| SpoolError::io("read spool dir", dir, e))? {
        let entry = entry.map_err(|e| SpoolError::io("read spool dir entry", dir, e))?;
        let name = entry.file_name();
        let Some(index) = spool_segment_index(name.to_string_lossy().as_ref()) else {
            continue;
        };
        let len = entry
            .metadata()
            .map_err(|e| SpoolError::io("stat spool segment", &entry.path(), e))?
            .len();
        total_bytes = total_bytes.saturating_add(len);
        max_index = Some(max_index.map_or(index, |current| current.max(index)));
    }
    let next_index = max_index.and_then(|idx| idx.checked_add(1)).unwrap_or(1);
    Ok((total_bytes, next_index))
}

fn spool_segment_path(dir: &Path, index: u64) -> PathBuf {
    dir.join(format!("spool-{index:016}.seg"))
}

fn spool_segment_index(name: &str) -> Option<u64> {
    name.strip_prefix("spool-")
        .and_then(|value| value.strip_suffix(".seg"))
        .and_then(|value| value.parse::<u64>().ok())
}

fn spool_segment_header(compression: SpoolCompression) -> &'static [u8] {
    match compression {
        SpoolCompression::None => SPOOL_SEGMENT_HEADER_V1,
        SpoolCompression::Rle => SPOOL_SEGMENT_HEADER_V2_RLE,
    }
}

fn spool_plain_record_bytes(line: &str) -> u64 {
    let line_bytes = u64::try_from(line.len()).unwrap_or(u64::MAX);
    SPOOL_RECORD_PREFIX_BYTES
        .saturating_add(line_bytes)
        .saturating_add(SPOOL_RECORD_SUFFIX_BYTES)
}

fn rle_compress(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut pos = 0usize;
    while pos < input.len() {
        let run_len = repeated_run_len(input, pos, 130);
        if run_len >= 4 {
            out.push(0x80 | u8::try_from(run_len - 3).unwrap_or(0));
            out.push(input[pos]);
            pos += run_len;
            continue;
        }

        let literal_start = pos;
        let mut literal_len = 0usize;
        while pos < input.len() && literal_len < 128 {
            let run_len = repeated_run_len(input, pos, 130);
            if run_len >= 4 {
                break;
            }
            let take = run_len.min(128 - literal_len);
            pos += take;
            literal_len += take;
            if take < run_len {
                break;
            }
        }
        out.push(u8::try_from(literal_len - 1).unwrap_or(0));
        out.extend_from_slice(&input[literal_start..literal_start + literal_len]);
    }
    out
}

fn repeated_run_len(input: &[u8], start: usize, max_len: usize) -> usize {
    let byte = input[start];
    let mut len = 1usize;
    while start + len < input.len() && len < max_len && input[start + len] == byte {
        len += 1;
    }
    len
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
struct DecodedSpoolRecord {
    line: String,
    compressed: bool,
    uncompressed_len: usize,
    payload_len: usize,
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
enum SpoolSegmentReadError {
    Unsupported(String),
    Corrupt(String),
}

#[cfg(test)]
impl SpoolSegmentReadError {
    fn detail(&self) -> &str {
        match self {
            Self::Unsupported(message) | Self::Corrupt(message) => message,
        }
    }
}

#[cfg(test)]
fn decode_spool_segment_bytes(
    bytes: &[u8],
) -> Result<Vec<DecodedSpoolRecord>, SpoolSegmentReadError> {
    if let Some(body) = bytes.strip_prefix(SPOOL_SEGMENT_HEADER_V1) {
        decode_plain_spool_records(body)
    } else if let Some(body) = bytes.strip_prefix(SPOOL_SEGMENT_HEADER_V2_RLE) {
        decode_rle_spool_records(body)
    } else if bytes.starts_with(b"aegishv-spool-v2 ") {
        Err(SpoolSegmentReadError::Unsupported(
            "unsupported spool segment compression or v2 record header".to_string(),
        ))
    } else {
        Err(SpoolSegmentReadError::Unsupported(
            "unsupported spool segment header".to_string(),
        ))
    }
}

#[cfg(test)]
fn decode_plain_spool_records(
    mut body: &[u8],
) -> Result<Vec<DecodedSpoolRecord>, SpoolSegmentReadError> {
    let mut records = Vec::new();
    while !body.is_empty() {
        let (len, rest) = parse_spool_hex_field(body)?;
        body = rest;
        let len = usize::try_from(len).map_err(|_| {
            SpoolSegmentReadError::Corrupt("plain spool record length exceeds usize".to_string())
        })?;
        if body.len() < len + 1 {
            return Err(SpoolSegmentReadError::Corrupt(
                "plain spool record body is truncated".to_string(),
            ));
        }
        let payload = &body[..len];
        if body[len] != b'\n' {
            return Err(SpoolSegmentReadError::Corrupt(
                "plain spool record missing trailing newline".to_string(),
            ));
        }
        let line = std::str::from_utf8(payload)
            .map_err(|_| {
                SpoolSegmentReadError::Corrupt("plain spool record is not UTF-8".to_string())
            })?
            .to_string();
        records.push(DecodedSpoolRecord {
            line,
            compressed: false,
            uncompressed_len: len,
            payload_len: len,
        });
        body = &body[len + 1..];
    }
    Ok(records)
}

#[cfg(test)]
fn decode_rle_spool_records(
    mut body: &[u8],
) -> Result<Vec<DecodedSpoolRecord>, SpoolSegmentReadError> {
    let mut records = Vec::new();
    while !body.is_empty() {
        let (uncompressed_len, rest) = parse_spool_hex_field(body)?;
        let (payload_len, rest) = parse_spool_hex_field(rest)?;
        body = rest;
        let uncompressed_len = usize::try_from(uncompressed_len).map_err(|_| {
            SpoolSegmentReadError::Corrupt(
                "compressed spool record uncompressed length exceeds usize".to_string(),
            )
        })?;
        let payload_len = usize::try_from(payload_len).map_err(|_| {
            SpoolSegmentReadError::Corrupt(
                "compressed spool record payload length exceeds usize".to_string(),
            )
        })?;
        if body.len() < payload_len + 1 {
            return Err(SpoolSegmentReadError::Corrupt(
                "compressed spool record payload is truncated".to_string(),
            ));
        }
        let payload = &body[..payload_len];
        if body[payload_len] != b'\n' {
            return Err(SpoolSegmentReadError::Corrupt(
                "compressed spool record missing trailing newline".to_string(),
            ));
        }
        let decoded = rle_decompress(payload, uncompressed_len)?;
        let line = String::from_utf8(decoded).map_err(|_| {
            SpoolSegmentReadError::Corrupt(
                "compressed spool record decompressed to non-UTF-8 data".to_string(),
            )
        })?;
        records.push(DecodedSpoolRecord {
            line,
            compressed: true,
            uncompressed_len,
            payload_len,
        });
        body = &body[payload_len + 1..];
    }
    Ok(records)
}

#[cfg(test)]
fn parse_spool_hex_field(bytes: &[u8]) -> Result<(u64, &[u8]), SpoolSegmentReadError> {
    if bytes.len() < 17 {
        return Err(SpoolSegmentReadError::Corrupt(
            "spool record hex field is truncated".to_string(),
        ));
    }
    if bytes[16] != b' ' {
        return Err(SpoolSegmentReadError::Corrupt(
            "spool record hex field missing separator".to_string(),
        ));
    }
    let text = std::str::from_utf8(&bytes[..16]).map_err(|_| {
        SpoolSegmentReadError::Corrupt("spool record hex field is not UTF-8".to_string())
    })?;
    let value = u64::from_str_radix(text, 16).map_err(|_| {
        SpoolSegmentReadError::Corrupt("spool record hex field is not hexadecimal".to_string())
    })?;
    Ok((value, &bytes[17..]))
}

#[cfg(test)]
fn rle_decompress(payload: &[u8], expected_len: usize) -> Result<Vec<u8>, SpoolSegmentReadError> {
    let mut out = Vec::with_capacity(expected_len);
    let mut pos = 0usize;
    while pos < payload.len() {
        let control = payload[pos];
        pos += 1;
        if control & 0x80 == 0 {
            let len = usize::from(control) + 1;
            if payload.len().saturating_sub(pos) < len {
                return Err(SpoolSegmentReadError::Corrupt(
                    "RLE literal run is truncated".to_string(),
                ));
            }
            out.extend_from_slice(&payload[pos..pos + len]);
            pos += len;
        } else {
            let len = usize::from(control & 0x7f) + 3;
            let Some(&byte) = payload.get(pos) else {
                return Err(SpoolSegmentReadError::Corrupt(
                    "RLE repeated run is truncated".to_string(),
                ));
            };
            pos += 1;
            out.resize(out.len().saturating_add(len), byte);
        }
        if out.len() > expected_len {
            return Err(SpoolSegmentReadError::Corrupt(
                "RLE payload expands past the declared length".to_string(),
            ));
        }
    }
    if out.len() != expected_len {
        return Err(SpoolSegmentReadError::Corrupt(
            "RLE payload does not match the declared length".to_string(),
        ));
    }
    Ok(out)
}

#[derive(Debug, PartialEq, Eq)]
enum ConfigReloadError {
    MissingConfigPath,
    Load(String),
    Build(String),
}

impl ConfigReloadError {
    fn detail(&self) -> String {
        match self {
            ConfigReloadError::MissingConfigPath => {
                "run was started without --config; no file is available to reload".to_string()
            }
            ConfigReloadError::Load(err) => format!("config load failed: {err}"),
            ConfigReloadError::Build(err) => format!("runtime rebuild failed: {err}"),
        }
    }
}

fn effective_quiet(cfg: &Config, quiet_flag: bool, no_quiet_flag: bool) -> bool {
    if quiet_flag {
        true
    } else if no_quiet_flag {
        false
    } else {
        cfg.general.quiet
    }
}

fn load_runtime_state(
    config_path: Option<&Path>,
    quiet_flag: bool,
    no_quiet_flag: bool,
) -> Result<RuntimeState, ConfigReloadError> {
    let path = config_path.ok_or(ConfigReloadError::MissingConfigPath)?;
    let cfg = Config::load(Some(path)).map_err(ConfigReloadError::Load)?;
    RuntimeState::new(&cfg, quiet_flag, no_quiet_flag).map_err(ConfigReloadError::Build)
}

fn apply_config_reload(
    config_path: Option<&Path>,
    runtime: &mut RuntimeState,
    quiet_flag: bool,
    no_quiet_flag: bool,
) -> Event {
    let Some(path) = config_path else {
        return sensor_event(
            Severity::Info,
            "config_reload_skipped",
            "received SIGHUP but run was started without --config; keeping current defaults"
                .to_string(),
        );
    };
    match load_runtime_state(Some(path), quiet_flag, no_quiet_flag) {
        Ok(next) => {
            let policy_rule_count = next.policy_rule_count;
            let qmp_mapping_count = next.qmp_mapping_count;
            *runtime = next;
            sensor_event(
                Severity::Info,
                "config_reload",
                format!(
                    "reloaded config from {}; applied {} enabled policy rules, {} QMP mappings, allowlist, identity lookup settings, W^X detector settings with fresh detector state, quiet, and flush cadence; collector source, queue, JSONL path, metrics listener, and PMU thread remain startup-only; file JSONL output is reopened separately on SIGHUP",
                    path.display(),
                    policy_rule_count,
                    qmp_mapping_count
                ),
            )
        }
        Err(err) => sensor_event(
            Severity::High,
            "config_reload_failed",
            format!(
                "SIGHUP reload from {} failed; keeping last good config: {}",
                path.display(),
                err.detail()
            ),
        ),
    }
}

fn jsonl_reopen_event(result: Result<JsonlReopenOutcome, String>) -> Event {
    match result {
        Ok(JsonlReopenOutcome::Reopened(path)) => sensor_event(
            Severity::Info,
            "jsonl_reopen",
            format!("reopened JSONL output file {} after SIGHUP", path.display()),
        ),
        Ok(JsonlReopenOutcome::StdoutSkipped) => sensor_event(
            Severity::Info,
            "jsonl_reopen_skipped",
            "SIGHUP does not reopen stdout JSONL output".to_string(),
        ),
        Err(err) => sensor_event(
            Severity::High,
            "jsonl_reopen_failed",
            format!("SIGHUP JSONL reopen failed; keeping existing output writer: {err}"),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn maybe_handle_sighup(
    sink: &mut dyn EventSink,
    metrics: &Metrics,
    loss: &mut LossTracker,
    config_path: Option<&Path>,
    runtime: &mut RuntimeState,
    quiet_flag: bool,
    no_quiet_flag: bool,
    replay_determinism: &mut ReplayDeterminism,
) -> Result<(), String> {
    if !observe_reload_signal() {
        return Ok(());
    }
    let mut ev = apply_config_reload(config_path, runtime, quiet_flag, no_quiet_flag);
    match ev.reason.as_deref() {
        Some("config_reload") => {
            metrics.mark_policy_ok();
            metrics.mark_actions_ok();
            metrics.set_identity_inventory(&runtime.identity.inventory_snapshot());
        }
        Some("config_reload_failed") => metrics.mark_policy_degraded(),
        _ => {}
    }
    let reopen_result = sink.reopen_jsonl();
    if reopen_result.is_err() {
        metrics.mark_output_degraded();
    }
    let mut reopen_ev = jsonl_reopen_event(reopen_result);
    metrics.set_wx_pages_tracked(runtime.wx.pages_tracked());
    emit(
        sink,
        metrics,
        loss,
        runtime.quiet,
        &mut ev,
        replay_determinism,
    )?;
    maybe_flush(sink, metrics, runtime.flush_every, ev.severity)?;
    emit(
        sink,
        metrics,
        loss,
        runtime.quiet,
        &mut reopen_ev,
        replay_determinism,
    )?;
    maybe_flush(sink, metrics, runtime.flush_every, reopen_ev.severity)?;
    sink.flush(metrics, "flush jsonl after SIGHUP handling")
}

#[allow(clippy::too_many_arguments)]
fn emit_tracepoint_diagnostics(
    sink: &mut dyn EventSink,
    metrics: &Metrics,
    loss: &mut LossTracker,
    diagnostics: &[TracepointDiagnostic],
    quiet: bool,
    flush_every: usize,
    replay_determinism: &mut ReplayDeterminism,
) -> Result<(), String> {
    let unhealthy = diagnostics
        .iter()
        .filter(|diagnostic| !diagnostic.is_ok())
        .collect::<Vec<_>>();
    if unhealthy.is_empty() {
        return Ok(());
    }
    let summary = unhealthy
        .iter()
        .map(|diagnostic| {
            format!(
                "{}/{} status={} missing_fields=[{}]",
                diagnostic.system,
                diagnostic.name,
                diagnostic.status.as_str(),
                diagnostic.missing_fields.join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let mut ev = sensor_event(
        Severity::High,
        "tracefs_format_diagnostic",
        format!(
            "tracefs metadata diagnostic: {summary}; parser remains text-based, but live tracefs metadata is not healthy"
        ),
    );
    ev.tags.push("tracefs".to_string());
    emit(sink, metrics, loss, quiet, &mut ev, replay_determinism)?;
    maybe_flush(sink, metrics, flush_every, ev.severity)
}

fn startup_lifecycle_event(
    cfg: &Config,
    runtime: &RuntimeState,
    startup: LifecycleStartup<'_>,
) -> Event {
    let deterministic_replay = if startup.deterministic_replay {
        ", deterministic_replay=enabled"
    } else {
        ""
    };
    let mut ev = sensor_event(
        Severity::Info,
        "sensor_startup",
        format!(
            "startup: version={}, mode={}, sensor=host-side KVM tracefs text sensor, config={}, jsonl={}, metrics_listener={}, queue_capacity={}, policy_rules={}, qmp_mappings={}, identity={}, stable_qmp_required={}, pmu_fallback={}, spool={}, wx_window_ms={}, wx_cooldown_ms={}{}; unsupported: type1=false, full_vmi=false, ept_npt_enforcement=false, syscall_integrity=false, hardware_pmu_sampling=false",
            env!("CARGO_PKG_VERSION"),
            startup.mode,
            startup.config_source,
            startup.jsonl_target,
            startup.metrics_listener,
            startup.queue_capacity,
            runtime.policy_rule_count,
            runtime.qmp_mapping_count,
            enabled_label(cfg.identity.enable),
            cfg.identity.require_stable_qmp_match,
            enabled_label(startup.pmu_enabled),
            enabled_label(cfg.spool.enable),
            cfg.general.wx_window_ms,
            cfg.general.wx_cooldown_ms,
            deterministic_replay
        ),
    );
    ev.tags.push("lifecycle".to_string());
    ev
}

fn shutdown_lifecycle_event(reason: &ShutdownReason, metrics: &Metrics) -> Event {
    let severity = match reason {
        ShutdownReason::Failure(_) => Severity::High,
        _ => Severity::Info,
    };
    let mut ev = sensor_event(
        severity,
        "sensor_shutdown",
        format!(
            "shutdown: reason={}, detail={}, dropped_total={}, json_write_failures_total={}, spool_events_total={}, spool_write_failures_total={}, spool_dropped_total={}",
            reason.label(),
            reason.detail(),
            metrics.dropped_total(),
            metrics.json_write_failures_total(),
            metrics.spool_events_total(),
            metrics.spool_write_failures_total(),
            metrics.spool_dropped_total()
        ),
    );
    ev.tags.push("lifecycle".to_string());
    ev
}

fn enabled_label(value: bool) -> &'static str {
    if value {
        "enabled"
    } else {
        "disabled"
    }
}

fn sensor_event(severity: Severity, reason: &str, message: String) -> Event {
    let mut ev = Event::base(
        Category::Sensor,
        severity,
        now_rfc3339(),
        "host".to_string(),
    );
    ev.reason = Some(reason.to_string());
    ev.message = Some(message);
    ev
}

fn shutdown_event(signal_name: &str) -> Event {
    sensor_event(
        Severity::Info,
        "shutdown_signal",
        format!("received {signal_name}; stopping collectors and flushing JSONL"),
    )
}

fn observe_shutdown_signal(stop: &AtomicBool, event_emitted: &mut bool) -> Option<&'static str> {
    if !SHUTDOWN_SIGNAL_RECEIVED.load(Ordering::Relaxed) {
        return None;
    }
    stop.store(true, Ordering::Relaxed);
    if *event_emitted {
        return None;
    }
    *event_emitted = true;
    Some(shutdown_signal_name())
}

fn shutdown_signal_name() -> &'static str {
    match SHUTDOWN_SIGNAL_KIND.load(Ordering::Relaxed) {
        SHUTDOWN_SIGINT => "SIGINT",
        SHUTDOWN_SIGTERM => "SIGTERM",
        SHUTDOWN_CONSOLE => "console control signal",
        _ => "shutdown signal",
    }
}

fn note_shutdown_signal(kind: u8) {
    let _ = SHUTDOWN_SIGNAL_KIND.compare_exchange(
        SHUTDOWN_NONE,
        kind,
        Ordering::Relaxed,
        Ordering::Relaxed,
    );
    SHUTDOWN_SIGNAL_RECEIVED.store(true, Ordering::Relaxed);
}

#[cfg(any(unix, test))]
fn note_reload_signal() {
    RELOAD_SIGNAL_RECEIVED.store(true, Ordering::Relaxed);
}

fn observe_reload_signal() -> bool {
    RELOAD_SIGNAL_RECEIVED.swap(false, Ordering::Relaxed)
}

fn reset_shutdown_signal_for_run() {
    SHUTDOWN_SIGNAL_KIND.store(SHUTDOWN_NONE, Ordering::Relaxed);
    SHUTDOWN_SIGNAL_RECEIVED.store(false, Ordering::Relaxed);
    RELOAD_SIGNAL_RECEIVED.store(false, Ordering::Relaxed);
}

fn install_shutdown_signal_handlers() -> Result<(), String> {
    if SHUTDOWN_SIGNAL_HANDLERS_INSTALLED.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    if let Err(err) = install_platform_signal_handlers() {
        SHUTDOWN_SIGNAL_HANDLERS_INSTALLED.store(false, Ordering::Release);
        return Err(err);
    }
    Ok(())
}

#[cfg(unix)]
fn install_platform_signal_handlers() -> Result<(), String> {
    const SIGHUP: i32 = 1;
    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;
    unsafe {
        install_unix_signal(SIGHUP, "SIGHUP")?;
        install_unix_signal(SIGINT, "SIGINT")?;
        install_unix_signal(SIGTERM, "SIGTERM")?;
    }
    Ok(())
}

#[cfg(unix)]
unsafe fn install_unix_signal(signum: i32, name: &str) -> Result<(), String> {
    extern "C" {
        fn signal(signum: i32, handler: extern "C" fn(i32)) -> usize;
    }
    const SIG_ERR: usize = usize::MAX;
    // The handler only writes atomics. It must not allocate, lock, or touch Rust-owned objects.
    let previous = signal(signum, handle_unix_signal);
    if previous == SIG_ERR {
        return Err(format!("install {name} handler: signal returned SIG_ERR"));
    }
    Ok(())
}

#[cfg(unix)]
extern "C" fn handle_unix_signal(signum: i32) {
    const SIGHUP: i32 = 1;
    const SIGTERM: i32 = 15;
    if signum == SIGHUP {
        note_reload_signal();
        return;
    }
    let kind = if signum == SIGTERM {
        SHUTDOWN_SIGTERM
    } else {
        SHUTDOWN_SIGINT
    };
    note_shutdown_signal(kind);
}

#[cfg(windows)]
fn install_platform_signal_handlers() -> Result<(), String> {
    type ConsoleHandler = Option<unsafe extern "system" fn(u32) -> i32>;
    extern "system" {
        fn SetConsoleCtrlHandler(handler: ConsoleHandler, add: i32) -> i32;
    }
    unsafe {
        if SetConsoleCtrlHandler(Some(handle_windows_console_signal), 1) == 0 {
            return Err("install console signal handler: SetConsoleCtrlHandler failed".to_string());
        }
    }
    Ok(())
}

#[cfg(windows)]
unsafe extern "system" fn handle_windows_console_signal(ctrl_type: u32) -> i32 {
    match ctrl_type {
        0 | 1 | 2 | 5 | 6 => {
            note_shutdown_signal(SHUTDOWN_CONSOLE);
            1
        }
        _ => 0,
    }
}

#[cfg(not(any(unix, windows)))]
fn install_platform_signal_handlers() -> Result<(), String> {
    Ok(())
}

fn maybe_flush(
    sink: &mut dyn EventSink,
    metrics: &Metrics,
    flush_every: usize,
    severity: Severity,
) -> Result<(), String> {
    sink.maybe_flush(metrics, flush_every, severity)
}

fn flush_jsonl(sink: &mut dyn EventSink, metrics: &Metrics, context: &str) -> Result<(), String> {
    sink.flush(metrics, context)
}

fn emit(
    sink: &mut dyn EventSink,
    metrics: &Metrics,
    loss: &mut LossTracker,
    quiet: bool,
    ev: &mut Event,
    replay_determinism: &mut ReplayDeterminism,
) -> Result<(), String> {
    replay_determinism.normalize(ev);
    loss.decorate(ev, metrics);
    metrics.inc_event(ev.category, ev.severity);
    let line = ev.to_json();
    let status = sink.write_event_line(metrics, ev.category, ev.severity, &line)?;
    loss.finish_emit(ev.sequence, status);
    if status == SinkWriteStatus::Written && !quiet && ev.severity.at_least(Severity::High) {
        eprintln!(
            "[{}] {:?} vm={} reason={} gpa={} rip={}",
            ev.severity.as_str(),
            ev.category,
            ev.vm,
            ev.reason.as_deref().unwrap_or(""),
            ev.addr
                .as_ref()
                .and_then(|a| a.gpa.as_deref())
                .unwrap_or(""),
            ev.addr
                .as_ref()
                .and_then(|a| a.rip.as_deref())
                .unwrap_or("")
        );
    }
    Ok(())
}

fn start_metrics_server(
    listen: &str,
    metrics: Metrics,
    stop: Arc<AtomicBool>,
    allow_bind_failure: bool,
) -> Result<Option<thread::JoinHandle<()>>, String> {
    let listen = listen.trim();
    if listen.is_empty() {
        metrics.mark_metrics_listener_disabled();
        return Ok(None);
    }
    let listener = match TcpListener::bind(listen) {
        Ok(listener) => {
            metrics.mark_metrics_listener_running();
            listener
        }
        Err(e) if allow_bind_failure => {
            metrics.mark_metrics_listener_degraded();
            eprintln!(
                "metrics listener bind failed on {listen}: {e}; continuing because metrics.allow_bind_failure=true"
            );
            return Ok(None);
        }
        Err(e) => {
            return Err(format!(
                "metrics listener bind failed on {listen}: {e}; use --listen '' to disable metrics or set metrics.allow_bind_failure=true for explicit degraded startup"
            ))
        }
    };
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("configure metrics listener {listen} nonblocking: {e}"))?;
    Ok(Some(thread::spawn(move || {
        serve_metrics(listener, metrics, stop)
    })))
}

fn serve_metrics(listener: TcpListener, metrics: Metrics, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buf = [0u8; 1024];
                let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                if path == "/metrics" {
                    let body = metrics.encode();
                    let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                    let _ = stream.write_all(resp.as_bytes());
                } else if path == "/healthz" || path == "/readyz" {
                    let (status, body) = health_http_response(path, &metrics);
                    let resp = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                    let _ = stream.write_all(resp.as_bytes());
                } else {
                    let body = "not found\n";
                    let resp = format!("HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                    let _ = stream.write_all(resp.as_bytes());
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100))
            }
            Err(_) => thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn health_http_response(path: &str, metrics: &Metrics) -> (&'static str, String) {
    let snapshot = metrics.health_snapshot();
    let ok = if path == "/readyz" {
        snapshot.ready
    } else {
        snapshot.healthy
    };
    let status = if ok {
        "200 OK"
    } else {
        "503 Service Unavailable"
    };
    (status, snapshot.to_json())
}

#[cfg(test)]
mod shutdown_tests {
    use super::*;
    use aegishv::metrics::ComponentStatus;
    use aegishv::trace_format::TracepointDiagnosticStatus;
    use std::io::Write;

    fn temp_config(contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "aegishv-reload-{}-{}.toml",
            std::process::id(),
            aegishv::util::next_sequence()
        ));
        let mut file = std::fs::File::create(&path).expect("create temp config");
        write!(file, "{contents}").expect("write temp config");
        path
    }

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aegishv-{label}-{}-{}",
            std::process::id(),
            aegishv::util::next_sequence()
        ))
    }

    fn temp_dir(label: &str) -> PathBuf {
        let path = temp_path(label);
        std::fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn write_kvm_exit_format(root: &Path) {
        let dir = root.join("events/kvm/kvm_exit");
        std::fs::create_dir_all(&dir).expect("create kvm_exit format dir");
        std::fs::write(
            dir.join("format"),
            "name: kvm_exit\nID: 123\nformat:\n\tfield:u32 vcpu_id;\toffset:8;\tsize:4;\tsigned:0;\n\tfield:u32 exit_reason;\toffset:12;\tsize:4;\tsigned:0;\n\tfield:unsigned long guest_rip;\toffset:16;\tsize:8;\tsigned:0;\n",
        )
        .expect("write kvm_exit format");
    }

    fn test_spool_config(dir: &Path, max_bytes: u64, segment_bytes: u64) -> SpoolConfig {
        SpoolConfig {
            enable: true,
            dir: dir.display().to_string(),
            max_bytes,
            segment_bytes,
            compression: SpoolCompression::None,
        }
    }

    fn test_compressed_spool_config(dir: &Path, max_bytes: u64, segment_bytes: u64) -> SpoolConfig {
        let mut cfg = test_spool_config(dir, max_bytes, segment_bytes);
        cfg.compression = SpoolCompression::Rle;
        cfg
    }

    fn memory_jsonl(target: JsonlTarget) -> JsonlOutput {
        JsonlOutput {
            target,
            writer: BufWriter::with_capacity(
                JSONL_BUFFER_BYTES,
                Box::new(Vec::<u8>::new()) as Box<dyn Write + Send>,
            ),
        }
    }

    fn failing_jsonl_event_sink(spool: EventSpool) -> JsonlEventSink {
        JsonlEventSink {
            output: JsonlOutput {
                target: JsonlTarget::Stdout,
                writer: BufWriter::with_capacity(1, Box::new(FailingWriter)),
            },
            spool,
            syslog: SyslogOutput::Disabled,
            journald: JournaldOutput::Disabled,
            pending_flush: 0,
        }
    }

    fn memory_jsonl_event_sink(syslog: SyslogOutput) -> JsonlEventSink {
        JsonlEventSink {
            output: memory_jsonl(JsonlTarget::Stdout),
            spool: EventSpool::Disabled,
            syslog,
            journald: JournaldOutput::Disabled,
            pending_flush: 0,
        }
    }

    fn memory_jsonl_event_sink_with_journald(journald: JournaldOutput) -> JsonlEventSink {
        JsonlEventSink {
            output: memory_jsonl(JsonlTarget::Stdout),
            spool: EventSpool::Disabled,
            syslog: SyslogOutput::Disabled,
            journald,
            pending_flush: 0,
        }
    }

    fn mark_ready_baseline(metrics: &Metrics) {
        metrics.mark_runtime_running();
        metrics.mark_collector_running();
        metrics.mark_metrics_listener_running();
        metrics.mark_output_ok();
        metrics.mark_policy_ok();
        metrics.mark_pmu_disabled();
        metrics.set_queue_depth(0, 4);
        metrics.mark_actions_ok();
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "test writer is closed",
            ))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl EventSink for Vec<u8> {
        fn write_event_line(
            &mut self,
            _metrics: &Metrics,
            _category: Category,
            _severity: Severity,
            line: &str,
        ) -> Result<SinkWriteStatus, String> {
            self.write_all(line.as_bytes())
                .and_then(|_| self.write_all(b"\n"))
                .map_err(|e| format!("write test sink: {e}"))?;
            Ok(SinkWriteStatus::Written)
        }

        fn maybe_flush(
            &mut self,
            _metrics: &Metrics,
            _flush_every: usize,
            _severity: Severity,
        ) -> Result<(), String> {
            Ok(())
        }

        fn flush(&mut self, _metrics: &Metrics, _context: &str) -> Result<(), String> {
            Ok(())
        }

        fn reopen_jsonl(&mut self) -> Result<JsonlReopenOutcome, String> {
            Ok(JsonlReopenOutcome::StdoutSkipped)
        }
    }

    #[test]
    fn shutdown_signal_sets_stop_and_emits_once() {
        reset_shutdown_signal_for_run();
        let stop = AtomicBool::new(false);
        let mut emitted = false;

        assert_eq!(observe_shutdown_signal(&stop, &mut emitted), None);
        assert!(!stop.load(Ordering::Relaxed));

        note_shutdown_signal(SHUTDOWN_SIGTERM);
        assert_eq!(
            observe_shutdown_signal(&stop, &mut emitted),
            Some("SIGTERM")
        );
        assert!(stop.load(Ordering::Relaxed));
        assert_eq!(observe_shutdown_signal(&stop, &mut emitted), None);

        reset_shutdown_signal_for_run();
    }

    #[test]
    fn shutdown_event_uses_sensor_contract() {
        let ev = shutdown_event("SIGINT");

        assert_eq!(ev.category, Category::Sensor);
        assert_eq!(ev.severity, Severity::Info);
        assert_eq!(ev.reason.as_deref(), Some("shutdown_signal"));
        assert!(ev
            .message
            .as_deref()
            .expect("shutdown message")
            .contains("SIGINT"));
    }

    #[test]
    fn empty_metrics_listen_disables_listener() {
        let metrics = Metrics::new().unwrap();
        let stop = Arc::new(AtomicBool::new(false));

        let handle = start_metrics_server("  ", metrics, stop, false).unwrap();

        assert!(handle.is_none());
    }

    #[test]
    fn metrics_listener_binds_when_address_is_available() {
        let metrics = Metrics::new().unwrap();
        let stop = Arc::new(AtomicBool::new(false));

        let handle = start_metrics_server("127.0.0.1:0", metrics, stop.clone(), false)
            .unwrap()
            .expect("metrics thread");

        stop.store(true, Ordering::Relaxed);
        handle.join().expect("metrics thread join");
    }

    #[test]
    fn metrics_bind_failure_is_fatal_by_default() {
        let blocker = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = blocker.local_addr().unwrap().to_string();
        let metrics = Metrics::new().unwrap();
        let stop = Arc::new(AtomicBool::new(false));

        let err = start_metrics_server(&addr, metrics, stop, false).unwrap_err();

        assert!(err.contains("metrics listener bind failed"));
        assert!(err.contains("--listen ''"));
        assert!(err.contains("metrics.allow_bind_failure=true"));
    }

    #[test]
    fn metrics_bind_failure_can_be_explicitly_degraded() {
        let blocker = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = blocker.local_addr().unwrap().to_string();
        let metrics = Metrics::new().unwrap();
        let stop = Arc::new(AtomicBool::new(false));

        let handle = start_metrics_server(&addr, metrics.clone(), stop, true).unwrap();

        assert!(handle.is_none());
        let snapshot = metrics.health_snapshot();
        assert_eq!(
            snapshot.components.metrics_listener,
            ComponentStatus::Degraded
        );
        assert!(snapshot.healthy);
        assert!(!snapshot.ready);
    }

    #[test]
    fn health_endpoint_allows_degraded_liveness_but_readyz_rejects_it() {
        let metrics = Metrics::new().unwrap();
        mark_ready_baseline(&metrics);
        metrics.mark_output_degraded();

        let (health_status, health_body) = health_http_response("/healthz", &metrics);
        let (ready_status, ready_body) = health_http_response("/readyz", &metrics);

        assert_eq!(health_status, "200 OK");
        assert_eq!(ready_status, "503 Service Unavailable");
        assert!(health_body.contains("\"status\":\"degraded\""));
        assert!(health_body.contains("\"output\":\"degraded\""));
        assert!(ready_body.contains("\"ready\":false"));
    }

    #[test]
    fn failed_runtime_rejects_healthz_and_readyz() {
        let metrics = Metrics::new().unwrap();
        mark_ready_baseline(&metrics);
        metrics.mark_collector_failed();
        metrics.mark_runtime_failed();

        let (health_status, health_body) = health_http_response("/healthz", &metrics);
        let (ready_status, ready_body) = health_http_response("/readyz", &metrics);

        assert_eq!(health_status, "503 Service Unavailable");
        assert_eq!(ready_status, "503 Service Unavailable");
        assert!(health_body.contains("\"status\":\"failed\""));
        assert!(ready_body.contains("\"collector\":\"failed\""));
    }

    #[test]
    fn syslog_sink_sends_udp_copy_with_bounded_priority() {
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let syslog = SyslogConfig {
            enable: true,
            address: receiver.local_addr().unwrap().to_string(),
            facility: "local0".to_string(),
            max_message_bytes: 8192,
        };
        let mut sink =
            memory_jsonl_event_sink(SyslogOutput::open(&syslog).expect("open syslog test sink"));
        let metrics = Metrics::new().unwrap();
        let mut loss = LossTracker::default();
        let mut replay_determinism = ReplayDeterminism::disabled();
        let mut ev = sensor_event(
            Severity::High,
            "syslog_test",
            "syslog UDP copy test".to_string(),
        );

        emit(
            &mut sink,
            &metrics,
            &mut loss,
            true,
            &mut ev,
            &mut replay_determinism,
        )
        .expect("emit to syslog sink");

        let mut buf = [0u8; 16 * 1024];
        let (n, _) = receiver
            .recv_from(&mut buf)
            .expect("receive syslog datagram");
        let message = std::str::from_utf8(&buf[..n]).expect("syslog utf-8");
        assert!(message.starts_with("<131>1 - aegishv - - - - "));
        assert!(message.contains("\"category\":\"sensor\""));
        assert!(message.contains("\"severity\":\"high\""));
        assert!(message.contains("\"reason\":\"syslog_test\""));
    }

    #[test]
    fn syslog_message_size_failure_is_counted_and_fatal() {
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        let syslog = SyslogConfig {
            enable: true,
            address: receiver.local_addr().unwrap().to_string(),
            facility: "local0".to_string(),
            max_message_bytes: 1,
        };
        let mut sink =
            memory_jsonl_event_sink(SyslogOutput::open(&syslog).expect("open syslog test sink"));
        let metrics = Metrics::new().unwrap();
        let mut loss = LossTracker::default();
        let mut replay_determinism = ReplayDeterminism::disabled();
        let mut ev = sensor_event(
            Severity::Info,
            "syslog_size_limit_test",
            "syslog message should exceed the configured test limit".to_string(),
        );

        let err = emit(
            &mut sink,
            &metrics,
            &mut loss,
            true,
            &mut ev,
            &mut replay_determinism,
        )
        .expect_err("oversized syslog message must fail explicitly");

        assert!(err.contains("exceeds syslog.max_message_bytes"));
        assert_eq!(metrics.syslog_write_failures_total(), 1);
        let snapshot = metrics.health_snapshot();
        assert_eq!(snapshot.components.output, ComponentStatus::Failed);
        assert!(!snapshot.healthy);
    }

    #[test]
    fn journald_sink_writes_bounded_structured_fields_to_test_writer() {
        let buffer = Arc::new(std::sync::Mutex::new(Vec::new()));
        let journald = JournaldOutput::test_memory(buffer.clone(), "aegishv-test", 8192);
        let mut sink = memory_jsonl_event_sink_with_journald(journald);
        let metrics = Metrics::new().unwrap();
        let mut loss = LossTracker::default();
        let mut replay_determinism = ReplayDeterminism::disabled();
        let mut ev = sensor_event(
            Severity::High,
            "journald_test",
            "journald structured copy test".to_string(),
        );

        emit(
            &mut sink,
            &metrics,
            &mut loss,
            true,
            &mut ev,
            &mut replay_determinism,
        )
        .expect("emit to journald test sink");

        let message =
            String::from_utf8(buffer.lock().expect("journald test buffer").clone()).unwrap();
        assert!(message.contains("PRIORITY=3\n"));
        assert!(message.contains("SYSLOG_IDENTIFIER=aegishv-test\n"));
        assert!(message.contains("AEGISHV_CATEGORY=sensor\n"));
        assert!(message.contains("AEGISHV_SEVERITY=high\n"));
        assert!(message.contains("MESSAGE={\"version\":1"));
        assert!(message.contains("\"reason\":\"journald_test\""));
    }

    #[test]
    fn journald_message_size_failure_is_counted_and_fatal() {
        let buffer = Arc::new(std::sync::Mutex::new(Vec::new()));
        let journald = JournaldOutput::test_memory(buffer, "aegishv-test", 1);
        let mut sink = memory_jsonl_event_sink_with_journald(journald);
        let metrics = Metrics::new().unwrap();
        let mut loss = LossTracker::default();
        let mut replay_determinism = ReplayDeterminism::disabled();
        let mut ev = sensor_event(
            Severity::Info,
            "journald_size_limit_test",
            "journald message should exceed the configured test limit".to_string(),
        );

        let err = emit(
            &mut sink,
            &metrics,
            &mut loss,
            true,
            &mut ev,
            &mut replay_determinism,
        )
        .expect_err("oversized journald message must fail explicitly");

        assert!(err.contains("exceeds journald.max_message_bytes"));
        assert_eq!(metrics.journald_write_failures_total(), 1);
        let snapshot = metrics.health_snapshot();
        assert_eq!(snapshot.components.output, ComponentStatus::Failed);
        assert!(!snapshot.healthy);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn enabled_journald_is_explicitly_unsupported_on_non_linux() {
        let cfg = JournaldConfig {
            enable: true,
            socket: "/run/systemd/journal/socket".to_string(),
            identifier: "aegishv".to_string(),
            max_message_bytes: 8192,
        };

        let err = match JournaldOutput::open(&cfg) {
            Ok(_) => panic!("enabled journald must not open on non-Linux hosts"),
            Err(e) => e,
        };

        assert!(err.contains("requires Linux"));
    }

    #[test]
    fn startup_lifecycle_event_summarizes_runtime_without_paths() {
        let mut cfg = Config::default();
        cfg.spool.enable = true;
        let runtime = RuntimeState::new(&cfg, false, false).unwrap();

        let ev = startup_lifecycle_event(
            &cfg,
            &runtime,
            LifecycleStartup {
                mode: "tracefs",
                config_source: "file",
                jsonl_target: "file",
                metrics_listener: "enabled",
                queue_capacity: 64,
                pmu_enabled: false,
                deterministic_replay: false,
            },
        );

        let message = ev.message.as_deref().expect("startup message");
        assert_eq!(ev.category, Category::Sensor);
        assert_eq!(ev.reason.as_deref(), Some("sensor_startup"));
        assert!(message.contains("mode=tracefs"));
        assert!(message.contains("policy_rules=0"));
        assert!(message.contains("spool=enabled"));
        assert!(message.contains("type1=false"));
        assert!(!message.contains("/run/libvirt"));
        assert!(!message.contains("/var/lib/aegishv"));
    }

    #[test]
    fn shutdown_lifecycle_event_reports_failure_and_loss_counters() {
        let metrics = Metrics::new().unwrap();
        metrics.inc_dropped("queue_full");
        metrics.inc_json_write_failure();
        metrics.inc_spool_event();
        metrics.inc_spool_write_failure();
        metrics.inc_spool_dropped();

        let reason = ShutdownReason::Failure("collector thread panicked".to_string());
        let ev = shutdown_lifecycle_event(&reason, &metrics);

        let message = ev.message.as_deref().expect("shutdown message");
        assert_eq!(ev.category, Category::Sensor);
        assert_eq!(ev.severity, Severity::High);
        assert_eq!(ev.reason.as_deref(), Some("sensor_shutdown"));
        assert!(message.contains("reason=failure"));
        assert!(message.contains("dropped_total=1"));
        assert!(message.contains("json_write_failures_total=1"));
        assert!(message.contains("spool_dropped_total=1"));
    }

    #[test]
    fn queue_drop_loss_report_uses_aggregate_counter_range() {
        let metrics = Metrics::new().unwrap();
        metrics.inc_dropped("queue_full");
        let mut loss = LossTracker::default();
        let mut out = Vec::<u8>::new();
        let mut replay_determinism = ReplayDeterminism::disabled();
        let mut ev = sensor_event(
            Severity::High,
            "loss_range_test",
            "forced queue drop".to_string(),
        );

        emit(
            &mut out,
            &metrics,
            &mut loss,
            true,
            &mut ev,
            &mut replay_determinism,
        )
        .expect("emit aggregate loss report");

        let output = String::from_utf8(out).expect("utf-8 jsonl");
        assert!(output.contains("\"data_loss\":true"));
        assert!(output.contains("\"dropped_since_last_event\":1"));
        assert!(output.contains("\"range_kind\":\"aggregate_counter\""));
        assert!(output.contains("\"sequence_gap_start\":null"));
        assert!(output.contains("\"sequence_gap_end\":null"));
    }

    #[test]
    fn emitted_sequence_gap_is_reported_with_exact_bounded_range() {
        let metrics = Metrics::new().unwrap();
        let mut loss = LossTracker::default();
        let mut out = Vec::<u8>::new();
        let mut replay_determinism = ReplayDeterminism::disabled();
        let mut first = sensor_event(
            Severity::Info,
            "sequence_gap_baseline",
            "baseline event".to_string(),
        );
        first.sequence = 10;
        emit(
            &mut out,
            &metrics,
            &mut loss,
            true,
            &mut first,
            &mut replay_determinism,
        )
        .expect("emit baseline event");
        let mut second = sensor_event(
            Severity::High,
            "sequence_gap_report",
            "sequence gap event".to_string(),
        );
        second.sequence = 13;

        emit(
            &mut out,
            &metrics,
            &mut loss,
            true,
            &mut second,
            &mut replay_determinism,
        )
        .expect("emit sequence gap report");

        let output = String::from_utf8(out).expect("utf-8 jsonl");
        let second_line = output.lines().nth(1).expect("second event");
        assert!(second_line.contains("\"data_loss\":true"));
        assert!(second_line.contains("\"dropped_since_last_event\":0"));
        assert!(second_line.contains("\"range_kind\":\"sequence_gap\""));
        assert!(second_line.contains("\"sequence_gap_start\":11"));
        assert!(second_line.contains("\"sequence_gap_end\":12"));
    }

    #[test]
    fn ignored_vm_sequence_is_not_reported_as_loss_gap() {
        let metrics = Metrics::new().unwrap();
        let mut loss = LossTracker::default();
        let mut out = Vec::<u8>::new();
        let mut replay_determinism = ReplayDeterminism::disabled();
        let mut cfg = Config::default();
        cfg.allow.ignore_vm = vec!["ignored-vm".to_string()];
        let runtime = RuntimeState::new(&cfg, false, false).unwrap();
        let mut first = sensor_event(
            Severity::Info,
            "sequence_gap_baseline",
            "baseline event".to_string(),
        );
        first.sequence = 100;
        emit(
            &mut out,
            &metrics,
            &mut loss,
            true,
            &mut first,
            &mut replay_determinism,
        )
        .expect("emit baseline event");
        let mut ignored = Event::base(
            Category::Exit,
            Severity::Info,
            "2026-01-01T00:00:00Z".to_string(),
            "ignored-vm".to_string(),
        );
        ignored.sequence = 101;
        assert!(runtime.policy.should_ignore_vm(&ignored.vm));
        loss.account_intentionally_skipped_sequence(ignored.sequence);
        let mut second = sensor_event(
            Severity::High,
            "sequence_gap_after_ignored_vm",
            "event after ignored VM".to_string(),
        );
        second.sequence = 102;

        emit(
            &mut out,
            &metrics,
            &mut loss,
            true,
            &mut second,
            &mut replay_determinism,
        )
        .expect("emit event after ignored VM");

        let output = String::from_utf8(out).expect("utf-8 jsonl");
        let second_line = output.lines().nth(1).expect("second emitted event");
        assert!(second_line.contains("\"data_loss\":false"));
        assert!(second_line.contains("\"loss\":null"));
        assert!(!second_line.contains("\"range_kind\":\"sequence_gap\""));
        assert!(!output.contains("\"vm\":\"ignored-vm\""));
    }

    #[test]
    fn replay_run_emits_single_startup_and_shutdown_lifecycle_events() {
        let replay = temp_path("empty-replay");
        let jsonl = temp_path("lifecycle-jsonl");
        std::fs::write(&replay, "").expect("write empty replay");

        run(
            temp_path("unused-tracefs"),
            Some(replay.clone()),
            None,
            jsonl.display().to_string(),
            "".to_string(),
            4,
            true,
            false,
            false,
        )
        .expect("empty replay run");

        let output = std::fs::read_to_string(&jsonl).expect("read lifecycle jsonl");
        assert_eq!(output.matches("\"reason\":\"sensor_startup\"").count(), 1);
        assert_eq!(output.matches("\"reason\":\"sensor_shutdown\"").count(), 1);
        assert!(output.contains("mode=replay"));
        assert!(output.contains("reason=clean"));
        assert!(!output.contains("type-1"));

        let _ = std::fs::remove_file(replay);
        let _ = std::fs::remove_file(jsonl);
    }

    #[test]
    fn deterministic_replay_requires_replay_input() {
        let err = run_cmd(vec!["--deterministic-replay".to_string()])
            .expect_err("deterministic replay must not run against live tracefs");

        assert!(err.contains("--deterministic-replay requires --replay"));
        assert!(err.contains("live tracefs output is not made deterministic"));
    }

    #[test]
    fn deterministic_replay_outputs_byte_identical_jsonl() {
        let replay = temp_path("deterministic-replay");
        let jsonl_a = temp_path("deterministic-a");
        let jsonl_b = temp_path("deterministic-b");
        std::fs::write(
            &replay,
            "qemu-system-x86-1234 [001] d..2 123.456: kvm_exit: reason EPT_VIOLATION rip 0x7f1234abcd gpa 0x1000 error_code 0x5\n",
        )
        .expect("write deterministic replay fixture");

        for jsonl in [&jsonl_a, &jsonl_b] {
            run(
                temp_path("unused-tracefs"),
                Some(replay.clone()),
                None,
                jsonl.display().to_string(),
                "".to_string(),
                4,
                true,
                false,
                true,
            )
            .expect("deterministic replay run");
        }

        let first = std::fs::read_to_string(&jsonl_a).expect("read first deterministic run");
        let second = std::fs::read_to_string(&jsonl_b).expect("read second deterministic run");

        assert_eq!(first, second);
        assert!(first.contains("\"ts\":\"2026-01-01T00:00:00.000Z\""));
        assert!(first.contains("\"monotonic_ms\":0"));
        assert!(first.contains("\"sequence\":1"));
        assert!(first.contains("\"event_id\":\"evt-deterministic-0000000000000001\""));
        assert!(first.contains("\"host_id\":\"deterministic-host\""));
        assert!(first.contains("\"sensor_id\":\"deterministic-sensor\""));
        assert!(first.contains("\"tenant_id\":\"deterministic-tenant\""));
        assert!(first.contains("deterministic_replay=enabled"));

        let _ = std::fs::remove_file(replay);
        let _ = std::fs::remove_file(jsonl_a);
        let _ = std::fs::remove_file(jsonl_b);
    }

    #[test]
    fn deterministic_replay_normalizes_action_id_with_event_id() {
        let mut replay_determinism = ReplayDeterminism::enabled();
        let mut ev = sensor_event(
            Severity::High,
            "action_id_test",
            "testing deterministic action ids".to_string(),
        );
        ev.action_id = Some("act-runtime-id".to_string());
        ev.wx = Some(aegishv::event::WxInfo {
            writer_rip: Some("0x4000".to_string()),
            executor_rip: Some("0x4010".to_string()),
            delta_ms: 17,
            page_size: Some(4096),
            confidence: 0.925,
        });
        ev.action = Some(aegishv::event::ActionInfo {
            rule: Some("policy-001".to_string()),
            kind: "pause_vm".to_string(),
            ok: true,
            status: "dry_run".to_string(),
            decision: "dry_run".to_string(),
            result: "dry_run".to_string(),
            detail: Some("action not executed".to_string()),
            latency_ms: Some(42),
            target_vm_id: Some("host-pid:222".to_string()),
            attempt: 0,
            max_attempts: 0,
            retry_count: 0,
            timeout_ms: 2000,
            timed_out: false,
            refused: false,
            failure_class: None,
        });

        replay_determinism.normalize(&mut ev);

        assert_eq!(ev.sequence, 1);
        assert_eq!(ev.event_id, "evt-deterministic-0000000000000001");
        assert_eq!(
            ev.action_id.as_deref(),
            Some("act-evt-deterministic-0000000000000001")
        );
        let wx = ev.wx.as_ref().expect("wx info");
        assert_eq!(wx.delta_ms, 0);
        assert_eq!(wx.confidence, 1.0);
        assert_eq!(ev.action.as_ref().unwrap().latency_ms, Some(0));
    }

    #[test]
    fn snapshot_json_option_writes_tracepoint_diagnostics() {
        let tracefs = temp_dir("snapshot-json-tracefs");
        std::fs::File::create(tracefs.join("trace_pipe")).expect("create trace_pipe");
        write_kvm_exit_format(&tracefs);
        let json = temp_path("snapshot-json");

        snapshot_cmd(vec![
            "--tracefs".to_string(),
            tracefs.display().to_string(),
            "--json".to_string(),
            json.display().to_string(),
        ])
        .expect("write snapshot json");

        let contents = std::fs::read_to_string(&json).expect("read snapshot json");
        assert!(contents.contains("\"tracepoints_ok\": true"));
        assert!(contents.contains("\"status\":\"ok\""));
        assert!(contents.contains("\"vm_inventory\":"));
        let _ = std::fs::remove_dir_all(tracefs);
        let _ = std::fs::remove_file(json);
    }

    #[test]
    fn snapshot_config_includes_bounded_vm_inventory() {
        let tracefs = temp_dir("snapshot-inventory-tracefs");
        std::fs::File::create(tracefs.join("trace_pipe")).expect("create trace_pipe");
        write_kvm_exit_format(&tracefs);
        let libvirt_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libvirt");
        let libvirt_dir = libvirt_dir.display().to_string().replace('\\', "/");
        let config = temp_config(&format!(
            "[identity]\nlibvirt_xml_dir = \"{}\"\n",
            libvirt_dir
        ));
        let json = temp_path("snapshot-inventory-json");

        snapshot_cmd(vec![
            "--tracefs".to_string(),
            tracefs.display().to_string(),
            "--config".to_string(),
            config.display().to_string(),
            "--json".to_string(),
            json.display().to_string(),
        ])
        .expect("write snapshot json");

        let contents = std::fs::read_to_string(&json).expect("read snapshot json");
        assert!(contents.contains("\"vm_inventory\":"));
        assert!(contents.contains("\"vm_count\":4"));
        assert!(contents.contains("\"freshness\":\"file_backed_snapshot\""));
        assert!(contents.contains("\"qmp\":{\"present\":true,\"status\":\"configured\"}"));
        assert!(!contents.contains("/run/libvirt/qemu"));
        let _ = std::fs::remove_dir_all(tracefs);
        let _ = std::fs::remove_file(config);
        let _ = std::fs::remove_file(json);
    }

    #[test]
    fn tracepoint_diagnostic_event_reports_unhealthy_metadata() {
        let metrics = Metrics::new().unwrap();
        let mut loss = LossTracker::default();
        let mut out = Vec::<u8>::new();
        let mut replay_determinism = ReplayDeterminism::disabled();
        let diagnostics = vec![TracepointDiagnostic {
            system: "kvm".to_string(),
            name: "kvm_exit".to_string(),
            status: TracepointDiagnosticStatus::MissingFields,
            missing_fields: vec!["exit_reason".to_string()],
            message: "tracepoint kvm/kvm_exit format metadata is missing expected field groups: exit_reason".to_string(),
        }];

        emit_tracepoint_diagnostics(
            &mut out,
            &metrics,
            &mut loss,
            &diagnostics,
            true,
            1,
            &mut replay_determinism,
        )
        .expect("emit tracepoint diagnostic");

        let output = String::from_utf8(out).expect("utf-8 jsonl");
        assert!(output.contains("\"reason\":\"tracefs_format_diagnostic\""));
        assert!(output.contains("status=missing_fields"));
        assert!(output.contains("missing_fields=[exit_reason]"));
        assert!(output.contains("\"tags\":[\"tracefs\"]"));
    }

    #[test]
    fn reload_signal_is_observed_once() {
        reset_shutdown_signal_for_run();

        note_reload_signal();

        assert!(observe_reload_signal());
        assert!(!observe_reload_signal());
    }

    #[test]
    fn config_reload_applies_safe_runtime_fields() {
        let mut runtime = RuntimeState::new(&Config::default(), false, false).unwrap();
        let path = temp_config(
            r#"
[general]
quiet = true
flush_every = 3

[allow]
ignore_vm = ["blocked-vm"]

[[policy.rules]]
name = "reload-noop"
id = "reload-noop"
mode = "dry_run"
action = { kind = "noop" }
match = { category = "wx", severity_at_least = "critical" }
"#,
        );

        let ev = apply_config_reload(Some(&path), &mut runtime, false, false);

        assert_eq!(ev.reason.as_deref(), Some("config_reload"));
        assert!(runtime.quiet);
        assert_eq!(runtime.flush_every, 3);
        assert_eq!(runtime.policy_rule_count, 1);
        assert!(runtime.policy.should_ignore_vm("blocked-vm"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn config_reload_failure_keeps_last_good_runtime() {
        let mut initial = Config::default();
        initial.general.flush_every = 7;
        initial.allow.ignore_vm = vec!["old-vm".to_string()];
        let mut runtime = RuntimeState::new(&initial, false, false).unwrap();
        let path = temp_config(
            r#"
[general]
flush_every = 2

[allow]
ignore_vm = ["new-vm"]

[[policy.rules]]
name = "bad-action"
action = { kind = "noop", }
"#,
        );

        let ev = apply_config_reload(Some(&path), &mut runtime, false, false);

        assert_eq!(ev.reason.as_deref(), Some("config_reload_failed"));
        assert_eq!(runtime.flush_every, 7);
        assert!(runtime.policy.should_ignore_vm("old-vm"));
        assert!(!runtime.policy.should_ignore_vm("new-vm"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn config_reload_without_path_is_explicitly_skipped() {
        let mut runtime = RuntimeState::new(&Config::default(), false, false).unwrap();

        let ev = apply_config_reload(None, &mut runtime, false, false);

        assert_eq!(ev.reason.as_deref(), Some("config_reload_skipped"));
        assert!(ev.message.as_deref().unwrap().contains("without --config"));
    }

    #[test]
    fn jsonl_file_reopen_flushes_buffer_and_continues_writing() {
        let path = temp_path("jsonl-reopen");
        let mut out = JsonlOutput::open(path.to_str().expect("utf-8 temp path")).unwrap();

        out.write_all(b"{\"before\":true}\n").unwrap();
        let ev = jsonl_reopen_event(out.reopen());
        out.write_all(b"{\"after\":true}\n").unwrap();
        out.flush().unwrap();

        assert_eq!(ev.reason.as_deref(), Some("jsonl_reopen"));
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("{\"before\":true}\n"));
        assert!(contents.contains("{\"after\":true}\n"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn stdout_jsonl_reopen_is_explicitly_skipped() {
        let mut out = memory_jsonl(JsonlTarget::Stdout);

        let ev = jsonl_reopen_event(out.reopen());

        assert_eq!(ev.reason.as_deref(), Some("jsonl_reopen_skipped"));
        out.write_all(b"{\"stdout\":true}\n").unwrap();
        out.flush().unwrap();
    }

    #[test]
    fn jsonl_reopen_failure_keeps_existing_writer() {
        let dir = temp_path("jsonl-reopen-dir");
        std::fs::create_dir_all(&dir).unwrap();
        let mut out = memory_jsonl(JsonlTarget::File(dir.clone()));

        let ev = jsonl_reopen_event(out.reopen());

        assert_eq!(ev.reason.as_deref(), Some("jsonl_reopen_failed"));
        out.write_all(b"{\"old_writer_still_works\":true}\n")
            .unwrap();
        out.flush().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn disabled_spool_keeps_jsonl_write_failure_fatal() {
        let metrics = Metrics::new().unwrap();
        let mut loss = LossTracker::default();
        let mut out = failing_jsonl_event_sink(EventSpool::Disabled);
        let mut replay_determinism = ReplayDeterminism::disabled();
        let mut ev = sensor_event(
            Severity::High,
            "jsonl_write_test",
            "forced output failure".to_string(),
        );

        let err = emit(
            &mut out,
            &metrics,
            &mut loss,
            true,
            &mut ev,
            &mut replay_determinism,
        )
        .expect_err("disabled spool keeps existing fatal write behavior");

        assert!(err.contains("write jsonl"));
        let encoded = metrics.encode();
        assert!(encoded.contains("aegishv_json_write_failures_total 1"));
        assert!(encoded.contains("aegishv_spool_events_total 0"));
    }

    #[test]
    fn enabled_spool_records_event_when_jsonl_write_fails() {
        let dir = temp_dir("event-spool");
        let cfg = test_spool_config(&dir, 65_536, 4096);
        let mut out = failing_jsonl_event_sink(EventSpool::open(&cfg).unwrap());
        let metrics = Metrics::new().unwrap();
        let mut loss = LossTracker::default();
        let mut replay_determinism = ReplayDeterminism::disabled();
        let mut ev = sensor_event(
            Severity::High,
            "spool_preserved",
            "forced output failure".to_string(),
        );

        emit(
            &mut out,
            &metrics,
            &mut loss,
            true,
            &mut ev,
            &mut replay_determinism,
        )
        .expect("enabled spool preserves the failed JSONL write");

        let entries = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        let contents = std::fs::read_to_string(&entries[0]).unwrap();
        let lines = contents.lines().collect::<Vec<_>>();
        assert_eq!(lines[0], "aegishv-spool-v1 len-hex-jsonl");
        let (len_hex, json) = lines[1]
            .split_once(' ')
            .expect("length-prefixed spool line");
        assert_eq!(usize::from_str_radix(len_hex, 16).unwrap(), json.len());
        assert!(json.contains("\"reason\":\"spool_preserved\""));

        let encoded = metrics.encode();
        assert!(encoded.contains("aegishv_json_write_failures_total 1"));
        assert!(encoded.contains("aegishv_spool_events_total 1"));
        assert!(encoded.contains("aegishv_spool_write_failures_total 0"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn compressed_spool_segment_is_decompressible_and_not_plain_json() {
        let dir = temp_dir("event-spool-rle");
        let cfg = test_compressed_spool_config(&dir, 65_536, 4096);
        let line = format!(
            "{{\"reason\":\"compressed_spool\",\"payload\":\"{}\"}}",
            "A".repeat(512)
        );

        {
            let mut spool = EventSpool::open(&cfg).expect("open compressed spool");
            spool.append(&line).expect("append compressed spool record");
        }

        let entries = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        let bytes = std::fs::read(&entries[0]).unwrap();
        assert!(bytes.starts_with(SPOOL_SEGMENT_HEADER_V2_RLE));
        assert!(!String::from_utf8_lossy(&bytes).contains(&"A".repeat(128)));

        let records = decode_spool_segment_bytes(&bytes).expect("decode compressed segment");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].line, line);
        assert!(records[0].compressed);
        assert_eq!(records[0].uncompressed_len, line.len());
        assert!(records[0].payload_len < records[0].uncompressed_len);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_compressed_spool_segment_is_rejected() {
        let mut bytes = SPOOL_SEGMENT_HEADER_V2_RLE.to_vec();
        write!(&mut bytes, "{:016x} {:016x} ", 10usize, 1usize).unwrap();
        bytes.push(0xff);
        bytes.push(b'\n');

        let err = decode_spool_segment_bytes(&bytes)
            .expect_err("truncated repeated run must be rejected");
        assert!(matches!(err, SpoolSegmentReadError::Corrupt(_)));
        assert!(err.detail().contains("RLE repeated run is truncated"));
    }

    #[test]
    fn compressed_spool_writes_new_segment_after_existing_corrupt_segment() {
        let dir = temp_dir("event-spool-rle-corrupt-existing");
        std::fs::write(spool_segment_path(&dir, 1), b"aegishv-spool-v2 bad\n")
            .expect("write corrupt existing segment");
        let cfg = test_compressed_spool_config(&dir, 65_536, 4096);
        let line = format!(
            "{{\"reason\":\"compressed_spool_recovery\",\"payload\":\"{}\"}}",
            "B".repeat(256)
        );

        {
            let mut spool = EventSpool::open(&cfg).expect("open compressed spool");
            spool.append(&line).expect("append after corrupt segment");
        }

        let next_segment = spool_segment_path(&dir, 2);
        let records = decode_spool_segment_bytes(&std::fs::read(next_segment).unwrap())
            .expect("decode new compressed segment");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].line, line);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unsupported_compressed_spool_segment_header_is_rejected() {
        let bytes = b"aegishv-spool-v2 compression=zstd record=hex-u64-payload\n".to_vec();
        let err =
            decode_spool_segment_bytes(&bytes).expect_err("unsupported compression must fail");
        assert!(matches!(err, SpoolSegmentReadError::Unsupported(_)));
        assert!(err.detail().contains("unsupported spool segment"));
    }

    #[test]
    fn enabled_spool_failure_is_counted_and_fatal() {
        let dir = temp_dir("event-spool-full");
        let cfg = test_spool_config(&dir, 1, 1);
        let mut out = failing_jsonl_event_sink(EventSpool::open(&cfg).unwrap());
        let metrics = Metrics::new().unwrap();
        let mut loss = LossTracker::default();
        let mut replay_determinism = ReplayDeterminism::disabled();
        let mut ev = sensor_event(
            Severity::High,
            "spool_full",
            "forced output and spool failure".to_string(),
        );

        let err = emit(
            &mut out,
            &metrics,
            &mut loss,
            true,
            &mut ev,
            &mut replay_determinism,
        )
        .expect_err("spool failure must not hide the lost event");

        assert!(err.contains("event spool failed"));
        let encoded = metrics.encode();
        assert!(encoded.contains("aegishv_spool_events_total 0"));
        assert!(encoded.contains("aegishv_spool_write_failures_total 1"));
        assert!(encoded.contains("aegishv_spool_dropped_total 1"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
