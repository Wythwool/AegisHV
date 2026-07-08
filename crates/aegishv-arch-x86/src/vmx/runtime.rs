use aegishv_hypervisor_core::ids::HostPhysical;

use super::features::{VmxError, VmxErrorKind};
use super::instructions::VmxInstructionExecutor;
use super::region::VmxonRegion;
use super::vmcs::{VmcsLifecycleState, VmcsRegion};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmxRuntimeState {
    Off,
    Vmxon,
    VmcsLoaded,
    GuestLaunched,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmxRuntime {
    vmxon: VmxonRegion,
    vmcs: VmcsRegion,
    state: VmxRuntimeState,
}

impl VmxRuntime {
    pub fn new(vmxon: VmxonRegion, vmcs: VmcsRegion) -> Result<Self, VmxError> {
        if vmxon.revision_id() != vmcs.revision_id() {
            return Err(VmxError::new(
                VmxErrorKind::InvalidRevisionId,
                "VMXON and VMCS regions must use the same VMCS revision id",
            ));
        }
        Ok(Self {
            vmxon,
            vmcs,
            state: VmxRuntimeState::Off,
        })
    }

    pub const fn state(&self) -> VmxRuntimeState {
        self.state
    }

    pub const fn vmxon_physical_address(&self) -> HostPhysical {
        self.vmxon.physical_address()
    }

    pub const fn vmcs_physical_address(&self) -> HostPhysical {
        self.vmcs.physical_address()
    }

    pub const fn vmcs_state(&self) -> VmcsLifecycleState {
        self.vmcs.state()
    }

    /// # Safety
    ///
    /// The caller must have validated CPUID, IA32_FEATURE_CONTROL, CR0/CR4 fixed
    /// bits, CR4.VMXE, and the identity mapping for the VMXON region on the
    /// current CPU before entering VMX operation.
    pub unsafe fn enter_vmx_operation<E: VmxInstructionExecutor>(
        &mut self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        if self.state != VmxRuntimeState::Off {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "VMXON requires the runtime to be off",
            ));
        }
        unsafe { executor.vmxon(self.vmxon.physical_address())? };
        self.state = VmxRuntimeState::Vmxon;
        Ok(())
    }

    /// # Safety
    ///
    /// The caller must be in VMX operation on the current CPU and must own the
    /// VMCS region exclusively while VMCLEAR and VMPTRLD run.
    pub unsafe fn load_vmcs<E: VmxInstructionExecutor>(
        &mut self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        if self.state != VmxRuntimeState::Vmxon {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "VMPTRLD requires VMX operation before loading the VMCS",
            ));
        }
        unsafe { self.vmcs.clear_with(executor)? };
        unsafe { self.vmcs.load_with(executor)? };
        self.state = VmxRuntimeState::VmcsLoaded;
        Ok(())
    }

    /// # Safety
    ///
    /// The caller must write all host, guest, control, MSR, and entry/exit VMCS
    /// fields needed by the processor before launching the guest.
    pub unsafe fn launch_guest<E: VmxInstructionExecutor>(
        &mut self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        if self.state != VmxRuntimeState::VmcsLoaded {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "VMLAUNCH requires a loaded VMCS runtime",
            ));
        }
        unsafe { self.vmcs.launch_with(executor)? };
        self.state = VmxRuntimeState::GuestLaunched;
        Ok(())
    }

    /// # Safety
    ///
    /// The caller must handle the last VM exit and restore host-side state
    /// required by the architecture before resuming the guest.
    pub unsafe fn resume_guest<E: VmxInstructionExecutor>(
        &mut self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        if self.state != VmxRuntimeState::GuestLaunched {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "VMRESUME requires a launched guest runtime",
            ));
        }
        unsafe { self.vmcs.resume_with(executor)? };
        Ok(())
    }

    /// # Safety
    ///
    /// The caller must only run this on the CPU that entered VMX operation and
    /// must have stopped using any current VMCS owned by that CPU.
    pub unsafe fn leave_vmx_operation<E: VmxInstructionExecutor>(
        &mut self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        if self.state == VmxRuntimeState::Off {
            return Ok(());
        }
        unsafe { executor.vmxoff()? };
        self.state = VmxRuntimeState::Off;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vmx::instructions::{tests_support::MockVmxInstructions, VmxInstruction};
    use crate::vmx::region::VmxRevisionId;

    fn revision() -> VmxRevisionId {
        VmxRevisionId::new(0x33).unwrap()
    }

    fn runtime() -> VmxRuntime {
        let vmxon =
            VmxonRegion::new(HostPhysical::new(0x4000).unwrap(), revision()).unwrap();
        let vmcs = VmcsRegion::allocate(HostPhysical::new(0x8000).unwrap(), revision()).unwrap();
        VmxRuntime::new(vmxon, vmcs).unwrap()
    }

    #[test]
    fn runtime_requires_matching_revision_ids() {
        let vmxon =
            VmxonRegion::new(HostPhysical::new(0x4000).unwrap(), revision()).unwrap();
        let vmcs = VmcsRegion::allocate(
            HostPhysical::new(0x8000).unwrap(),
            VmxRevisionId::new(0x44).unwrap(),
        )
        .unwrap();

        let err = VmxRuntime::new(vmxon, vmcs).unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::InvalidRevisionId);
    }

    #[test]
    fn runtime_enters_loads_launches_and_resumes_guest() {
        let mut runtime = runtime();
        let mut executor = MockVmxInstructions::default();

        unsafe { runtime.enter_vmx_operation(&mut executor) }.unwrap();
        unsafe { runtime.load_vmcs(&mut executor) }.unwrap();
        unsafe { runtime.launch_guest(&mut executor) }.unwrap();
        unsafe { runtime.resume_guest(&mut executor) }.unwrap();

        assert_eq!(executor.vmxon_region.unwrap().get(), 0x4000);
        assert_eq!(executor.current_vmcs.unwrap().get(), 0x8000);
        assert_eq!(executor.launch_count, 1);
        assert_eq!(executor.resume_count, 1);
        assert_eq!(runtime.state(), VmxRuntimeState::GuestLaunched);
        assert_eq!(runtime.vmcs_state(), VmcsLifecycleState::Launched);
    }

    #[test]
    fn runtime_refuses_launch_before_vmcs_load() {
        let mut runtime = runtime();
        let mut executor = MockVmxInstructions::default();

        unsafe { runtime.enter_vmx_operation(&mut executor) }.unwrap();
        let err = unsafe { runtime.launch_guest(&mut executor) }.unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::InvalidVmcsState);
        assert_eq!(runtime.state(), VmxRuntimeState::Vmxon);
    }

    #[test]
    fn runtime_keeps_loaded_state_when_launch_fails() {
        let mut runtime = runtime();
        let mut executor = MockVmxInstructions::default();

        unsafe { runtime.enter_vmx_operation(&mut executor) }.unwrap();
        unsafe { runtime.load_vmcs(&mut executor) }.unwrap();
        executor.fail_next(VmxInstruction::Vmlaunch);

        let err = unsafe { runtime.launch_guest(&mut executor) }.unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::InstructionFailed);
        assert_eq!(runtime.state(), VmxRuntimeState::VmcsLoaded);
        assert_eq!(runtime.vmcs_state(), VmcsLifecycleState::Loaded);
    }

    #[test]
    fn runtime_leave_runs_vmxoff() {
        let mut runtime = runtime();
        let mut executor = MockVmxInstructions::default();

        unsafe { runtime.enter_vmx_operation(&mut executor) }.unwrap();
        unsafe { runtime.leave_vmx_operation(&mut executor) }.unwrap();

        assert_eq!(runtime.state(), VmxRuntimeState::Off);
        assert!(executor.vmxon_region.is_none());
    }
}
