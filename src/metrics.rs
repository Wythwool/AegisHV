use crate::event::{Category, IdentityConfidence, Severity};
use crate::identity::{
    IdentityCacheResult, IdentityConflictReason, IdentityEnrichment, VmInventorySnapshot,
};
use crate::vmi::VmiErrorKind;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct Metrics {
    inner: Arc<Inner>,
}

struct Inner {
    started_at: Instant,
    runtime_status: AtomicU8,
    collector_status: AtomicU8,
    metrics_listener_status: AtomicU8,
    output_status: AtomicU8,
    policy_status: AtomicU8,
    pmu_status: AtomicU8,
    queue_status: AtomicU8,
    action_status: AtomicU8,
    lines_ingested_total: AtomicU64,
    trace_inputs_parsed_total: AtomicU64,
    trace_inputs_unrelated_tracepoint_total: AtomicU64,
    trace_inputs_unsupported_line_total: AtomicU64,
    trace_inputs_malformed_kvm_exit_total: AtomicU64,
    trace_inputs_parser_degraded_total: AtomicU64,
    trace_inputs_parser_bug_total: AtomicU64,
    parsed_total: AtomicU64,
    parse_errors_total: AtomicU64,
    unsupported_total: AtomicU64,
    unrelated_tracepoint_total: AtomicU64,
    dropped_total: AtomicU64,
    dropped_by_reason: Mutex<HashMap<String, u64>>,
    collector_eof_total: AtomicU64,
    collector_errors_total: AtomicU64,
    tracefs_read_errors_total: AtomicU64,
    events_total: Mutex<HashMap<(String, String), u64>>,
    wx_total: AtomicU64,
    wx_prune_total: AtomicU64,
    wx_cooldown_suppressed_total: AtomicU64,
    policy_matches_total: Mutex<HashMap<String, u64>>,
    policy_suppressed_total: Mutex<HashMap<(String, String), u64>>,
    policy_actions_total: Mutex<HashMap<(String, bool), u64>>,
    pmu_samples_total: AtomicU64,
    pmu_read_failures_total: AtomicU64,
    pmu_targets: AtomicI64,
    qmp_failures_total: Mutex<HashMap<String, u64>>,
    identity_cache_lookups_total: [AtomicU64; IdentityCacheResult::COUNT],
    identity_enrichments_total: [AtomicU64; IDENTITY_CONFIDENCE_SERIES],
    identity_conflicts_total: [AtomicU64; IdentityConflictReason::COUNT],
    identity_inventory_vms: AtomicI64,
    identity_inventory_degraded: AtomicI64,
    identity_qmp_safety_refusals_total: [AtomicU64; IdentityQmpRefusalReason::COUNT],
    queue_depth: AtomicI64,
    queue_capacity: AtomicI64,
    wx_pages_tracked: AtomicI64,
    json_write_failures_total: AtomicU64,
    syslog_write_failures_total: AtomicU64,
    journald_write_failures_total: AtomicU64,
    spool_events_total: AtomicU64,
    spool_write_failures_total: AtomicU64,
    spool_dropped_total: AtomicU64,
    action_latency_sum_ms: AtomicU64,
    action_latency_count: AtomicU64,
    qmp_latency_sum_ms: AtomicU64,
    qmp_latency_count: AtomicU64,
    parse_latency_sum_us: AtomicU64,
    parse_latency_count: AtomicU64,
    vmi_memory_read_attempts_total: AtomicU64,
    vmi_memory_read_successes_total: AtomicU64,
    vmi_memory_read_failures_total: Mutex<HashMap<String, u64>>,
    vmi_translation_attempts_total: Mutex<HashMap<(String, String), u64>>,
    vmi_translation_successes_total: Mutex<HashMap<(String, String), u64>>,
    vmi_translation_failures_total: Mutex<HashMap<(String, String, String), u64>>,
    vmi_register_access_attempts_total: AtomicU64,
    vmi_register_access_failures_total: Mutex<HashMap<String, u64>>,
    vmi_profile_lookup_attempts_total: AtomicU64,
    vmi_profile_lookup_misses_total: AtomicU64,
    vmi_profile_lookup_failures_total: Mutex<HashMap<String, u64>>,
    vmi_fixture_load_attempts_total: AtomicU64,
    vmi_fixture_load_failures_total: Mutex<HashMap<String, u64>>,
    vmi_unsupported_backend_calls_total: AtomicU64,
}

const IDENTITY_CONFIDENCE_VALUES: [IdentityConfidence; 3] = [
    IdentityConfidence::Low,
    IdentityConfidence::Medium,
    IdentityConfidence::High,
];
const IDENTITY_CONFIDENCE_SERIES: usize = IDENTITY_CONFIDENCE_VALUES.len() * 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceInputReason {
    Parsed,
    UnrelatedTracepoint,
    UnsupportedLine,
    MalformedKvmExit,
    ParserDegraded,
    ParserBug,
}

// VMI metric labels are limited to fixed enums and VmiErrorKind. Do not encode
// guest addresses, fixture paths, build strings, or backend error details here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmiMetricArchitecture {
    X86_64,
    Arm64,
}

impl VmiMetricArchitecture {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Arm64 => "arm64",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmiMetricTranslationMode {
    X86_64FourLevel,
    X86_64La57,
    Arm64Stage1Size4K,
    Arm64Stage1Size16K,
    Arm64Stage1Size64K,
}

impl VmiMetricTranslationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::X86_64FourLevel => "x86_64-4level",
            Self::X86_64La57 => "x86_64-la57",
            Self::Arm64Stage1Size4K => "arm64-stage1-4k",
            Self::Arm64Stage1Size16K => "arm64-stage1-16k",
            Self::Arm64Stage1Size64K => "arm64-stage1-64k",
        }
    }
}

