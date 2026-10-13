use crate::detectors::{
    score_detection, AttributionQuality, DetectionKind, DetectionRecord, DetectionScore,
    DetectionSource, DetectorError, ProfileConfidence, ScoreFactors, SourceReliability,
};
use crate::event::{IdentityConfidence, Severity};
use crate::linux_integrity::{LinuxIntegrityReport, LinuxTextHashStatus};
use crate::windows_integrity::{WindowsIntegrityReport, WindowsTextHashStatus};

const DETECTOR_ID: &str = "kernel_text_tamper";

struct TextFinding<'a> {
    os: &'a str,
    owner: &'a str,
    start: u64,
    end: u64,
    detail: String,
    base_severity: Severity,
    vm_id: Option<&'a str>,
}

pub fn linux_kernel_text_findings(
    report: &LinuxIntegrityReport,
    source: DetectionSource,
    vm_id: Option<&str>,
) -> Result<Vec<DetectionRecord>, DetectorError> {
    if report.results.is_empty() {
        return Err(DetectorError::Unsupported {
            detector: DETECTOR_ID.to_string(),
            detail: "Linux kernel text report has no hash results".to_string(),
        });
    }
    let mut out = Vec::new();
    for result in &report.results {
        if result.status == LinuxTextHashStatus::Match {
            continue;
        }
        let base = match result.status {
            LinuxTextHashStatus::Mismatch => Severity::High,
            LinuxTextHashStatus::UnknownBaseline => Severity::Medium,
            LinuxTextHashStatus::Match => Severity::Info,
        };
        let detail = format!(
            "linux text range '{}' hash status is {}",
            result.owner,
            result.status.as_str()
        );
        out.push(text_record(
            TextFinding {
                os: "linux",
                owner: &result.owner,
                start: result.start,
                end: result.end,
                detail,
                base_severity: base,
                vm_id,
            },
            source.clone(),
        ));
    }
    Ok(out)
}

pub fn windows_kernel_text_findings(
    report: &WindowsIntegrityReport,
    source: DetectionSource,
    vm_id: Option<&str>,
) -> Result<Vec<DetectionRecord>, DetectorError> {
    if report.results.is_empty() {
        return Err(DetectorError::Unsupported {
            detector: DETECTOR_ID.to_string(),
            detail: "Windows kernel text report has no hash results".to_string(),
        });
    }
    let mut out = Vec::new();
    for result in &report.results {
        if result.status == WindowsTextHashStatus::Match {
            continue;
        }
        let base = match result.status {
            WindowsTextHashStatus::Mismatch => Severity::High,
            WindowsTextHashStatus::UnknownBaseline => Severity::Medium,
            WindowsTextHashStatus::Match => Severity::Info,
        };
        let detail = format!(
            "windows text range '{}' hash status is {}",
            result.owner,
            result.status.as_str()
        );
        out.push(text_record(
            TextFinding {
                os: "windows",
                owner: &result.owner,
                start: result.start,
                end: result.end,
                detail,
                base_severity: base,
                vm_id,
            },
            source.clone(),
        ));
    }
    Ok(out)
}

fn text_record(finding: TextFinding<'_>, source: DetectionSource) -> DetectionRecord {
    let score = text_score(finding.base_severity, &source);
    let mut record = DetectionRecord::new(
        DETECTOR_ID,
        DetectionKind::KernelTextTamper,
        format!("{} kernel text hash drift", finding.os),
        finding.detail,
        source,
        score,
    )
    .with_entity(finding.owner)
    .with_range(finding.start, finding.end)
    .with_tag(finding.os)
    .with_tag("text_hash");
    if let Some(vm_id) = finding.vm_id {
        record = record.with_vm_id(vm_id);
    }
    record
}

fn text_score(base_severity: Severity, source: &DetectionSource) -> DetectionScore {
    score_detection(ScoreFactors {
        base_severity,
        source: source.reliability,
        attribution: AttributionQuality::GuestSymbol,
        profile: source.profile,
        identity: IdentityConfidence::Medium,
        data_loss: false,
        policy_match: false,
    })
}

pub fn offline_source(name: impl Into<String>, profile: ProfileConfidence) -> DetectionSource {
    DetectionSource::new(name, SourceReliability::OfflineSnapshot, profile)
}
