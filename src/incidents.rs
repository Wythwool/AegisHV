use std::collections::{BTreeMap, BTreeSet};

use crate::detectors::dedupe::AggregatedDetection;
use crate::detectors::DetectionKind;
use crate::event::Severity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentStatus {
    Open,
    Updated,
}

impl IncidentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Updated => "updated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidentRecord {
    pub incident_id: String,
    pub vm_id: String,
    pub status: IncidentStatus,
    pub severity: Severity,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub detection_count: u64,
    pub kinds: Vec<DetectionKind>,
    pub summary: String,
}

pub fn correlate_incidents(detections: &[AggregatedDetection]) -> Vec<IncidentRecord> {
    let mut by_vm: BTreeMap<String, Vec<&AggregatedDetection>> = BTreeMap::new();
    for detection in detections {
        if let Some(vm_id) = &detection.key.vm_id {
            by_vm.entry(vm_id.clone()).or_default().push(detection);
        }
    }

    let mut out = Vec::new();
    for (vm_id, items) in by_vm {
        let kinds = items.iter().map(|item| item.kind).collect::<BTreeSet<_>>();
        if !has_core_chain(&kinds) {
            continue;
        }
        let first_seen_ms = items
            .iter()
            .map(|item| item.first_seen_ms)
            .min()
            .unwrap_or(0);
        let last_seen_ms = items
            .iter()
            .map(|item| item.last_seen_ms)
            .max()
            .unwrap_or(first_seen_ms);
        let detection_count = items.iter().map(|item| item.count).sum();
        let severity = items
            .iter()
            .map(|item| item.latest.severity)
            .max_by_key(|severity| severity_rank(*severity))
            .unwrap_or(Severity::High);
        let kinds_vec = kinds.into_iter().collect::<Vec<_>>();
        out.push(IncidentRecord {
            incident_id: format!("incident:{vm_id}:{first_seen_ms}:wx-syscall-text"),
            vm_id: vm_id.clone(),
            status: if detection_count > kinds_vec.len() as u64 {
                IncidentStatus::Updated
            } else {
                IncidentStatus::Open
            },
            severity,
            first_seen_ms,
            last_seen_ms,
            detection_count,
            kinds: kinds_vec,
            summary: format!(
                "VM {vm_id} has W^X correlation, syscall hook evidence, and kernel text hash drift"
            ),
        });
    }
    out
}

fn has_core_chain(kinds: &BTreeSet<DetectionKind>) -> bool {
    kinds.contains(&DetectionKind::WxCorrelation)
        && kinds.contains(&DetectionKind::SyscallHook)
        && kinds.contains(&DetectionKind::KernelTextTamper)
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Info => 0,
        Severity::Low => 1,
        Severity::Medium => 2,
        Severity::High => 3,
        Severity::Critical => 4,
    }
}
