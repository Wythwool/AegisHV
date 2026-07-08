use aegishv_hypervisor_core::ids::{GuestVirtual, HostPhysical};

use super::asid::SvmAsid;
use super::features::{EferValue, SvmError, SvmErrorKind};
use super::instructions::SvmInstructionExecutor;
use super::vmcb::Vmcb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvmRuntimeState {
    Off,
    SvmeEnabled,
    VmcbLoaded,
    GuestExited,
    HostStateSaved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvmRuntime {
    vmcb_physical: HostPhysical,
    efer: EferValue,
    state: SvmRuntimeState,
}

impl SvmRuntime {
    pub fn new(vmcb_physical: HostPhysical) -> Result<Self, SvmError> {
        Ok(Self {
            vmcb_physical: Vmcb::validate_physical_address(vmcb_physical)?,
            efer: EferValue::new(0),
            state: SvmRuntimeState::Off,
        })
    }

    pub const fn vmcb_physical_address(&self) -> HostPhysical {
        self.vmcb_physical
    }

    pub const fn efer(&self) -> EferValue {
        self.efer
    }

    pub const fn state(&self) -> SvmRuntimeState {
        self.state
    }

    /// # Safety
    ///
    /// The caller must have validated CPUID SVM support and must run this on the
    /// CPU that will own the following VMCB operations.
    pub unsafe fn enable_svme<E: SvmInstructionExecutor>(
        &mut self,
        executor: &mut E,
        efer: EferValue,
    ) -> Result<EferValue, SvmError> {
        if self.state != SvmRuntimeState::Off {
            return Err(SvmError::new(
                SvmErrorKind::InvalidVmcbState,
                "EFER.SVME setup requires the SVM runtime to be off",
            ));
        }
        let enabled = unsafe { executor.enable_svme(efer)? };
        self.efer = enabled;
        self.state = SvmRuntimeState::SvmeEnabled;
        Ok(enabled)
    }

    /// # Safety
    ///
    /// The caller must prepare a valid VMCB for this CPU before VMLOAD runs.
    pub unsafe fn load_vmcb<E: SvmInstructionExecutor>(
        &mut self,
        executor: &mut E,
    ) -> Result<(), SvmError> {
        match self.state {
            SvmRuntimeState::SvmeEnabled | SvmRuntimeState::HostStateSaved => {
                unsafe { executor.vmload(self.vmcb_physical)? };
                self.state = SvmRuntimeState::VmcbLoaded;
                Ok(())
            }
            _ => Err(SvmError::new(
                SvmErrorKind::InvalidVmcbState,
                "VMLOAD requires EFER.SVME and a loadable VMCB state",
            )),
        }
    }

    /// # Safety
    ///
    /// The caller must ensure the VMCB is complete, host state is recoverable,
    /// and any intercept policy needed for the guest is already installed.
    pub unsafe fn run_guest_once<E: SvmInstructionExecutor>(
        &mut self,
        executor: &mut E,
    ) -> Result<(), SvmError> {
        if self.state != SvmRuntimeState::VmcbLoaded {
            return Err(SvmError::new(
                SvmErrorKind::InvalidVmcbState,
                "VMRUN requires a loaded VMCB",
            ));
        }
        unsafe { executor.vmrun(self.vmcb_physical)? };
        self.state = SvmRuntimeState::GuestExited;
        Ok(())
    }

    /// # Safety
    ///
    /// The caller must call this on the same CPU after a VMRUN return path when
    /// host state must be saved back to the VMCB.
    pub unsafe fn save_host_state<E: SvmInstructionExecutor>(
        &mut self,
        executor: &mut E,
    ) -> Result<(), SvmError> {
        if self.state != SvmRuntimeState::GuestExited {
            return Err(SvmError::new(
                SvmErrorKind::InvalidVmcbState,
                "VMSAVE requires a returned VMRUN path",
            ));
        }
        unsafe { executor.vmsave(self.vmcb_physical)? };
        self.state = SvmRuntimeState::HostStateSaved;
        Ok(())
    }

    /// # Safety
    ///
    /// The caller must invalidate on the CPU that owns the ASID and must ensure
    /// the guest virtual address belongs to that ASID's address space.
    pub unsafe fn invalidate_address<E: SvmInstructionExecutor>(
        &self,
        executor: &mut E,
        asid: SvmAsid,
        guest_virtual: GuestVirtual,
    ) -> Result<(), SvmError> {
        if self.state == SvmRuntimeState::Off {
            return Err(SvmError::new(
                SvmErrorKind::InvalidVmcbState,
                "INVLPGA requires EFER.SVME before invalidation",
            ));
        }
        unsafe { executor.invlpga(guest_virtual.get(), asid.get()) }
    }

    /// # Safety
    ///
    /// The caller must invalidate on the CPU that owns the ASID.
    pub unsafe fn invalidate_asid<E: SvmInstructionExecutor>(
        &self,
        executor: &mut E,
        asid: SvmAsid,
    ) -> Result<(), SvmError> {
        if self.state == SvmRuntimeState::Off {
            return Err(SvmError::new(
                SvmErrorKind::InvalidVmcbState,
                "INVLPGA requires EFER.SVME before invalidation",
            ));
        }
        unsafe { executor.invlpga(0, asid.get()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svm::instructions::{tests_support::MockSvmInstructions, SvmInstruction};

    fn runtime() -> SvmRuntime {
        SvmRuntime::new(HostPhysical::new(0x8000).unwrap()).unwrap()
    }

    #[test]
    fn runtime_rejects_bad_vmcb_address() {
        let err = SvmRuntime::new(HostPhysical::new(0x8100).unwrap()).unwrap_err();

        assert_eq!(err.kind, SvmErrorKind::InvalidVmcbAddress);
    }

    #[test]
    fn runtime_enables_loads_runs_saves_and_invalidates() {
        let mut runtime = runtime();
        let mut executor = MockSvmInstructions::default();
        let asid = SvmAsid::new(3).unwrap();

        let enabled = unsafe { runtime.enable_svme(&mut executor, EferValue::new(0x500)) }.unwrap();
        unsafe { runtime.load_vmcb(&mut executor) }.unwrap();
        unsafe { runtime.run_guest_once(&mut executor) }.unwrap();
        unsafe { runtime.save_host_state(&mut executor) }.unwrap();
        unsafe { runtime.invalidate_address(&mut executor, asid, GuestVirtual::new(0x4000)) }
            .unwrap();

        assert!(enabled.svme_enabled());
        assert_eq!(runtime.state(), SvmRuntimeState::HostStateSaved);
        assert_eq!(executor.enable_count, 1);
        assert_eq!(executor.vmload_count, 1);
        assert_eq!(executor.vmrun_count, 1);
        assert_eq!(executor.vmsave_count, 1);
        assert_eq!(executor.invlpga_count, 1);
        assert_eq!(executor.last_invlpga, Some((0x4000, 3)));
    }

    #[test]
    fn runtime_refuses_run_before_vmload() {
        let mut runtime = runtime();
        let mut executor = MockSvmInstructions::default();

        unsafe { runtime.enable_svme(&mut executor, EferValue::new(0)) }.unwrap();
        let err = unsafe { runtime.run_guest_once(&mut executor) }.unwrap_err();

        assert_eq!(err.kind, SvmErrorKind::InvalidVmcbState);
        assert_eq!(runtime.state(), SvmRuntimeState::SvmeEnabled);
    }

    #[test]
    fn runtime_keeps_loaded_state_when_vmrun_fails() {
        let mut runtime = runtime();
        let mut executor = MockSvmInstructions::default();

        unsafe { runtime.enable_svme(&mut executor, EferValue::new(0)) }.unwrap();
        unsafe { runtime.load_vmcb(&mut executor) }.unwrap();
        executor.fail_next(SvmInstruction::Vmrun);

        let err = unsafe { runtime.run_guest_once(&mut executor) }.unwrap_err();

        assert_eq!(err.kind, SvmErrorKind::InstructionFailed);
        assert_eq!(runtime.state(), SvmRuntimeState::VmcbLoaded);
    }

    #[test]
    fn runtime_keeps_exit_state_when_vmsave_fails() {
        let mut runtime = runtime();
        let mut executor = MockSvmInstructions::default();

        unsafe { runtime.enable_svme(&mut executor, EferValue::new(0)) }.unwrap();
        unsafe { runtime.load_vmcb(&mut executor) }.unwrap();
        unsafe { runtime.run_guest_once(&mut executor) }.unwrap();
        executor.fail_next(SvmInstruction::Vmsave);

        let err = unsafe { runtime.save_host_state(&mut executor) }.unwrap_err();

        assert_eq!(err.kind, SvmErrorKind::InstructionFailed);
        assert_eq!(runtime.state(), SvmRuntimeState::GuestExited);
    }

    #[test]
    fn runtime_rejects_invalidation_before_svme() {
        let runtime = runtime();
        let mut executor = MockSvmInstructions::default();
        let asid = SvmAsid::new(1).unwrap();

        let err = unsafe { runtime.invalidate_asid(&mut executor, asid) }.unwrap_err();

        assert_eq!(err.kind, SvmErrorKind::InvalidVmcbState);
        assert_eq!(executor.invlpga_count, 0);
    }
}
