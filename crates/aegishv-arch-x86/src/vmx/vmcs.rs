use aegishv_hypervisor_core::ids::{GuestPhysical, GuestVirtual, HostPhysical};

use super::features::{
    is_canonical_u64, validate_control_register, CrFixedBits, VmxError, VmxErrorKind,
};
use super::instructions::VmxInstructionExecutor;
use super::region::{VmxRegion, VmxRevisionId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmcsField(u64);

impl VmcsField {
    pub const VIRTUAL_PROCESSOR_ID: Self = Self(0x0000);

    pub const GUEST_ES_SELECTOR: Self = Self(0x0800);
    pub const GUEST_CS_SELECTOR: Self = Self(0x0802);
    pub const GUEST_SS_SELECTOR: Self = Self(0x0804);
    pub const GUEST_DS_SELECTOR: Self = Self(0x0806);
    pub const GUEST_FS_SELECTOR: Self = Self(0x0808);
    pub const GUEST_GS_SELECTOR: Self = Self(0x080a);
    pub const GUEST_LDTR_SELECTOR: Self = Self(0x080c);
    pub const GUEST_TR_SELECTOR: Self = Self(0x080e);

    pub const HOST_ES_SELECTOR: Self = Self(0x0c00);
    pub const HOST_CS_SELECTOR: Self = Self(0x0c02);
    pub const HOST_SS_SELECTOR: Self = Self(0x0c04);
    pub const HOST_DS_SELECTOR: Self = Self(0x0c06);
    pub const HOST_FS_SELECTOR: Self = Self(0x0c08);
    pub const HOST_GS_SELECTOR: Self = Self(0x0c0a);
    pub const HOST_TR_SELECTOR: Self = Self(0x0c0c);

    pub const IO_BITMAP_A: Self = Self(0x2000);
    pub const IO_BITMAP_B: Self = Self(0x2002);
    pub const MSR_BITMAP: Self = Self(0x2004);
    pub const TSC_OFFSET: Self = Self(0x2010);
    pub const EPT_POINTER: Self = Self(0x201a);
    pub const VMCS_LINK_POINTER: Self = Self(0x2800);
    pub const GUEST_IA32_PAT: Self = Self(0x2804);
    pub const GUEST_IA32_EFER: Self = Self(0x2806);
    pub const HOST_IA32_PAT: Self = Self(0x2c00);
    pub const HOST_IA32_EFER: Self = Self(0x2c02);

    pub const HOST_CR0: Self = Self(0x6c00);
    pub const HOST_CR3: Self = Self(0x6c02);
    pub const HOST_CR4: Self = Self(0x6c04);
    pub const HOST_FS_BASE: Self = Self(0x6c06);
    pub const HOST_GS_BASE: Self = Self(0x6c08);
    pub const HOST_TR_BASE: Self = Self(0x6c0a);
    pub const HOST_GDTR_BASE: Self = Self(0x6c0c);
    pub const HOST_IDTR_BASE: Self = Self(0x6c0e);
    pub const HOST_IA32_SYSENTER_ESP: Self = Self(0x6c10);
    pub const HOST_IA32_SYSENTER_EIP: Self = Self(0x6c12);
    pub const HOST_RSP: Self = Self(0x6c14);
    pub const HOST_RIP: Self = Self(0x6c16);

    pub const GUEST_CR0: Self = Self(0x6800);
    pub const GUEST_CR3: Self = Self(0x6802);
    pub const GUEST_CR4: Self = Self(0x6804);
    pub const GUEST_ES_BASE: Self = Self(0x6806);
    pub const GUEST_CS_BASE: Self = Self(0x6808);
    pub const GUEST_SS_BASE: Self = Self(0x680a);
    pub const GUEST_DS_BASE: Self = Self(0x680c);
    pub const GUEST_FS_BASE: Self = Self(0x680e);
    pub const GUEST_GS_BASE: Self = Self(0x6810);
    pub const GUEST_LDTR_BASE: Self = Self(0x6812);
    pub const GUEST_TR_BASE: Self = Self(0x6814);
    pub const GUEST_GDTR_BASE: Self = Self(0x6816);
    pub const GUEST_IDTR_BASE: Self = Self(0x6818);
    pub const GUEST_DR7: Self = Self(0x681a);
    pub const GUEST_RSP: Self = Self(0x681c);
    pub const GUEST_RIP: Self = Self(0x681e);
    pub const GUEST_RFLAGS: Self = Self(0x6820);
    pub const GUEST_PENDING_DEBUG_EXCEPTIONS: Self = Self(0x6822);
    pub const GUEST_IA32_SYSENTER_ESP: Self = Self(0x6824);
    pub const GUEST_IA32_SYSENTER_EIP: Self = Self(0x6826);

    pub const PIN_BASED_CONTROLS: Self = Self(0x4000);
    pub const PRIMARY_PROCESSOR_CONTROLS: Self = Self(0x4002);
    pub const EXCEPTION_BITMAP: Self = Self(0x4004);
    pub const PAGE_FAULT_ERROR_CODE_MASK: Self = Self(0x4006);
    pub const PAGE_FAULT_ERROR_CODE_MATCH: Self = Self(0x4008);
    pub const CR3_TARGET_COUNT: Self = Self(0x400a);
    pub const EXIT_CONTROLS: Self = Self(0x400c);
    pub const VM_EXIT_MSR_STORE_COUNT: Self = Self(0x400e);
    pub const VM_EXIT_MSR_LOAD_COUNT: Self = Self(0x4010);
    pub const ENTRY_CONTROLS: Self = Self(0x4012);
    pub const VM_ENTRY_MSR_LOAD_COUNT: Self = Self(0x4014);
    pub const VM_ENTRY_INTERRUPTION_INFO: Self = Self(0x4016);
    pub const VM_ENTRY_EXCEPTION_ERROR_CODE: Self = Self(0x4018);
    pub const VM_ENTRY_INSTRUCTION_LENGTH: Self = Self(0x401a);
    pub const SECONDARY_PROCESSOR_CONTROLS: Self = Self(0x401e);

    pub const VM_INSTRUCTION_ERROR: Self = Self(0x4400);
    pub const VM_EXIT_REASON: Self = Self(0x4402);
    pub const VM_EXIT_INTERRUPTION_INFO: Self = Self(0x4404);
    pub const VM_EXIT_INSTRUCTION_LENGTH: Self = Self(0x440c);

    pub const GUEST_ES_LIMIT: Self = Self(0x4800);
    pub const GUEST_CS_LIMIT: Self = Self(0x4802);
    pub const GUEST_SS_LIMIT: Self = Self(0x4804);
    pub const GUEST_DS_LIMIT: Self = Self(0x4806);
    pub const GUEST_FS_LIMIT: Self = Self(0x4808);
    pub const GUEST_GS_LIMIT: Self = Self(0x480a);
    pub const GUEST_LDTR_LIMIT: Self = Self(0x480c);
    pub const GUEST_TR_LIMIT: Self = Self(0x480e);
    pub const GUEST_GDTR_LIMIT: Self = Self(0x4810);
    pub const GUEST_IDTR_LIMIT: Self = Self(0x4812);
    pub const GUEST_ES_ACCESS_RIGHTS: Self = Self(0x4814);
    pub const GUEST_CS_ACCESS_RIGHTS: Self = Self(0x4816);
    pub const GUEST_SS_ACCESS_RIGHTS: Self = Self(0x4818);
    pub const GUEST_DS_ACCESS_RIGHTS: Self = Self(0x481a);
    pub const GUEST_FS_ACCESS_RIGHTS: Self = Self(0x481c);
    pub const GUEST_GS_ACCESS_RIGHTS: Self = Self(0x481e);
    pub const GUEST_LDTR_ACCESS_RIGHTS: Self = Self(0x4820);
    pub const GUEST_TR_ACCESS_RIGHTS: Self = Self(0x4822);
    pub const GUEST_INTERRUPTIBILITY: Self = Self(0x4824);
    pub const GUEST_ACTIVITY_STATE: Self = Self(0x4826);
    pub const GUEST_SMBASE: Self = Self(0x4828);
    pub const GUEST_IA32_SYSENTER_CS: Self = Self(0x482a);
    pub const VMX_PREEMPTION_TIMER_VALUE: Self = Self(0x482e);
    pub const HOST_IA32_SYSENTER_CS: Self = Self(0x4c00);

    pub const CR0_GUEST_HOST_MASK: Self = Self(0x6000);
    pub const CR4_GUEST_HOST_MASK: Self = Self(0x6002);
    pub const CR0_READ_SHADOW: Self = Self(0x6004);
    pub const CR4_READ_SHADOW: Self = Self(0x6006);
    pub const EXIT_QUALIFICATION: Self = Self(0x6400);

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
