use std::time::Duration;

use aegishv::detectors::{
    score_detection, AttributionQuality, ConfidenceLevel, DetectionKind, DetectionRecord,
    DetectionSource, Detector, DetectorBudget, DetectorInput, DetectorOutcome, DetectorRunConfig,
    DetectorRunStatus, DetectorScheduler, ProfileConfidence, ScoreFactors, SourceReliability,
};
use aegishv::event::{IdentityConfidence, Severity};

fn source() -> DetectionSource {
    DetectionSource::new(
        "offline-test",
        SourceReliability::VerifiedSnapshot,
        ProfileConfidence::VerifiedSnapshot,
    )
}

fn record(id: &str) -> DetectionRecord {
    let score = score_detection(ScoreFactors {
        base_severity: Severity::High,
        source: SourceReliability::VerifiedSnapshot,
        attribution: AttributionQuality::GuestSymbol,
        profile: ProfileConfidence::VerifiedSnapshot,
        identity: IdentityConfidence::High,
        data_loss: false,
        policy_match: true,
    });
    DetectionRecord::new(
        id,
        DetectionKind::KernelTextTamper,
        "kernel text drift",
        "hash mismatch",
        source(),
        score,
    )
    .with_vm_id("vm-a")
}

#[derive(Clone)]
struct StaticDetector {
    id: &'static str,
    outcome: DetectorOutcome,
}

impl Detector for StaticDetector {
    fn id(&self) -> &'static str {
        self.id
    }

    fn run(
        &self,
        _input: &DetectorInput,
    ) -> Result<DetectorOutcome, aegishv::detectors::DetectorError> {
        Ok(self.outcome.clone())
    }
}

#[test]
fn scoring_uses_source_attribution_profile_identity_and_policy_signal() {
    let high = score_detection(ScoreFactors {
        base_severity: Severity::Critical,
        source: SourceReliability::Tracefs,
        attribution: AttributionQuality::GuestAddress,
        profile: ProfileConfidence::None,
        identity: IdentityConfidence::Low,
        data_loss: false,
        policy_match: false,
    });
    let lossy = score_detection(ScoreFactors {
        data_loss: true,
        ..ScoreFactors {
            base_severity: Severity::Critical,
            source: SourceReliability::Tracefs,
            attribution: AttributionQuality::GuestAddress,
            profile: ProfileConfidence::None,
            identity: IdentityConfidence::Low,
            data_loss: false,
            policy_match: false,
        }
    });

    assert_eq!(high.severity, Severity::Critical);
    assert_eq!(high.confidence.level, ConfidenceLevel::High);
    assert_eq!(lossy.severity, Severity::High);
    assert!(lossy.confidence.score < high.confidence.score);
}

#[test]
fn scheduler_runs_enabled_detectors_and_keeps_disabled_detectors_out_of_hot_path() {
    let mut scheduler = DetectorScheduler::new();
    scheduler.add_detector(
        StaticDetector {
            id: "enabled",
            outcome: DetectorOutcome::Findings(vec![record("enabled")]),
        },
        DetectorRunConfig::default(),
    );
    scheduler.add_detector(
        StaticDetector {
            id: "disabled",
            outcome: DetectorOutcome::Findings(vec![record("disabled")]),
        },
        DetectorRunConfig {
            enabled: false,
            ..DetectorRunConfig::default()
        },
    );

    let report = scheduler.run(&DetectorInput::new(Some("vm-a".to_string())));

    assert_eq!(report.detections.len(), 1);
    assert_eq!(report.runs[0].status, DetectorRunStatus::Findings);
    assert_eq!(report.runs[1].status, DetectorRunStatus::Disabled);
}

#[test]
fn scheduler_records_unsupported_and_degraded_paths_explicitly() {
    let mut scheduler = DetectorScheduler::new();
    scheduler.add_detector(
        StaticDetector {
            id: "unsupported",
            outcome: DetectorOutcome::Unsupported {
                reason: "missing VMI backend".to_string(),
            },
        },
        DetectorRunConfig::default(),
    );
    scheduler.add_detector(
        StaticDetector {
            id: "degraded",
            outcome: DetectorOutcome::Degraded {
                reason: "profile is partial".to_string(),
                findings: vec![record("degraded")],
            },
        },
        DetectorRunConfig::default(),
    );

    let report = scheduler.run(&DetectorInput::new(Some("vm-a".to_string())));

    assert_eq!(report.runs[0].status, DetectorRunStatus::Unsupported);
    assert_eq!(report.runs[1].status, DetectorRunStatus::Degraded);
    assert_eq!(report.budget_metrics.unsupported_runs, 1);
    assert_eq!(report.budget_metrics.degraded_runs, 1);
    assert_eq!(report.detections.len(), 1);
}

#[test]
fn scheduler_marks_over_budget_and_truncates_findings() {
    let mut scheduler = DetectorScheduler::new();
    scheduler.add_detector(
        StaticDetector {
            id: "noisy",
            outcome: DetectorOutcome::Findings(vec![record("a"), record("b"), record("c")]),
        },
        DetectorRunConfig {
            enabled: true,
            budget: DetectorBudget::new(Duration::from_nanos(1), 2).expect("budget"),
        },
    );

    let report = scheduler.run(&DetectorInput::new(Some("vm-a".to_string())));

    assert_eq!(report.runs[0].status, DetectorRunStatus::OverBudget);
    assert_eq!(report.detections.len(), 2);
    assert_eq!(report.budget_metrics.truncated_findings, 1);
    assert_eq!(report.budget_metrics.over_budget_runs, 1);
}

#[test]
fn detector_budget_rejects_zero_values() {
    let err = DetectorBudget::new(Duration::ZERO, 1).expect_err("zero runtime must fail");
    assert!(err.to_string().contains("max_runtime"));

    let err = DetectorBudget::new(Duration::from_millis(1), 0).expect_err("zero findings");
    assert!(err.to_string().contains("max_findings"));
}
