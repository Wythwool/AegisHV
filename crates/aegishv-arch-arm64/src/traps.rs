use aegishv_hypervisor_core::ids::GuestPhysical;

use crate::esr::{EsrEl2, ExceptionClass, Stage2Fault};
use crate::features::{Arm64Error, Arm64ErrorKind};
use crate::stage2::{Stage2Access, Stage2Mapping, Stage2Permissions};
use crate::tlbi::{TlbiPlan, Vmid};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmcPolicy {
    Deny,
    ForwardToFirmware,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapAction {
    Resume,
    HaltVcpu,
    YieldVcpu,
    DenySmc,
    ForwardSmc,
    Stage2PermissionTrap(Stage2Access),
}

pub fn handle_sync_trap(esr: EsrEl2, smc_policy: SmcPolicy) -> Result<TrapAction, Arm64Error> {
    match esr.exception_class() {
        ExceptionClass::Hvc64 => Ok(TrapAction::Resume),
        ExceptionClass::Smc64 => match smc_policy {
            SmcPolicy::Deny => Ok(TrapAction::DenySmc),
            SmcPolicy::ForwardToFirmware => Ok(TrapAction::ForwardSmc),
        },
        ExceptionClass::WfiWfe => {
            if esr.iss() & 1 == 0 {
                Ok(TrapAction::HaltVcpu)
            } else {
                Ok(TrapAction::YieldVcpu)
            }
        }
        ExceptionClass::InstructionAbortLowerEl | ExceptionClass::DataAbortLowerEl => {
            Err(Arm64Error::new(
                Arm64ErrorKind::UnsupportedTrap,
                "ARM64 abort trap requires Stage-2 fault decode",
            ))
        }
        ExceptionClass::Unknown(_) => Err(Arm64Error::new(
            Arm64ErrorKind::UnsupportedTrap,
            "ARM64 synchronous exception class is not handled by the lab model",
        )),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arm64TrapKind {
    Execute,
    Write,
}

impl Arm64TrapKind {
    const fn access(self) -> Stage2Access {
        match self {
            Self::Execute => Stage2Access::Execute,
            Self::Write => Stage2Access::Write,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arm64TrapState {
    Armed,
    Hit,
    TemporaryWindow,
    Rearmed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arm64Stage2Trap {
    pub kind: Arm64TrapKind,
    pub ipa: GuestPhysical,
    pub original_permissions: Stage2Permissions,
    pub armed_permissions: Stage2Permissions,
    pub state: Arm64TrapState,
}

impl Arm64Stage2Trap {
    pub fn arm_execute(mapping: Stage2Mapping) -> Result<Self, Arm64Error> {
        Self::arm(mapping, Arm64TrapKind::Execute)
    }

    pub fn arm_write(mapping: Stage2Mapping) -> Result<Self, Arm64Error> {
        Self::arm(mapping, Arm64TrapKind::Write)
    }

    fn arm(mapping: Stage2Mapping, kind: Arm64TrapKind) -> Result<Self, Arm64Error> {
        if !mapping.permissions.allows(kind.access()) {
            return Err(Arm64Error::new(
                Arm64ErrorKind::InvalidStage2Mapping,
                "ARM64 trap cannot remove a Stage-2 access bit that is already absent",
            ));
        }
        let armed_permissions = match kind {
            Arm64TrapKind::Execute => mapping.permissions.without_execute(),
            Arm64TrapKind::Write => mapping.permissions.without_write(),
        };
        Ok(Self {
            kind,
            ipa: mapping.ipa,
            original_permissions: mapping.permissions,
            armed_permissions,
            state: Arm64TrapState::Armed,
        })
    }

    pub fn record_fault(&mut self, fault: Stage2Fault) -> Result<TrapAction, Arm64Error> {
        if self.state != Arm64TrapState::Armed && self.state != Arm64TrapState::Rearmed {
            return Err(Arm64Error::new(
                Arm64ErrorKind::UnsupportedTrap,
                "ARM64 Stage-2 trap fault arrived while trap was not armed",
            ));
        }
        if fault.ipa != Some(self.ipa) || fault.access != self.kind.access() {
            return Err(Arm64Error::new(
                Arm64ErrorKind::UnsupportedTrap,
                "ARM64 Stage-2 fault does not match the armed trap",
            ));
        }
        self.state = Arm64TrapState::Hit;
        Ok(TrapAction::Stage2PermissionTrap(fault.access))
    }

    pub fn open_write_window(&mut self) -> Result<Arm64WxEvent, Arm64Error> {
        if self.kind != Arm64TrapKind::Write || self.state != Arm64TrapState::Hit {
            return Err(Arm64Error::new(
                Arm64ErrorKind::UnsupportedTrap,
                "ARM64 write window requires a write trap hit",
            ));
        }
        self.state = Arm64TrapState::TemporaryWindow;
        Ok(Arm64WxEvent {
            ipa: self.ipa,
            previous_permissions: self.armed_permissions,
            window_permissions: self.original_permissions.with_write(),
            writable_executable: self.original_permissions.execute,
        })
    }

    pub fn rearm_with_tlbi(&mut self, vmid: Vmid) -> Result<TlbiPlan, Arm64Error> {
        self.state = Arm64TrapState::Rearmed;
        Ok(TlbiPlan::ipa(vmid, self.ipa))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arm64WxEvent {
    pub ipa: GuestPhysical,
    pub previous_permissions: Stage2Permissions,
    pub window_permissions: Stage2Permissions,
    pub writable_executable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::Granule;
    use crate::features::VmidBits;
    use crate::stage2::{Stage2MemoryAttr, Stage2Shareability};
    use crate::tlbi::Vmid;
    use aegishv_hypervisor_core::ids::HostPhysical;

    fn mapping() -> Stage2Mapping {
        Stage2Mapping::new(
            GuestPhysical::new(0x4000).unwrap(),
            HostPhysical::new(0x8000).unwrap(),
            4096,
            Granule::Size4K,
            Stage2Permissions::READ_WRITE_EXECUTE,
            Stage2MemoryAttr::NormalWriteBack,
            Stage2Shareability::InnerShareable,
        )
        .unwrap()
    }

    #[test]
    fn wfi_and_wfe_route_to_scheduler_actions() {
        assert_eq!(
            handle_sync_trap(EsrEl2::new(0x01 << 26), SmcPolicy::Deny).unwrap(),
            TrapAction::HaltVcpu
        );
        assert_eq!(
            handle_sync_trap(EsrEl2::new((0x01 << 26) | 1), SmcPolicy::Deny).unwrap(),
            TrapAction::YieldVcpu
        );
    }

    #[test]
    fn smc_policy_is_explicit() {
        assert_eq!(
            handle_sync_trap(EsrEl2::new(0x17 << 26), SmcPolicy::Deny).unwrap(),
            TrapAction::DenySmc
        );
    }

    #[test]
    fn write_trap_reports_wx_window_and_tlbi_rearm() {
        let mut trap = Arm64Stage2Trap::arm_write(mapping()).unwrap();
        let esr = EsrEl2::new((0x24 << 26) | (1 << 6) | 0b001101);
        let fault = Stage2Fault::decode(esr, 0, 0x40).unwrap();
        trap.record_fault(fault).unwrap();
        let event = trap.open_write_window().unwrap();
        let tlbi = trap
            .rearm_with_tlbi(Vmid::new(1, VmidBits::Bits8).unwrap())
            .unwrap();

        assert!(event.writable_executable);
        assert!(tlbi.needs_dsb_ish);
    }
}
