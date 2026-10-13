use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::time::{Duration, Instant};

use crate::event::{IdentityConfidence, Severity};
use crate::vmi::VmiErrorKind;

pub mod dedupe;
pub mod hidden_module;
pub mod hidden_process;
pub mod jit;
pub mod kernel_text;
pub mod memory;
pub mod pmu_anomaly;
pub mod state;
pub mod syscall_hooks;
pub mod wx;

pub const DETECTION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DetectionKind {
    KernelTextTamper,
    SyscallHook,
    HiddenProcess,
    HiddenModule,
    ExecutableAnonymousMemory,
    RwxMapping,
    WxCorrelation,
    PmuAnomaly,
}

impl DetectionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::KernelTextTamper => "kernel_text_tamper",
            Self::SyscallHook => "syscall_hook",
            Self::HiddenProcess => "hidden_process",
            Self::HiddenModule => "hidden_module",
            Self::ExecutableAnonymousMemory => "executable_anonymous_memory",
            Self::RwxMapping => "rwx_mapping",
            Self::WxCorrelation => "wx_correlation",
            Self::PmuAnomaly => "pmu_anomaly",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "kernel_text_tamper" => Self::KernelTextTamper,
            "syscall_hook" => Self::SyscallHook,
            "hidden_process" => Self::HiddenProcess,
            "hidden_module" => Self::HiddenModule,
            "executable_anonymous_memory" => Self::ExecutableAnonymousMemory,
            "rwx_mapping" => Self::RwxMapping,
            "wx_correlation" => Self::WxCorrelation,
            "pmu_anomaly" => Self::PmuAnomaly,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceReliability {
    Unsupported,
    SyntheticFixture,
    Tracefs,
    OfflineSnapshot,
    VerifiedSnapshot,
}

