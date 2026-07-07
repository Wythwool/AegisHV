use std::collections::BTreeMap;

use crate::detectors::{DetectionKind, DetectionRecord, DetectorError};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DetectionDedupeKey {
    pub detector_id: String,
    pub vm_id: Option<String>,
    pub entity: Option<String>,
    pub range_start: Option<u64>,
    pub range_end: Option<u64>,
    pub symbol: Option<String>,
}

impl DetectionDedupeKey {
    pub fn from_record(record: &DetectionRecord) -> Self {
        Self {
            detector_id: record.detector_id.clone(),
            vm_id: record.vm_id.clone(),
            entity: record.entity.clone(),
            range_start: record.range_start,
            range_end: record.range_end,
            symbol: record.symbol.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggregatedDetection {
    pub key: DetectionDedupeKey,
    pub kind: DetectionKind,
    pub count: u64,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub latest: DetectionRecord,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DetectionAggregator {
    entries: BTreeMap<DetectionDedupeKey, AggregatedDetection>,
}

impl DetectionAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(
        &mut self,
        record: DetectionRecord,
        observed_ms: u64,
    ) -> Result<AggregatedDetection, DetectorError> {
        if observed_ms == 0 {
            return Err(DetectorError::MalformedInput {
                detail: "detector observation timestamp must be non-zero".to_string(),
            });
        }
        let key = DetectionDedupeKey::from_record(&record);
        let entry = self
            .entries
            .entry(key.clone())
            .or_insert_with(|| AggregatedDetection {
                key,
                kind: record.kind,
                count: 0,
                first_seen_ms: observed_ms,
                last_seen_ms: observed_ms,
                latest: record.clone(),
            });
        entry.count = entry.count.saturating_add(1);
        entry.last_seen_ms = observed_ms;
        entry.latest = record;
        Ok(entry.clone())
    }

    pub fn get(&self, key: &DetectionDedupeKey) -> Option<&AggregatedDetection> {
        self.entries.get(key)
    }

    pub fn entries(&self) -> impl Iterator<Item = &AggregatedDetection> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
