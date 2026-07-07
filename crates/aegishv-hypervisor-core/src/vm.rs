use crate::error::{CoreError, CoreErrorKind};
use crate::ids::VmId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmState {
    Created,
    Configured,
    Runnable,
    Running,
    Paused,
    Stopping,
    Stopped,
    Crashed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vm {
    pub id: VmId,
    pub state: VmState,
}

impl Vm {
    pub const fn new(id: VmId) -> Self {
        Self {
            id,
            state: VmState::Created,
        }
    }

    pub fn transition(&mut self, next: VmState) -> Result<(), CoreError> {
        if !allowed_transition(self.state, next) {
            return Err(CoreError::new(
                CoreErrorKind::InvalidTransition,
                "VM lifecycle transition is not allowed",
            ));
        }
        self.state = next;
        Ok(())
    }
}

pub const fn allowed_transition(from: VmState, to: VmState) -> bool {
    match (from, to) {
        (VmState::Created, VmState::Configured) => true,
        (VmState::Configured, VmState::Runnable) => true,
        (VmState::Runnable, VmState::Running) => true,
        (VmState::Running, VmState::Paused) => true,
        (VmState::Paused, VmState::Running) => true,
        (VmState::Running, VmState::Stopping) => true,
        (VmState::Paused, VmState::Stopping) => true,
        (VmState::Runnable, VmState::Stopping) => true,
        (VmState::Stopping, VmState::Stopped) => true,
        (VmState::Created, VmState::Crashed) => true,
        (VmState::Configured, VmState::Crashed) => true,
        (VmState::Runnable, VmState::Crashed) => true,
        (VmState::Running, VmState::Crashed) => true,
        (VmState::Paused, VmState::Crashed) => true,
        (VmState::Stopping, VmState::Crashed) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_lifecycle_accepts_normal_run_pause_stop_flow() {
        let mut vm = Vm::new(VmId::new(1).unwrap());

        for state in [
            VmState::Configured,
            VmState::Runnable,
            VmState::Running,
            VmState::Paused,
            VmState::Running,
            VmState::Stopping,
            VmState::Stopped,
        ] {
            vm.transition(state).unwrap();
        }

        assert_eq!(vm.state, VmState::Stopped);
    }

    #[test]
    fn vm_lifecycle_rejects_starting_before_configuration() {
        let mut vm = Vm::new(VmId::new(1).unwrap());

        assert_eq!(
            vm.transition(VmState::Running).unwrap_err().kind,
            CoreErrorKind::InvalidTransition
        );
    }

    #[test]
    fn vm_lifecycle_allows_crash_record_from_active_states() {
        let mut vm = Vm::new(VmId::new(1).unwrap());
        vm.transition(VmState::Configured).unwrap();
        vm.transition(VmState::Runnable).unwrap();

        vm.transition(VmState::Crashed).unwrap();

        assert_eq!(vm.state, VmState::Crashed);
    }
}
