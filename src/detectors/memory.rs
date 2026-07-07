use crate::detectors::jit::JitAllowlist;
use crate::detectors::{
    score_detection, AttributionQuality, DetectionKind, DetectionRecord, DetectionSource,
    DetectorError, ScoreFactors,
};
use crate::event::{IdentityConfidence, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionKind {
    Anonymous,
    FileBacked,
    Jit,
    Stack,
    Unknown,
}

impl MemoryRegionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anonymous => "anonymous",
            Self::FileBacked => "file_backed",
            Self::Jit => "jit",
            Self::Stack => "stack",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryMapping {
    pub vm_id: Option<String>,
    pub process: Option<String>,
    pub module: Option<String>,
    pub start: u64,
    pub end: u64,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    pub kind: MemoryRegionKind,
    pub source: DetectionSource,
}

impl MemoryMapping {
    pub fn validate(&self) -> Result<(), DetectorError> {
        if self.end <= self.start {
            return Err(DetectorError::MalformedInput {
                detail: format!(
                    "memory mapping range 0x{:x}..0x{:x} is empty or inverted",
                    self.start, self.end
                ),
            });
        }
        Ok(())
    }
}

pub fn detect_executable_anonymous_mappings(
    mappings: &[MemoryMapping],
    allowlist: &JitAllowlist,
) -> Result<Vec<DetectionRecord>, DetectorError> {
    let mut out = Vec::new();
    for mapping in mappings {
        mapping.validate()?;
        if !mapping.executable || mapping.kind != MemoryRegionKind::Anonymous {
            continue;
        }
        if allowlist.allows(mapping) {
            continue;
        }
        out.push(mapping_record(
            "exec_anon",
            DetectionKind::ExecutableAnonymousMemory,
            "anonymous executable guest mapping",
            "anonymous memory is executable and not covered by the JIT allowlist",
            Severity::Medium,
            mapping,
        ));
    }
    Ok(out)
}

pub fn detect_rwx_mappings(
    mappings: &[MemoryMapping],
) -> Result<Vec<DetectionRecord>, DetectorError> {
    let mut out = Vec::new();
    for mapping in mappings {
        mapping.validate()?;
        if !(mapping.readable && mapping.writable && mapping.executable) {
            continue;
        }
        out.push(mapping_record(
            "rwx_mapping",
            DetectionKind::RwxMapping,
            "guest mapping is readable, writable, and executable",
            "mapping has R, W, and X permissions at the same time",
            Severity::High,
            mapping,
        ));
    }
    Ok(out)
}

fn mapping_record(
    detector_id: &str,
    kind: DetectionKind,
    title: &str,
    detail: &str,
    severity: Severity,
    mapping: &MemoryMapping,
) -> DetectionRecord {
    let attribution = if mapping.module.is_some() {
        AttributionQuality::GuestSymbol
    } else if mapping.process.is_some() {
        AttributionQuality::GuestProcess
    } else {
        AttributionQuality::GuestAddress
    };
    let score = score_detection(ScoreFactors {
        base_severity: severity,
        source: mapping.source.reliability,
        attribution,
        profile: mapping.source.profile,
        identity: IdentityConfidence::Medium,
        data_loss: false,
        policy_match: false,
    });
    let mut record = DetectionRecord::new(
        detector_id,
        kind,
        title,
        detail,
        mapping.source.clone(),
        score,
    )
    .with_range(mapping.start, mapping.end)
    .with_tag(mapping.kind.as_str());
    if let Some(vm_id) = &mapping.vm_id {
        record = record.with_vm_id(vm_id);
    }
    if let Some(process) = &mapping.process {
        record = record.with_entity(process);
    }
    if let Some(module) = &mapping.module {
        record = record.with_symbol(module);
    }
    record
}
