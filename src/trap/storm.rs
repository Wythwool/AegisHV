use std::collections::BTreeMap;

use super::{TrapError, TrapErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapStormMode {
    FailOpen,
    FailClosed,
}

impl TrapStormMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FailOpen => "fail_open",
            Self::FailClosed => "fail_closed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TrapStormKey {
    pub owner_vm: String,
    pub address_space: String,
    pub page: u64,
    pub vcpu_id: Option<u32>,
}

impl TrapStormKey {
    pub fn new(
        owner_vm: impl Into<String>,
        address_space: impl Into<String>,
        page: u64,
        vcpu_id: Option<u32>,
    ) -> Result<Self, TrapError> {
        let owner_vm = owner_vm.into();
        let address_space = address_space.into();
        if owner_vm.trim().is_empty() || address_space.trim().is_empty() {
            return Err(TrapError::new(
                TrapErrorKind::MalformedInput,
                "trap storm key requires VM and address-space identifiers",
            ));
        }
        Ok(Self {
            owner_vm,
            address_space,
            page,
            vcpu_id,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapStormDecision {
    Allow,
    ThrottleFailOpen,
    ThrottleFailClosed,
}

impl TrapStormDecision {
    pub fn throttled(self) -> bool {
        matches!(self, Self::ThrottleFailOpen | Self::ThrottleFailClosed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrapStormBucket {
    window_start_ms: u64,
    hits: u32,
}

#[derive(Debug, Clone)]
pub struct TrapStormGuard {
    window_ms: u64,
    max_hits: u32,
    mode: TrapStormMode,
    buckets: BTreeMap<TrapStormKey, TrapStormBucket>,
}

impl TrapStormGuard {
    pub fn new(window_ms: u64, max_hits: u32, mode: TrapStormMode) -> Result<Self, TrapError> {
        if window_ms == 0 {
            return Err(TrapError::new(
                TrapErrorKind::MalformedInput,
                "trap storm window must be at least one millisecond",
            ));
        }
        if max_hits == 0 {
            return Err(TrapError::new(
                TrapErrorKind::MalformedInput,
                "trap storm max_hits must be greater than zero",
            ));
        }
        Ok(Self {
            window_ms,
            max_hits,
            mode,
            buckets: BTreeMap::new(),
        })
    }

    pub fn observe(&mut self, key: TrapStormKey, now_ms: u64) -> TrapStormDecision {
        let bucket = self.buckets.entry(key).or_insert(TrapStormBucket {
            window_start_ms: now_ms,
            hits: 0,
        });
        if now_ms.saturating_sub(bucket.window_start_ms) >= self.window_ms {
            bucket.window_start_ms = now_ms;
            bucket.hits = 0;
        }
        bucket.hits = bucket.hits.saturating_add(1);
        if bucket.hits <= self.max_hits {
            return TrapStormDecision::Allow;
        }
        match self.mode {
            TrapStormMode::FailOpen => TrapStormDecision::ThrottleFailOpen,
            TrapStormMode::FailClosed => TrapStormDecision::ThrottleFailClosed,
        }
    }
}

impl Default for TrapStormGuard {
    fn default() -> Self {
        Self::new(1000, 256, TrapStormMode::FailClosed).expect("default storm guard is valid")
    }
}
