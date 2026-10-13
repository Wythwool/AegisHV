use crate::features::{Arm64Error, Arm64ErrorKind};

pub const CNTHCTL_EL2_EL1PCTEN: u64 = 1 << 0;
pub const CNTHCTL_EL2_EL1PCEN: u64 = 1 << 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtualTimerState {
    pub cnthctl_el2: u64,
    pub cntvoff_el2: u64,
    pub cval: u64,
    pub enabled: bool,
    pub masked: bool,
}

impl VirtualTimerState {
    pub const fn new(cnthctl_el2: u64, cntvoff_el2: u64) -> Self {
        Self {
            cnthctl_el2,
            cntvoff_el2,
            cval: 0,
            enabled: false,
            masked: true,
        }
    }

    pub const fn el1_physical_timer_visible(self) -> bool {
        self.cnthctl_el2 & (CNTHCTL_EL2_EL1PCTEN | CNTHCTL_EL2_EL1PCEN)
            == (CNTHCTL_EL2_EL1PCTEN | CNTHCTL_EL2_EL1PCEN)
    }

    pub fn program(&mut self, cval: u64, now: u64) -> Result<(), Arm64Error> {
        if cval <= now {
            return Err(Arm64Error::new(
                Arm64ErrorKind::InvalidTimerState,
                "ARM64 virtual timer compare value must be in the future",
            ));
        }
        self.cval = cval;
        self.enabled = true;
        self.masked = false;
        Ok(())
    }

    pub const fn should_fire(self, now: u64) -> bool {
        self.enabled && !self.masked && now >= self.cval
    }

    pub fn handle_trap(&mut self, now: u64) -> Result<bool, Arm64Error> {
        if !self.enabled {
            return Err(Arm64Error::new(
                Arm64ErrorKind::InvalidTimerState,
                "ARM64 virtual timer trap arrived while timer is disabled",
            ));
        }
        Ok(self.should_fire(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_visibility_requires_both_cnthctl_bits() {
        let state = VirtualTimerState::new(CNTHCTL_EL2_EL1PCTEN, 0);
        assert!(!state.el1_physical_timer_visible());

        let state = VirtualTimerState::new(CNTHCTL_EL2_EL1PCTEN | CNTHCTL_EL2_EL1PCEN, 0);
        assert!(state.el1_physical_timer_visible());
    }

    #[test]
    fn timer_program_rejects_past_compare_value() {
        let mut state = VirtualTimerState::new(0, 0);

        assert_eq!(
            state.program(10, 10).unwrap_err().kind,
            Arm64ErrorKind::InvalidTimerState
        );
    }

    #[test]
    fn timer_trap_reports_pending_state() {
        let mut state = VirtualTimerState::new(0, 0);
        state.program(20, 10).unwrap();

        assert!(!state.handle_trap(19).unwrap());
        assert!(state.handle_trap(20).unwrap());
    }
}
