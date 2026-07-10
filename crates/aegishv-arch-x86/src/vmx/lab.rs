use super::exits::VmxExitReason;
use super::features::{VmxError, VmxErrorKind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RequiredExitCoverage {
    pub cpuid: bool,
    pub hlt: bool,
    pub rdmsr: bool,
    pub wrmsr: bool,
    pub io_instruction: bool,
    pub cr_access: bool,
    pub ept_violation: bool,
    pub monitor_trap_flag: bool,
    pub preemption_timer: bool,
}

impl RequiredExitCoverage {
    pub const fn minimal_linux_lab() -> Self {
        Self {
            cpuid: true,
            hlt: true,
            rdmsr: true,
            wrmsr: true,
            io_instruction: true,
            cr_access: true,
            ept_violation: true,
            monitor_trap_flag: true,
            preemption_timer: true,
        }
    }

    pub fn mark(&mut self, reason: VmxExitReason) {
        match reason {
            VmxExitReason::Cpuid => self.cpuid = true,
            VmxExitReason::Hlt => self.hlt = true,
            VmxExitReason::Rdmsr => self.rdmsr = true,
            VmxExitReason::Wrmsr => self.wrmsr = true,
            VmxExitReason::IoInstruction => self.io_instruction = true,
            VmxExitReason::CrAccess => self.cr_access = true,
            VmxExitReason::EptViolation => self.ept_violation = true,
            VmxExitReason::MonitorTrapFlag => self.monitor_trap_flag = true,
            VmxExitReason::PreemptionTimer => self.preemption_timer = true,
            VmxExitReason::VmEntryFailure(_) => {}
            VmxExitReason::Unknown(_) => {}
        }
    }

    pub fn contains(self, required: Self) -> bool {
        (!required.cpuid || self.cpuid)
            && (!required.hlt || self.hlt)
            && (!required.rdmsr || self.rdmsr)
            && (!required.wrmsr || self.wrmsr)
            && (!required.io_instruction || self.io_instruction)
            && (!required.cr_access || self.cr_access)
            && (!required.ept_violation || self.ept_violation)
            && (!required.monitor_trap_flag || self.monitor_trap_flag)
            && (!required.preemption_timer || self.preemption_timer)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinuxGuestLabPlan {
    pub vmx_available: bool,
    pub ept_available: bool,
    pub vpid_available: bool,
    pub observed_exits: RequiredExitCoverage,
}

impl LinuxGuestLabPlan {
    pub const fn new(
        vmx_available: bool,
        ept_available: bool,
        vpid_available: bool,
        observed_exits: RequiredExitCoverage,
    ) -> Self {
        Self {
            vmx_available,
            ept_available,
            vpid_available,
            observed_exits,
        }
    }

    pub fn validate_minimal_linux_guest(self) -> Result<Self, VmxError> {
        if !self.vmx_available {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "minimal Linux VMX lab requires VMX capability",
            ));
        }
        if !self.ept_available {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "minimal Linux VMX lab requires EPT capability",
            ));
        }
        if !self.vpid_available {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "minimal Linux VMX lab requires VPID capability",
            ));
        }
        if !self
            .observed_exits
            .contains(RequiredExitCoverage::minimal_linux_lab())
        {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedExit,
                "minimal Linux VMX lab has not covered all required exits",
            ));
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_lab_plan_requires_vmx_ept_and_vpid() {
        let err =
            LinuxGuestLabPlan::new(false, true, true, RequiredExitCoverage::minimal_linux_lab())
                .validate_minimal_linux_guest()
                .unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::UnsupportedCapability);
    }

    #[test]
    fn linux_lab_plan_rejects_missing_required_exit_coverage() {
        let err = LinuxGuestLabPlan::new(true, true, true, RequiredExitCoverage::default())
            .validate_minimal_linux_guest()
            .unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::UnsupportedExit);
    }

    #[test]
    fn linux_lab_plan_accepts_explicit_coverage() {
        let mut coverage = RequiredExitCoverage::default();
        for reason in [
            VmxExitReason::Cpuid,
            VmxExitReason::Hlt,
            VmxExitReason::Rdmsr,
            VmxExitReason::Wrmsr,
            VmxExitReason::IoInstruction,
            VmxExitReason::CrAccess,
            VmxExitReason::EptViolation,
            VmxExitReason::MonitorTrapFlag,
            VmxExitReason::PreemptionTimer,
        ] {
            coverage.mark(reason);
        }

        assert!(LinuxGuestLabPlan::new(true, true, true, coverage)
            .validate_minimal_linux_guest()
            .is_ok());
    }
}
