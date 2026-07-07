use crate::event::{category_from_str, severity_from_str, IdentityConfidence};
use crate::pattern::Pattern;
use crate::util::{clamp_u64, clamp_usize, parse_bool, parse_string_value};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::Path;

pub const PMU_REDISCOVER_MS_MIN: u64 = 1000;
pub const PMU_REDISCOVER_MS_MAX: u64 = 3_600_000;
pub const SPOOL_MAX_BYTES_MIN: u64 = 4096;
pub const SPOOL_MAX_BYTES_MAX: u64 = 1_099_511_627_776;
pub const SPOOL_SEGMENT_BYTES_MIN: u64 = 4096;
pub const SYSLOG_MAX_MESSAGE_BYTES_MIN: usize = 512;
pub const SYSLOG_MAX_MESSAGE_BYTES_MAX: usize = 65_507;
pub const JOURNALD_MAX_MESSAGE_BYTES_MIN: usize = 512;
pub const JOURNALD_MAX_MESSAGE_BYTES_MAX: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub version: u32,
    pub general: General,
    pub allow: Allow,
    pub wx_allow: WxAllow,
    pub pmu: Pmu,
    pub metrics: MetricsConfig,
    pub spool: Spool,
    pub syslog: Syslog,
    pub journald: Journald,
    pub identity: Identity,
    pub actions: Actions,
    pub policy: Policy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct General {
    pub wx_window_ms: u64,
    pub wx_cooldown_ms: u64,
    pub wx_max_pages: usize,
    pub page_size: u64,
    pub flush_every: usize,
    pub quiet: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Allow {
    pub ignore_vm: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WxAllow {
    pub entries: Vec<WxAllowEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WxAllowEntry {
    pub vm: String,
    pub gpa_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pmu {
    pub enable: bool,
    pub sample_ms: u64,
    pub qemu_pid: i32,
    pub vm_regex: String,
    pub rediscover_ms: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetricsConfig {
    pub allow_bind_failure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spool {
    pub enable: bool,
    pub dir: String,
    pub max_bytes: u64,
    pub segment_bytes: u64,
    pub compression: SpoolCompression,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpoolCompression {
    None,
    Rle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syslog {
    pub enable: bool,
    pub address: String,
    pub facility: String,
    pub max_message_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Journald {
    pub enable: bool,
    pub socket: String,
    pub identifier: String,
    pub max_message_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub enable: bool,
    pub cache_ms: u64,
    pub qmp_socket_dirs: Vec<String>,
    pub libvirt_xml_dir: String,
    pub require_stable_qmp_match: bool,
    pub min_action_confidence: IdentityConfidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actions {
    pub timeout_ms: u64,
    pub retries: u32,
    pub dump_root: String,
    pub qmp: Vec<QmpMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QmpMapping {
    /// Stable id pattern. Prefer `libvirt:<uuid>` or `name:<domain>`. Kept as a pattern for
    /// staged migrations, but config validation rejects unsupported regex syntax.
    pub vm_id: String,
    pub vm: String,
    pub socket: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub name: String,
    pub id: String,
    pub match_: Match,
    pub action: Option<Action>,
    pub actions: Vec<Action>,
    pub cooldown_ms: u64,
    pub priority: i32,
    pub mode: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Match {
    pub category: String,
    pub severity_at_least: String,
    pub reason_regex: String,
    pub vm_regex: String,
    pub trap_type: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Action {
    pub kind: String,
    pub output_path: String,
    pub nic: String,
}

impl Default for General {
    fn default() -> Self {
        Self {
            wx_window_ms: 5000,
            wx_cooldown_ms: 30_000,
            wx_max_pages: 200_000,
            page_size: 4096,
            flush_every: 64,
            quiet: false,
        }
    }
}

impl Default for Pmu {
    fn default() -> Self {
        Self {
            enable: false,
            sample_ms: 1000,
            qemu_pid: 0,
            vm_regex: String::new(),
            rediscover_ms: 30_000,
        }
    }
}

impl Default for Spool {
    fn default() -> Self {
        Self {
            enable: false,
            dir: "/var/lib/aegishv/spool".to_string(),
            max_bytes: 67_108_864,
            segment_bytes: 4_194_304,
            compression: SpoolCompression::None,
        }
    }
}

impl SpoolCompression {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Rle => "rle",
        }
    }
}

impl Default for Syslog {
    fn default() -> Self {
        Self {
            enable: false,
            address: "127.0.0.1:514".to_string(),
            facility: "local0".to_string(),
            max_message_bytes: 8192,
        }
    }
}

impl Default for Journald {
    fn default() -> Self {
        Self {
            enable: false,
            socket: "/run/systemd/journal/socket".to_string(),
            identifier: "aegishv".to_string(),
            max_message_bytes: 8192,
        }
    }
}

impl Default for Identity {
    fn default() -> Self {
        Self {
            enable: true,
            cache_ms: 5000,
            qmp_socket_dirs: vec![
                "/run/libvirt/qemu".to_string(),
                "/var/run/libvirt/qemu".to_string(),
                "/run/qemu".to_string(),
            ],
            libvirt_xml_dir: String::new(),
            require_stable_qmp_match: true,
            min_action_confidence: IdentityConfidence::High,
        }
    }
}

impl Default for Actions {
    fn default() -> Self {
        Self {
            timeout_ms: 2000,
            retries: 1,
            dump_root: "/var/lib/aegishv/dumps".to_string(),
            qmp: Vec::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            general: General::default(),
            allow: Allow::default(),
            wx_allow: WxAllow::default(),
            pmu: Pmu::default(),
            metrics: MetricsConfig::default(),
            spool: Spool::default(),
            syslog: Syslog::default(),
            journald: Journald::default(),
            identity: Identity::default(),
            actions: Actions::default(),
            policy: Policy::default(),
        }
    }
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            name: String::new(),
            id: String::new(),
            match_: Match::default(),
            action: None,
            actions: Vec::new(),
            cooldown_ms: 0,
            priority: 100,
            mode: "enforce".to_string(),
            enabled: true,
        }
    }
}

impl Rule {
    pub fn all_actions(&self) -> Vec<Action> {
        let mut out = Vec::new();
        if let Some(a) = &self.action {
            out.push(a.clone());
        }
        out.extend(self.actions.clone());
        out
    }
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self, String> {
        let mut cfg = Config::default();
        if let Some(path) = path {
            let s = std::fs::read_to_string(path)
                .map_err(|e| format!("read config {}: {e}", path.display()))?;
            cfg = parse_config(&s)?;
        }
        cfg.normalize();
        cfg.validate()?;
        Ok(cfg)
    }

    fn normalize(&mut self) {
        self.general.wx_window_ms = clamp_u64(self.general.wx_window_ms, 1, 86_400_000);
        self.general.wx_cooldown_ms = clamp_u64(self.general.wx_cooldown_ms, 0, 86_400_000);
        self.general.wx_max_pages = clamp_usize(self.general.wx_max_pages, 1024, 10_000_000);
        self.general.flush_every = clamp_usize(self.general.flush_every, 1, 8192);
        if !matches!(self.general.page_size, 4096 | 2_097_152 | 1_073_741_824) {
            self.general.page_size = 4096;
        }
        self.pmu.sample_ms = clamp_u64(self.pmu.sample_ms, 100, 60_000);
        self.identity.cache_ms = clamp_u64(self.identity.cache_ms, 100, 300_000);
        self.actions.timeout_ms = clamp_u64(self.actions.timeout_ms, 100, 120_000);
        self.actions.retries = self.actions.retries.min(10);
        for (idx, r) in self.policy.rules.iter_mut().enumerate() {
            if r.id.trim().is_empty() {
                r.id = format!("rule-{}", idx + 1);
            }
            if r.name.trim().is_empty() {
                r.name = r.id.clone();
            }
        }
    }

    fn validate(&self) -> Result<(), String> {
        for pat in &self.allow.ignore_vm {
            Pattern::compile(pat)
                .map_err(|e| format!("invalid allow.ignore_vm pattern '{}': {e}", pat))?;
        }
        for ent in &self.wx_allow.entries {
            if !ent.vm.trim().is_empty() {
                Pattern::compile(&ent.vm)
                    .map_err(|e| format!("invalid wx_allow vm pattern '{}': {e}", ent.vm))?;
            }
            if !ent.gpa_prefix.trim().is_empty() && !ent.gpa_prefix.starts_with("0x") {
                return Err(format!(
                    "invalid wx_allow gpa_prefix '{}': expected 0x prefix",
                    ent.gpa_prefix
                ));
            }
        }
        if !self.pmu.vm_regex.trim().is_empty() {
            Pattern::compile(&self.pmu.vm_regex)
                .map_err(|e| format!("invalid pmu.vm_regex '{}': {e}", self.pmu.vm_regex))?;
        }
        if !(PMU_REDISCOVER_MS_MIN..=PMU_REDISCOVER_MS_MAX).contains(&self.pmu.rediscover_ms) {
            return Err(format!(
                "invalid pmu.rediscover_ms {}: expected {}..={} milliseconds",
                self.pmu.rediscover_ms, PMU_REDISCOVER_MS_MIN, PMU_REDISCOVER_MS_MAX
            ));
        }
        if self.spool.enable && self.spool.dir.trim().is_empty() {
            return Err(
                "invalid spool.dir: enabled spool requires a non-empty directory".to_string(),
            );
        }
        if !self.identity.libvirt_xml_dir.trim().is_empty()
            && !Path::new(&self.identity.libvirt_xml_dir).is_dir()
        {
            return Err(format!(
                "invalid identity.libvirt_xml_dir '{}': expected an existing directory of mocked libvirt domain XML files",
                self.identity.libvirt_xml_dir
            ));
        }
        if self.identity.min_action_confidence == IdentityConfidence::Low {
            return Err(
                "invalid identity.min_action_confidence: low confidence cannot authorize QMP actions"
                    .to_string(),
            );
        }
        if !(SPOOL_MAX_BYTES_MIN..=SPOOL_MAX_BYTES_MAX).contains(&self.spool.max_bytes) {
            return Err(format!(
                "invalid spool.max_bytes {}: expected {}..={} bytes",
                self.spool.max_bytes, SPOOL_MAX_BYTES_MIN, SPOOL_MAX_BYTES_MAX
            ));
        }
        if !(SPOOL_SEGMENT_BYTES_MIN..=self.spool.max_bytes).contains(&self.spool.segment_bytes) {
            return Err(format!(
                "invalid spool.segment_bytes {}: expected {}..={} bytes",
                self.spool.segment_bytes, SPOOL_SEGMENT_BYTES_MIN, self.spool.max_bytes
            ));
        }
        if self.syslog.enable {
            if self.syslog.address.trim().is_empty() {
                return Err(
                    "invalid syslog.address: enabled syslog requires an ip:port target".to_string(),
                );
            }
            self.syslog.address.parse::<SocketAddr>().map_err(|_| {
                format!(
                    "invalid syslog.address '{}': expected numeric ip:port",
                    self.syslog.address
                )
            })?;
        }
        if syslog_facility_code(&self.syslog.facility).is_none() {
            return Err(format!(
                "invalid syslog.facility '{}': expected user, daemon, or local0..local7",
                self.syslog.facility
            ));
        }
        if !(SYSLOG_MAX_MESSAGE_BYTES_MIN..=SYSLOG_MAX_MESSAGE_BYTES_MAX)
            .contains(&self.syslog.max_message_bytes)
        {
            return Err(format!(
                "invalid syslog.max_message_bytes {}: expected {}..={} bytes",
                self.syslog.max_message_bytes,
                SYSLOG_MAX_MESSAGE_BYTES_MIN,
                SYSLOG_MAX_MESSAGE_BYTES_MAX
            ));
        }
        if self.journald.enable && self.journald.socket.trim().is_empty() {
            return Err(
                "invalid journald.socket: enabled journald requires a non-empty socket path"
                    .to_string(),
            );
        }
        if !is_safe_journald_identifier(&self.journald.identifier) {
            return Err(format!(
                "invalid journald.identifier '{}': expected 1..=64 ASCII letters, digits, '.', '_' or '-'",
                self.journald.identifier
            ));
        }
        if !(JOURNALD_MAX_MESSAGE_BYTES_MIN..=JOURNALD_MAX_MESSAGE_BYTES_MAX)
            .contains(&self.journald.max_message_bytes)
        {
            return Err(format!(
                "invalid journald.max_message_bytes {}: expected {}..={} bytes",
                self.journald.max_message_bytes,
                JOURNALD_MAX_MESSAGE_BYTES_MIN,
                JOURNALD_MAX_MESSAGE_BYTES_MAX
            ));
        }
        for q in &self.actions.qmp {
            if q.socket.trim().is_empty() {
                return Err("actions.qmp socket must not be empty".to_string());
            }
            if q.vm_id.trim().is_empty() && q.vm.trim().is_empty() {
                return Err("actions.qmp requires vm_id or vm pattern".to_string());
            }
            if !q.vm_id.trim().is_empty() {
                Pattern::compile(&q.vm_id)
                    .map_err(|e| format!("invalid actions.qmp vm_id pattern '{}': {e}", q.vm_id))?;
            }
            if !q.vm.trim().is_empty() {
                Pattern::compile(&q.vm)
                    .map_err(|e| format!("invalid actions.qmp vm pattern '{}': {e}", q.vm))?;
            }
        }
        for r in &self.policy.rules {
            if !matches!(r.mode.as_str(), "enforce" | "dry_run" | "suppress") {
                return Err(format!("invalid mode '{}' in rule '{}'", r.mode, r.name));
            }
            if !r.match_.category.trim().is_empty()
                && category_from_str(&r.match_.category).is_none()
            {
                return Err(format!(
                    "invalid category '{}' in rule '{}'",
                    r.match_.category, r.name
                ));
            }
            if !r.match_.severity_at_least.trim().is_empty()
                && severity_from_str(&r.match_.severity_at_least).is_none()
            {
                return Err(format!(
                    "invalid severity '{}' in rule '{}'",
                    r.match_.severity_at_least, r.name
                ));
            }
            if !r.match_.reason_regex.trim().is_empty() {
                Pattern::compile(&r.match_.reason_regex).map_err(|e| {
                    format!(
                        "invalid reason_regex '{}' in rule '{}': {e}",
                        r.match_.reason_regex, r.name
                    )
                })?;
            }
            if !r.match_.vm_regex.trim().is_empty() {
                Pattern::compile(&r.match_.vm_regex).map_err(|e| {
                    format!(
                        "invalid vm_regex '{}' in rule '{}': {e}",
                        r.match_.vm_regex, r.name
                    )
                })?;
            }
            for a in r.all_actions() {
                validate_action(&a, &r.name)?;
            }
        }
        Ok(())
    }
}

fn validate_action(a: &Action, rule: &str) -> Result<(), String> {
    match a.kind.as_str() {
        "pause_vm" | "resume_vm" | "manual_approval" | "noop" => Ok(()),
        "dump_guest_memory" => {
            if a.output_path.trim().is_empty() {
                Err(format!(
                    "dump_guest_memory requires action.output_path in rule '{rule}'"
                ))
            } else {
                Ok(())
            }
        }
        "quarantine_nic" => {
            if a.nic.trim().is_empty() {
                Err(format!(
                    "quarantine_nic requires action.nic in rule '{rule}'"
                ))
            } else {
                Ok(())
            }
        }
        other => Err(format!("invalid action kind '{other}' in rule '{rule}'")),
    }
}

fn parse_config(s: &str) -> Result<Config, String> {
    let mut cfg = Config::default();
    let mut section = String::new();
    let mut current_rule: Option<Rule> = None;
    let mut current_qmp: Option<QmpMapping> = None;
    let mut current_wx: Option<WxAllowEntry> = None;
    let mut declared_tables = HashSet::new();
    let mut seen_keys = HashSet::new();

    for (line_no, raw) in s.lines().enumerate() {
        let line_number = line_no + 1;
        let line = strip_comment(raw)
            .map_err(|e| line_error(line_number, e))?
            .trim()
            .to_string();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = parse_section_header(&line) {
            let header = header.map_err(|e| line_error(line_number, e))?;
            flush_rule(&mut cfg, &mut current_rule);
            flush_qmp(&mut cfg, &mut current_qmp);
            flush_wx(&mut cfg, &mut current_wx);
            seen_keys.clear();
            section = match header {
                SectionHeader::Table(name) => {
                    if !is_supported_table(&name) {
                        return Err(line_error(
                            line_number,
                            format!("unsupported section [{name}]"),
                        ));
                    }
                    if !declared_tables.insert(name.clone()) {
                        return Err(line_error(
                            line_number,
                            format!("duplicate section [{name}]"),
                        ));
                    }
                    name
                }
                SectionHeader::Array(name) => match name.as_str() {
                    "policy.rules" => {
                        current_rule = Some(Rule::default());
                        name
                    }
                    "actions.qmp" => {
                        current_qmp = Some(QmpMapping {
                            vm_id: String::new(),
                            vm: String::new(),
                            socket: String::new(),
                        });
                        name
                    }
                    "wx_allow.entries" => {
                        current_wx = Some(WxAllowEntry::default());
                        name
                    }
                    _ => {
                        return Err(line_error(
                            line_number,
                            format!("unsupported array section [[{name}]]"),
                        ))
                    }
                },
            };
            continue;
        }
        let (key, value) = split_key_value(&line).map_err(|e| line_error(line_number, e))?;
        if !seen_keys.insert(key.to_string()) {
            return Err(line_error(
                line_number,
                format!("duplicate key '{key}' in [{}]", section_label(&section)),
            ));
        };
        let result = match section.as_str() {
            "" => set_root(&mut cfg, key, value),
            "general" => set_general(&mut cfg.general, key, value),
            "allow" => set_allow(&mut cfg.allow, key, value),
            "pmu" => set_pmu(&mut cfg.pmu, key, value),
            "metrics" => set_metrics(&mut cfg.metrics, key, value),
            "spool" => set_spool(&mut cfg.spool, key, value),
            "syslog" => set_syslog(&mut cfg.syslog, key, value),
            "journald" => set_journald(&mut cfg.journald, key, value),
            "identity" => set_identity(&mut cfg.identity, key, value),
            "actions" => set_actions(&mut cfg.actions, key, value),
            "actions.qmp" => match current_qmp.as_mut() {
                Some(q) => set_qmp(q, key, value),
                None => Err("internal qmp parser state".to_string()),
            },
            "wx_allow.entries" => match current_wx.as_mut() {
                Some(w) => set_wx(w, key, value),
                None => Err("internal wx parser state".to_string()),
            },
            "policy.rules" => match current_rule.as_mut() {
                Some(r) => set_rule(r, key, value),
                None => Err("internal rule parser state".to_string()),
            },
            other => Err(format!("unsupported section [{}]", other)),
        };
        if let Err(e) = result {
            return Err(line_error(line_number, e));
        }
    }
    flush_rule(&mut cfg, &mut current_rule);
    flush_qmp(&mut cfg, &mut current_qmp);
    flush_wx(&mut cfg, &mut current_wx);
    Ok(cfg)
}

enum SectionHeader {
    Table(String),
    Array(String),
}

fn parse_section_header(line: &str) -> Option<Result<SectionHeader, String>> {
    if !line.starts_with('[') {
        return None;
    }
    if line.starts_with("[[") {
        return Some(parse_array_header(line));
    }
    Some(parse_table_header(line))
}

fn parse_array_header(line: &str) -> Result<SectionHeader, String> {
    let name = line
        .strip_prefix("[[")
        .and_then(|v| v.strip_suffix("]]"))
        .ok_or_else(|| "malformed array section header".to_string())?
        .trim();
    validate_section_name(name)?;
    Ok(SectionHeader::Array(name.to_string()))
}

fn parse_table_header(line: &str) -> Result<SectionHeader, String> {
    let name = line
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .ok_or_else(|| "malformed section header".to_string())?
        .trim();
    validate_section_name(name)?;
    Ok(SectionHeader::Table(name.to_string()))
}

fn validate_section_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains('[') || name.contains(']') {
        return Err("malformed section header".to_string());
    }
    Ok(())
}

fn is_supported_table(section: &str) -> bool {
    matches!(
        section,
        "general"
            | "allow"
            | "pmu"
            | "metrics"
            | "spool"
            | "syslog"
            | "journald"
            | "identity"
            | "actions"
    )
}

fn section_label(section: &str) -> &str {
    if section.is_empty() {
        "root"
    } else {
        section
    }
}

fn line_error(line_number: usize, detail: impl AsRef<str>) -> String {
    format!("line {line_number}: {}", detail.as_ref())
}

fn strip_comment(s: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut in_str = false;
    let mut escape = false;
    for c in s.chars() {
        if escape {
            out.push(c);
            escape = false;
            continue;
        }
        match c {
            '\\' if in_str => {
                out.push(c);
                escape = true;
            }
            '"' => {
                in_str = !in_str;
                out.push(c);
            }
            '#' if !in_str => break,
            _ => out.push(c),
        }
    }
    if escape {
        return Err("unterminated escape in string".to_string());
    }
    if in_str {
        return Err("unterminated string".to_string());
    }
    Ok(out)
}

fn split_key_value(line: &str) -> Result<(&str, &str), String> {
    let mut in_str = false;
    let mut escape = false;
    for (idx, c) in line.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match c {
            '\\' if in_str => escape = true,
            '"' => in_str = !in_str,
            '=' if !in_str => {
                let key = line[..idx].trim();
                let value = line[idx + 1..].trim();
                if key.is_empty() {
                    return Err("empty config key".to_string());
                }
                if value.is_empty() {
                    return Err(format!("missing value for key '{key}'"));
                }
                return Ok((key, value));
            }
            _ => {}
        }
    }
    Err("expected key = value assignment".to_string())
}

fn parse_string_array_strict(value: &str) -> Result<Vec<String>, String> {
    let t = value.trim();
    let mut rest = t
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .ok_or_else(|| "expected string array".to_string())?
        .trim();
    let mut values = Vec::new();
    if rest.is_empty() {
        return Ok(values);
    }
    loop {
        if rest.starts_with(',') {
            return Err("empty string array element".to_string());
        }
        if !rest.starts_with('"') {
            return Err("string array elements must be quoted".to_string());
        }
        let (value, consumed) = parse_quoted_value(rest)?;
        values.push(value);
        rest = rest[consumed..].trim_start();
        if rest.is_empty() {
            break;
        }
        if !rest.starts_with(',') {
            return Err("expected comma between string array elements".to_string());
        }
        rest = rest[1..].trim_start();
        if rest.is_empty() {
            return Err("trailing comma in string array".to_string());
        }
    }
    Ok(values)
}

fn parse_quoted_value(value: &str) -> Result<(String, usize), String> {
    let mut out = String::new();
    let mut escape = false;
    for (idx, c) in value.char_indices().skip(1) {
        if escape {
            match c {
                '"' => out.push('"'),
                'n' => out.push('\n'),
                '\\' => out.push('\\'),
                other => {
                    out.push('\\');
                    out.push(other);
                }
            }
            escape = false;
            continue;
        }
        match c {
            '\\' => escape = true,
            '"' => return Ok((out, idx + c.len_utf8())),
            _ => out.push(c),
        }
    }
    if escape {
        Err("unterminated escape in string".to_string())
    } else {
        Err("unterminated string".to_string())
    }
}

fn parse_inline_value(value: &str) -> Result<String, String> {
    let t = value.trim();
    if t.starts_with('"') {
        let (parsed, consumed) = parse_quoted_value(t)?;
        if !t[consumed..].trim().is_empty() {
            return Err("unexpected content after quoted string".to_string());
        }
        return Ok(parsed);
    }
    if t.is_empty() {
        return Err("empty inline value".to_string());
    }
    if t.chars().any(|c| matches!(c, '[' | ']' | '{' | '}')) {
        return Err("nested inline syntax is not supported".to_string());
    }
    Ok(parse_string_value(t))
}

fn set_root(cfg: &mut Config, key: &str, value: &str) -> Result<(), String> {
    match key {
        "version" => cfg.version = value.parse().map_err(|_| "invalid version".to_string())?,
        _ => return Err(format!("unknown root key '{key}'")),
    }
    Ok(())
}

fn set_general(g: &mut General, key: &str, value: &str) -> Result<(), String> {
    match key {
        "wx_window_ms" => {
            g.wx_window_ms = value
                .parse()
                .map_err(|_| "invalid wx_window_ms".to_string())?
        }
        "wx_cooldown_ms" => {
            g.wx_cooldown_ms = value
                .parse()
                .map_err(|_| "invalid wx_cooldown_ms".to_string())?
        }
        "wx_max_pages" => {
            g.wx_max_pages = value
                .parse()
                .map_err(|_| "invalid wx_max_pages".to_string())?
        }
        "page_size" => g.page_size = value.parse().map_err(|_| "invalid page_size".to_string())?,
        "flush_every" => {
            g.flush_every = value
                .parse()
                .map_err(|_| "invalid flush_every".to_string())?
        }
        "quiet" => g.quiet = parse_bool(value).ok_or("invalid quiet")?,
        _ => return Err(format!("unknown general key '{key}'")),
    }
    Ok(())
}

fn set_allow(a: &mut Allow, key: &str, value: &str) -> Result<(), String> {
    match key {
        "ignore_vm" => a.ignore_vm = parse_string_array_strict(value)?,
        _ => return Err(format!("unknown allow key '{key}'")),
    }
    Ok(())
}

fn set_pmu(p: &mut Pmu, key: &str, value: &str) -> Result<(), String> {
    match key {
        "enable" => p.enable = parse_bool(value).ok_or("invalid pmu.enable")?,
        "sample_ms" => {
            p.sample_ms = value
                .parse()
                .map_err(|_| "invalid pmu.sample_ms".to_string())?
        }
        "qemu_pid" => {
            p.qemu_pid = value
                .parse()
                .map_err(|_| "invalid pmu.qemu_pid".to_string())?
        }
        "vm_regex" => p.vm_regex = parse_string_value(value),
        "rediscover_ms" => {
            p.rediscover_ms = value
                .parse()
                .map_err(|_| "invalid pmu.rediscover_ms".to_string())?
        }
        _ => return Err(format!("unknown pmu key '{key}'")),
    }
    Ok(())
}

fn set_metrics(m: &mut MetricsConfig, key: &str, value: &str) -> Result<(), String> {
    match key {
        "allow_bind_failure" => {
            m.allow_bind_failure = parse_bool(value).ok_or("invalid metrics.allow_bind_failure")?
        }
        _ => return Err(format!("unknown metrics key '{key}'")),
    }
    Ok(())
}

fn set_spool(s: &mut Spool, key: &str, value: &str) -> Result<(), String> {
    match key {
        "enable" => s.enable = parse_bool(value).ok_or("invalid spool.enable")?,
        "dir" => s.dir = parse_string_value(value),
        "max_bytes" => {
            s.max_bytes = value
                .parse()
                .map_err(|_| "invalid spool.max_bytes".to_string())?
        }
        "segment_bytes" => {
            s.segment_bytes = value
                .parse()
                .map_err(|_| "invalid spool.segment_bytes".to_string())?
        }
        "compression" => s.compression = parse_spool_compression(value)?,
        _ => return Err(format!("unknown spool key '{key}'")),
    }
    Ok(())
}

fn parse_spool_compression(value: &str) -> Result<SpoolCompression, String> {
    let parsed = parse_string_value(value);
    match parsed.as_str() {
        "none" => Ok(SpoolCompression::None),
        "rle" => Ok(SpoolCompression::Rle),
        _ => Err(format!(
            "invalid spool.compression '{}': expected \"none\" or \"rle\"",
            parsed
        )),
    }
}

fn set_syslog(s: &mut Syslog, key: &str, value: &str) -> Result<(), String> {
    match key {
        "enable" => s.enable = parse_bool(value).ok_or("invalid syslog.enable")?,
        "address" => s.address = parse_string_value(value),
        "facility" => s.facility = parse_string_value(value),
        "max_message_bytes" => {
            s.max_message_bytes = value
                .parse()
                .map_err(|_| "invalid syslog.max_message_bytes".to_string())?
        }
        _ => return Err(format!("unknown syslog key '{key}'")),
    }
    Ok(())
}

fn set_journald(j: &mut Journald, key: &str, value: &str) -> Result<(), String> {
    match key {
        "enable" => j.enable = parse_bool(value).ok_or("invalid journald.enable")?,
        "socket" => j.socket = parse_string_value(value),
        "identifier" => j.identifier = parse_string_value(value),
        "max_message_bytes" => {
            j.max_message_bytes = value
                .parse()
                .map_err(|_| "invalid journald.max_message_bytes".to_string())?
        }
        _ => return Err(format!("unknown journald key '{key}'")),
    }
    Ok(())
}

pub fn syslog_facility_code(facility: &str) -> Option<u8> {
    let normalized = facility.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "user" => Some(1),
        "daemon" => Some(3),
        "local0" => Some(16),
        "local1" => Some(17),
        "local2" => Some(18),
        "local3" => Some(19),
        "local4" => Some(20),
        "local5" => Some(21),
        "local6" => Some(22),
        "local7" => Some(23),
        _ => None,
    }
}

fn is_safe_journald_identifier(identifier: &str) -> bool {
    (1..=64).contains(&identifier.len())
        && identifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn set_identity(i: &mut Identity, key: &str, value: &str) -> Result<(), String> {
    match key {
        "enable" => i.enable = parse_bool(value).ok_or("invalid identity.enable")?,
        "cache_ms" => {
            i.cache_ms = value
                .parse()
                .map_err(|_| "invalid identity.cache_ms".to_string())?
        }
        "qmp_socket_dirs" => i.qmp_socket_dirs = parse_string_array_strict(value)?,
        "libvirt_xml_dir" => i.libvirt_xml_dir = parse_string_value(value),
        "require_stable_qmp_match" => {
            i.require_stable_qmp_match =
                parse_bool(value).ok_or("invalid identity.require_stable_qmp_match")?
        }
        "min_action_confidence" => {
            i.min_action_confidence = parse_min_action_confidence(value)?;
        }
        _ => return Err(format!("unknown identity key '{key}'")),
    }
    Ok(())
}

fn parse_min_action_confidence(value: &str) -> Result<IdentityConfidence, String> {
    let parsed = parse_string_value(value);
    match parsed.as_str() {
        "medium" => Ok(IdentityConfidence::Medium),
        "high" => Ok(IdentityConfidence::High),
        "low" => Err(
            "invalid identity.min_action_confidence: low confidence cannot authorize QMP actions"
                .to_string(),
        ),
        _ => Err(format!(
            "invalid identity.min_action_confidence '{}': expected \"medium\" or \"high\"",
            parsed
        )),
    }
}

fn set_actions(a: &mut Actions, key: &str, value: &str) -> Result<(), String> {
    match key {
        "timeout_ms" => {
            a.timeout_ms = value
                .parse()
                .map_err(|_| "invalid actions.timeout_ms".to_string())?
        }
        "retries" => {
            a.retries = value
                .parse()
                .map_err(|_| "invalid actions.retries".to_string())?
        }
        "dump_root" => a.dump_root = parse_string_value(value),
        _ => return Err(format!("unknown actions key '{key}'")),
    }
    Ok(())
}

fn set_qmp(q: &mut QmpMapping, key: &str, value: &str) -> Result<(), String> {
    match key {
        "vm_id" => q.vm_id = parse_string_value(value),
        "vm" => q.vm = parse_string_value(value),
        "socket" => q.socket = parse_string_value(value),
        _ => return Err(format!("unknown actions.qmp key '{key}'")),
    }
    Ok(())
}

fn set_wx(w: &mut WxAllowEntry, key: &str, value: &str) -> Result<(), String> {
    match key {
        "vm" => w.vm = parse_string_value(value),
        "gpa_prefix" => w.gpa_prefix = parse_string_value(value),
        _ => return Err(format!("unknown wx_allow.entries key '{key}'")),
    }
    Ok(())
}

fn set_rule(r: &mut Rule, key: &str, value: &str) -> Result<(), String> {
    match key {
        "name" => r.name = parse_string_value(value),
        "id" => r.id = parse_string_value(value),
        "priority" => {
            r.priority = value
                .parse()
                .map_err(|_| "invalid rule.priority".to_string())?
        }
        "cooldown_ms" => {
            r.cooldown_ms = value
                .parse()
                .map_err(|_| "invalid rule.cooldown_ms".to_string())?
        }
        "mode" => r.mode = parse_string_value(value),
        "enabled" => r.enabled = parse_bool(value).ok_or("invalid rule.enabled")?,
        "match" => r.match_ = parse_match_inline(value)?,
        "action" => r.action = Some(parse_action_inline(value)?),
        "actions" => r.actions = parse_actions_array(value)?,
        _ => return Err(format!("unknown policy.rules key '{key}'")),
    }
    Ok(())
}

fn parse_match_inline(value: &str) -> Result<Match, String> {
    let mut m = Match::default();
    for (k, v) in parse_inline_table(value)? {
        match k.as_str() {
            "category" => m.category = v,
            "severity_at_least" => m.severity_at_least = v,
            "reason_regex" => m.reason_regex = v,
            "vm_regex" => m.vm_regex = v,
            "trap_type" => m.trap_type = v,
            _ => return Err(format!("unknown match key '{k}'")),
        }
    }
    Ok(m)
}

fn parse_action_inline(value: &str) -> Result<Action, String> {
    let mut a = Action::default();
    for (k, v) in parse_inline_table(value)? {
        match k.as_str() {
            "kind" => a.kind = v,
            "output_path" => a.output_path = v,
            "nic" => a.nic = v,
            _ => return Err(format!("unknown action key '{k}'")),
        }
    }
    Ok(a)
}

fn parse_actions_array(value: &str) -> Result<Vec<Action>, String> {
    let t = value.trim();
    let mut rest = t
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .ok_or("actions must be an array")?
        .trim();
    let mut out = Vec::new();
    if rest.is_empty() {
        return Ok(out);
    }
    loop {
        if !rest.starts_with('{') {
            return Err("actions array entries must be inline action tables".to_string());
        }
        let consumed = find_inline_table_end(rest)?;
        out.push(parse_action_inline(&rest[..consumed])?);
        rest = rest[consumed..].trim_start();
        if rest.is_empty() {
            break;
        }
        if !rest.starts_with(',') {
            return Err("expected comma between actions array entries".to_string());
        }
        rest = rest[1..].trim_start();
        if rest.is_empty() {
            return Err("trailing comma in actions array".to_string());
        }
    }
    Ok(out)
}

fn find_inline_table_end(input: &str) -> Result<usize, String> {
    let mut in_str = false;
    let mut escape = false;
    for (idx, c) in input.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match c {
            '\\' if in_str => escape = true,
            '"' => in_str = !in_str,
            '{' if !in_str && idx != 0 => {
                return Err("nested inline tables are not supported".to_string())
            }
            '[' if !in_str => return Err("nested arrays are not supported".to_string()),
            '}' if !in_str => return Ok(idx + c.len_utf8()),
            _ => {}
        }
    }
    if escape {
        Err("unterminated escape in action table".to_string())
    } else if in_str {
        Err("unterminated string in action table".to_string())
    } else {
        Err("unterminated action table".to_string())
    }
}

fn parse_inline_table(value: &str) -> Result<Vec<(String, String)>, String> {
    let t = value.trim();
    let inner = t
        .strip_prefix('{')
        .and_then(|v| v.strip_suffix('}'))
        .ok_or_else(|| format!("expected inline table, got {value}"))?;
    let parts = split_comma_parts(inner, "inline table")?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for part in parts {
        let (k, v) = split_key_value(&part)
            .map_err(|e| format!("invalid inline table part '{part}': {e}"))?;
        if !seen.insert(k.to_string()) {
            return Err(format!("duplicate inline key '{k}'"));
        }
        out.push((k.to_string(), parse_inline_value(v)?));
    }
    Ok(out)
}

fn split_comma_parts(input: &str, context: &str) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_str = false;
    let mut escape = false;
    for c in input.chars() {
        if escape {
            current.push(c);
            escape = false;
            continue;
        }
        match c {
            '\\' if in_str => {
                current.push(c);
                escape = true;
            }
            '"' => {
                in_str = !in_str;
                current.push(c);
            }
            ',' if !in_str => {
                if current.trim().is_empty() {
                    return Err(format!("empty {context} entry"));
                }
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if escape {
        return Err(format!("unterminated escape in {context}"));
    }
    if in_str {
        return Err(format!("unterminated string in {context}"));
    }
    if current.trim().is_empty() {
        if input.trim().is_empty() {
            return Ok(parts);
        }
        return Err(format!("trailing comma in {context}"));
    }
    parts.push(current.trim().to_string());
    Ok(parts)
}

fn flush_rule(cfg: &mut Config, current_rule: &mut Option<Rule>) {
    if let Some(rule) = current_rule.take() {
        cfg.policy.rules.push(rule);
    }
}

fn flush_qmp(cfg: &mut Config, current_qmp: &mut Option<QmpMapping>) {
    if let Some(q) = current_qmp.take() {
        cfg.actions.qmp.push(q);
    }
}

fn flush_wx(cfg: &mut Config, current_wx: &mut Option<WxAllowEntry>) {
    if let Some(w) = current_wx.take() {
        cfg.wx_allow.entries.push(w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_config(contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("aegishv-config-{}.toml", std::process::id()));
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{}", contents).unwrap();
        path
    }

    #[test]
    fn rejects_bad_pattern() {
        let p = temp_config(
            r#"
[[policy.rules]]
name = "bad"
action = { kind = "pause_vm" }
match = { category = "wx", reason_regex = "(" }
"#,
        );
        let err = Config::load(Some(&p)).unwrap_err();
        assert!(err.contains("invalid reason_regex"));
        let _ = std::fs::remove_file(p);
    }
}
