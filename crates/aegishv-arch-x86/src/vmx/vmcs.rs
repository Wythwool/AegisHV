use aegishv_hypervisor_core::ids::{GuestPhysical, GuestVirtual, HostPhysical};

use super::features::{
    is_canonical_u64, validate_control_register, CrFixedBits, VmxError, VmxErrorKind,
};
use super::instructions::VmxInstructionExecutor;
use super::region::{VmxRegion, VmxRevisionId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmcsField(u64);

impl VmcsField {
    pub const HOST_CR0: Self = Self(0x6c00);
    pub const HOST_CR3: Self = Self(0x6c02);
    pub const HOST_CR4: Self = Self(0x6c04);
    pub const HOST_RSP: Self = Self(0x6c14);
    pub const HOST_RIP: Self = Self(0x6c16);

    pub const GUEST_CR0: Self = Self(0x6800);
    pub const GUEST_CR3: Self = Self(0x6802);
    pub const GUEST_CR4: Self = Self(0x6804);
    pub const GUEST_RSP: Self = Self(0x681c);
    pub const GUEST_RIP: Self = Self(0x681e);
    pub const GUEST_RFLAGS: Self = Self(0x6820);

    pub const PIN_BASED_CONTROLS: Self = Self(0x4000);
    pub const PRIMARY_PROCESSOR_CONTROLS: Self = Self(0x4002);
    pub const EXIT_CONTROLS: Self = Self(0x400c);
    pub const ENTRY_CONTROLS: Self = Self(0x4012);
    pub const SECONDARY_PROCESSOR_CONTROLS: Self = Self(0x401e);

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmcsLifecycleState {
    Allocated,
    Cleared,
    Loaded,
    Launched,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmcsRegion {
    region: VmxRegion,
    state: VmcsLifecycleState,
}

impl VmcsRegion {
    pub fn allocate(
        physical_address: HostPhysical,
        revision_id: VmxRevisionId,
    ) -> Result<Self, VmxError> {
        Ok(Self {
            region: VmxRegion::new(physical_address, revision_id)?,
            state: VmcsLifecycleState::Allocated,
        })
    }

    pub const fn physical_address(&self) -> HostPhysical {
        self.region.physical_address()
    }

    pub const fn revision_id(&self) -> VmxRevisionId {
        self.region.revision_id()
    }

    pub const fn state(&self) -> VmcsLifecycleState {
        self.state
    }

    /// # Safety
    ///
    /// The caller must ensure the current CPU is in VMX operation and that the
    /// VMCS region is owned exclusively by the current CPU while VMCLEAR runs.
    pub unsafe fn clear_with<E: VmxInstructionExecutor>(
        &mut self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        match self.state {
            VmcsLifecycleState::Allocated | VmcsLifecycleState::Loaded => {
                unsafe { executor.vmclear(self.region.physical_address())? };
                self.state = VmcsLifecycleState::Cleared;
                Ok(())
            }
            VmcsLifecycleState::Cleared => Ok(()),
            VmcsLifecycleState::Launched => Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "a launched VMCS must exit or be reset before VMCLEAR is modeled",
            )),
        }
    }

    /// # Safety
    ///
    /// The caller must ensure VMX operation is active on this CPU and that the
    /// VMCS region was cleared before loading.
    pub unsafe fn load_with<E: VmxInstructionExecutor>(
        &mut self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        if self.state != VmcsLifecycleState::Cleared {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "VMPTRLD requires a cleared VMCS region",
            ));
        }
        unsafe { executor.vmptrld(self.region.physical_address())? };
        self.state = VmcsLifecycleState::Loaded;
        Ok(())
    }

    pub fn mark_launched(&mut self) -> Result<(), VmxError> {
        if self.state != VmcsLifecycleState::Loaded {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "VMLAUNCH requires a loaded VMCS",
            ));
        }
        self.state = VmcsLifecycleState::Launched;
        Ok(())
    }

    /// # Safety
    ///
    /// The caller must fully initialize the current VMCS and make sure all
    /// guest, host, control, MSR, and entry/exit fields satisfy the processor's
    /// VM-entry checks before this method runs VMLAUNCH.
    pub unsafe fn launch_with<E: VmxInstructionExecutor>(
        &mut self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        if self.state != VmcsLifecycleState::Loaded {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "VMLAUNCH requires a loaded VMCS",
            ));
        }
        unsafe { executor.vmlaunch()? };
        self.state = VmcsLifecycleState::Launched;
        Ok(())
    }

    /// # Safety
    ///
    /// The caller must handle the previous VM exit and restore any host-side
    /// state required by the processor before resuming VMX non-root operation.
    pub unsafe fn resume_with<E: VmxInstructionExecutor>(
        &mut self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        if self.state != VmcsLifecycleState::Launched {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsState,
                "VMRESUME requires a launched VMCS",
            ));
        }
        unsafe { executor.vmresume()? };
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostState64 {
    pub cr0: u64,
    pub cr3: HostPhysical,
    pub cr4: u64,
    pub rsp: u64,
    pub rip: u64,
}

