use crate::detectors::{
    score_detection, AttributionQuality, DetectionKind, DetectionRecord, DetectionScore,
    DetectionSource, ProfileConfidence, ScoreFactors, SourceReliability,
};
use crate::event::{IdentityConfidence, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PmuRatioSample {
    pub cycles: u64,
    pub instructions: u64,
}

impl PmuRatioSample {
    pub const fn new(cycles: u64, instructions: u64) -> Option<Self> {
        if instructions == 0 {
            None
        } else {
            Some(Self {
                cycles,
                instructions,
            })
        }
    }

    fn cycles_per_kilo_instruction(self) -> u64 {
        self.cycles.saturating_mul(1000) / self.instructions
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmuBaseline {
    vm_id: String,
    sample_count: u32,
    mean_cycles_per_kilo_instruction: u64,
    alert_percent: u32,
}

impl PmuBaseline {
    pub fn new(vm_id: impl Into<String>, alert_percent: u32) -> Result<Self, &'static str> {
        let vm_id = vm_id.into();
        if vm_id.trim().is_empty() {
            return Err("PMU baseline requires a stable VM id");
        }
        if !(1..=10_000).contains(&alert_percent) {
            return Err("PMU baseline alert percent must be between 1 and 10000");
        }
        Ok(Self {
            vm_id,
            sample_count: 0,
            mean_cycles_per_kilo_instruction: 0,
            alert_percent,
        })
    }

    pub const fn sample_count(&self) -> u32 {
        self.sample_count
    }

    pub const fn mean_cycles_per_kilo_instruction(&self) -> u64 {
        self.mean_cycles_per_kilo_instruction
    }

    pub fn update(&mut self, sample: PmuRatioSample) {
        let value = sample.cycles_per_kilo_instruction();
        self.sample_count = self.sample_count.saturating_add(1);
        if self.sample_count == 1 {
            self.mean_cycles_per_kilo_instruction = value;
            return;
        }
        let previous_weight = self.sample_count.saturating_sub(1) as u128;
        let next = (self.mean_cycles_per_kilo_instruction as u128)
            .saturating_mul(previous_weight)
            .saturating_add(value as u128)
            / self.sample_count as u128;
        self.mean_cycles_per_kilo_instruction = next.min(u64::MAX as u128) as u64;
    }

    pub fn evaluate(&self, sample: PmuRatioSample) -> Option<DetectionRecord> {
        if self.sample_count < 3 || self.mean_cycles_per_kilo_instruction == 0 {
            return None;
        }
        let value = sample.cycles_per_kilo_instruction();
        let baseline = self.mean_cycles_per_kilo_instruction;
        let diff = value.abs_diff(baseline);
        if diff.saturating_mul(100) < baseline.saturating_mul(self.alert_percent as u64) {
            return None;
        }
        let score = pmu_anomaly_score();
        Some(
            DetectionRecord::new(
                "pmu-anomaly-cpi",
                DetectionKind::PmuAnomaly,
                "PMU ratio anomaly",
                format!(
                    "cycles_per_kinst={} baseline={} samples={}",
                    value, baseline, self.sample_count
                ),
                DetectionSource::new(
                    "offline-pmu-baseline",
                    SourceReliability::OfflineSnapshot,
                    ProfileConfidence::Synthetic,
                ),
                score,
            )
            .with_vm_id(self.vm_id.clone())
            .with_tag("pmu_baseline"),
        )
    }
}

fn pmu_anomaly_score() -> DetectionScore {
    score_detection(ScoreFactors {
        base_severity: Severity::Medium,
        source: SourceReliability::OfflineSnapshot,
        attribution: AttributionQuality::HostOnly,
        profile: ProfileConfidence::Synthetic,
        identity: IdentityConfidence::Medium,
        data_loss: false,
        policy_match: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pmu_baseline_updates_offline_ratio_without_zero_instruction_samples() {
        assert!(PmuRatioSample::new(10, 0).is_none());
        let mut baseline = PmuBaseline::new("libvirt:vm-a", 50).unwrap();

        baseline.update(PmuRatioSample::new(1000, 1000).unwrap());
        baseline.update(PmuRatioSample::new(1100, 1000).unwrap());
        baseline.update(PmuRatioSample::new(900, 1000).unwrap());

        assert_eq!(baseline.sample_count(), 3);
        assert_eq!(baseline.mean_cycles_per_kilo_instruction(), 1000);
    }

    #[test]
    fn pmu_baseline_reports_anomaly_only_after_enough_history() {
        let mut baseline = PmuBaseline::new("libvirt:vm-a", 50).unwrap();
        baseline.update(PmuRatioSample::new(1000, 1000).unwrap());
        baseline.update(PmuRatioSample::new(1000, 1000).unwrap());

        assert!(baseline
            .evaluate(PmuRatioSample::new(3000, 1000).unwrap())
            .is_none());

        baseline.update(PmuRatioSample::new(1000, 1000).unwrap());
        let finding = baseline
            .evaluate(PmuRatioSample::new(3000, 1000).unwrap())
            .unwrap();

        assert_eq!(finding.kind, DetectionKind::PmuAnomaly);
        assert_eq!(finding.vm_id.as_deref(), Some("libvirt:vm-a"));
    }

    #[test]
    fn pmu_baseline_rejects_unstable_identity_and_bad_threshold() {
        assert!(PmuBaseline::new("", 50).is_err());
        assert!(PmuBaseline::new("libvirt:vm-a", 0).is_err());
    }
}
