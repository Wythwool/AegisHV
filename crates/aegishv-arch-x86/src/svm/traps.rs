use aegishv_hypervisor_core::ids::GuestPhysical;

use super::features::{SvmError, SvmErrorKind};
use super::npt::{NptAccess, NptMapping, NptPermissions};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvmTrapKind {
    Execute,
    Write,
}

impl SvmTrapKind {
    const fn access(self) -> NptAccess {
        match self {
            Self::Execute => NptAccess::Execute,
            Self::Write => NptAccess::Write,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvmTrapState {
    Armed,
    Hit,
    TemporaryWindow,
    Rearmed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvmTrap {
    pub kind: SvmTrapKind,
    pub guest_physical: GuestPhysical,
    pub original_permissions: NptPermissions,
    pub armed_permissions: NptPermissions,
    pub state: SvmTrapState,
}

impl SvmTrap {
    pub fn arm_execute(mapping: NptMapping) -> Result<Self, SvmError> {
        Self::arm(mapping, SvmTrapKind::Execute)
    }

    pub fn arm_write(mapping: NptMapping) -> Result<Self, SvmError> {
        Self::arm(mapping, SvmTrapKind::Write)
    }

    fn arm(mapping: NptMapping, kind: SvmTrapKind) -> Result<Self, SvmError> {
        if !mapping.permissions.allows(kind.access()) {
            return Err(SvmError::new(
                SvmErrorKind::InvalidNptMapping,
                "SVM trap cannot remove an NPT access bit that is already absent",
            ));
        }
        let armed_permissions = match kind {
            SvmTrapKind::Execute => mapping.permissions.without_execute(),
            SvmTrapKind::Write => mapping.permissions.without_write(),
        };
        Ok(Self {
            kind,
            guest_physical: mapping.guest_physical,
            original_permissions: mapping.permissions,
            armed_permissions,
            state: SvmTrapState::Armed,
        })
    }

    pub fn record_hit(&mut self) -> Result<(), SvmError> {
        if self.state != SvmTrapState::Armed && self.state != SvmTrapState::Rearmed {
            return Err(SvmError::new(
                SvmErrorKind::InvalidVmcbState,
                "SVM trap hit arrived while the trap was not armed",
            ));
        }
        self.state = SvmTrapState::Hit;
        Ok(())
    }

    pub fn open_write_window(&mut self) -> Result<SvmWxEvent, SvmError> {
        if self.kind != SvmTrapKind::Write || self.state != SvmTrapState::Hit {
            return Err(SvmError::new(
                SvmErrorKind::InvalidVmcbState,
                "SVM write window requires a write trap hit",
            ));
        }
        self.state = SvmTrapState::TemporaryWindow;
        Ok(SvmWxEvent {
            guest_physical: self.guest_physical,
            previous_permissions: self.armed_permissions,
            window_permissions: self.original_permissions.with_write(),
            writable_executable: self.original_permissions.execute,
        })
    }

    pub fn rearm_after_step(&mut self) -> Result<(), SvmError> {
        if self.state != SvmTrapState::Hit && self.state != SvmTrapState::TemporaryWindow {
            return Err(SvmError::new(
                SvmErrorKind::InvalidVmcbState,
                "SVM trap can only re-arm after a hit or temporary window",
            ));
        }
        self.state = SvmTrapState::Rearmed;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvmWxEvent {
    pub guest_physical: GuestPhysical,
    pub previous_permissions: NptPermissions,
    pub window_permissions: NptPermissions,
    pub writable_executable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svm::npt::NptPageSize;
    use aegishv_hypervisor_core::ids::HostPhysical;

    fn mapping() -> NptMapping {
        NptMapping::new(
            GuestPhysical::new(0x1000).unwrap(),
            HostPhysical::new(0x2000).unwrap(),
            NptPageSize::Size4K,
            NptPermissions::READ_WRITE_EXECUTE,
        )
        .unwrap()
    }

    #[test]
    fn execute_trap_removes_execute_permission() {
        let trap = SvmTrap::arm_execute(mapping()).unwrap();

        assert!(trap.original_permissions.execute);
        assert!(!trap.armed_permissions.execute);
    }

    #[test]
    fn write_trap_reports_wx_window() {
        let mut trap = SvmTrap::arm_write(mapping()).unwrap();
        trap.record_hit().unwrap();
        let event = trap.open_write_window().unwrap();

        assert!(event.writable_executable);
        assert!(event.window_permissions.write);
    }

    #[test]
    fn trap_rejects_double_hit_without_rearm() {
        let mut trap = SvmTrap::arm_execute(mapping()).unwrap();
        trap.record_hit().unwrap();

        assert_eq!(
            trap.record_hit().unwrap_err().kind,
            SvmErrorKind::InvalidVmcbState
        );
    }
}
