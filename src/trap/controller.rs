use std::collections::BTreeMap;

use super::invalidation::{plan_invalidation, InvalidationPlan, InvalidationScope};
use super::jit::{JitTrapContext, JitTrapPolicy};
use super::singlestep::SingleStepStrategy;
use super::stage2::{
    PageSize, Stage2BackendKind, Stage2Mapping, Stage2Permissions, TrapAccessKind,
};
use super::stage2_model::Stage2Table;
use super::storm::{TrapStormDecision, TrapStormGuard, TrapStormKey};
use super::{TrapError, TrapErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrapKind {
    Read,
    Write,
    Execute,
}

impl TrapKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Execute => "execute",
        }
    }

    pub fn access(self) -> TrapAccessKind {
        match self {
            Self::Read => TrapAccessKind::Read,
            Self::Write => TrapAccessKind::Write,
            Self::Execute => TrapAccessKind::Execute,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapState {
    Armed,
    Hit,
    Classifying,
    AllowedStep,
    Denied,
    Rearmed,
    Disabled,
    StormThrottled,
}

impl TrapState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Armed => "armed",
            Self::Hit => "hit",
            Self::Classifying => "classifying",
            Self::AllowedStep => "allowed_step",
            Self::Denied => "denied",
            Self::Rearmed => "rearmed",
            Self::Disabled => "disabled",
            Self::StormThrottled => "storm_throttled",
        }
    }

    fn accepts_hit(self) -> bool {
        matches!(self, Self::Armed | Self::Rearmed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrapClassification {
    AllowStep,
    Deny(String),
    JitTemporaryWindow(JitTrapContext),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapDecision {
    AllowStep,
    AllowTemporaryWrite,
    Deny,
    FailOpen,
    FailClosed,
}

impl TrapDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AllowStep => "allow_step",
            Self::AllowTemporaryWrite => "allow_temporary_write",
            Self::Deny => "deny",
            Self::FailOpen => "fail_open",
            Self::FailClosed => "fail_closed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrapHit {
    pub owner_vm: String,
    pub address_space: String,
    pub gpa: u64,
    pub vcpu_id: Option<u32>,
    pub rip: Option<u64>,
    pub kind: TrapKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrapOutcome {
    pub trap_id: String,
    pub trap_kind: TrapKind,
    pub state: TrapState,
    pub decision: TrapDecision,
    pub backend: Stage2BackendKind,
    pub page: u64,
    pub page_size: PageSize,
    pub permissions_before: Stage2Permissions,
    pub permissions_after: Stage2Permissions,
    pub invalidation: InvalidationPlan,
    pub note: String,
}

#[derive(Debug, Clone)]
struct TrapRecord {
    id: String,
    kind: TrapKind,
    owner_vm: String,
    address_space: String,
    page: u64,
    page_size: PageSize,
    original_permissions: Stage2Permissions,
    trapped_permissions: Stage2Permissions,
    state: TrapState,
}

#[derive(Debug, Clone)]
pub struct TrapController {
    backend: Stage2BackendKind,
    table: Stage2Table,
    traps: BTreeMap<String, TrapRecord>,
    storm: TrapStormGuard,
    jit_policy: JitTrapPolicy,
    single_step: SingleStepStrategy,
}

impl TrapController {
    pub fn new(
        backend: Stage2BackendKind,
        table: Stage2Table,
        storm: TrapStormGuard,
        jit_policy: JitTrapPolicy,
        single_step: SingleStepStrategy,
    ) -> Self {
        Self {
            backend,
            table,
            traps: BTreeMap::new(),
            storm,
            jit_policy,
            single_step,
        }
    }

    pub fn table(&self) -> &Stage2Table {
        &self.table
    }

    pub fn trap_state(&self, trap_id: &str) -> Option<TrapState> {
        self.traps.get(trap_id).map(|trap| trap.state)
    }

    pub fn arm_trap(
        &mut self,
        owner_vm: &str,
        address_space: &str,
        gpa: u64,
        kind: TrapKind,
    ) -> Result<String, TrapError> {
        let mapping = self
            .table
            .lookup(owner_vm, address_space, gpa)
            .ok_or_else(|| {
                TrapError::new(
                    TrapErrorKind::NotMapped,
                    format!("cannot arm {} trap: {gpa:#x} is not mapped", kind.as_str()),
                )
            })?
            .clone();
        let trapped_permissions = mapping.permissions.without_access(kind.access());
        let page = mapping.page_size.align_down(gpa);
        let trap_id = trap_id(owner_vm, address_space, page, kind);
        let previous =
            self.table
                .set_permissions(owner_vm, address_space, page, trapped_permissions)?;
        let record = TrapRecord {
            id: trap_id.clone(),
            kind,
            owner_vm: owner_vm.to_string(),
            address_space: address_space.to_string(),
            page,
            page_size: mapping.page_size,
            original_permissions: previous,
            trapped_permissions,
            state: TrapState::Armed,
        };
        self.traps.insert(trap_id.clone(), record);
        Ok(trap_id)
    }

    pub fn handle_hit(
        &mut self,
        hit: TrapHit,
        classification: TrapClassification,
        now_ms: u64,
    ) -> Result<TrapOutcome, TrapError> {
        let trap_id = trap_id(
            &hit.owner_vm,
            &hit.address_space,
            PageSize::Size4K.align_down(hit.gpa),
            hit.kind,
        );
        let trap_id = if self.traps.contains_key(&trap_id) {
            trap_id
        } else {
            self.find_covering_trap(&hit)?
        };
        let storm_key = TrapStormKey::new(
            &hit.owner_vm,
            &hit.address_space,
            self.traps
                .get(&trap_id)
                .map(|trap| trap.page)
                .unwrap_or_else(|| PageSize::Size4K.align_down(hit.gpa)),
            hit.vcpu_id,
        )?;
        match self.storm.observe(storm_key, now_ms) {
            TrapStormDecision::Allow => {}
            TrapStormDecision::ThrottleFailOpen => {
                return self.finish_hit(
                    trap_id,
                    TrapState::StormThrottled,
                    TrapDecision::FailOpen,
                    true,
                    "trap storm throttled page; policy is fail-open".to_string(),
                );
            }
            TrapStormDecision::ThrottleFailClosed => {
                return self.finish_hit(
                    trap_id,
                    TrapState::StormThrottled,
                    TrapDecision::FailClosed,
                    false,
                    "trap storm throttled page; policy is fail-closed".to_string(),
                );
            }
        }

        {
            let trap = self
                .traps
                .get_mut(&trap_id)
                .ok_or_else(|| missing_trap(&trap_id))?;
            if !trap.state.accepts_hit() {
                return Err(TrapError::new(
                    TrapErrorKind::InvalidState,
                    format!(
                        "trap {} cannot accept hit while state is {}",
                        trap.id,
                        trap.state.as_str()
                    ),
                ));
            }
            trap.state = TrapState::Hit;
            trap.state = TrapState::Classifying;
        }

        match classification {
            TrapClassification::AllowStep => self.finish_hit(
                trap_id,
                TrapState::AllowedStep,
                TrapDecision::AllowStep,
                true,
                format!("single-step strategy {}", self.single_step.as_str()),
            ),
            TrapClassification::Deny(reason) => self.finish_hit(
                trap_id,
                TrapState::Denied,
                TrapDecision::Deny,
                false,
                reason,
            ),
            TrapClassification::JitTemporaryWindow(ctx) => {
                let decision = self.jit_policy.evaluate(&ctx);
                if decision.allowed {
                    self.finish_hit(
                        trap_id,
                        TrapState::AllowedStep,
                        TrapDecision::AllowTemporaryWrite,
                        true,
                        decision.reason,
                    )
                } else {
                    self.finish_hit(
                        trap_id,
                        TrapState::Denied,
                        TrapDecision::Deny,
                        false,
                        decision.reason,
                    )
                }
            }
        }
    }

    pub fn rearm(&mut self, trap_id: &str) -> Result<TrapOutcome, TrapError> {
        let (owner_vm, address_space, page, trapped_permissions, before) = {
            let trap = self
                .traps
                .get_mut(trap_id)
                .ok_or_else(|| missing_trap(trap_id))?;
            if trap.state != TrapState::AllowedStep && trap.state != TrapState::StormThrottled {
                return Err(TrapError::new(
                    TrapErrorKind::InvalidState,
                    format!(
                        "trap {} cannot re-arm from state {}",
                        trap.id,
                        trap.state.as_str()
                    ),
                ));
            }
            trap.state = TrapState::Rearmed;
            (
                trap.owner_vm.clone(),
                trap.address_space.clone(),
                trap.page,
                trap.trapped_permissions,
                trap.original_permissions,
            )
        };
        self.table
            .set_permissions(&owner_vm, &address_space, page, trapped_permissions)?;
        let invalidation = self.invalidation_for(trap_id)?;
        let trap = self
            .traps
            .get(trap_id)
            .ok_or_else(|| missing_trap(trap_id))?;
        Ok(TrapOutcome {
            trap_id: trap.id.clone(),
            trap_kind: trap.kind,
            state: trap.state,
            decision: TrapDecision::AllowStep,
            backend: self.backend,
            page: trap.page,
            page_size: trap.page_size,
            permissions_before: before,
            permissions_after: trapped_permissions,
            invalidation,
            note: "trap re-armed after temporary window".to_string(),
        })
    }

    pub fn disable(&mut self, trap_id: &str) -> Result<(), TrapError> {
        let trap = self
            .traps
            .get_mut(trap_id)
            .ok_or_else(|| missing_trap(trap_id))?;
        self.table.set_permissions(
            &trap.owner_vm,
            &trap.address_space,
            trap.page,
            trap.original_permissions,
        )?;
        trap.state = TrapState::Disabled;
        Ok(())
    }

    fn finish_hit(
        &mut self,
        trap_id: String,
        state: TrapState,
        decision: TrapDecision,
        allow_window: bool,
        note: String,
    ) -> Result<TrapOutcome, TrapError> {
        let (owner_vm, address_space, page, before, after) = {
            let trap = self
                .traps
                .get_mut(&trap_id)
                .ok_or_else(|| missing_trap(&trap_id))?;
            trap.state = state;
            let before = trap.trapped_permissions;
            let after = if allow_window {
                trap.original_permissions
            } else {
                trap.trapped_permissions
            };
            (
                trap.owner_vm.clone(),
                trap.address_space.clone(),
                trap.page,
                before,
                after,
            )
        };
        if allow_window {
            self.table
                .set_permissions(&owner_vm, &address_space, page, after)?;
        }
        let invalidation = self.invalidation_for(&trap_id)?;
        let trap = self
            .traps
            .get(&trap_id)
            .ok_or_else(|| missing_trap(&trap_id))?;
        Ok(TrapOutcome {
            trap_id: trap.id.clone(),
            trap_kind: trap.kind,
            state: trap.state,
            decision,
            backend: self.backend,
            page: trap.page,
            page_size: trap.page_size,
            permissions_before: before,
            permissions_after: after,
            invalidation,
            note,
        })
    }

    fn find_covering_trap(&self, hit: &TrapHit) -> Result<String, TrapError> {
        self.traps
            .values()
            .find(|trap| {
                trap.kind == hit.kind
                    && trap.owner_vm == hit.owner_vm
                    && trap.address_space == hit.address_space
                    && trap.page <= hit.gpa
                    && hit.gpa < trap.page + trap.page_size.bytes()
            })
            .map(|trap| trap.id.clone())
            .ok_or_else(|| {
                TrapError::new(
                    TrapErrorKind::NotMapped,
                    format!(
                        "no armed {} trap covers {:#x} for vm={} address_space={}",
                        hit.kind.as_str(),
                        hit.gpa,
                        hit.owner_vm,
                        hit.address_space
                    ),
                )
            })
    }

    fn invalidation_for(&self, trap_id: &str) -> Result<InvalidationPlan, TrapError> {
        let trap = self
            .traps
            .get(trap_id)
            .ok_or_else(|| missing_trap(trap_id))?;
        plan_invalidation(
            self.backend,
            &InvalidationScope::SinglePage {
                owner_vm: trap.owner_vm.clone(),
                address_space: trap.address_space.clone(),
                gpa: trap.page,
                page_size: trap.page_size,
            },
        )
    }
}

pub fn armable_mapping(
    owner_vm: &str,
    address_space: &str,
    base: u64,
    page_size: PageSize,
    permissions: Stage2Permissions,
) -> Result<Stage2Mapping, TrapError> {
    Stage2Mapping::new(
        owner_vm,
        address_space,
        base,
        page_size,
        super::stage2::MemoryType::WriteBack,
        permissions,
    )
}

fn trap_id(owner_vm: &str, address_space: &str, page: u64, kind: TrapKind) -> String {
    format!(
        "trap:{owner_vm}:{address_space}:{page:#x}:{}",
        kind.as_str()
    )
}

fn missing_trap(trap_id: &str) -> TrapError {
    TrapError::new(
        TrapErrorKind::NotMapped,
        format!("trap {trap_id} is not armed"),
    )
}
