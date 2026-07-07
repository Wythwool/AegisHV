use crate::util::{
    json_opt_f64, json_opt_i32, json_opt_string, json_opt_u64, json_str, monotonic_ms,
    next_event_id, next_sequence, runtime_metadata,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }

    pub fn at_least(self, other: Severity) -> bool {
        self.rank() >= other.rank()
    }

    fn rank(self) -> u8 {
        match self {
            Severity::Info => 0,
            Severity::Low => 1,
            Severity::Medium => 2,
            Severity::High => 3,
            Severity::Critical => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Exit,
    Wx,
    Pmu,
    Policy,
    Snapshot,
    Sensor,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Exit => "exit",
            Category::Wx => "wx",
            Category::Pmu => "pmu",
            Category::Policy => "policy",
            Category::Snapshot => "snapshot",
            Category::Sensor => "sensor",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddrInfo {
    pub rip: Option<String>,
    pub gva: Option<String>,
    pub gpa: Option<String>,
    pub qual: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViolationBits {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WxInfo {
    pub writer_rip: Option<String>,
    pub executor_rip: Option<String>,
    pub delta_ms: u64,
    pub page_size: Option<u64>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmuInfo {
    pub pid: i32,
    pub tid: i32,
    pub thread: String,
    pub cycles_delta: Option<u64>,
    pub instr_delta: Option<u64>,
    pub cache_ref_delta: Option<u64>,
    pub cache_miss_delta: Option<u64>,
    pub branch_delta: Option<u64>,
    pub branch_miss_delta: Option<u64>,
    pub sample_ms: u64,
    pub source: String,
    pub grouped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrapInfo {
    pub trap_id: String,
    pub trap_kind: String,
    pub backend: String,
    pub page: String,
    pub permissions_before: Option<ViolationBits>,
    pub permissions_after: Option<ViolationBits>,
    pub decision: String,
    pub invalidation_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionInfo {
    pub rule: Option<String>,
    pub kind: String,
    pub ok: bool,
    pub status: String,
    pub decision: String,
    pub result: String,
    pub detail: Option<String>,
    pub latency_ms: Option<u64>,
    pub target_vm_id: Option<String>,
    pub attempt: u32,
    pub max_attempts: u32,
    pub retry_count: u32,
    pub timeout_ms: u64,
    pub timed_out: bool,
    pub refused: bool,
    pub failure_class: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityConfidence {
    Low,
    Medium,
    High,
}

impl IdentityConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    pub fn meets(self, required: Self) -> bool {
        self.rank() >= required.rank()
    }

    fn rank(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
        }
    }
}

impl Default for IdentityConfidence {
    fn default() -> Self {
        Self::Low
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityInfo {
    pub sources: Vec<String>,
    pub confidence: IdentityConfidence,
    pub start_time_verified: bool,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LossInfo {
    pub dropped_since_last_event: u64,
    pub dropped_total: u64,
    pub reason: String,
    pub range_kind: LossRangeKind,
    pub sequence_gap_start: Option<u64>,
    pub sequence_gap_end: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LossRangeKind {
    AggregateCounter,
    SequenceGap,
    AggregateCounterAndSequenceGap,
}

impl LossRangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AggregateCounter => "aggregate_counter",
            Self::SequenceGap => "sequence_gap",
            Self::AggregateCounterAndSequenceGap => "aggregate_counter_and_sequence_gap",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub version: u32,
    pub schema_version: u32,
    pub ts: String,
    pub monotonic_ms: u128,
    pub sequence: u64,
    pub event_id: String,
    pub host_id: Option<String>,
    pub sensor_id: Option<String>,

    pub category: Category,
    pub severity: Severity,

    pub vm: String,
    pub vm_id: Option<String>,
    pub vm_name: Option<String>,
    pub tenant_id: Option<String>,
    pub raw_comm: Option<String>,
    pub host_pid: Option<i32>,
    pub host_tid: Option<i32>,
    pub host_start_time_ticks: Option<u64>,
    pub identity: Option<IdentityInfo>,

    /// Linux trace header CPU, e.g. `[001]`. This is not a guest vCPU id.
    pub host_cpu: Option<i32>,

    /// Guest vCPU id when the tracepoint exposes one. Kept separate from host_cpu.
    pub vcpu_id: Option<i32>,

    /// Backward-compatible alias for `vcpu_id`; never populated from trace header CPU.
    pub vcpu: Option<i32>,

    pub arch: Option<String>,
    pub cr3: Option<String>,
    pub asid: Option<String>,
    pub vmid: Option<String>,
    pub vpid: Option<String>,
    pub privilege_level: Option<String>,

    pub guest_os: Option<String>,
    pub guest_process: Option<String>,
    pub guest_thread: Option<String>,
    pub guest_module: Option<String>,
    pub guest_symbol: Option<String>,

    pub reason: Option<String>,
    pub trap_type: Option<String>,
    pub message: Option<String>,
    pub tags: Vec<String>,

    pub correlation_id: Option<String>,
    pub rule_id: Option<String>,
    pub decision: Option<String>,
    pub action_id: Option<String>,
    pub action_status: Option<String>,
    pub data_loss: bool,
    pub loss: Option<LossInfo>,

    pub addr: Option<AddrInfo>,
    pub violation: Option<ViolationBits>,
    pub page_permissions_before: Option<ViolationBits>,
    pub page_permissions_after: Option<ViolationBits>,
    pub wx: Option<WxInfo>,
    pub trap: Option<TrapInfo>,
    pub pmu: Option<PmuInfo>,
    pub action: Option<ActionInfo>,
}

impl Event {
    pub fn base(category: Category, severity: Severity, ts: String, vm: String) -> Self {
        let metadata = runtime_metadata();
        Self {
            version: 1,
            schema_version: 2,
            ts,
            monotonic_ms: monotonic_ms(),
            sequence: next_sequence(),
            event_id: next_event_id(),
            host_id: metadata.host_id.clone(),
            sensor_id: metadata.sensor_id.clone(),
            category,
            severity,
            vm,
            vm_id: None,
            vm_name: None,
            tenant_id: metadata.tenant_id.clone(),
            raw_comm: None,
            host_pid: None,
            host_tid: None,
            host_start_time_ticks: None,
            identity: None,
            host_cpu: None,
            vcpu_id: None,
            vcpu: None,
            arch: None,
            cr3: None,
            asid: None,
            vmid: None,
            vpid: None,
            privilege_level: None,
            guest_os: None,
            guest_process: None,
            guest_thread: None,
            guest_module: None,
            guest_symbol: None,
            reason: None,
            trap_type: None,
            message: None,
            tags: Vec::new(),
            correlation_id: None,
            rule_id: None,
            decision: None,
            action_id: None,
            action_status: None,
            data_loss: false,
            loss: None,
            addr: None,
            violation: None,
            page_permissions_before: None,
            page_permissions_after: None,
            wx: None,
            trap: None,
            pmu: None,
            action: None,
        }
    }

    pub fn with_loss(&mut self, dropped_since_last_event: u64, dropped_total: u64, reason: &str) {
        self.with_loss_report(dropped_since_last_event, dropped_total, reason, None);
    }

    pub fn with_loss_report(
        &mut self,
        dropped_since_last_event: u64,
        dropped_total: u64,
        reason: &str,
        sequence_gap: Option<(u64, u64)>,
    ) {
        if dropped_since_last_event > 0 || sequence_gap.is_some() {
            let range_kind = match (dropped_since_last_event > 0, sequence_gap.is_some()) {
                (true, true) => LossRangeKind::AggregateCounterAndSequenceGap,
                (true, false) => LossRangeKind::AggregateCounter,
                (false, true) => LossRangeKind::SequenceGap,
                (false, false) => LossRangeKind::AggregateCounter,
            };
            self.data_loss = true;
            self.loss = Some(LossInfo {
                dropped_since_last_event,
                dropped_total,
                reason: reason.to_string(),
                range_kind,
                sequence_gap_start: sequence_gap.map(|(start, _)| start),
                sequence_gap_end: sequence_gap.map(|(_, end)| end),
            });
        }
    }

    pub fn to_json(&self) -> String {
        let tags = self
            .tags
            .iter()
            .map(|t| json_str(t))
            .collect::<Vec<_>>()
            .join(",");
        let mut fields = Vec::with_capacity(73);
        fields.push(format!("\"version\":{}", self.version));
        fields.push(format!("\"schema_version\":{}", self.schema_version));
        fields.push(format!("\"ts\":{}", json_str(&self.ts)));
        fields.push(format!("\"monotonic_ms\":{}", self.monotonic_ms));
        fields.push(format!("\"sequence\":{}", self.sequence));
        fields.push(format!("\"event_id\":{}", json_str(&self.event_id)));
        fields.push(format!("\"host_id\":{}", json_opt_string(&self.host_id)));
        fields.push(format!(
            "\"sensor_id\":{}",
            json_opt_string(&self.sensor_id)
        ));
        fields.push(format!("\"category\":{}", json_str(self.category.as_str())));
        fields.push(format!("\"severity\":{}", json_str(self.severity.as_str())));
        fields.push(format!("\"vm\":{}", json_str(&self.vm)));
        fields.push(format!("\"vm_id\":{}", json_opt_string(&self.vm_id)));
        fields.push(format!("\"vm_name\":{}", json_opt_string(&self.vm_name)));
        fields.push(format!(
            "\"tenant_id\":{}",
            json_opt_string(&self.tenant_id)
        ));
        fields.push(format!("\"raw_comm\":{}", json_opt_string(&self.raw_comm)));
        fields.push(format!("\"host_pid\":{}", json_opt_i32(self.host_pid)));
        fields.push(format!("\"host_tid\":{}", json_opt_i32(self.host_tid)));
        fields.push(format!(
            "\"host_start_time_ticks\":{}",
            json_opt_u64(self.host_start_time_ticks)
        ));
        fields.push(format!("\"identity\":{}", identity_json(&self.identity)));
        fields.push(format!("\"host_cpu\":{}", json_opt_i32(self.host_cpu)));
        fields.push(format!("\"vcpu_id\":{}", json_opt_i32(self.vcpu_id)));
        fields.push(format!("\"vcpu\":{}", json_opt_i32(self.vcpu)));
        fields.push(format!("\"arch\":{}", json_opt_string(&self.arch)));
        fields.push(format!("\"cr3\":{}", json_opt_string(&self.cr3)));
        fields.push(format!("\"asid\":{}", json_opt_string(&self.asid)));
        fields.push(format!("\"vmid\":{}", json_opt_string(&self.vmid)));
        fields.push(format!("\"vpid\":{}", json_opt_string(&self.vpid)));
        fields.push(format!(
            "\"privilege_level\":{}",
            json_opt_string(&self.privilege_level)
        ));
        fields.push(format!("\"guest_os\":{}", json_opt_string(&self.guest_os)));
        fields.push(format!(
            "\"guest_process\":{}",
            json_opt_string(&self.guest_process)
        ));
        fields.push(format!(
            "\"guest_thread\":{}",
            json_opt_string(&self.guest_thread)
        ));
        fields.push(format!(
            "\"guest_module\":{}",
            json_opt_string(&self.guest_module)
        ));
        fields.push(format!(
            "\"guest_symbol\":{}",
            json_opt_string(&self.guest_symbol)
        ));
        fields.push(format!("\"reason\":{}", json_opt_string(&self.reason)));
        fields.push(format!(
            "\"trap_type\":{}",
            json_opt_string(&self.trap_type)
        ));
        fields.push(format!("\"message\":{}", json_opt_string(&self.message)));
        fields.push(format!("\"tags\":[{}]", tags));
        fields.push(format!(
            "\"correlation_id\":{}",
            json_opt_string(&self.correlation_id)
        ));
        fields.push(format!("\"rule_id\":{}", json_opt_string(&self.rule_id)));
        fields.push(format!("\"decision\":{}", json_opt_string(&self.decision)));
        fields.push(format!(
            "\"action_id\":{}",
            json_opt_string(&self.action_id)
        ));
        fields.push(format!(
            "\"action_status\":{}",
            json_opt_string(&self.action_status)
        ));
        fields.push(format!("\"data_loss\":{}", self.data_loss));
        fields.push(format!("\"loss\":{}", loss_json(&self.loss)));
        fields.push(format!("\"addr\":{}", addr_json(&self.addr)));
        fields.push(format!("\"violation\":{}", bits_json(&self.violation)));
        fields.push(format!(
            "\"page_permissions_before\":{}",
            bits_json(&self.page_permissions_before)
        ));
        fields.push(format!(
            "\"page_permissions_after\":{}",
            bits_json(&self.page_permissions_after)
        ));
        fields.push(format!("\"wx\":{}", wx_json(&self.wx)));
        fields.push(format!("\"trap\":{}", trap_json(&self.trap)));
        fields.push(format!("\"pmu\":{}", pmu_json(&self.pmu)));
        fields.push(format!("\"action\":{}", action_json(&self.action)));
        format!("{{{}}}", fields.join(","))
    }
}

fn identity_json(v: &Option<IdentityInfo>) -> String {
    match v {
        Some(identity) => {
            let sources = identity
                .sources
                .iter()
                .map(|source| json_str(source))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"sources\":[{}],\"confidence\":{},\"start_time_verified\":{},\"ambiguous\":{}}}",
                sources,
                json_str(identity.confidence.as_str()),
                identity.start_time_verified,
                identity.ambiguous
            )
        }
        None => "null".to_string(),
    }
}

fn addr_json(v: &Option<AddrInfo>) -> String {
    match v {
        Some(a) => format!(
            "{{\"rip\":{},\"gva\":{},\"gpa\":{},\"qual\":{}}}",
            json_opt_string(&a.rip),
            json_opt_string(&a.gva),
            json_opt_string(&a.gpa),
            json_opt_string(&a.qual)
        ),
        None => "null".to_string(),
    }
}

fn bits_json(v: &Option<ViolationBits>) -> String {
    match v {
        Some(b) => format!(
            "{{\"read\":{},\"write\":{},\"exec\":{}}}",
            b.read, b.write, b.exec
        ),
        None => "null".to_string(),
    }
}

fn wx_json(v: &Option<WxInfo>) -> String {
    match v {
        Some(w) => format!(
            "{{\"writer_rip\":{},\"executor_rip\":{},\"delta_ms\":{},\"page_size\":{},\"confidence\":{}}}",
            json_opt_string(&w.writer_rip),
            json_opt_string(&w.executor_rip),
            w.delta_ms,
            json_opt_u64(w.page_size),
            json_opt_f64(Some(w.confidence))
        ),
        None => "null".to_string(),
    }
}

fn trap_json(v: &Option<TrapInfo>) -> String {
    match v {
        Some(t) => format!(
            "{{\"trap_id\":{},\"trap_kind\":{},\"backend\":{},\"page\":{},\"permissions_before\":{},\"permissions_after\":{},\"decision\":{},\"invalidation_status\":{}}}",
            json_str(&t.trap_id),
            json_str(&t.trap_kind),
            json_str(&t.backend),
            json_str(&t.page),
            bits_json(&t.permissions_before),
            bits_json(&t.permissions_after),
            json_str(&t.decision),
            json_str(&t.invalidation_status)
        ),
        None => "null".to_string(),
    }
}

fn pmu_json(v: &Option<PmuInfo>) -> String {
    match v {
        Some(p) => format!(
            "{{\"pid\":{},\"tid\":{},\"thread\":{},\"cycles_delta\":{},\"instr_delta\":{},\"cache_ref_delta\":{},\"cache_miss_delta\":{},\"branch_delta\":{},\"branch_miss_delta\":{},\"sample_ms\":{},\"source\":{},\"grouped\":{}}}",
            p.pid,
            p.tid,
            json_str(&p.thread),
            json_opt_u64(p.cycles_delta),
            json_opt_u64(p.instr_delta),
            json_opt_u64(p.cache_ref_delta),
            json_opt_u64(p.cache_miss_delta),
            json_opt_u64(p.branch_delta),
            json_opt_u64(p.branch_miss_delta),
            p.sample_ms,
            json_str(&p.source),
            p.grouped
        ),
        None => "null".to_string(),
    }
}

fn action_json(v: &Option<ActionInfo>) -> String {
    match v {
        Some(a) => format!(
            "{{\"rule\":{},\"kind\":{},\"ok\":{},\"status\":{},\"decision\":{},\"result\":{},\"detail\":{},\"latency_ms\":{},\"target_vm_id\":{},\"attempt\":{},\"max_attempts\":{},\"retry_count\":{},\"timeout_ms\":{},\"timed_out\":{},\"refused\":{},\"failure_class\":{}}}",
            json_opt_string(&a.rule),
            json_str(&a.kind),
            a.ok,
            json_str(&a.status),
            json_str(&a.decision),
            json_str(&a.result),
            json_opt_string(&a.detail),
            json_opt_u64(a.latency_ms),
            json_opt_string(&a.target_vm_id),
            a.attempt,
            a.max_attempts,
            a.retry_count,
            a.timeout_ms,
            a.timed_out,
            a.refused,
            json_opt_string(&a.failure_class)
        ),
        None => "null".to_string(),
    }
}

fn loss_json(v: &Option<LossInfo>) -> String {
    match v {
        Some(l) => format!(
            "{{\"dropped_since_last_event\":{},\"dropped_total\":{},\"reason\":{},\"range_kind\":{},\"sequence_gap_start\":{},\"sequence_gap_end\":{}}}",
            l.dropped_since_last_event,
            l.dropped_total,
            json_str(&l.reason),
            json_str(l.range_kind.as_str()),
            json_opt_u64(l.sequence_gap_start),
            json_opt_u64(l.sequence_gap_end)
        ),
        None => "null".to_string(),
    }
}

pub fn severity_from_str(s: &str) -> Option<Severity> {
    match s.to_ascii_lowercase().as_str() {
        "info" => Some(Severity::Info),
        "low" => Some(Severity::Low),
        "medium" => Some(Severity::Medium),
        "high" => Some(Severity::High),
        "critical" => Some(Severity::Critical),
        _ => None,
    }
}

pub fn category_from_str(s: &str) -> Option<Category> {
    match s.to_ascii_lowercase().as_str() {
        "exit" => Some(Category::Exit),
        "wx" => Some(Category::Wx),
        "pmu" => Some(Category::Pmu),
        "policy" => Some(Category::Policy),
        "snapshot" => Some(Category::Snapshot),
        "sensor" => Some(Category::Sensor),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_json_contains_required_ids() {
        let ev = Event::base(
            Category::Sensor,
            Severity::Info,
            "2026-01-01T00:00:00Z".to_string(),
            "host".to_string(),
        );
        let j = ev.to_json();
        assert!(j.contains("\"event_id\""));
        assert!(j.contains("\"ts\":\"2026-01-01T00:00:00Z\""));
        assert!(j.contains("\"monotonic_ms\""));
        assert!(j.contains("\"sequence\""));
        assert!(j.contains("\"schema_version\":2"));
    }

    #[test]
    fn action_json_contains_structured_audit_fields() {
        let mut ev = Event::base(
            Category::Policy,
            Severity::High,
            "2026-01-01T00:00:00Z".to_string(),
            "vm-a".to_string(),
        );
        ev.action = Some(ActionInfo {
            rule: Some("rule-1".to_string()),
            kind: "pause_vm".to_string(),
            ok: false,
            status: "timeout".to_string(),
            decision: "executed".to_string(),
            result: "timeout".to_string(),
            detail: Some("QMP did not respond for stop".to_string()),
            latency_ms: Some(100),
            target_vm_id: Some("libvirt:111".to_string()),
            attempt: 2,
            max_attempts: 3,
            retry_count: 1,
            timeout_ms: 100,
            timed_out: true,
            refused: false,
            failure_class: Some("timeout".to_string()),
        });

        let json = ev.to_json();

        assert!(json.contains("\"decision\":\"executed\""));
        assert!(json.contains("\"result\":\"timeout\""));
        assert!(json.contains("\"attempt\":2"));
        assert!(json.contains("\"max_attempts\":3"));
        assert!(json.contains("\"retry_count\":1"));
        assert!(json.contains("\"timeout_ms\":100"));
        assert!(json.contains("\"timed_out\":true"));
        assert!(json.contains("\"refused\":false"));
        assert!(json.contains("\"failure_class\":\"timeout\""));
    }

    #[test]
    fn event_json_contains_identity_source_metadata() {
        let mut ev = Event::base(
            Category::Exit,
            Severity::Info,
            "2026-01-01T00:00:00Z".to_string(),
            "qemu".to_string(),
        );
        ev.identity = Some(IdentityInfo {
            sources: vec!["trace_comm".to_string(), "fallback_pid".to_string()],
            confidence: IdentityConfidence::Low,
            start_time_verified: false,
            ambiguous: false,
        });

        let json = ev.to_json();

        assert!(json.contains("\"identity\":{"));
        assert!(json.contains("\"sources\":[\"trace_comm\",\"fallback_pid\"]"));
        assert!(json.contains("\"confidence\":\"low\""));
        assert!(json.contains("\"start_time_verified\":false"));
        assert!(json.contains("\"ambiguous\":false"));
    }

    #[test]
    fn loss_json_reports_aggregate_counter_without_inventing_sequence_range() {
        let mut ev = Event::base(
            Category::Sensor,
            Severity::High,
            "2026-01-01T00:00:00Z".to_string(),
            "host".to_string(),
        );

        ev.with_loss(3, 7, "queue_full_or_output_backpressure");

        let json = ev.to_json();
        assert!(json.contains("\"data_loss\":true"));
        assert!(json.contains("\"dropped_since_last_event\":3"));
        assert!(json.contains("\"dropped_total\":7"));
        assert!(json.contains("\"range_kind\":\"aggregate_counter\""));
        assert!(json.contains("\"sequence_gap_start\":null"));
        assert!(json.contains("\"sequence_gap_end\":null"));
    }

    #[test]
    fn loss_json_reports_known_sequence_gap_range() {
        let mut ev = Event::base(
            Category::Sensor,
            Severity::High,
            "2026-01-01T00:00:00Z".to_string(),
            "host".to_string(),
        );

        ev.with_loss_report(0, 0, "sequence_gap", Some((10, 12)));

        let json = ev.to_json();
        assert!(json.contains("\"data_loss\":true"));
        assert!(json.contains("\"range_kind\":\"sequence_gap\""));
        assert!(json.contains("\"sequence_gap_start\":10"));
        assert!(json.contains("\"sequence_gap_end\":12"));
    }
}
