use std::collections::BTreeSet;

use crate::detectors::{
    score_detection, AttributionQuality, DetectionKind, DetectionRecord, DetectionSource,
    DetectorError, ScoreFactors,
};
use crate::event::{IdentityConfidence, Severity};

const DETECTOR_ID: &str = "hidden_process";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessKey {
    pub pid: u64,
    pub image: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInventory {
    pub source: DetectionSource,
    pub supported: bool,
    pub processes: Vec<ProcessKey>,
}

pub fn detect_hidden_processes(
    memory_inventory: &ProcessInventory,
    os_inventory: &ProcessInventory,
    vm_id: Option<&str>,
) -> Result<Vec<DetectionRecord>, DetectorError> {
    if !memory_inventory.supported {
        return Err(DetectorError::Unsupported {
            detector: DETECTOR_ID.to_string(),
            detail: "memory process inventory is unsupported".to_string(),
        });
    }
    if !os_inventory.supported {
        return Err(DetectorError::Unsupported {
            detector: DETECTOR_ID.to_string(),
            detail: "OS process inventory is unsupported".to_string(),
        });
    }

    let os_seen = os_inventory
        .processes
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    for process in &memory_inventory.processes {
        if os_seen.contains(process) {
            continue;
        }
        let score = score_detection(ScoreFactors {
            base_severity: Severity::High,
            source: memory_inventory.source.reliability,
            attribution: AttributionQuality::GuestProcess,
            profile: memory_inventory.source.profile,
            identity: IdentityConfidence::Medium,
            data_loss: false,
            policy_match: false,
        });
        let mut record = DetectionRecord::new(
            DETECTOR_ID,
            DetectionKind::HiddenProcess,
            "process present in memory inventory but missing from OS inventory",
            format!(
                "pid {} image '{}' is absent from OS process inventory",
                process.pid, process.image
            ),
            memory_inventory.source.clone(),
            score,
        )
        .with_entity(format!("{}:{}", process.pid, process.image))
        .with_tag("inventory_mismatch");
        if let Some(vm_id) = vm_id {
            record = record.with_vm_id(vm_id);
        }
        out.push(record);
    }
    Ok(out)
}
