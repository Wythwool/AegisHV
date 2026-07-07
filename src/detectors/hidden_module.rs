use std::collections::BTreeSet;

use crate::detectors::{
    score_detection, AttributionQuality, DetectionKind, DetectionRecord, DetectionSource,
    DetectorError, ScoreFactors,
};
use crate::event::{IdentityConfidence, Severity};

const DETECTOR_ID: &str = "hidden_module";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModuleKey {
    pub name: String,
    pub base: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInventory {
    pub source: DetectionSource,
    pub supported: bool,
    pub modules: Vec<ModuleKey>,
}

pub fn detect_hidden_modules(
    memory_inventory: &ModuleInventory,
    os_inventory: &ModuleInventory,
    vm_id: Option<&str>,
) -> Result<Vec<DetectionRecord>, DetectorError> {
    if !memory_inventory.supported {
        return Err(DetectorError::Unsupported {
            detector: DETECTOR_ID.to_string(),
            detail: "memory module inventory is unsupported".to_string(),
        });
    }
    if !os_inventory.supported {
        return Err(DetectorError::Unsupported {
            detector: DETECTOR_ID.to_string(),
            detail: "OS module inventory is unsupported".to_string(),
        });
    }

    let os_seen = os_inventory
        .modules
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    for module in &memory_inventory.modules {
        if os_seen.contains(module) {
            continue;
        }
        let score = score_detection(ScoreFactors {
            base_severity: Severity::High,
            source: memory_inventory.source.reliability,
            attribution: AttributionQuality::GuestSymbol,
            profile: memory_inventory.source.profile,
            identity: IdentityConfidence::Medium,
            data_loss: false,
            policy_match: false,
        });
        let mut record = DetectionRecord::new(
            DETECTOR_ID,
            DetectionKind::HiddenModule,
            "module present in memory inventory but missing from OS inventory",
            format!(
                "module '{}' at 0x{:x} is absent from OS module inventory",
                module.name, module.base
            ),
            memory_inventory.source.clone(),
            score,
        )
        .with_entity(&module.name)
        .with_range(module.base, module.base)
        .with_tag("inventory_mismatch");
        if let Some(vm_id) = vm_id {
            record = record.with_vm_id(vm_id);
        }
        out.push(record);
    }
    Ok(out)
}
