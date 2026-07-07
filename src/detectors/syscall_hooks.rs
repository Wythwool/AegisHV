use std::collections::BTreeSet;

use crate::detectors::{
    score_detection, AttributionQuality, DetectionKind, DetectionRecord, DetectionSource,
    DetectorError, ScoreFactors,
};
use crate::event::{IdentityConfidence, Severity};
use crate::linux_syscall::{LinuxLstarReport, LinuxSyscallTableReport};
use crate::windows_syscall::{WindowsLstarReport, WindowsSsdtReport};

const DETECTOR_ID: &str = "syscall_hook";

pub fn linux_syscall_hook_findings(
    table: &LinuxSyscallTableReport,
    lstar: &LinuxLstarReport,
    source: DetectionSource,
    vm_id: Option<&str>,
) -> Result<Vec<DetectionRecord>, DetectorError> {
    if table.entries.is_empty() {
        return Err(DetectorError::Unsupported {
            detector: DETECTOR_ID.to_string(),
            detail: "Linux syscall table report has no entries".to_string(),
        });
    }
    let mut details = BTreeSet::new();
    details.extend(lstar.findings.iter().cloned());
    details.extend(table.findings.iter().cloned());
    Ok(details
        .into_iter()
        .map(|detail| syscall_record("linux", detail, source.clone(), vm_id))
        .collect())
}

pub fn windows_syscall_hook_findings(
    ssdt: &WindowsSsdtReport,
    lstar: &WindowsLstarReport,
    source: DetectionSource,
    vm_id: Option<&str>,
) -> Result<Vec<DetectionRecord>, DetectorError> {
    if ssdt.entries.is_empty() && ssdt.findings.is_empty() {
        return Err(DetectorError::Unsupported {
            detector: DETECTOR_ID.to_string(),
            detail: "Windows SSDT report has no entries or findings".to_string(),
        });
    }
    let mut details = BTreeSet::new();
    details.extend(lstar.findings.iter().cloned());
    details.extend(ssdt.findings.iter().cloned());
    Ok(details
        .into_iter()
        .map(|detail| syscall_record("windows", detail, source.clone(), vm_id))
        .collect())
}

fn syscall_record(
    os: &str,
    detail: String,
    source: DetectionSource,
    vm_id: Option<&str>,
) -> DetectionRecord {
    let score = score_detection(ScoreFactors {
        base_severity: Severity::High,
        source: source.reliability,
        attribution: AttributionQuality::GuestSymbol,
        profile: source.profile,
        identity: IdentityConfidence::Medium,
        data_loss: false,
        policy_match: false,
    });
    let mut record = DetectionRecord::new(
        DETECTOR_ID,
        DetectionKind::SyscallHook,
        format!("{os} syscall path drift"),
        detail,
        source,
        score,
    )
    .with_tag(os)
    .with_tag("syscall");
    if let Some(vm_id) = vm_id {
        record = record.with_vm_id(vm_id);
    }
    record
}
