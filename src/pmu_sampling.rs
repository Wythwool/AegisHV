use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PmuSamplingError {
    CounterWentBackwards,
    InvalidTimeWindow,
    MissingStableIdentity,
    StalePidTarget,
    RingFull,
}

impl fmt::Display for PmuSamplingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CounterWentBackwards => write!(f, "perf counter value moved backwards"),
            Self::InvalidTimeWindow => write!(f, "perf counter time window is invalid"),
            Self::MissingStableIdentity => {
                write!(f, "PMU target is missing stable VM or vCPU identity")
            }
            Self::StalePidTarget => write!(f, "PMU target PID/TID start time no longer matches"),
            Self::RingFull => write!(f, "perf ring model is full and recorded sample loss"),
        }
    }
}

impl std::error::Error for PmuSamplingError {}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PerfCounterSet {
    pub cycles: Option<u64>,
    pub instructions: Option<u64>,
    pub cache_refs: Option<u64>,
    pub cache_misses: Option<u64>,
    pub branches: Option<u64>,
    pub branch_misses: Option<u64>,
}

impl PerfCounterSet {
    fn checked_delta(self, next: Self) -> Result<Self, PmuSamplingError> {
        Ok(Self {
            cycles: delta_opt(self.cycles, next.cycles)?,
            instructions: delta_opt(self.instructions, next.instructions)?,
            cache_refs: delta_opt(self.cache_refs, next.cache_refs)?,
            cache_misses: delta_opt(self.cache_misses, next.cache_misses)?,
            branches: delta_opt(self.branches, next.branches)?,
            branch_misses: delta_opt(self.branch_misses, next.branch_misses)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupedPerfSnapshot {
    pub counters: PerfCounterSet,
    pub time_enabled: u64,
    pub time_running: u64,
}

impl GroupedPerfSnapshot {
    pub fn delta(self, next: Self) -> Result<GroupedPerfDelta, PmuSamplingError> {
        if next.time_enabled < self.time_enabled || next.time_running < self.time_running {
            return Err(PmuSamplingError::InvalidTimeWindow);
        }
        let time_enabled_delta = next.time_enabled - self.time_enabled;
        let time_running_delta = next.time_running - self.time_running;
        let counters = self.counters.checked_delta(next.counters)?;
        Ok(GroupedPerfDelta {
            counters,
            time_enabled_delta,
            time_running_delta,
            scaled: time_running_delta != 0 && time_running_delta != time_enabled_delta,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupedPerfDelta {
    pub counters: PerfCounterSet,
    pub time_enabled_delta: u64,
    pub time_running_delta: u64,
    pub scaled: bool,
}

impl GroupedPerfDelta {
    pub fn scaled_value(&self, value: Option<u64>) -> Option<u64> {
        let value = value?;
        if !self.scaled || self.time_running_delta == 0 {
            return Some(value);
        }
        let scaled = (value as u128).saturating_mul(self.time_enabled_delta as u128)
            / self.time_running_delta as u128;
        Some(scaled.min(u64::MAX as u128) as u64)
    }
}

fn delta_opt(prev: Option<u64>, next: Option<u64>) -> Result<Option<u64>, PmuSamplingError> {
    match (prev, next) {
        (Some(prev), Some(next)) if next >= prev => Ok(Some(next - prev)),
        (Some(_), Some(_)) => Err(PmuSamplingError::CounterWentBackwards),
        _ => Ok(None),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StablePmuTarget {
    pub vm_id: String,
    pub vcpu_id: u16,
    pub pid: i32,
    pub tid: i32,
    pub pid_start_time_ticks: u64,
    pub tid_start_time_ticks: u64,
}

impl StablePmuTarget {
    pub fn new(
        vm_id: String,
        vcpu_id: u16,
        pid: i32,
        tid: i32,
        pid_start_time_ticks: u64,
        tid_start_time_ticks: u64,
    ) -> Result<Self, PmuSamplingError> {
        if vm_id.trim().is_empty() || pid <= 0 || tid <= 0 {
            return Err(PmuSamplingError::MissingStableIdentity);
        }
        Ok(Self {
            vm_id,
            vcpu_id,
            pid,
            tid,
            pid_start_time_ticks,
            tid_start_time_ticks,
        })
    }

    pub fn validate_current(
        &self,
        observed_pid_start: u64,
        observed_tid_start: u64,
    ) -> Result<(), PmuSamplingError> {
        if self.pid_start_time_ticks != observed_pid_start
            || self.tid_start_time_ticks != observed_tid_start
        {
            return Err(PmuSamplingError::StalePidTarget);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfSampleKind {
    CounterOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerfSample {
    pub kind: PerfSampleKind,
    pub ip: Option<u64>,
    pub weight: Option<u64>,
}

pub struct PerfRingBufferModel<const N: usize> {
    samples: [Option<PerfSample>; N],
    len: usize,
    dropped: u64,
}

impl<const N: usize> PerfRingBufferModel<N> {
    pub const fn new() -> Self {
        Self {
            samples: [None; N],
            len: 0,
            dropped: 0,
        }
    }

    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    pub fn samples(&self) -> impl Iterator<Item = PerfSample> + '_ {
        self.samples[..self.len].iter().filter_map(|sample| *sample)
    }

    pub fn push(&mut self, sample: PerfSample) -> Result<(), PmuSamplingError> {
        if self.len >= N {
            self.dropped = self.dropped.saturating_add(1);
            return Err(PmuSamplingError::RingFull);
        }
        self.samples[self.len] = Some(sample);
        self.len += 1;
        Ok(())
    }
}

impl<const N: usize> Default for PerfRingBufferModel<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntelPebsCapability {
    pub precise_ip: bool,
    pub load_latency: bool,
    pub store_latency: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmdIbsCapability {
    pub fetch_sampling: bool,
    pub op_sampling: bool,
    pub branch_target: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmSpeCapability {
    pub profiling: bool,
    pub physical_address: bool,
    pub timestamps: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PmuCapabilitySet {
    pub pebs: Option<IntelPebsCapability>,
    pub ibs: Option<AmdIbsCapability>,
    pub spe: Option<ArmSpeCapability>,
}

impl PmuCapabilitySet {
    pub const fn no_precise_sampling() -> Self {
        Self {
            pebs: None,
            ibs: None,
            spe: None,
        }
    }

    pub const fn has_precise_sampling(self) -> bool {
        matches!(self.pebs, Some(pebs) if pebs.precise_ip)
            || matches!(self.ibs, Some(ibs) if ibs.fetch_sampling || ibs.op_sampling)
            || matches!(self.spe, Some(spe) if spe.profiling)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_perf_delta_preserves_unavailable_counters_as_none() {
        let prev = GroupedPerfSnapshot {
            counters: PerfCounterSet {
                cycles: Some(100),
                instructions: None,
                cache_refs: Some(5),
                ..PerfCounterSet::default()
            },
            time_enabled: 10,
            time_running: 5,
        };
        let next = GroupedPerfSnapshot {
            counters: PerfCounterSet {
                cycles: Some(140),
                instructions: None,
                cache_refs: Some(9),
                ..PerfCounterSet::default()
            },
            time_enabled: 20,
            time_running: 10,
        };

        let delta = prev.delta(next).unwrap();

        assert_eq!(delta.counters.cycles, Some(40));
        assert_eq!(delta.counters.instructions, None);
        assert_eq!(delta.scaled_value(delta.counters.cycles), Some(80));
    }

    #[test]
    fn grouped_perf_delta_rejects_backward_counters_and_bad_time() {
        let prev = GroupedPerfSnapshot {
            counters: PerfCounterSet {
                cycles: Some(10),
                ..PerfCounterSet::default()
            },
            time_enabled: 20,
            time_running: 20,
        };
        let backwards = GroupedPerfSnapshot {
            counters: PerfCounterSet {
                cycles: Some(9),
                ..PerfCounterSet::default()
            },
            time_enabled: 21,
            time_running: 21,
        };
        let bad_time = GroupedPerfSnapshot {
            counters: PerfCounterSet::default(),
            time_enabled: 19,
            time_running: 21,
        };

        assert_eq!(
            prev.delta(backwards).unwrap_err(),
            PmuSamplingError::CounterWentBackwards
        );
        assert_eq!(
            prev.delta(bad_time).unwrap_err(),
            PmuSamplingError::InvalidTimeWindow
        );
    }

    #[test]
    fn stable_pmu_target_rejects_missing_identity_and_stale_pid() {
        assert_eq!(
            StablePmuTarget::new(String::new(), 0, 1, 2, 10, 20).unwrap_err(),
            PmuSamplingError::MissingStableIdentity
        );
        let target = StablePmuTarget::new("libvirt:vm-a".to_string(), 0, 100, 101, 10, 20).unwrap();

        target.validate_current(10, 20).unwrap();
        assert_eq!(
            target.validate_current(11, 20).unwrap_err(),
            PmuSamplingError::StalePidTarget
        );
    }

    #[test]
    fn perf_ring_model_counts_loss_when_full() {
        let mut ring = PerfRingBufferModel::<1>::new();
        let sample = PerfSample {
            kind: PerfSampleKind::CounterOverflow,
            ip: Some(0x401000),
            weight: Some(9),
        };

        ring.push(sample).unwrap();
        assert_eq!(ring.push(sample).unwrap_err(), PmuSamplingError::RingFull);
        assert_eq!(ring.dropped(), 1);
        assert_eq!(ring.samples().count(), 1);
    }

    #[test]
    fn precise_sampling_capability_is_explicit() {
        let none = PmuCapabilitySet::no_precise_sampling();
        let pebs = PmuCapabilitySet {
            pebs: Some(IntelPebsCapability {
                precise_ip: true,
                load_latency: false,
                store_latency: false,
            }),
            ibs: None,
            spe: None,
        };

        assert!(!none.has_precise_sampling());
        assert!(pebs.has_precise_sampling());
    }
}