impl SourceReliability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::SyntheticFixture => "synthetic_fixture",
            Self::Tracefs => "tracefs",
            Self::OfflineSnapshot => "offline_snapshot",
            Self::VerifiedSnapshot => "verified_snapshot",
        }
    }

    fn score(self) -> i32 {
        match self {
            Self::Unsupported => 0,
            Self::SyntheticFixture => 15,
            Self::Tracefs => 45,
            Self::OfflineSnapshot => 60,
            Self::VerifiedSnapshot => 80,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttributionQuality {
    Unknown,
    HostOnly,
    GuestAddress,
    GuestProcess,
    GuestSymbol,
}

impl AttributionQuality {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::HostOnly => "host_only",
            Self::GuestAddress => "guest_address",
            Self::GuestProcess => "guest_process",
            Self::GuestSymbol => "guest_symbol",
        }
    }

    fn score(self) -> i32 {
        match self {
            Self::Unknown => 0,
            Self::HostOnly => 15,
            Self::GuestAddress => 35,
            Self::GuestProcess => 55,
            Self::GuestSymbol => 70,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileConfidence {
    None,
    Synthetic,
    ExactBuild,
    VerifiedSnapshot,
}

impl ProfileConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Synthetic => "synthetic",
            Self::ExactBuild => "exact_build",
            Self::VerifiedSnapshot => "verified_snapshot",
        }
    }

    fn score(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Synthetic => 20,
            Self::ExactBuild => 55,
            Self::VerifiedSnapshot => 75,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfidenceLevel {
    Low,
    Medium,
    High,
}

impl ConfidenceLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DetectionConfidence {
    pub score: u8,
    pub level: ConfidenceLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreFactors {
    pub base_severity: Severity,
    pub source: SourceReliability,
    pub attribution: AttributionQuality,
    pub profile: ProfileConfidence,
    pub identity: IdentityConfidence,
    pub data_loss: bool,
    pub policy_match: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectionScore {
    pub severity: Severity,
    pub confidence: DetectionConfidence,
}

pub fn score_detection(factors: ScoreFactors) -> DetectionScore {
    let mut raw =
        10 + factors.source.score() + factors.attribution.score() + factors.profile.score();
    raw += match factors.identity {
        IdentityConfidence::Low => 0,
        IdentityConfidence::Medium => 10,
        IdentityConfidence::High => 20,
    };
    if factors.policy_match {
        raw += 10;
    }
    if factors.data_loss {
        raw -= 25;
    }
    let score = raw.clamp(0, 100) as u8;
    let level = if score >= 75 {
        ConfidenceLevel::High
    } else if score >= 40 {
        ConfidenceLevel::Medium
    } else {
        ConfidenceLevel::Low
    };
    let severity = adjust_severity(factors.base_severity, factors.data_loss, level);
    DetectionScore {
        severity,
        confidence: DetectionConfidence { score, level },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionSource {
    pub name: String,
    pub reliability: SourceReliability,
    pub profile: ProfileConfidence,
}

impl DetectionSource {
    pub fn new(
        name: impl Into<String>,
        reliability: SourceReliability,
        profile: ProfileConfidence,
    ) -> Self {
        Self {
            name: name.into(),
            reliability,
            profile,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionRecord {
    pub schema_version: u32,
    pub detector_id: String,
    pub kind: DetectionKind,
    pub title: String,
    pub detail: String,
    pub vm_id: Option<String>,
    pub entity: Option<String>,
    pub range_start: Option<u64>,
    pub range_end: Option<u64>,
    pub symbol: Option<String>,
    pub source: DetectionSource,
    pub severity: Severity,
    pub confidence: DetectionConfidence,
    pub tags: Vec<String>,
}

impl DetectionRecord {
    pub fn new(
        detector_id: impl Into<String>,
        kind: DetectionKind,
        title: impl Into<String>,
        detail: impl Into<String>,
        source: DetectionSource,
        score: DetectionScore,
    ) -> Self {
        Self {
            schema_version: DETECTION_SCHEMA_VERSION,
            detector_id: detector_id.into(),
            kind,
            title: title.into(),
            detail: detail.into(),
            vm_id: None,
            entity: None,
            range_start: None,
            range_end: None,
            symbol: None,
            source,
            severity: score.severity,
            confidence: score.confidence,
            tags: Vec::new(),
        }
    }

    pub fn with_vm_id(mut self, vm_id: impl Into<String>) -> Self {
        self.vm_id = Some(vm_id.into());
        self
    }

    pub fn with_entity(mut self, entity: impl Into<String>) -> Self {
        self.entity = Some(entity.into());
        self
    }

    pub fn with_range(mut self, start: u64, end: u64) -> Self {
        self.range_start = Some(start);
        self.range_end = Some(end);
        self
    }

    pub fn with_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.symbol = Some(symbol.into());
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectorError {
    MalformedInput { detail: String },
    Unsupported { detector: String, detail: String },
    Degraded { detector: String, detail: String },
}

impl DetectorError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::MalformedInput { .. } => VmiErrorKind::Malformed,
            Self::Unsupported { .. } => VmiErrorKind::Unsupported,
            Self::Degraded { .. } => VmiErrorKind::TemporarilyUnavailable,
        }
    }
}

impl fmt::Display for DetectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedInput { detail } => write!(f, "detector input is malformed: {detail}"),
            Self::Unsupported { detector, detail } => {
                write!(f, "detector '{detector}' is unsupported: {detail}")
            }
            Self::Degraded { detector, detail } => {
                write!(f, "detector '{detector}' is degraded: {detail}")
            }
        }
    }
}

impl Error for DetectorError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectorOutcome {
    Clean,
    Findings(Vec<DetectionRecord>),
    Unsupported {
        reason: String,
    },
    Degraded {
        reason: String,
        findings: Vec<DetectionRecord>,
    },
}

impl DetectorOutcome {
    pub fn finding_count(&self) -> usize {
        match self {
            Self::Clean | Self::Unsupported { .. } => 0,
            Self::Findings(records) => records.len(),
            Self::Degraded { findings, .. } => findings.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectorInput {
    pub vm_id: Option<String>,
    pub data_loss: bool,
    pub attributes: BTreeMap<String, String>,
}

impl DetectorInput {
    pub fn new(vm_id: Option<String>) -> Self {
        Self {
            vm_id,
            data_loss: false,
            attributes: BTreeMap::new(),
        }
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }
}

pub trait Detector {
    fn id(&self) -> &'static str;
    fn run(&self, input: &DetectorInput) -> Result<DetectorOutcome, DetectorError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectorBudget {
    pub max_runtime: Duration,
    pub max_findings: usize,
}

impl DetectorBudget {
    pub fn new(max_runtime: Duration, max_findings: usize) -> Result<Self, DetectorError> {
        if max_runtime.is_zero() {
            return Err(DetectorError::MalformedInput {
                detail: "detector budget max_runtime must be non-zero".to_string(),
            });
        }
        if max_findings == 0 {
            return Err(DetectorError::MalformedInput {
                detail: "detector budget max_findings must be non-zero".to_string(),
            });
        }
        Ok(Self {
            max_runtime,
            max_findings,
        })
    }
}

impl Default for DetectorBudget {
    fn default() -> Self {
        Self {
            max_runtime: Duration::from_millis(50),
            max_findings: 128,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectorRunConfig {
    pub enabled: bool,
    pub budget: DetectorBudget,
}

impl Default for DetectorRunConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            budget: DetectorBudget::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectorRunStatus {
    Disabled,
    Clean,
    Findings,
    Unsupported,
    Degraded,
    OverBudget,
    Failed,
}

impl DetectorRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Clean => "clean",
            Self::Findings => "findings",
            Self::Unsupported => "unsupported",
            Self::Degraded => "degraded",
            Self::OverBudget => "over_budget",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectorRunSummary {
    pub detector_id: String,
    pub status: DetectorRunStatus,
    pub elapsed: Duration,
    pub finding_count: usize,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetectorBudgetMetrics {
    pub over_budget_runs: u64,
    pub truncated_findings: u64,
    pub unsupported_runs: u64,
    pub degraded_runs: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetectorSchedulerReport {
    pub detections: Vec<DetectionRecord>,
    pub runs: Vec<DetectorRunSummary>,
    pub budget_metrics: DetectorBudgetMetrics,
}

pub struct DetectorScheduler {
    detectors: Vec<ScheduledDetector>,
}

struct ScheduledDetector {
    detector: Box<dyn Detector>,
    config: DetectorRunConfig,
}

impl DetectorScheduler {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
        }
    }

    pub fn add_detector<D>(&mut self, detector: D, config: DetectorRunConfig)
    where
        D: Detector + 'static,
    {
        self.detectors.push(ScheduledDetector {
            detector: Box::new(detector),
            config,
        });
    }

    pub fn run(&self, input: &DetectorInput) -> DetectorSchedulerReport {
        let mut report = DetectorSchedulerReport::default();
        for scheduled in &self.detectors {
            let id = scheduled.detector.id().to_string();
            if !scheduled.config.enabled {
                report.runs.push(DetectorRunSummary {
                    detector_id: id,
                    status: DetectorRunStatus::Disabled,
                    elapsed: Duration::ZERO,
                    finding_count: 0,
                    detail: None,
                });
                continue;
            }

            let started = Instant::now();
            let outcome = scheduled.detector.run(input);
            let elapsed = started.elapsed();
            match outcome {
                Ok(outcome) => {
                    let mut status = match &outcome {
                        DetectorOutcome::Clean => DetectorRunStatus::Clean,
                        DetectorOutcome::Findings(_) => DetectorRunStatus::Findings,
                        DetectorOutcome::Unsupported { .. } => DetectorRunStatus::Unsupported,
                        DetectorOutcome::Degraded { .. } => DetectorRunStatus::Degraded,
                    };
                    let detail = match &outcome {
                        DetectorOutcome::Unsupported { reason }
                        | DetectorOutcome::Degraded { reason, .. } => Some(reason.clone()),
                        DetectorOutcome::Clean | DetectorOutcome::Findings(_) => None,
                    };
                    let mut findings = collect_findings(outcome);
                    let original_count = findings.len();
                    if findings.len() > scheduled.config.budget.max_findings {
                        findings.truncate(scheduled.config.budget.max_findings);
                        report.budget_metrics.truncated_findings +=
                            (original_count - findings.len()) as u64;
                        status = DetectorRunStatus::OverBudget;
                    }
                    if elapsed > scheduled.config.budget.max_runtime {
                        report.budget_metrics.over_budget_runs += 1;
                        status = DetectorRunStatus::OverBudget;
                    }
                    if status == DetectorRunStatus::Unsupported {
                        report.budget_metrics.unsupported_runs += 1;
                    }
                    if status == DetectorRunStatus::Degraded {
                        report.budget_metrics.degraded_runs += 1;
                    }
                    let finding_count = findings.len();
                    report.detections.extend(findings);
                    report.runs.push(DetectorRunSummary {
                        detector_id: id,
                        status,
                        elapsed,
                        finding_count,
                        detail,
                    });
                }
                Err(err) => {
                    report.runs.push(DetectorRunSummary {
                        detector_id: id,
                        status: DetectorRunStatus::Failed,
                        elapsed,
                        finding_count: 0,
                        detail: Some(err.to_string()),
                    });
                }
            }
        }
        report
    }
}

impl Default for DetectorScheduler {
    fn default() -> Self {
        Self::new()
    }
}

fn collect_findings(outcome: DetectorOutcome) -> Vec<DetectionRecord> {
    match outcome {
        DetectorOutcome::Clean | DetectorOutcome::Unsupported { .. } => Vec::new(),
        DetectorOutcome::Findings(records) => records,
        DetectorOutcome::Degraded { findings, .. } => findings,
    }
}

fn adjust_severity(base: Severity, data_loss: bool, confidence: ConfidenceLevel) -> Severity {
    if !data_loss && confidence != ConfidenceLevel::Low {
        return base;
    }
    match base {
        Severity::Critical => Severity::High,
        Severity::High => Severity::Medium,
        Severity::Medium => Severity::Low,
        Severity::Low | Severity::Info => base,
    }
}