impl HostState64 {
    pub const fn validate(self) -> Result<Self, VmxError> {
        if !is_canonical_u64(self.rsp) || !is_canonical_u64(self.rip) {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS host RIP and RSP must be canonical addresses",
            ));
        }
        Ok(self)
    }

    /// # Safety
    ///
    /// The caller must ensure a current VMCS is loaded and the values were read
    /// from the same CPU that will enter VMX non-root operation.
    pub unsafe fn write_to<E: VmxInstructionExecutor>(
        self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        self.validate()?;
        unsafe {
            executor.vmwrite(VmcsField::HOST_CR0.raw(), self.cr0)?;
            executor.vmwrite(VmcsField::HOST_CR3.raw(), self.cr3.get())?;
            executor.vmwrite(VmcsField::HOST_CR4.raw(), self.cr4)?;
            executor.vmwrite(VmcsField::HOST_RSP.raw(), self.rsp)?;
            executor.vmwrite(VmcsField::HOST_RIP.raw(), self.rip)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuestState64 {
    pub cr0: u64,
    pub cr3: GuestPhysical,
    pub cr4: u64,
    pub rsp: u64,
    pub rip: GuestVirtual,
    pub rflags: u64,
}

impl GuestState64 {
    pub fn toy_64bit(
        cr0: u64,
        cr3: GuestPhysical,
        cr4: u64,
        rsp: u64,
        rip: GuestVirtual,
        cr0_fixed: CrFixedBits,
        cr4_fixed: CrFixedBits,
    ) -> Result<Self, VmxError> {
        validate_control_register(
            cr0,
            cr0_fixed,
            "guest CR0 violates IA32_VMX_CR0_FIXED0/FIXED1",
        )?;
        validate_control_register(
            cr4,
            cr4_fixed,
            "guest CR4 violates IA32_VMX_CR4_FIXED0/FIXED1",
        )?;
        if !is_canonical_u64(rsp) || !is_canonical_u64(rip.get()) {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS guest RIP and RSP must be canonical addresses",
            ));
        }
        Ok(Self {
            cr0,
            cr3,
            cr4,
            rsp,
            rip,
            rflags: 0x2,
        })
    }

    /// # Safety
    ///
    /// The caller must ensure a current VMCS is loaded and that the guest state
    /// matches the entry controls that will be written for the VM.
    pub unsafe fn write_to<E: VmxInstructionExecutor>(
        self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        unsafe {
            executor.vmwrite(VmcsField::GUEST_CR0.raw(), self.cr0)?;
            executor.vmwrite(VmcsField::GUEST_CR3.raw(), self.cr3.get())?;
            executor.vmwrite(VmcsField::GUEST_CR4.raw(), self.cr4)?;
            executor.vmwrite(VmcsField::GUEST_RSP.raw(), self.rsp)?;
            executor.vmwrite(VmcsField::GUEST_RIP.raw(), self.rip.get())?;
            executor.vmwrite(VmcsField::GUEST_RFLAGS.raw(), self.rflags)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vmx::instructions::tests_support::MockVmxInstructions;

    fn revision() -> VmxRevisionId {
        VmxRevisionId::new(0x33).unwrap()
    }

    #[test]
    fn vmcs_lifecycle_requires_clear_before_load_and_load_before_launch() {
        let mut vmcs =
            VmcsRegion::allocate(HostPhysical::new(0x8000).unwrap(), revision()).unwrap();
        let mut executor = MockVmxInstructions::default();

        let err = unsafe { vmcs.load_with(&mut executor) }.unwrap_err();
        assert_eq!(err.kind, VmxErrorKind::InvalidVmcsState);

        unsafe { vmcs.clear_with(&mut executor) }.unwrap();
        assert_eq!(vmcs.state(), VmcsLifecycleState::Cleared);
        unsafe { vmcs.load_with(&mut executor) }.unwrap();
        assert_eq!(vmcs.state(), VmcsLifecycleState::Loaded);
        unsafe { vmcs.launch_with(&mut executor) }.unwrap();
        assert_eq!(vmcs.state(), VmcsLifecycleState::Launched);
        assert_eq!(executor.launch_count, 1);
    }

    #[test]
    fn vmcs_resume_requires_a_launched_vmcs() {
        let mut vmcs =
            VmcsRegion::allocate(HostPhysical::new(0x8000).unwrap(), revision()).unwrap();
        let mut executor = MockVmxInstructions::default();

        let err = unsafe { vmcs.resume_with(&mut executor) }.unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::InvalidVmcsState);
    }

    #[test]
    fn vmcs_launch_failure_keeps_loaded_state() {
        let mut vmcs =
            VmcsRegion::allocate(HostPhysical::new(0x8000).unwrap(), revision()).unwrap();
        let mut executor = MockVmxInstructions::default();

        unsafe { vmcs.clear_with(&mut executor) }.unwrap();
        unsafe { vmcs.load_with(&mut executor) }.unwrap();
        executor.fail_next(crate::vmx::instructions::VmxInstruction::Vmlaunch);

        let err = unsafe { vmcs.launch_with(&mut executor) }.unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::InstructionFailed);
        assert_eq!(vmcs.state(), VmcsLifecycleState::Loaded);
    }

    #[test]
    fn vmcs_resume_runs_after_launch() {
        let mut vmcs =
            VmcsRegion::allocate(HostPhysical::new(0x8000).unwrap(), revision()).unwrap();
        let mut executor = MockVmxInstructions::default();

        unsafe { vmcs.clear_with(&mut executor) }.unwrap();
        unsafe { vmcs.load_with(&mut executor) }.unwrap();
        unsafe { vmcs.launch_with(&mut executor) }.unwrap();
        unsafe { vmcs.resume_with(&mut executor) }.unwrap();

        assert_eq!(executor.resume_count, 1);
        assert_eq!(vmcs.state(), VmcsLifecycleState::Launched);
    }

    #[test]
    fn host_state_rejects_noncanonical_stack_pointer() {
        let state = HostState64 {
            cr0: 0,
            cr3: HostPhysical::new(0x1000).unwrap(),
            cr4: 0,
            rsp: 0x0001_0000_0000_0000,
            rip: 0xffff_8000_0000_0000,
        };

        assert_eq!(
            state.validate().unwrap_err().kind,
            VmxErrorKind::InvalidGuestState
        );
    }

    #[test]
    fn toy_guest_state_rejects_bad_cr_fixed_bits() {
        let err = GuestState64::toy_64bit(
            0,
            GuestPhysical::new(0x1000).unwrap(),
            0,
            0x7000,
            GuestVirtual::new(0x100000),
            CrFixedBits::new(1, u64::MAX),
            CrFixedBits::new(0, u64::MAX),
        )
        .unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::InvalidGuestState);
    }

    #[test]
    fn guest_state_writer_uses_guest_vmcs_fields() {
        let state = GuestState64::toy_64bit(
            1,
            GuestPhysical::new(0x2000).unwrap(),
            0,
            0x7000,
            GuestVirtual::new(0x100000),
            CrFixedBits::new(1, u64::MAX),
            CrFixedBits::new(0, u64::MAX),
        )
        .unwrap();
        let mut executor = MockVmxInstructions::default();

        unsafe { state.write_to(&mut executor) }.unwrap();

        assert_eq!(
            executor.last_write,
            Some((VmcsField::GUEST_RFLAGS.raw(), 0x2))
        );
    }
}
