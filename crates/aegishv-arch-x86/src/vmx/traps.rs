use aegishv_hypervisor_core::ids::GuestPhysical;

use super::controls::PRIMARY_MONITOR_TRAP_FLAG;
use super::ept::{EptAccess, EptMapping, EptPermissions};
use super::features::{VmxError, VmxErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmxTrapKind {
    Execute,
    Write,
}

impl VmxTrapKind {
    const fn access(self) -> EptAccess {
        match self {
            Self::Execute => EptAccess::Execute,
            Self::Write => EptAccess::Write,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmxTrapState {
    Armed,
    Hit,
    TemporaryWindow,
    AwaitingSingleStep,
    Rearmed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmxTrap {
    pub kind: VmxTrapKind,
    pub guest_physical: GuestPhysical,
    pub original_permissions: EptPermissions,
    pub armed_permissions: EptPermissions,
    pub state: VmxTrapState,
}

impl VmxTrap {
    pub fn arm_execute(mapping: EptMapping) -> Result<Self, VmxError> {
        Self::arm(mapping, VmxTrapKind::Execute)
    }

    pub fn arm_write(mapping: EptMapping) -> Result<Self, VmxError> {
        Self::arm(mapping, VmxTrapKind::Write)
    }

    fn arm(mapping: EptMapping, kind: VmxTrapKind) -> Result<Self, VmxError> {
        if !mapping.permissions.allows(kind.access()) {
            return Err(VmxError::new(
                VmxErrorKind::InvalidEptMapping,
                "VMX trap cannot remove an access bit that is already absent",
            ));
        }
        let armed_permissions = match kind {
            VmxTrapKind::Execute => mapping.permissions.without_execute(),
            VmxTrapKind::Write => mapping.permissions.without_write(),
        };
        Ok(Self {
            kind,
            guest_physical: mapping.guest_physical,
            original_permissions: mapping.permissions,
            armed_permissions,
            state: VmxTrapState::Armed,
        })
    }

    pub fn record_hit(&mut self) -> Result<(), VmxError> {
        if self.state != VmxTrapState::Armed && self.state != VmxTrapState::Rearmed {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "VMX trap hit arrived while the trap was not armed",
            ));
        }
        self.state = VmxTrapState::Hit;
        Ok(())
    }

    pub fn open_temporary_write_window(&mut self) -> Result<WxEnforcementEvent, VmxError> {
        if self.kind != VmxTrapKind::Write || self.state != VmxTrapState::Hit {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "temporary write window requires a write trap hit",
            ));
        }
        self.state = VmxTrapState::TemporaryWindow;
        Ok(WxEnforcementEvent {
            guest_physical: self.guest_physical,
            previous_permissions: self.armed_permissions,
            window_permissions: self.original_permissions.with_write(),
            writable_executable: self.original_permissions.execute,
        })
    }

    pub fn await_single_step(&mut self, mtf: &mut MonitorTrapFlag) -> Result<(), VmxError> {
        mtf.arm_for(self.guest_physical)?;
        self.state = VmxTrapState::AwaitingSingleStep;
        Ok(())
    }

    pub fn rearm_after_step(&mut self, mtf: &mut MonitorTrapFlag) -> Result<(), VmxError> {
        mtf.complete_for(self.guest_physical)?;
        self.state = VmxTrapState::Rearmed;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WxEnforcementEvent {
    pub guest_physical: GuestPhysical,
    pub previous_permissions: EptPermissions,
    pub window_permissions: EptPermissions,
    pub writable_executable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonitorTrapFlag {
    supported: bool,
    active_gpa: Option<GuestPhysical>,
}

impl MonitorTrapFlag {
    pub const fn from_primary_controls(primary_controls: u32) -> Self {
        Self {
            supported: primary_controls & PRIMARY_MONITOR_TRAP_FLAG != 0,
            active_gpa: None,
        }
    }

    pub const fn unsupported() -> Self {
        Self {
            supported: false,
            active_gpa: None,
        }
    }

    pub const fn supported() -> Self {
        Self {
            supported: true,
            active_gpa: None,
        }
    }

    pub fn arm_for(&mut self, guest_physical: GuestPhysical) -> Result<(), VmxError> {
        if !self.supported {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "Monitor Trap Flag is not available in the current VMX controls",
            ));
        }
        if self.active_gpa.is_some() {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "Monitor Trap Flag is already armed for another trap",
            ));
        }
        self.active_gpa = Some(guest_physical);
        Ok(())
    }

    pub fn complete_for(&mut self, guest_physical: GuestPhysical) -> Result<(), VmxError> {
        if self.active_gpa != Some(guest_physical) {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "Monitor Trap Flag completion did not match the active trap",
            ));
        }
        self.active_gpa = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vmx::ept::{EptMemoryType, EptPageSize};
    use aegishv_hypervisor_core::ids::HostPhysical;

    fn executable_mapping() -> EptMapping {
        EptMapping::new(
            GuestPhysical::new(0x1000).unwrap(),
            HostPhysical::new(0x2000).unwrap(),
            EptPageSize::Size4K,
            EptPermissions::READ_WRITE_EXECUTE,
            EptMemoryType::WriteBack,
        )
        .unwrap()
    }

    #[test]
    fn execute_trap_removes_execute_permission() {
        let trap = VmxTrap::arm_execute(executable_mapping()).unwrap();

        assert!(trap.original_permissions.execute);
        assert!(!trap.armed_permissions.execute);
    }

    #[test]
    fn write_trap_reports_wx_temporary_window() {
        let mut trap = VmxTrap::arm_write(executable_mapping()).unwrap();
        trap.record_hit().unwrap();

        let event = trap.open_temporary_write_window().unwrap();

        assert!(event.writable_executable);
        assert!(event.window_permissions.write);
    }

    #[test]
    fn mtf_reports_unsupported_fallback() {
        let mut mtf = MonitorTrapFlag::unsupported();
        let mut trap = VmxTrap::arm_execute(executable_mapping()).unwrap();
        trap.record_hit().unwrap();

        assert_eq!(
            trap.await_single_step(&mut mtf).unwrap_err().kind,
            VmxErrorKind::UnsupportedCapability
        );
    }

    #[test]
    fn mtf_support_comes_from_primary_controls() {
        assert!(MonitorTrapFlag::from_primary_controls(PRIMARY_MONITOR_TRAP_FLAG).supported);
        assert!(!MonitorTrapFlag::from_primary_controls(0).supported);
    }

    #[test]
    fn mtf_single_step_rearms_matching_trap() {
        let mut mtf = MonitorTrapFlag::supported();
        let mut trap = VmxTrap::arm_execute(executable_mapping()).unwrap();
        trap.record_hit().unwrap();
        trap.await_single_step(&mut mtf).unwrap();
        trap.rearm_after_step(&mut mtf).unwrap();

        assert_eq!(trap.state, VmxTrapState::Rearmed);
    }
}