impl TraceInputReason {
    const ALL: [Self; 6] = [
        Self::Parsed,
        Self::UnrelatedTracepoint,
        Self::UnsupportedLine,
        Self::MalformedKvmExit,
        Self::ParserDegraded,
        Self::ParserBug,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parsed => "parsed",
            Self::UnrelatedTracepoint => "unrelated_tracepoint",
            Self::UnsupportedLine => "unsupported_line",
            Self::MalformedKvmExit => "malformed_kvm_exit",
            Self::ParserDegraded => "parser_degraded",
            Self::ParserBug => "parser_bug",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityQmpRefusalReason {
    StableIdentityRequired,
    AmbiguousIdentity,
    ConflictingStableMapping,
    MissingIdentity,
    LowConfidence,
    UnverifiedIdentity,
    PidOnlyIdentity,
    StaleIdentity,
    ConflictingIdentity,
}

impl IdentityQmpRefusalReason {
    pub const ALL: [Self; 9] = [
        Self::StableIdentityRequired,
        Self::AmbiguousIdentity,
        Self::ConflictingStableMapping,
        Self::MissingIdentity,
        Self::LowConfidence,
        Self::UnverifiedIdentity,
        Self::PidOnlyIdentity,
        Self::StaleIdentity,
        Self::ConflictingIdentity,
    ];
    pub const COUNT: usize = Self::ALL.len();

    pub fn index(self) -> usize {
        match self {
            Self::StableIdentityRequired => 0,
            Self::AmbiguousIdentity => 1,
            Self::ConflictingStableMapping => 2,
            Self::MissingIdentity => 3,
            Self::LowConfidence => 4,
            Self::UnverifiedIdentity => 5,
            Self::PidOnlyIdentity => 6,
            Self::StaleIdentity => 7,
            Self::ConflictingIdentity => 8,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::StableIdentityRequired => "stable_identity_required",
            Self::AmbiguousIdentity => "ambiguous_identity",
            Self::ConflictingStableMapping => "conflicting_stable_mapping",
            Self::MissingIdentity => "missing_identity",
            Self::LowConfidence => "low_confidence",
            Self::UnverifiedIdentity => "unverified_identity",
            Self::PidOnlyIdentity => "pid_only_identity",
            Self::StaleIdentity => "stale_identity",
            Self::ConflictingIdentity => "conflicting_identity",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ComponentStatus {
    Starting = 0,
    Running = 1,
    Ok = 2,
    Disabled = 3,
    Degraded = 4,
    Failed = 5,
    Stopped = 6,
    Stopping = 7,
}

impl ComponentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Ok => "ok",
            Self::Disabled => "disabled",
            Self::Degraded => "degraded",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
            Self::Stopping => "stopping",
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Running,
            2 => Self::Ok,
            3 => Self::Disabled,
            4 => Self::Degraded,
            5 => Self::Failed,
            6 => Self::Stopped,
            7 => Self::Stopping,
            _ => Self::Starting,
        }
    }

    fn is_ready_ok(self) -> bool {
        matches!(self, Self::Running | Self::Ok | Self::Disabled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthSnapshot {
    pub status: &'static str,
    pub healthy: bool,
    pub ready: bool,
    pub components: HealthComponents,
}

impl HealthSnapshot {
    pub fn to_json(&self) -> String {
        let mut components = String::new();
        for (idx, (name, status)) in self.components.iter().iter().enumerate() {
            if idx > 0 {
                components.push(',');
            }
            components.push_str(&format!("\"{name}\":\"{}\"", status.as_str()));
        }
        format!(
            "{{\"schema_version\":1,\"status\":\"{}\",\"healthy\":{},\"ready\":{},\"components\":{{{components}}}}}\n",
            self.status, self.healthy, self.ready
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthComponents {
    pub runtime: ComponentStatus,
    pub collector: ComponentStatus,
    pub metrics_listener: ComponentStatus,
    pub output: ComponentStatus,
    pub policy: ComponentStatus,
    pub pmu: ComponentStatus,
    pub queue: ComponentStatus,
    pub actions: ComponentStatus,
}

impl HealthComponents {
    fn iter(&self) -> [(&'static str, ComponentStatus); 8] {
        [
            ("runtime", self.runtime),
            ("collector", self.collector),
            ("metrics_listener", self.metrics_listener),
            ("output", self.output),
            ("policy", self.policy),
            ("pmu", self.pmu),
            ("queue", self.queue),
            ("actions", self.actions),
        ]
    }
}

impl Metrics {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            inner: Arc::new(Inner {
                started_at: Instant::now(),
                runtime_status: AtomicU8::new(ComponentStatus::Starting as u8),
                collector_status: AtomicU8::new(ComponentStatus::Starting as u8),
                metrics_listener_status: AtomicU8::new(ComponentStatus::Starting as u8),
                output_status: AtomicU8::new(ComponentStatus::Starting as u8),
                policy_status: AtomicU8::new(ComponentStatus::Starting as u8),
                pmu_status: AtomicU8::new(ComponentStatus::Starting as u8),
                queue_status: AtomicU8::new(ComponentStatus::Starting as u8),
                action_status: AtomicU8::new(ComponentStatus::Starting as u8),
                lines_ingested_total: AtomicU64::new(0),
                trace_inputs_parsed_total: AtomicU64::new(0),
                trace_inputs_unrelated_tracepoint_total: AtomicU64::new(0),
                trace_inputs_unsupported_line_total: AtomicU64::new(0),
                trace_inputs_malformed_kvm_exit_total: AtomicU64::new(0),
                trace_inputs_parser_degraded_total: AtomicU64::new(0),
                trace_inputs_parser_bug_total: AtomicU64::new(0),
                parsed_total: AtomicU64::new(0),
                parse_errors_total: AtomicU64::new(0),
                unsupported_total: AtomicU64::new(0),
                unrelated_tracepoint_total: AtomicU64::new(0),
                dropped_total: AtomicU64::new(0),
                dropped_by_reason: Mutex::new(HashMap::new()),
                collector_eof_total: AtomicU64::new(0),
                collector_errors_total: AtomicU64::new(0),
                tracefs_read_errors_total: AtomicU64::new(0),
                events_total: Mutex::new(HashMap::new()),
                wx_total: AtomicU64::new(0),
                wx_prune_total: AtomicU64::new(0),
                wx_cooldown_suppressed_total: AtomicU64::new(0),
                policy_matches_total: Mutex::new(HashMap::new()),
                policy_suppressed_total: Mutex::new(HashMap::new()),
                policy_actions_total: Mutex::new(HashMap::new()),
                pmu_samples_total: AtomicU64::new(0),
                pmu_read_failures_total: AtomicU64::new(0),
                pmu_targets: AtomicI64::new(0),
                qmp_failures_total: Mutex::new(HashMap::new()),
                identity_cache_lookups_total: std::array::from_fn(|_| AtomicU64::new(0)),
                identity_enrichments_total: std::array::from_fn(|_| AtomicU64::new(0)),
                identity_conflicts_total: std::array::from_fn(|_| AtomicU64::new(0)),
                identity_inventory_vms: AtomicI64::new(0),
                identity_inventory_degraded: AtomicI64::new(0),
                identity_qmp_safety_refusals_total: std::array::from_fn(|_| AtomicU64::new(0)),
                queue_depth: AtomicI64::new(0),
                queue_capacity: AtomicI64::new(0),
                wx_pages_tracked: AtomicI64::new(0),
                json_write_failures_total: AtomicU64::new(0),
                syslog_write_failures_total: AtomicU64::new(0),
                journald_write_failures_total: AtomicU64::new(0),
                spool_events_total: AtomicU64::new(0),
                spool_write_failures_total: AtomicU64::new(0),
                spool_dropped_total: AtomicU64::new(0),
                action_latency_sum_ms: AtomicU64::new(0),
                action_latency_count: AtomicU64::new(0),
                qmp_latency_sum_ms: AtomicU64::new(0),
                qmp_latency_count: AtomicU64::new(0),
                parse_latency_sum_us: AtomicU64::new(0),
                parse_latency_count: AtomicU64::new(0),
                vmi_memory_read_attempts_total: AtomicU64::new(0),
                vmi_memory_read_successes_total: AtomicU64::new(0),
                vmi_memory_read_failures_total: Mutex::new(HashMap::new()),
                vmi_translation_attempts_total: Mutex::new(HashMap::new()),
                vmi_translation_successes_total: Mutex::new(HashMap::new()),
                vmi_translation_failures_total: Mutex::new(HashMap::new()),
                vmi_register_access_attempts_total: AtomicU64::new(0),
                vmi_register_access_failures_total: Mutex::new(HashMap::new()),
                vmi_profile_lookup_attempts_total: AtomicU64::new(0),
                vmi_profile_lookup_misses_total: AtomicU64::new(0),
                vmi_profile_lookup_failures_total: Mutex::new(HashMap::new()),
                vmi_fixture_load_attempts_total: AtomicU64::new(0),
                vmi_fixture_load_failures_total: Mutex::new(HashMap::new()),
                vmi_unsupported_backend_calls_total: AtomicU64::new(0),
            }),
        })
    }

    pub fn inc_event(&self, category: Category, severity: Severity) {
        inc_map(
            &self.inner.events_total,
            (category.as_str().to_string(), severity.as_str().to_string()),
        );
    }
    pub fn inc_ingest_line(&self) {
        self.inner
            .lines_ingested_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_trace_input(&self, reason: TraceInputReason) {
        self.trace_input_counter(reason)
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_parse_ok(&self) {
        self.inner.parsed_total.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_parse_error(&self) {
        self.inner
            .parse_errors_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_unsupported(&self) {
        self.inner.unsupported_total.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_unrelated_tracepoint(&self) {
        self.inner
            .unrelated_tracepoint_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_dropped(&self, reason: &str) {
        self.inner.dropped_total.fetch_add(1, Ordering::Relaxed);
        if reason.contains("queue") {
            self.set_component(&self.inner.queue_status, ComponentStatus::Degraded);
        }
        let mut map = self.inner.dropped_by_reason.lock().expect("metrics lock");
        *map.entry(reason.to_string()).or_insert(0) += 1;
    }
    pub fn dropped_total(&self) -> u64 {
        self.inner.dropped_total.load(Ordering::Relaxed)
    }
    pub fn json_write_failures_total(&self) -> u64 {
        self.inner.json_write_failures_total.load(Ordering::Relaxed)
    }
    pub fn syslog_write_failures_total(&self) -> u64 {
        self.inner
            .syslog_write_failures_total
            .load(Ordering::Relaxed)
    }
    pub fn journald_write_failures_total(&self) -> u64 {
        self.inner
            .journald_write_failures_total
            .load(Ordering::Relaxed)
    }
    pub fn spool_events_total(&self) -> u64 {
        self.inner.spool_events_total.load(Ordering::Relaxed)
    }
    pub fn spool_write_failures_total(&self) -> u64 {
        self.inner
            .spool_write_failures_total
            .load(Ordering::Relaxed)
    }
    pub fn spool_dropped_total(&self) -> u64 {
        self.inner.spool_dropped_total.load(Ordering::Relaxed)
    }
    pub fn inc_collector_eof(&self) {
        self.inner
            .collector_eof_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_collector_error(&self) {
        self.inner
            .collector_errors_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_tracefs_read_error(&self) {
        self.inner
            .tracefs_read_errors_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_vmi_memory_read_attempt(&self) {
        self.inner
            .vmi_memory_read_attempts_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_vmi_memory_read_success(&self) {
        self.inner
            .vmi_memory_read_successes_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_vmi_memory_read_failure(&self, kind: VmiErrorKind) {
        inc_map(
            &self.inner.vmi_memory_read_failures_total,
            vmi_error_key(kind),
        );
    }
    pub fn record_vmi_translation_attempt(
        &self,
        architecture: VmiMetricArchitecture,
        mode: VmiMetricTranslationMode,
    ) {
        inc_map(
            &self.inner.vmi_translation_attempts_total,
            vmi_translation_key(architecture, mode),
        );
    }
    pub fn record_vmi_translation_success(
        &self,
        architecture: VmiMetricArchitecture,
        mode: VmiMetricTranslationMode,
    ) {
        inc_map(
            &self.inner.vmi_translation_successes_total,
            vmi_translation_key(architecture, mode),
        );
    }
    pub fn record_vmi_translation_failure(
        &self,
        architecture: VmiMetricArchitecture,
        mode: VmiMetricTranslationMode,
        kind: VmiErrorKind,
    ) {
        inc_map(
            &self.inner.vmi_translation_failures_total,
            (
                architecture.as_str().to_string(),
                mode.as_str().to_string(),
                vmi_error_key(kind),
            ),
        );
    }
    pub fn record_vmi_register_access_attempt(&self) {
        self.inner
            .vmi_register_access_attempts_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_vmi_register_access_failure(&self, kind: VmiErrorKind) {
        inc_map(
            &self.inner.vmi_register_access_failures_total,
            vmi_error_key(kind),
        );
    }
    pub fn record_vmi_profile_lookup_attempt(&self) {
        self.inner
            .vmi_profile_lookup_attempts_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_vmi_profile_lookup_miss(&self) {
        self.inner
            .vmi_profile_lookup_misses_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_vmi_profile_lookup_failure(&self, kind: VmiErrorKind) {
        inc_map(
            &self.inner.vmi_profile_lookup_failures_total,
            vmi_error_key(kind),
        );
    }
    pub fn record_vmi_fixture_load_attempt(&self) {
        self.inner
            .vmi_fixture_load_attempts_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn record_vmi_fixture_load_failure(&self, kind: VmiErrorKind) {
        inc_map(
            &self.inner.vmi_fixture_load_failures_total,
            vmi_error_key(kind),
        );
    }
    pub fn record_vmi_unsupported_backend_call(&self) {
        self.inner
            .vmi_unsupported_backend_calls_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_wx(&self) {
        self.inner.wx_total.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_wx_prune(&self, count: u64) {
        self.inner
            .wx_prune_total
            .fetch_add(count, Ordering::Relaxed);
    }
    pub fn inc_wx_cooldown_suppressed(&self, count: u64) {
        self.inner
            .wx_cooldown_suppressed_total
            .fetch_add(count, Ordering::Relaxed);
    }
    pub fn inc_policy_match(&self, rule: &str) {
        inc_map(&self.inner.policy_matches_total, rule.to_string());
    }
    pub fn inc_policy_suppressed(&self, rule: &str, reason: &str) {
        inc_map(
            &self.inner.policy_suppressed_total,
            (rule.to_string(), reason.to_string()),
        );
    }
    pub fn inc_policy_action(&self, kind: &str, ok: bool) {
        if !ok {
            self.mark_actions_degraded();
        }
        inc_map(&self.inner.policy_actions_total, (kind.to_string(), ok));
    }
    pub fn inc_pmu(&self) {
        self.inner.pmu_samples_total.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_pmu_read_failure(&self) {
        self.mark_pmu_degraded();
        self.inner
            .pmu_read_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn set_pmu_targets(&self, targets: usize) {
        self.inner
            .pmu_targets
            .store(targets as i64, Ordering::Relaxed);
    }
    pub fn inc_qmp_failure(&self, kind: &str) {
        self.mark_actions_degraded();
        inc_map(&self.inner.qmp_failures_total, kind.to_string());
    }
    pub fn record_identity_enrichment(&self, enrichment: &IdentityEnrichment) {
        if let Some(result) = enrichment.cache_result {
            self.inner.identity_cache_lookups_total[result.index()].fetch_add(1, Ordering::Relaxed);
        }
        if let Some(confidence) = enrichment.confidence {
            self.inner.identity_enrichments_total
                [identity_confidence_index(confidence, enrichment.ambiguous)]
            .fetch_add(1, Ordering::Relaxed);
        }
        if let Some(reason) = enrichment.conflict_reason {
            self.inner.identity_conflicts_total[reason.index()].fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn set_identity_inventory(&self, snapshot: &VmInventorySnapshot) {
        self.inner
            .identity_inventory_vms
            .store(snapshot.vm_count as i64, Ordering::Relaxed);
        self.inner
            .identity_inventory_degraded
            .store(if snapshot.degraded { 1 } else { 0 }, Ordering::Relaxed);
    }
    pub fn inc_identity_qmp_safety_refusal(&self, reason: IdentityQmpRefusalReason) {
        self.mark_actions_degraded();
        self.inner.identity_qmp_safety_refusals_total[reason.index()]
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_json_write_failure(&self) {
        self.inner
            .json_write_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_syslog_write_failure(&self) {
        self.inner
            .syslog_write_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_journald_write_failure(&self) {
        self.inner
            .journald_write_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_spool_event(&self) {
        self.inner
            .spool_events_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_spool_write_failure(&self) {
        self.inner
            .spool_write_failures_total
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_spool_dropped(&self) {
        self.inner
            .spool_dropped_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn mark_runtime_running(&self) {
        self.set_component(&self.inner.runtime_status, ComponentStatus::Running);
    }
    pub fn mark_runtime_stopping(&self) {
        self.set_component(&self.inner.runtime_status, ComponentStatus::Stopping);
    }
    pub fn mark_runtime_stopped(&self) {
        self.set_component(&self.inner.runtime_status, ComponentStatus::Stopped);
    }
    pub fn mark_runtime_failed(&self) {
        self.set_component(&self.inner.runtime_status, ComponentStatus::Failed);
    }
    pub fn mark_collector_running(&self) {
        self.set_component(&self.inner.collector_status, ComponentStatus::Running);
    }
    pub fn mark_collector_stopped(&self) {
        self.set_component(&self.inner.collector_status, ComponentStatus::Stopped);
    }
    pub fn mark_collector_failed(&self) {
        self.set_component(&self.inner.collector_status, ComponentStatus::Failed);
    }
    pub fn mark_metrics_listener_running(&self) {
        self.set_component(
            &self.inner.metrics_listener_status,
            ComponentStatus::Running,
        );
    }
    pub fn mark_metrics_listener_disabled(&self) {
        self.set_component(
            &self.inner.metrics_listener_status,
            ComponentStatus::Disabled,
        );
    }
    pub fn mark_metrics_listener_degraded(&self) {
        self.set_component(
            &self.inner.metrics_listener_status,
            ComponentStatus::Degraded,
        );
    }
    pub fn mark_output_ok(&self) {
        self.set_component(&self.inner.output_status, ComponentStatus::Ok);
    }
    pub fn mark_output_degraded(&self) {
        self.set_component(&self.inner.output_status, ComponentStatus::Degraded);
    }
    pub fn mark_output_failed(&self) {
        self.set_component(&self.inner.output_status, ComponentStatus::Failed);
    }
    pub fn mark_policy_ok(&self) {
        self.set_component(&self.inner.policy_status, ComponentStatus::Ok);
    }
    pub fn mark_policy_degraded(&self) {
        self.set_component(&self.inner.policy_status, ComponentStatus::Degraded);
    }
    pub fn mark_policy_failed(&self) {
        self.set_component(&self.inner.policy_status, ComponentStatus::Failed);
    }
    pub fn mark_pmu_running(&self) {
        self.set_component(&self.inner.pmu_status, ComponentStatus::Running);
    }
    pub fn mark_pmu_disabled(&self) {
        self.set_component(&self.inner.pmu_status, ComponentStatus::Disabled);
    }
    pub fn mark_pmu_degraded(&self) {
        self.set_component(&self.inner.pmu_status, ComponentStatus::Degraded);
    }
    pub fn mark_pmu_failed(&self) {
        self.set_component(&self.inner.pmu_status, ComponentStatus::Failed);
    }
    pub fn mark_actions_ok(&self) {
        self.set_component(&self.inner.action_status, ComponentStatus::Ok);
    }
    pub fn mark_actions_degraded(&self) {
        self.set_component(&self.inner.action_status, ComponentStatus::Degraded);
    }
    pub fn mark_actions_failed(&self) {
        self.set_component(&self.inner.action_status, ComponentStatus::Failed);
    }

    pub fn set_queue_depth(&self, depth: i64, capacity: usize) {
        let capacity = queue_capacity_i64(capacity);
        self.inner.queue_capacity.store(capacity, Ordering::Relaxed);
        self.inner
            .queue_depth
            .store(depth.clamp(0, capacity), Ordering::Relaxed);
        self.update_queue_status(depth.clamp(0, capacity), capacity);
    }
    pub fn record_queue_send_attempt(&self, capacity: usize) {
        let capacity = queue_capacity_i64(capacity);
        self.inner.queue_capacity.store(capacity, Ordering::Relaxed);
        let _ =
            self.inner
                .queue_depth
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |depth| {
                    Some(depth.saturating_add(1).min(capacity))
                });
        self.update_queue_status(self.queue_depth(), capacity);
    }
    pub fn record_queue_send_rejected(&self) {
        self.record_queue_receive();
    }
    pub fn record_queue_full(&self, capacity: usize) {
        let capacity = queue_capacity_i64(capacity);
        self.inner.queue_capacity.store(capacity, Ordering::Relaxed);
        self.inner.queue_depth.store(capacity, Ordering::Relaxed);
        self.set_component(&self.inner.queue_status, ComponentStatus::Degraded);
    }
    pub fn record_queue_receive(&self) {
        let _ =
            self.inner
                .queue_depth
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |depth| {
                    Some(depth.saturating_sub(1).max(0))
                });
        self.update_queue_status(self.queue_depth(), self.queue_capacity());
    }
    pub fn queue_depth(&self) -> i64 {
        self.inner.queue_depth.load(Ordering::Relaxed)
    }
    pub fn queue_capacity(&self) -> i64 {
        self.inner.queue_capacity.load(Ordering::Relaxed)
    }
    pub fn set_wx_pages_tracked(&self, count: usize) {
        self.inner
            .wx_pages_tracked
            .store(count as i64, Ordering::Relaxed);
    }
    pub fn observe_parse_latency_ms(&self, ms: f64) {
        self.inner
            .parse_latency_sum_us
            .fetch_add((ms * 1000.0).round() as u64, Ordering::Relaxed);
        self.inner
            .parse_latency_count
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn observe_qmp_latency_ms(&self, ms: f64) {
        self.inner
            .qmp_latency_sum_ms
            .fetch_add(ms.round() as u64, Ordering::Relaxed);
        self.inner.qmp_latency_count.fetch_add(1, Ordering::Relaxed);
    }
    pub fn observe_action_latency_ms(&self, ms: f64) {
        self.inner
            .action_latency_sum_ms
            .fetch_add(ms.round() as u64, Ordering::Relaxed);
        self.inner
            .action_latency_count
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn mark_unhealthy(&self) {
        self.mark_runtime_failed();
    }
    pub fn mark_healthy(&self) {
        self.mark_runtime_running();
    }
    pub fn is_healthy(&self) -> bool {
        self.health_snapshot().healthy
    }

    pub fn health_snapshot(&self) -> HealthSnapshot {
        let components = HealthComponents {
            runtime: self.component_status(&self.inner.runtime_status),
            collector: self.component_status(&self.inner.collector_status),
            metrics_listener: self.component_status(&self.inner.metrics_listener_status),
            output: self.component_status(&self.inner.output_status),
            policy: self.component_status(&self.inner.policy_status),
            pmu: self.component_status(&self.inner.pmu_status),
            queue: self.component_status(&self.inner.queue_status),
            actions: self.component_status(&self.inner.action_status),
        };
        let statuses = components.iter();
        let failed = statuses
            .iter()
            .any(|(_, status)| *status == ComponentStatus::Failed);
        let degraded = statuses
            .iter()
            .any(|(_, status)| *status == ComponentStatus::Degraded);
        let starting = statuses
            .iter()
            .any(|(_, status)| *status == ComponentStatus::Starting);
        let status = if failed {
            "failed"
        } else if components.runtime == ComponentStatus::Stopping {
            "stopping"
        } else if components.runtime == ComponentStatus::Stopped {
            "stopped"
        } else if starting {
            "starting"
        } else if degraded {
            "degraded"
        } else {
            "ok"
        };
        let healthy = !failed
            && !matches!(
                components.runtime,
                ComponentStatus::Stopping | ComponentStatus::Stopped
            );
        let ready = components.runtime == ComponentStatus::Running
            && components.collector == ComponentStatus::Running
            && components.output == ComponentStatus::Ok
            && components.policy == ComponentStatus::Ok
            && components.queue == ComponentStatus::Ok
            && components.actions == ComponentStatus::Ok
            && components.metrics_listener.is_ready_ok()
            && components.pmu.is_ready_ok()
            && !degraded
            && !failed;
        HealthSnapshot {
            status,
            healthy,
            ready,
            components,
        }
    }

    fn set_component(&self, target: &AtomicU8, status: ComponentStatus) {
        target.store(status as u8, Ordering::Relaxed);
    }

    fn component_status(&self, target: &AtomicU8) -> ComponentStatus {
        ComponentStatus::from_u8(target.load(Ordering::Relaxed))
    }

    fn trace_input_counter(&self, reason: TraceInputReason) -> &AtomicU64 {
        match reason {
            TraceInputReason::Parsed => &self.inner.trace_inputs_parsed_total,
            TraceInputReason::UnrelatedTracepoint => {
                &self.inner.trace_inputs_unrelated_tracepoint_total
            }
            TraceInputReason::UnsupportedLine => &self.inner.trace_inputs_unsupported_line_total,
            TraceInputReason::MalformedKvmExit => &self.inner.trace_inputs_malformed_kvm_exit_total,
            TraceInputReason::ParserDegraded => &self.inner.trace_inputs_parser_degraded_total,
            TraceInputReason::ParserBug => &self.inner.trace_inputs_parser_bug_total,
        }
    }

    fn update_queue_status(&self, depth: i64, capacity: i64) {
        let status = if capacity > 0 && depth >= capacity {
            ComponentStatus::Degraded
        } else {
            ComponentStatus::Ok
        };
        self.set_component(&self.inner.queue_status, status);
    }

    pub fn encode(&self) -> String {
        let mut out = String::new();
        let health = self.health_snapshot();
        out.push_str(
            "# HELP aegishv_build_info Build information.\n# TYPE aegishv_build_info gauge\n",
        );
        out.push_str(&format!(
            "aegishv_build_info{{version=\"{}\"}} 1\n",
            env!("CARGO_PKG_VERSION")
        ));
        gauge(
            &mut out,
            "aegishv_health_status",
            if health.healthy { 1.0 } else { 0.0 },
        );
        gauge(
            &mut out,
            "aegishv_readiness_status",
            if health.ready { 1.0 } else { 0.0 },
        );
        out.push_str("# TYPE aegishv_component_status gauge\n");
        for (name, status) in health.components.iter() {
            out.push_str(&format!(
                "aegishv_component_status{{component=\"{}\",state=\"{}\"}} 1\n",
                esc_label(name),
                status.as_str()
            ));
        }
        gauge(
            &mut out,
            "aegishv_uptime_seconds",
            self.inner.started_at.elapsed().as_secs_f64(),
        );
        counter(
            &mut out,
            "aegishv_lines_ingested_total",
            self.inner.lines_ingested_total.load(Ordering::Relaxed),
        );
        out.push_str("# TYPE aegishv_trace_inputs_total counter\n");
        for reason in TraceInputReason::ALL {
            out.push_str(&format!(
                "aegishv_trace_inputs_total{{reason=\"{}\"}} {}\n",
                reason.as_str(),
                self.trace_input_counter(reason).load(Ordering::Relaxed)
            ));
        }
        counter(
            &mut out,
            "aegishv_parsed_total",
            self.inner.parsed_total.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_parse_errors_total",
            self.inner.parse_errors_total.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_unsupported_total",
            self.inner.unsupported_total.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_unrelated_tracepoint_total",
            self.inner
                .unrelated_tracepoint_total
                .load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_dropped_total",
            self.inner.dropped_total.load(Ordering::Relaxed),
        );
        for (reason, value) in snapshot_map(&self.inner.dropped_by_reason) {
            out.push_str(&format!(
                "aegishv_dropped_by_reason_total{{reason=\"{}\"}} {}\n",
                esc_label(&reason),
                value
            ));
        }
        counter(
            &mut out,
            "aegishv_collector_eof_total",
            self.inner.collector_eof_total.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_collector_errors_total",
            self.inner.collector_errors_total.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_tracefs_read_errors_total",
            self.inner.tracefs_read_errors_total.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_vmi_memory_read_attempts_total",
            self.inner
                .vmi_memory_read_attempts_total
                .load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_vmi_memory_read_successes_total",
            self.inner
                .vmi_memory_read_successes_total
                .load(Ordering::Relaxed),
        );
        out.push_str("# TYPE aegishv_vmi_memory_read_failures_total counter\n");
        for (kind, value) in snapshot_map(&self.inner.vmi_memory_read_failures_total) {
            out.push_str(&format!(
                "aegishv_vmi_memory_read_failures_total{{kind=\"{}\"}} {}\n",
                esc_label(&kind),
                value
            ));
        }
        out.push_str("# TYPE aegishv_vmi_translation_attempts_total counter\n");
        for ((architecture, mode), value) in
            snapshot_map(&self.inner.vmi_translation_attempts_total)
        {
            out.push_str(&format!(
                "aegishv_vmi_translation_attempts_total{{architecture=\"{}\",mode=\"{}\"}} {}\n",
                esc_label(&architecture),
                esc_label(&mode),
                value
            ));
        }
        out.push_str("# TYPE aegishv_vmi_translation_successes_total counter\n");
        for ((architecture, mode), value) in
            snapshot_map(&self.inner.vmi_translation_successes_total)
        {
            out.push_str(&format!(
                "aegishv_vmi_translation_successes_total{{architecture=\"{}\",mode=\"{}\"}} {}\n",
                esc_label(&architecture),
                esc_label(&mode),
                value
            ));
        }
        out.push_str("# TYPE aegishv_vmi_translation_failures_total counter\n");
        for ((architecture, mode, kind), value) in
            snapshot_map(&self.inner.vmi_translation_failures_total)
        {
            out.push_str(&format!(
                "aegishv_vmi_translation_failures_total{{architecture=\"{}\",mode=\"{}\",kind=\"{}\"}} {}\n",
                esc_label(&architecture),
                esc_label(&mode),
                esc_label(&kind),
                value
            ));
        }
        counter(
            &mut out,
            "aegishv_vmi_register_access_attempts_total",
            self.inner
                .vmi_register_access_attempts_total
                .load(Ordering::Relaxed),
        );
        out.push_str("# TYPE aegishv_vmi_register_access_failures_total counter\n");
        for (kind, value) in snapshot_map(&self.inner.vmi_register_access_failures_total) {
            out.push_str(&format!(
                "aegishv_vmi_register_access_failures_total{{kind=\"{}\"}} {}\n",
                esc_label(&kind),
                value
            ));
        }
        counter(
            &mut out,
            "aegishv_vmi_profile_lookup_attempts_total",
            self.inner
                .vmi_profile_lookup_attempts_total
                .load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_vmi_profile_lookup_misses_total",
            self.inner
                .vmi_profile_lookup_misses_total
                .load(Ordering::Relaxed),
        );
        out.push_str("# TYPE aegishv_vmi_profile_lookup_failures_total counter\n");
        for (kind, value) in snapshot_map(&self.inner.vmi_profile_lookup_failures_total) {
            out.push_str(&format!(
                "aegishv_vmi_profile_lookup_failures_total{{kind=\"{}\"}} {}\n",
                esc_label(&kind),
                value
            ));
        }
        counter(
            &mut out,
            "aegishv_vmi_fixture_load_attempts_total",
            self.inner
                .vmi_fixture_load_attempts_total
                .load(Ordering::Relaxed),
        );
        out.push_str("# TYPE aegishv_vmi_fixture_load_failures_total counter\n");
        for (kind, value) in snapshot_map(&self.inner.vmi_fixture_load_failures_total) {
            out.push_str(&format!(
                "aegishv_vmi_fixture_load_failures_total{{kind=\"{}\"}} {}\n",
                esc_label(&kind),
                value
            ));
        }
        counter(
            &mut out,
            "aegishv_vmi_unsupported_backend_calls_total",
            self.inner
                .vmi_unsupported_backend_calls_total
                .load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_wx_total",
            self.inner.wx_total.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_wx_prune_total",
            self.inner.wx_prune_total.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_wx_cooldown_suppressed_total",
            self.inner
                .wx_cooldown_suppressed_total
                .load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_pmu_samples_total",
            self.inner.pmu_samples_total.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_pmu_read_failures_total",
            self.inner.pmu_read_failures_total.load(Ordering::Relaxed),
        );
        gauge(
            &mut out,
            "aegishv_pmu_targets",
            self.inner.pmu_targets.load(Ordering::Relaxed) as f64,
        );
        gauge(
            &mut out,
            "aegishv_queue_depth",
            self.inner.queue_depth.load(Ordering::Relaxed) as f64,
        );
        let cap = self.inner.queue_capacity.load(Ordering::Relaxed);
        gauge(&mut out, "aegishv_queue_capacity", cap as f64);
        let util = if cap > 0 {
            self.inner.queue_depth.load(Ordering::Relaxed) as f64 / cap as f64
        } else {
            0.0
        };
        gauge(&mut out, "aegishv_queue_utilization", util);
        gauge(
            &mut out,
            "aegishv_wx_pages_tracked",
            self.inner.wx_pages_tracked.load(Ordering::Relaxed) as f64,
        );
        counter(
            &mut out,
            "aegishv_json_write_failures_total",
            self.inner.json_write_failures_total.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_syslog_write_failures_total",
            self.inner
                .syslog_write_failures_total
                .load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_journald_write_failures_total",
            self.inner
                .journald_write_failures_total
                .load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_spool_events_total",
            self.inner.spool_events_total.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_spool_write_failures_total",
            self.inner
                .spool_write_failures_total
                .load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_spool_dropped_total",
            self.inner.spool_dropped_total.load(Ordering::Relaxed),
        );
        for ((category, severity), value) in snapshot_map(&self.inner.events_total) {
            out.push_str(&format!(
                "aegishv_events_total{{category=\"{}\",severity=\"{}\"}} {}\n",
                esc_label(&category),
                esc_label(&severity),
                value
            ));
        }
        for (rule, value) in snapshot_map(&self.inner.policy_matches_total) {
            out.push_str(&format!(
                "aegishv_policy_matches_total{{rule=\"{}\"}} {}\n",
                esc_label(&rule),
                value
            ));
        }
        for ((rule, reason), value) in snapshot_map(&self.inner.policy_suppressed_total) {
            out.push_str(&format!(
                "aegishv_policy_suppressed_total{{rule=\"{}\",reason=\"{}\"}} {}\n",
                esc_label(&rule),
                esc_label(&reason),
                value
            ));
        }
        for ((kind, ok), value) in snapshot_map(&self.inner.policy_actions_total) {
            out.push_str(&format!(
                "aegishv_policy_actions_total{{kind=\"{}\",ok=\"{}\"}} {}\n",
                esc_label(&kind),
                ok,
                value
            ));
        }
        for (kind, value) in snapshot_map(&self.inner.qmp_failures_total) {
            out.push_str(&format!(
                "aegishv_qmp_failures_total{{kind=\"{}\"}} {}\n",
                esc_label(&kind),
                value
            ));
        }
        out.push_str("# TYPE aegishv_identity_cache_lookups_total counter\n");
        for result in IdentityCacheResult::ALL {
            out.push_str(&format!(
                "aegishv_identity_cache_lookups_total{{result=\"{}\"}} {}\n",
                result.as_str(),
                self.inner.identity_cache_lookups_total[result.index()].load(Ordering::Relaxed)
            ));
        }
        out.push_str("# TYPE aegishv_identity_enrichments_total counter\n");
        for ambiguous in [false, true] {
            for confidence in IDENTITY_CONFIDENCE_VALUES {
                out.push_str(&format!(
                    "aegishv_identity_enrichments_total{{confidence=\"{}\",ambiguous=\"{}\"}} {}\n",
                    confidence.as_str(),
                    ambiguous,
                    self.inner.identity_enrichments_total
                        [identity_confidence_index(confidence, ambiguous)]
                    .load(Ordering::Relaxed)
                ));
            }
        }
        out.push_str("# TYPE aegishv_identity_conflicts_total counter\n");
        for reason in IdentityConflictReason::ALL {
            out.push_str(&format!(
                "aegishv_identity_conflicts_total{{reason=\"{}\"}} {}\n",
                reason.as_str(),
                self.inner.identity_conflicts_total[reason.index()].load(Ordering::Relaxed)
            ));
        }
        gauge(
            &mut out,
            "aegishv_identity_inventory_vms",
            self.inner.identity_inventory_vms.load(Ordering::Relaxed) as f64,
        );
        gauge(
            &mut out,
            "aegishv_identity_inventory_degraded",
            self.inner
                .identity_inventory_degraded
                .load(Ordering::Relaxed) as f64,
        );
        out.push_str("# TYPE aegishv_identity_qmp_safety_refusals_total counter\n");
        for reason in IdentityQmpRefusalReason::ALL {
            out.push_str(&format!(
                "aegishv_identity_qmp_safety_refusals_total{{reason=\"{}\"}} {}\n",
                reason.as_str(),
                self.inner.identity_qmp_safety_refusals_total[reason.index()]
                    .load(Ordering::Relaxed)
            ));
        }
        counter(
            &mut out,
            "aegishv_action_latency_count",
            self.inner.action_latency_count.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_action_latency_sum_ms",
            self.inner.action_latency_sum_ms.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_qmp_latency_count",
            self.inner.qmp_latency_count.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_qmp_latency_sum_ms",
            self.inner.qmp_latency_sum_ms.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_parse_latency_count",
            self.inner.parse_latency_count.load(Ordering::Relaxed),
        );
        counter(
            &mut out,
            "aegishv_parse_latency_sum_us",
            self.inner.parse_latency_sum_us.load(Ordering::Relaxed),
        );
        out
    }
}

fn inc_map<K: Eq + std::hash::Hash>(m: &Mutex<HashMap<K, u64>>, key: K) {
    let mut map = m.lock().expect("metrics lock");
    *map.entry(key).or_insert(0) += 1;
}

fn snapshot_map<K: Clone + Eq + std::hash::Hash>(m: &Mutex<HashMap<K, u64>>) -> Vec<(K, u64)> {
    m.lock()
        .expect("metrics lock")
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect()
}

fn vmi_error_key(kind: VmiErrorKind) -> String {
    kind.as_str().to_string()
}

fn vmi_translation_key(
    architecture: VmiMetricArchitecture,
    mode: VmiMetricTranslationMode,
) -> (String, String) {
    (architecture.as_str().to_string(), mode.as_str().to_string())
}

fn counter(out: &mut String, name: &str, value: u64) {
    out.push_str(&format!("# TYPE {name} counter\n{name} {value}\n"));
}

fn gauge(out: &mut String, name: &str, value: f64) {
    out.push_str(&format!("# TYPE {name} gauge\n{name} {value:.6}\n"));
}

fn queue_capacity_i64(capacity: usize) -> i64 {
    i64::try_from(capacity).unwrap_or(i64::MAX)
}

fn identity_confidence_index(confidence: IdentityConfidence, ambiguous: bool) -> usize {
    let confidence_index = match confidence {
        IdentityConfidence::Low => 0,
        IdentityConfidence::Medium => 1,
        IdentityConfidence::High => 2,
    };
    confidence_index
        + if ambiguous {
            IDENTITY_CONFIDENCE_VALUES.len()
        } else {
            0
        }
}

fn esc_label(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
