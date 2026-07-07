use aegishv_hypervisor_core::ids::HostPhysical;

use super::exits::{handle_hlt, GeneralRegisters, SvmExitAction, SvmExitCode};
use super::features::{EferValue, SvmError, SvmErrorKind, SvmFeatureSet};
use super::instructions::SvmInstructionExecutor;
use super::vmcb::{Vmcb, INTERCEPT_HLT};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RequiredInterceptCoverage {
    pub cpuid: bool,
    pub msr: bool,
    pub cr: bool,
    pub io: bool,
    pub hlt: bool,
    pub pause: bool,
    pub nested_page_fault: bool,
}

impl RequiredInterceptCoverage {
    pub const fn tiny_guest() -> Self {
        Self {
            cpuid: true,
            msr: true,
            cr: true,
            io: true,
            hlt: true,
            pause: true,
            nested_page_fault: true,
        }
    }

    pub fn mark(&mut self, code: SvmExitCode) {
        match code {
            SvmExitCode::Cpuid => self.cpuid = true,
            SvmExitCode::Msr => self.msr = true,
            SvmExitCode::CrRead(_) | SvmExitCode::CrWrite(_) => self.cr = true,
            SvmExitCode::Ioio => self.io = true,
            SvmExitCode::Hlt => self.hlt = true,
            SvmExitCode::Pause => self.pause = true,
            SvmExitCode::NestedPageFault => self.nested_page_fault = true,
            SvmExitCode::Unknown(_) => {}
        }
    }

    pub fn contains(self, required: Self) -> bool {
        (!required.cpuid || self.cpuid)
            && (!required.msr || self.msr)
            && (!required.cr || self.cr)
            && (!required.io || self.io)
            && (!required.hlt || self.hlt)
            && (!required.pause || self.pause)
            && (!required.nested_page_fault || self.nested_page_fault)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvmTinyGuestPlan {
    pub features: SvmFeatureSet,
    pub intercepts: RequiredInterceptCoverage,
    pub vmcb_physical: HostPhysical,
}

impl SvmTinyGuestPlan {
    pub fn new(
        features: SvmFeatureSet,
        intercepts: RequiredInterceptCoverage,
        vmcb_physical: HostPhysical,
    ) -> Result<Self, SvmError> {
        features.validate_for_npt_lab()?;
        Vmcb::validate_physical_address(vmcb_physical)?;
        if !intercepts.contains(RequiredInterceptCoverage::tiny_guest()) {
            return Err(SvmError::new(
                SvmErrorKind::UnsupportedExit,
                "tiny SVM lab has not covered all required intercepts",
            ));
        }
        Ok(Self {
            features,
            intercepts,
            vmcb_physical,
        })
    }

    /// # Safety
    ///
    /// The caller must use this only in an opt-in hardware lab. The VMCB must
    /// belong to the current CPU, and host state must be recoverable after VMRUN.
    pub unsafe fn run_once_with<E: SvmInstructionExecutor>(
        self,
        executor: &mut E,
        vmcb: &mut Vmcb,
        regs: &mut GeneralRegisters,
    ) -> Result<SvmExitAction, SvmError> {
        vmcb.require_hlt_intercept()?;
        unsafe {
            executor.enable_svme(EferValue::new(vmcb.state.efer()))?;
            executor.vmload(self.vmcb_physical)?;
            executor.vmrun(self.vmcb_physical)?;
            executor.vmsave(self.vmcb_physical)?;
        }
        match SvmExitCode::from_raw(vmcb.control.exit_code()) {
            SvmExitCode::Hlt => handle_hlt(regs, 1),
            _ => Err(SvmError::new(
                SvmErrorKind::UnsupportedExit,
                "tiny SVM lab run only handles the HLT intercept",
            )),
        }
    }
}

pub fn prepare_tiny_hlt_vmcb(asid: u32, nested_root: HostPhysical) -> Vmcb {
    let mut vmcb = Vmcb::zeroed();
    vmcb.control.set_guest_asid(asid);
    vmcb.control.set_misc_intercepts(INTERCEPT_HLT);
    vmcb.control.set_nested_paging(true, nested_root);
    vmcb.state.set_efer(EferValue::new(0).with_svme().raw());
    vmcb.state.set_rflags(0x2);
    vmcb
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svm::features::{SvmCpuidExt1, SvmCpuidLeaf, CPUID_EXT1_ECX_SVM, CPUID_SVM_EDX_NPT};
    use crate::svm::instructions::tests_support::MockSvmInstructions;

    fn features() -> SvmFeatureSet {
        SvmFeatureSet::from_cpuid(
            SvmCpuidExt1 {
                ecx: CPUID_EXT1_ECX_SVM,
            },
            SvmCpuidLeaf {
                ebx: 8,
                edx: CPUID_SVM_EDX_NPT,
            },
        )
    }

    #[test]
    fn tiny_guest_plan_rejects_missing_intercepts() {
        let err = SvmTinyGuestPlan::new(
            features(),
            RequiredInterceptCoverage::default(),
            HostPhysical::new(0x8000).unwrap(),
        )
        .unwrap_err();

        assert_eq!(err.kind, SvmErrorKind::UnsupportedExit);
    }

    #[test]
    fn tiny_guest_run_handles_hlt_exit_with_mock_executor() {
        let plan = SvmTinyGuestPlan::new(
            features(),
            RequiredInterceptCoverage::tiny_guest(),
            HostPhysical::new(0x8000).unwrap(),
        )
        .unwrap();
        let mut vmcb = prepare_tiny_hlt_vmcb(1, HostPhysical::new(0x9000).unwrap());
        vmcb.control
            .set_exit_code(super::super::exits::SVM_EXIT_HLT);
        let mut regs = GeneralRegisters {
            rip: 0x1000,
            ..Default::default()
        };
        let mut executor = MockSvmInstructions::default();

        let action = unsafe { plan.run_once_with(&mut executor, &mut vmcb, &mut regs) }.unwrap();

        assert_eq!(action, SvmExitAction::HaltGuest);
        assert_eq!(regs.rip, 0x1001);
        assert_eq!(executor.last_vmrun.unwrap().get(), 0x8000);
    }
}
