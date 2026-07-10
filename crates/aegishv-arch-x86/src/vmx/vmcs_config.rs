use aegishv_hypervisor_core::ids::{GuestPhysical, GuestVirtual, HostPhysical};

use super::controls::{
    VmxControlFields, ENTRY_IA32E_MODE_GUEST, ENTRY_LOAD_IA32_EFER, ENTRY_LOAD_IA32_PAT,
    EXIT_HOST_ADDRESS_SPACE_SIZE, EXIT_LOAD_IA32_EFER, EXIT_LOAD_IA32_PAT, EXIT_SAVE_IA32_EFER,
    EXIT_SAVE_IA32_PAT, PIN_BASED_NMI_EXITING, PIN_BASED_VMX_PREEMPTION_TIMER,
    PRIMARY_ACTIVATE_SECONDARY_CONTROLS, PRIMARY_HLT_EXITING, PRIMARY_USE_IO_BITMAPS,
    PRIMARY_USE_MSR_BITMAPS, SECONDARY_ENABLE_EPT, SECONDARY_ENABLE_VPID,
};
use super::ept::EptPointer;
use super::features::{
    is_canonical_u64, validate_control_register, CrFixedBits, VmxError, VmxErrorKind,
};
use super::instructions::VmxInstructionExecutor;
use super::vmcs::VmcsField;

pub const VMX_CR0_PROTECTED_MODE_ENABLE: u64 = 1 << 0;
pub const VMX_CR0_MONITOR_COPROCESSOR: u64 = 1 << 1;
pub const VMX_CR0_EMULATION: u64 = 1 << 2;
pub const VMX_CR0_TASK_SWITCHED: u64 = 1 << 3;
pub const VMX_CR0_NUMERIC_ERROR: u64 = 1 << 5;
pub const VMX_CR0_PAGING: u64 = 1 << 31;
pub const VMX_CR4_PAE: u64 = 1 << 5;
pub const VMX_CR4_OSFXSR: u64 = 1 << 9;
pub const VMX_CR4_VMXE: u64 = 1 << 13;
pub const VMX_EFER_LME: u64 = 1 << 8;
pub const VMX_EFER_LMA: u64 = 1 << 10;
pub const VMX_INTERCEPTION_BITMAP_PAGE_SIZE: u64 = 4096;
pub const VMX_INTERCEPTION_BITMAP_MAX_PHYSICAL_EXCLUSIVE: u64 = 1_u64 << 32;
pub const VMX_TOY_EXCEPTION_BITMAP: u32 = u32::MAX;
pub const VMX_TOY_GUEST_PAT_RAW: u64 = 0x0006_0705_0401_0006;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmxPat(u64);

impl VmxPat {
    pub const fn new(raw: u64) -> Result<Self, VmxError> {
        let mut remaining = raw;
        let mut index = 0;
        while index < 8 {
            match remaining as u8 {
                0 | 1 | 4 | 5 | 6 | 7 => {}
                _ => {
                    return Err(VmxError::new(
                        VmxErrorKind::InvalidVmcsField,
                        "each IA32_PAT entry must encode UC, WC, WT, WP, WB, or UC-",
                    ));
                }
            }
            remaining >>= 8;
            index += 1;
        }
        Ok(Self(raw))
    }

    pub const fn toy_guest() -> Self {
        Self(VMX_TOY_GUEST_PAT_RAW)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub fn validate_owned_host_mappings(self) -> Result<Self, VmxError> {
        self.validate()?;
        if self.0 as u8 != 6 {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsField,
                "owned host mappings require write-back in IA32_PAT entry zero",
            ));
        }
        Ok(self)
    }

    fn validate(self) -> Result<Self, VmxError> {
        Self::new(self.0)
    }
}

const SEGMENT_ACCESS_RIGHTS_RESERVED: u32 = 0xfffe_0f00;
const SEGMENT_ACCESS_RIGHTS_PRESENT: u32 = 1 << 7;
const SEGMENT_ACCESS_RIGHTS_LONG_MODE: u32 = 1 << 13;
const SEGMENT_ACCESS_RIGHTS_DEFAULT_BIG: u32 = 1 << 14;
const SEGMENT_ACCESS_RIGHTS_UNUSABLE: u32 = 1 << 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmcsSegmentState {
    pub selector: u16,
    pub base: u64,
    pub limit: u32,
    pub access_rights: u32,
}

impl VmcsSegmentState {
    pub const fn new(selector: u16, base: u64, limit: u32, access_rights: u32) -> Self {
        Self {
            selector,
            base,
            limit,
            access_rights,
        }
    }

    pub const fn unusable() -> Self {
        Self::new(0, 0, 0, SEGMENT_ACCESS_RIGHTS_UNUSABLE)
    }

    pub const fn toy_code64() -> Self {
        Self::new(0x08, 0, u32::MAX, 0xa09b)
    }

    pub const fn toy_data64() -> Self {
        Self::new(0x10, 0, u32::MAX, 0xc093)
    }

    pub const fn toy_busy_tss64() -> Self {
        Self::new(0x18, 0, 0x67, 0x008b)
    }

    pub fn validate(self) -> Result<Self, VmxError> {
        if self.access_rights & SEGMENT_ACCESS_RIGHTS_RESERVED != 0 {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS segment access rights set a reserved bit",
            ));
        }
        if self.access_rights & SEGMENT_ACCESS_RIGHTS_UNUSABLE != 0 {
            if self.selector != 0 {
                return Err(VmxError::new(
                    VmxErrorKind::InvalidGuestState,
                    "an unusable VMCS segment must use a null selector",
                ));
            }
            return Ok(self);
        }
        if self.access_rights & SEGMENT_ACCESS_RIGHTS_PRESENT == 0 {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "a usable VMCS segment must be present",
            ));
        }
        if !is_canonical_u64(self.base) {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS segment base must be canonical",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmcsDescriptorTableState {
    pub base: u64,
    pub limit: u32,
}

impl VmcsDescriptorTableState {
    pub const fn new(base: u64, limit: u32) -> Self {
        Self { base, limit }
    }

    pub fn validate(self) -> Result<Self, VmxError> {
        if !is_canonical_u64(self.base) {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS descriptor-table base must be canonical",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmcsHostSelectors {
    pub es: u16,
    pub cs: u16,
    pub ss: u16,
    pub ds: u16,
    pub fs: u16,
    pub gs: u16,
    pub tr: u16,
}

impl VmcsHostSelectors {
    pub fn validate(self) -> Result<Self, VmxError> {
        if self.cs == 0 || self.tr == 0 {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS host CS and TR selectors must be non-null",
            ));
        }
        if self.es & 7 != 0
            || self.cs & 7 != 0
            || self.ss & 7 != 0
            || self.ds & 7 != 0
            || self.fs & 7 != 0
            || self.gs & 7 != 0
            || self.tr & 7 != 0
        {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS host selectors must use GDT entries at privilege level zero",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmcsHostState64 {
    pub cr0: u64,
    pub cr3: HostPhysical,
    pub cr4: u64,
    pub selectors: VmcsHostSelectors,
    pub fs_base: u64,
    pub gs_base: u64,
    pub tr_base: u64,
    pub gdtr_base: u64,
    pub idtr_base: u64,
    pub sysenter_cs: u32,
    pub sysenter_esp: u64,
    pub sysenter_eip: u64,
    pub pat: VmxPat,
    pub efer: u64,
    pub rsp: u64,
    pub rip: u64,
}

impl VmcsHostState64 {
    pub fn validate(self) -> Result<Self, VmxError> {
        self.selectors.validate()?;
        self.pat.validate()?;
        if self.cr0 & (VMX_CR0_PROTECTED_MODE_ENABLE | VMX_CR0_PAGING)
            != (VMX_CR0_PROTECTED_MODE_ENABLE | VMX_CR0_PAGING)
            || self.cr4 & (VMX_CR4_PAE | VMX_CR4_VMXE) != (VMX_CR4_PAE | VMX_CR4_VMXE)
        {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS host control registers do not describe 64-bit paging",
            ));
        }
        if self.efer & VMX_EFER_LMA == 0 {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS host EFER must have long mode active",
            ));
        }
        if !is_canonical_u64(self.fs_base)
            || !is_canonical_u64(self.gs_base)
            || !is_canonical_u64(self.tr_base)
            || !is_canonical_u64(self.gdtr_base)
            || !is_canonical_u64(self.idtr_base)
            || !is_canonical_u64(self.sysenter_esp)
            || !is_canonical_u64(self.sysenter_eip)
            || !is_canonical_u64(self.rsp)
            || !is_canonical_u64(self.rip)
        {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS host base, stack, and instruction pointers must be canonical",
            ));
        }
        Ok(self)
    }

    /// # Safety
    ///
    /// The caller must own the loaded current VMCS on this CPU. Every value
    /// must have been captured after the final host control-state changes.
    pub unsafe fn write_to<E: VmxInstructionExecutor>(
        self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        self.validate()?;
        // SAFETY: the caller guarantees a current, exclusively owned VMCS and
        // validation above establishes the architectural host-state shape.
        unsafe {
            executor.vmwrite(VmcsField::HOST_ES_SELECTOR.raw(), self.selectors.es.into())?;
            executor.vmwrite(VmcsField::HOST_CS_SELECTOR.raw(), self.selectors.cs.into())?;
            executor.vmwrite(VmcsField::HOST_SS_SELECTOR.raw(), self.selectors.ss.into())?;
            executor.vmwrite(VmcsField::HOST_DS_SELECTOR.raw(), self.selectors.ds.into())?;
            executor.vmwrite(VmcsField::HOST_FS_SELECTOR.raw(), self.selectors.fs.into())?;
            executor.vmwrite(VmcsField::HOST_GS_SELECTOR.raw(), self.selectors.gs.into())?;
            executor.vmwrite(VmcsField::HOST_TR_SELECTOR.raw(), self.selectors.tr.into())?;
            executor.vmwrite(VmcsField::HOST_CR0.raw(), self.cr0)?;
            executor.vmwrite(VmcsField::HOST_CR3.raw(), self.cr3.get())?;
            executor.vmwrite(VmcsField::HOST_CR4.raw(), self.cr4)?;
            executor.vmwrite(VmcsField::HOST_FS_BASE.raw(), self.fs_base)?;
            executor.vmwrite(VmcsField::HOST_GS_BASE.raw(), self.gs_base)?;
            executor.vmwrite(VmcsField::HOST_TR_BASE.raw(), self.tr_base)?;
            executor.vmwrite(VmcsField::HOST_GDTR_BASE.raw(), self.gdtr_base)?;
            executor.vmwrite(VmcsField::HOST_IDTR_BASE.raw(), self.idtr_base)?;
            executor.vmwrite(
                VmcsField::HOST_IA32_SYSENTER_CS.raw(),
                self.sysenter_cs.into(),
            )?;
            executor.vmwrite(VmcsField::HOST_IA32_SYSENTER_ESP.raw(), self.sysenter_esp)?;
            executor.vmwrite(VmcsField::HOST_IA32_SYSENTER_EIP.raw(), self.sysenter_eip)?;
            executor.vmwrite(VmcsField::HOST_IA32_PAT.raw(), self.pat.raw())?;
            executor.vmwrite(VmcsField::HOST_IA32_EFER.raw(), self.efer)?;
            executor.vmwrite(VmcsField::HOST_RSP.raw(), self.rsp)?;
            executor.vmwrite(VmcsField::HOST_RIP.raw(), self.rip)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmcsGuestSegments {
    pub es: VmcsSegmentState,
    pub cs: VmcsSegmentState,
    pub ss: VmcsSegmentState,
    pub ds: VmcsSegmentState,
    pub fs: VmcsSegmentState,
    pub gs: VmcsSegmentState,
    pub ldtr: VmcsSegmentState,
    pub tr: VmcsSegmentState,
}

impl VmcsGuestSegments {
    pub const fn toy_long_mode() -> Self {
        Self {
            es: VmcsSegmentState::toy_data64(),
            cs: VmcsSegmentState::toy_code64(),
            ss: VmcsSegmentState::toy_data64(),
            ds: VmcsSegmentState::toy_data64(),
            fs: VmcsSegmentState::toy_data64(),
            gs: VmcsSegmentState::toy_data64(),
            ldtr: VmcsSegmentState::unusable(),
            tr: VmcsSegmentState::toy_busy_tss64(),
        }
    }

    pub fn validate(self) -> Result<Self, VmxError> {
        self.es.validate()?;
        self.cs.validate()?;
        self.ss.validate()?;
        self.ds.validate()?;
        self.fs.validate()?;
        self.gs.validate()?;
        self.ldtr.validate()?;
        self.tr.validate()?;
        if self.cs.access_rights & SEGMENT_ACCESS_RIGHTS_LONG_MODE == 0
            || self.cs.access_rights & SEGMENT_ACCESS_RIGHTS_DEFAULT_BIG != 0
        {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "64-bit guest CS must set L and clear D/B",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmcsGuestState64 {
    pub cr0: u64,
    pub cr3: GuestPhysical,
    pub cr4: u64,
    pub segments: VmcsGuestSegments,
    pub gdtr: VmcsDescriptorTableState,
    pub idtr: VmcsDescriptorTableState,
    pub dr7: u64,
    pub rsp: u64,
    pub rip: GuestVirtual,
    pub rflags: u64,
    pub pending_debug_exceptions: u64,
    pub interruptibility: u32,
    pub activity_state: u32,
    pub smbase: u32,
    pub sysenter_cs: u32,
    pub sysenter_esp: u64,
    pub sysenter_eip: u64,
    pub pat: VmxPat,
    pub efer: u64,
}

impl VmcsGuestState64 {
    pub fn toy_long_mode(
        cr0_fixed: CrFixedBits,
        cr3: GuestPhysical,
        cr4_fixed: CrFixedBits,
        rsp: u64,
        rip: GuestVirtual,
    ) -> Result<Self, VmxError> {
        let cr0 = validate_control_register(
            VMX_CR0_PROTECTED_MODE_ENABLE
                | VMX_CR0_MONITOR_COPROCESSOR
                | VMX_CR0_TASK_SWITCHED
                | VMX_CR0_NUMERIC_ERROR
                | VMX_CR0_PAGING
                | cr0_fixed.fixed0,
            cr0_fixed,
            "toy guest CR0 cannot satisfy the CPU's VMX fixed bits",
        )?;
        let cr4 = validate_control_register(
            VMX_CR4_PAE | VMX_CR4_OSFXSR | VMX_CR4_VMXE | cr4_fixed.fixed0,
            cr4_fixed,
            "toy guest CR4 cannot satisfy the CPU's VMX fixed bits",
        )?;
        let state = Self {
            cr0,
            cr3,
            cr4,
            segments: VmcsGuestSegments::toy_long_mode(),
            gdtr: VmcsDescriptorTableState::new(0, 0),
            idtr: VmcsDescriptorTableState::new(0, 0),
            dr7: 0x400,
            rsp,
            rip,
            rflags: 0x2,
            pending_debug_exceptions: 0,
            interruptibility: 0,
            activity_state: 0,
            smbase: 0,
            sysenter_cs: 0,
            sysenter_esp: 0,
            sysenter_eip: 0,
            pat: VmxPat::toy_guest(),
            efer: VMX_EFER_LME | VMX_EFER_LMA,
        };
        state.validate()
    }

    pub fn validate(self) -> Result<Self, VmxError> {
        self.pat.validate()?;
        if self.cr0 & (VMX_CR0_PROTECTED_MODE_ENABLE | VMX_CR0_PAGING)
            != (VMX_CR0_PROTECTED_MODE_ENABLE | VMX_CR0_PAGING)
            || self.cr0 & VMX_CR0_NUMERIC_ERROR == 0
            || self.cr4 & (VMX_CR4_PAE | VMX_CR4_VMXE) != (VMX_CR4_PAE | VMX_CR4_VMXE)
        {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS guest control registers do not describe 64-bit paging",
            ));
        }
        if self.cr3.get() == 0 || self.cr3.get() % 4096 != 0 {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS guest CR3 must name a nonzero aligned PML4 GPA",
            ));
        }
        if self.efer & (VMX_EFER_LME | VMX_EFER_LMA) != (VMX_EFER_LME | VMX_EFER_LMA) {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS guest EFER must enable and activate long mode",
            ));
        }
        if !is_canonical_u64(self.rsp)
            || !is_canonical_u64(self.rip.get())
            || !is_canonical_u64(self.sysenter_esp)
            || !is_canonical_u64(self.sysenter_eip)
        {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "VMCS guest stack, instruction, and SYSENTER pointers must be canonical",
            ));
        }
        if self.rflags & 0x2 == 0 || self.activity_state != 0 || self.interruptibility != 0 {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "toy guest must start active, unblocked, and with reserved RFLAGS bit 1 set",
            ));
        }
        self.segments.validate()?;
        self.gdtr.validate()?;
        self.idtr.validate()?;
        Ok(self)
    }

    /// # Safety
    ///
    /// The caller must own the loaded current VMCS and must keep the guest page
    /// tables and EPT mappings backing this state alive until VMXOFF.
    pub unsafe fn write_to<E: VmxInstructionExecutor>(
        self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        self.validate()?;
        // SAFETY: the current VMCS is exclusively owned by the caller and the
        // complete validated guest state is written before any VM entry.
        unsafe {
            write_segment(
                executor,
                self.segments.es,
                VmcsField::GUEST_ES_SELECTOR,
                VmcsField::GUEST_ES_BASE,
                VmcsField::GUEST_ES_LIMIT,
                VmcsField::GUEST_ES_ACCESS_RIGHTS,
            )?;
            write_segment(
                executor,
                self.segments.cs,
                VmcsField::GUEST_CS_SELECTOR,
                VmcsField::GUEST_CS_BASE,
                VmcsField::GUEST_CS_LIMIT,
                VmcsField::GUEST_CS_ACCESS_RIGHTS,
            )?;
            write_segment(
                executor,
                self.segments.ss,
                VmcsField::GUEST_SS_SELECTOR,
                VmcsField::GUEST_SS_BASE,
                VmcsField::GUEST_SS_LIMIT,
                VmcsField::GUEST_SS_ACCESS_RIGHTS,
            )?;
            write_segment(
                executor,
                self.segments.ds,
                VmcsField::GUEST_DS_SELECTOR,
                VmcsField::GUEST_DS_BASE,
                VmcsField::GUEST_DS_LIMIT,
                VmcsField::GUEST_DS_ACCESS_RIGHTS,
            )?;
            write_segment(
                executor,
                self.segments.fs,
                VmcsField::GUEST_FS_SELECTOR,
                VmcsField::GUEST_FS_BASE,
                VmcsField::GUEST_FS_LIMIT,
                VmcsField::GUEST_FS_ACCESS_RIGHTS,
            )?;
            write_segment(
                executor,
                self.segments.gs,
                VmcsField::GUEST_GS_SELECTOR,
                VmcsField::GUEST_GS_BASE,
                VmcsField::GUEST_GS_LIMIT,
                VmcsField::GUEST_GS_ACCESS_RIGHTS,
            )?;
            write_segment(
                executor,
                self.segments.ldtr,
                VmcsField::GUEST_LDTR_SELECTOR,
                VmcsField::GUEST_LDTR_BASE,
                VmcsField::GUEST_LDTR_LIMIT,
                VmcsField::GUEST_LDTR_ACCESS_RIGHTS,
            )?;
            write_segment(
                executor,
                self.segments.tr,
                VmcsField::GUEST_TR_SELECTOR,
                VmcsField::GUEST_TR_BASE,
                VmcsField::GUEST_TR_LIMIT,
                VmcsField::GUEST_TR_ACCESS_RIGHTS,
            )?;
            executor.vmwrite(VmcsField::GUEST_GDTR_BASE.raw(), self.gdtr.base)?;
            executor.vmwrite(VmcsField::GUEST_GDTR_LIMIT.raw(), self.gdtr.limit.into())?;
            executor.vmwrite(VmcsField::GUEST_IDTR_BASE.raw(), self.idtr.base)?;
            executor.vmwrite(VmcsField::GUEST_IDTR_LIMIT.raw(), self.idtr.limit.into())?;
            executor.vmwrite(VmcsField::GUEST_CR0.raw(), self.cr0)?;
            executor.vmwrite(VmcsField::GUEST_CR3.raw(), self.cr3.get())?;
            executor.vmwrite(VmcsField::GUEST_CR4.raw(), self.cr4)?;
            executor.vmwrite(VmcsField::GUEST_DR7.raw(), self.dr7)?;
            executor.vmwrite(VmcsField::GUEST_RSP.raw(), self.rsp)?;
            executor.vmwrite(VmcsField::GUEST_RIP.raw(), self.rip.get())?;
            executor.vmwrite(VmcsField::GUEST_RFLAGS.raw(), self.rflags)?;
            executor.vmwrite(
                VmcsField::GUEST_PENDING_DEBUG_EXCEPTIONS.raw(),
                self.pending_debug_exceptions,
            )?;
            executor.vmwrite(
                VmcsField::GUEST_INTERRUPTIBILITY.raw(),
                self.interruptibility.into(),
            )?;
            executor.vmwrite(
                VmcsField::GUEST_ACTIVITY_STATE.raw(),
                self.activity_state.into(),
            )?;
            executor.vmwrite(VmcsField::GUEST_SMBASE.raw(), self.smbase.into())?;
            executor.vmwrite(
                VmcsField::GUEST_IA32_SYSENTER_CS.raw(),
                self.sysenter_cs.into(),
            )?;
            executor.vmwrite(VmcsField::GUEST_IA32_SYSENTER_ESP.raw(), self.sysenter_esp)?;
            executor.vmwrite(VmcsField::GUEST_IA32_SYSENTER_EIP.raw(), self.sysenter_eip)?;
            executor.vmwrite(VmcsField::GUEST_IA32_PAT.raw(), self.pat.raw())?;
            executor.vmwrite(VmcsField::GUEST_IA32_EFER.raw(), self.efer)?;
            executor.vmwrite(VmcsField::VMCS_LINK_POINTER.raw(), u64::MAX)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmcsInterceptionBitmaps {
    io_bitmap_a: HostPhysical,
    io_bitmap_b: HostPhysical,
    msr_bitmap: HostPhysical,
}

impl VmcsInterceptionBitmaps {
    pub fn new(
        io_bitmap_a: HostPhysical,
        io_bitmap_b: HostPhysical,
        msr_bitmap: HostPhysical,
    ) -> Result<Self, VmxError> {
        let addresses = [io_bitmap_a, io_bitmap_b, msr_bitmap];
        for (index, address) in addresses.iter().enumerate() {
            let raw = address.get();
            if raw == 0
                || raw % VMX_INTERCEPTION_BITMAP_PAGE_SIZE != 0
                || raw
                    .checked_add(VMX_INTERCEPTION_BITMAP_PAGE_SIZE)
                    .filter(|end| *end <= VMX_INTERCEPTION_BITMAP_MAX_PHYSICAL_EXCLUSIVE)
                    .is_none()
                || addresses[..index].contains(address)
            {
                return Err(VmxError::new(
                    VmxErrorKind::InvalidVmcsField,
                    "VMX interception bitmap pages must be nonzero, distinct, 4K-aligned, and below 4 GiB",
                ));
            }
        }
        Ok(Self {
            io_bitmap_a,
            io_bitmap_b,
            msr_bitmap,
        })
    }

    pub const fn io_bitmap_a(self) -> HostPhysical {
        self.io_bitmap_a
    }

    pub const fn io_bitmap_b(self) -> HostPhysical {
        self.io_bitmap_b
    }

    pub const fn msr_bitmap(self) -> HostPhysical {
        self.msr_bitmap
    }

    fn validate(self) -> Result<Self, VmxError> {
        Self::new(self.io_bitmap_a, self.io_bitmap_b, self.msr_bitmap)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmcsExecutionState {
    pub controls: VmxControlFields,
    pub ept_pointer: EptPointer,
    pub interception_bitmaps: VmcsInterceptionBitmaps,
    pub preemption_timer_value: u32,
    pub exception_bitmap: u32,
    pub cr0_guest_host_mask: u64,
    pub cr4_guest_host_mask: u64,
    pub cr0_read_shadow: u64,
    pub cr4_read_shadow: u64,
}

impl VmcsExecutionState {
    pub fn toy_isolated(
        controls: VmxControlFields,
        ept_pointer: EptPointer,
        interception_bitmaps: VmcsInterceptionBitmaps,
        guest: VmcsGuestState64,
        cr0_fixed: CrFixedBits,
        cr4_fixed: CrFixedBits,
    ) -> Result<Self, VmxError> {
        let state = Self {
            controls,
            ept_pointer,
            interception_bitmaps,
            preemption_timer_value: 0,
            exception_bitmap: VMX_TOY_EXCEPTION_BITMAP,
            cr0_guest_host_mask: cr0_fixed.fixed0
                | !cr0_fixed.fixed1
                | VMX_CR0_PROTECTED_MODE_ENABLE
                | VMX_CR0_MONITOR_COPROCESSOR
                | VMX_CR0_EMULATION
                | VMX_CR0_TASK_SWITCHED
                | VMX_CR0_NUMERIC_ERROR
                | VMX_CR0_PAGING,
            cr4_guest_host_mask: cr4_fixed.fixed0
                | !cr4_fixed.fixed1
                | VMX_CR4_PAE
                | VMX_CR4_OSFXSR
                | VMX_CR4_VMXE,
            cr0_read_shadow: guest.cr0,
            cr4_read_shadow: guest.cr4 & !VMX_CR4_VMXE,
        };
        state.validate(guest)
    }

    pub fn validate(self, guest: VmcsGuestState64) -> Result<Self, VmxError> {
        guest.validate()?;
        if guest.pat != VmxPat::toy_guest() {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsField,
                "isolated toy guest must use the deliberate IA32_PAT profile",
            ));
        }
        if guest.cr0 & (VMX_CR0_MONITOR_COPROCESSOR | VMX_CR0_TASK_SWITCHED)
            != (VMX_CR0_MONITOR_COPROCESSOR | VMX_CR0_TASK_SWITCHED)
            || guest.cr0 & VMX_CR0_EMULATION != 0
            || guest.cr4 & VMX_CR4_OSFXSR == 0
        {
            return Err(VmxError::new(
                VmxErrorKind::InvalidGuestState,
                "isolated toy guest requires the fixed FPU/SIMD guard state",
            ));
        }
        self.interception_bitmaps.validate()?;
        self.validate_interception_controls()?;
        if self.controls.pin_based & PIN_BASED_NMI_EXITING == 0
            || self.controls.pin_based & PIN_BASED_VMX_PREEMPTION_TIMER == 0
            || self.controls.primary & PRIMARY_HLT_EXITING == 0
            || self.controls.primary & PRIMARY_ACTIVATE_SECONDARY_CONTROLS == 0
            || self.controls.secondary & SECONDARY_ENABLE_EPT == 0
            || self.controls.secondary & SECONDARY_ENABLE_VPID != 0
            || self.controls.exit & EXIT_HOST_ADDRESS_SPACE_SIZE == 0
            || self.controls.exit & EXIT_SAVE_IA32_PAT == 0
            || self.controls.exit & EXIT_LOAD_IA32_PAT == 0
            || self.controls.exit & EXIT_SAVE_IA32_EFER == 0
            || self.controls.exit & EXIT_LOAD_IA32_EFER == 0
            || self.controls.entry & ENTRY_IA32E_MODE_GUEST == 0
            || self.controls.entry & ENTRY_LOAD_IA32_PAT == 0
            || self.controls.entry & ENTRY_LOAD_IA32_EFER == 0
            || self.ept_pointer.root().get() == 0
            || self.preemption_timer_value != 0
            || self.exception_bitmap != VMX_TOY_EXCEPTION_BITMAP
        {
            return Err(VmxError::new(
                VmxErrorKind::InvalidControlBits,
                "VMCS execution state does not satisfy the isolated toy guest contract",
            ));
        }
        if self.cr0_guest_host_mask
            & (VMX_CR0_PROTECTED_MODE_ENABLE
                | VMX_CR0_MONITOR_COPROCESSOR
                | VMX_CR0_EMULATION
                | VMX_CR0_TASK_SWITCHED
                | VMX_CR0_NUMERIC_ERROR
                | VMX_CR0_PAGING)
            != (VMX_CR0_PROTECTED_MODE_ENABLE
                | VMX_CR0_MONITOR_COPROCESSOR
                | VMX_CR0_EMULATION
                | VMX_CR0_TASK_SWITCHED
                | VMX_CR0_NUMERIC_ERROR
                | VMX_CR0_PAGING)
            || self.cr4_guest_host_mask & (VMX_CR4_PAE | VMX_CR4_OSFXSR | VMX_CR4_VMXE)
                != (VMX_CR4_PAE | VMX_CR4_OSFXSR | VMX_CR4_VMXE)
            || self.cr0_read_shadow != guest.cr0
            || self.cr4_read_shadow != guest.cr4 & !VMX_CR4_VMXE
        {
            return Err(VmxError::new(
                VmxErrorKind::InvalidControlBits,
                "VMCS CR masks do not isolate the toy guest control-register state",
            ));
        }
        Ok(self)
    }

    fn validate_interception_controls(self) -> Result<(), VmxError> {
        if self.controls.primary & PRIMARY_USE_IO_BITMAPS == 0
            || self.controls.primary & PRIMARY_USE_MSR_BITMAPS == 0
        {
            return Err(VmxError::new(
                VmxErrorKind::InvalidControlBits,
                "toy VMX entry requires active I/O and MSR bitmaps",
            ));
        }
        Ok(())
    }

    /// # Safety
    ///
    /// The caller must own a loaded current VMCS. The EPT hierarchy rooted at
    /// `ept_pointer` and all three interception bitmap pages must remain
    /// allocated, unchanged, and usable by VMX hardware through every guest
    /// entry and resume that uses this execution state.
    pub unsafe fn write_to<E: VmxInstructionExecutor>(
        self,
        guest: VmcsGuestState64,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        self.validate(guest)?;
        // SAFETY: the caller guarantees a current VMCS and that the validated
        // EPT hierarchy and interception bitmap pages remain live and stable.
        unsafe {
            executor.vmwrite(
                VmcsField::PIN_BASED_CONTROLS.raw(),
                self.controls.pin_based.into(),
            )?;
            executor.vmwrite(
                VmcsField::PRIMARY_PROCESSOR_CONTROLS.raw(),
                self.controls.primary.into(),
            )?;
            executor.vmwrite(
                VmcsField::SECONDARY_PROCESSOR_CONTROLS.raw(),
                self.controls.secondary.into(),
            )?;
            executor.vmwrite(VmcsField::EXIT_CONTROLS.raw(), self.controls.exit.into())?;
            executor.vmwrite(VmcsField::ENTRY_CONTROLS.raw(), self.controls.entry.into())?;
            executor.vmwrite(VmcsField::VIRTUAL_PROCESSOR_ID.raw(), 0)?;
            executor.vmwrite(
                VmcsField::IO_BITMAP_A.raw(),
                self.interception_bitmaps.io_bitmap_a().get(),
            )?;
            executor.vmwrite(
                VmcsField::IO_BITMAP_B.raw(),
                self.interception_bitmaps.io_bitmap_b().get(),
            )?;
            executor.vmwrite(
                VmcsField::MSR_BITMAP.raw(),
                self.interception_bitmaps.msr_bitmap().get(),
            )?;
            executor.vmwrite(VmcsField::TSC_OFFSET.raw(), 0)?;
            executor.vmwrite(VmcsField::EPT_POINTER.raw(), self.ept_pointer.raw())?;
            executor.vmwrite(
                VmcsField::VMX_PREEMPTION_TIMER_VALUE.raw(),
                self.preemption_timer_value.into(),
            )?;
            executor.vmwrite(
                VmcsField::EXCEPTION_BITMAP.raw(),
                self.exception_bitmap.into(),
            )?;
            executor.vmwrite(VmcsField::PAGE_FAULT_ERROR_CODE_MASK.raw(), 0)?;
            executor.vmwrite(VmcsField::PAGE_FAULT_ERROR_CODE_MATCH.raw(), 0)?;
            executor.vmwrite(VmcsField::CR3_TARGET_COUNT.raw(), 0)?;
            executor.vmwrite(VmcsField::VM_EXIT_MSR_STORE_COUNT.raw(), 0)?;
            executor.vmwrite(VmcsField::VM_EXIT_MSR_LOAD_COUNT.raw(), 0)?;
            executor.vmwrite(VmcsField::VM_ENTRY_MSR_LOAD_COUNT.raw(), 0)?;
            executor.vmwrite(VmcsField::VM_ENTRY_INTERRUPTION_INFO.raw(), 0)?;
            executor.vmwrite(VmcsField::VM_ENTRY_EXCEPTION_ERROR_CODE.raw(), 0)?;
            executor.vmwrite(VmcsField::VM_ENTRY_INSTRUCTION_LENGTH.raw(), 0)?;
            executor.vmwrite(
                VmcsField::CR0_GUEST_HOST_MASK.raw(),
                self.cr0_guest_host_mask,
            )?;
            executor.vmwrite(
                VmcsField::CR4_GUEST_HOST_MASK.raw(),
                self.cr4_guest_host_mask,
            )?;
            executor.vmwrite(VmcsField::CR0_READ_SHADOW.raw(), self.cr0_read_shadow)?;
            executor.vmwrite(VmcsField::CR4_READ_SHADOW.raw(), self.cr4_read_shadow)?;
        }
        Ok(())
    }

    /// Reads back the control and bitmap-address VMCS fields that make the
    /// interception policy active.
    ///
    /// # Safety
    ///
    /// The caller must own the loaded current VMCS on this CPU and must call
    /// this only after this execution state was written to that VMCS.
    pub unsafe fn verify_interception_fields<E: VmxInstructionExecutor>(
        self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        self.interception_bitmaps.validate()?;
        self.validate_interception_controls()?;
        let expected = [
            (
                VmcsField::PIN_BASED_CONTROLS,
                u64::from(self.controls.pin_based),
            ),
            (
                VmcsField::PRIMARY_PROCESSOR_CONTROLS,
                u64::from(self.controls.primary),
            ),
            (
                VmcsField::SECONDARY_PROCESSOR_CONTROLS,
                u64::from(self.controls.secondary),
            ),
            (VmcsField::EXIT_CONTROLS, u64::from(self.controls.exit)),
            (VmcsField::ENTRY_CONTROLS, u64::from(self.controls.entry)),
            (
                VmcsField::IO_BITMAP_A,
                self.interception_bitmaps.io_bitmap_a().get(),
            ),
            (
                VmcsField::IO_BITMAP_B,
                self.interception_bitmaps.io_bitmap_b().get(),
            ),
            (
                VmcsField::MSR_BITMAP,
                self.interception_bitmaps.msr_bitmap().get(),
            ),
            (
                VmcsField::EXCEPTION_BITMAP,
                u64::from(self.exception_bitmap),
            ),
            (VmcsField::CR0_GUEST_HOST_MASK, self.cr0_guest_host_mask),
            (VmcsField::CR4_GUEST_HOST_MASK, self.cr4_guest_host_mask),
            (VmcsField::CR0_READ_SHADOW, self.cr0_read_shadow),
            (VmcsField::CR4_READ_SHADOW, self.cr4_read_shadow),
            (VmcsField::GUEST_CR0, self.cr0_read_shadow),
            (VmcsField::GUEST_CR4, self.cr4_read_shadow | VMX_CR4_VMXE),
        ];
        for (field, value) in expected {
            // SAFETY: the caller established that this CPU owns a current VMCS
            // containing these readable control fields.
            if unsafe { executor.vmread(field.raw())? } != value {
                return Err(VmxError::new(
                    VmxErrorKind::InvalidVmcsField,
                    "VMCS isolation-field readback differs from the validated state",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MinimalVmcsConfiguration {
    pub execution: VmcsExecutionState,
    pub host: VmcsHostState64,
    pub guest: VmcsGuestState64,
}

impl MinimalVmcsConfiguration {
    pub fn validate(self) -> Result<Self, VmxError> {
        self.host.validate()?;
        self.host.pat.validate_owned_host_mappings()?;
        self.guest.validate()?;
        if self.guest.pat != VmxPat::toy_guest() || self.host.pat == self.guest.pat {
            return Err(VmxError::new(
                VmxErrorKind::InvalidVmcsField,
                "toy guest PAT must use the deliberate profile and differ from host PAT",
            ));
        }
        self.execution.validate(self.guest)?;
        Ok(self)
    }

    /// # Safety
    ///
    /// The caller must own the loaded VMCS on the current CPU and guarantee
    /// that every referenced host, guest, EPT, and interception-bitmap page
    /// remains live and stable. No VM entry may occur until this method
    /// succeeds completely.
    pub unsafe fn write_to<E: VmxInstructionExecutor>(
        self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        self.validate()?;
        // SAFETY: one validated configuration is written to the caller-owned
        // current VMCS before it is made eligible for VMLAUNCH.
        unsafe {
            self.execution.write_to(self.guest, executor)?;
            self.guest.write_to(executor)?;
            self.host.write_to(executor)?;
        }
        Ok(())
    }

    /// Reads back every VMCS field required for the live PAT and FPU guards.
    ///
    /// # Safety
    ///
    /// The caller must own the loaded current VMCS on this CPU and must call
    /// this only after this complete configuration was written to that VMCS.
    pub unsafe fn verify_isolation_fields<E: VmxInstructionExecutor>(
        self,
        executor: &mut E,
    ) -> Result<(), VmxError> {
        self.validate()?;
        // SAFETY: the caller guarantees that this configuration owns the
        // current VMCS and was written before the readback.
        unsafe { self.execution.verify_interception_fields(executor)? };
        for (field, value) in [
            (VmcsField::GUEST_IA32_PAT, self.guest.pat.raw()),
            (VmcsField::HOST_IA32_PAT, self.host.pat.raw()),
        ] {
            // SAFETY: the caller established exclusive current-VMCS ownership.
            if unsafe { executor.vmread(field.raw())? } != value {
                return Err(VmxError::new(
                    VmxErrorKind::InvalidVmcsField,
                    "VMCS PAT readback differs from the validated configuration",
                ));
            }
        }
        Ok(())
    }
}

unsafe fn write_segment<E: VmxInstructionExecutor>(
    executor: &mut E,
    segment: VmcsSegmentState,
    selector: VmcsField,
    base: VmcsField,
    limit: VmcsField,
    access_rights: VmcsField,
) -> Result<(), VmxError> {
    // SAFETY: the caller owns the current VMCS and supplies the exact four
    // encodings belonging to this already validated segment cache.
    unsafe {
        executor.vmwrite(selector.raw(), segment.selector.into())?;
        executor.vmwrite(base.raw(), segment.base)?;
        executor.vmwrite(limit.raw(), segment.limit.into())?;
        executor.vmwrite(access_rights.raw(), segment.access_rights.into())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vmx::controls::{VmxControlMsr, VmxControlMsrs, VmxControlRequest};
    use crate::vmx::ept::{
        EptCapabilities, EPT_VPID_CAP_MEMORY_TYPE_WB, EPT_VPID_CAP_PAGE_WALK_LENGTH_4,
    };
    use crate::vmx::instructions::tests_support::MockVmxInstructions;

    fn controls() -> VmxControlFields {
        VmxControlMsrs {
            pin_based: VmxControlMsr::new(
                super::super::controls::VmxControlGroup::PinBased,
                0,
                u32::MAX,
            ),
            primary: VmxControlMsr::new(
                super::super::controls::VmxControlGroup::PrimaryProcessor,
                0,
                u32::MAX,
            ),
            secondary: VmxControlMsr::new(
                super::super::controls::VmxControlGroup::SecondaryProcessor,
                0,
                u32::MAX,
            ),
            exit: VmxControlMsr::new(super::super::controls::VmxControlGroup::Exit, 0, u32::MAX),
            entry: VmxControlMsr::new(super::super::controls::VmxControlGroup::Entry, 0, u32::MAX),
        }
        .build(VmxControlRequest::toy_hlt_guest())
        .unwrap()
    }

    fn host() -> VmcsHostState64 {
        VmcsHostState64 {
            cr0: VMX_CR0_PROTECTED_MODE_ENABLE | VMX_CR0_PAGING,
            cr3: HostPhysical::new(0x9000).unwrap(),
            cr4: VMX_CR4_PAE | VMX_CR4_VMXE,
            selectors: VmcsHostSelectors {
                es: 0,
                cs: 0x08,
                ss: 0x10,
                ds: 0,
                fs: 0,
                gs: 0,
                tr: 0x18,
            },
            fs_base: 0,
            gs_base: 0,
            tr_base: 0xffff_8000_0000_1000,
            gdtr_base: 0xffff_8000_0000_2000,
            idtr_base: 0xffff_8000_0000_3000,
            sysenter_cs: 0,
            sysenter_esp: 0,
            sysenter_eip: 0,
            pat: VmxPat::new(0x0007_0406_0007_0406).unwrap(),
            efer: VMX_EFER_LME | VMX_EFER_LMA,
            rsp: 0xffff_8000_0000_4000,
            rip: 0xffff_8000_0000_5000,
        }
    }

    fn guest() -> VmcsGuestState64 {
        VmcsGuestState64::toy_long_mode(
            CrFixedBits::new(
                VMX_CR0_PROTECTED_MODE_ENABLE | VMX_CR0_NUMERIC_ERROR | VMX_CR0_PAGING,
                u64::MAX,
            ),
            GuestPhysical::new(0x2000).unwrap(),
            CrFixedBits::new(VMX_CR4_PAE | VMX_CR4_VMXE, u64::MAX),
            0x1ff0,
            GuestVirtual::new(0x1000),
        )
        .unwrap()
    }

    fn interception_bitmaps() -> VmcsInterceptionBitmaps {
        VmcsInterceptionBitmaps::new(
            HostPhysical::new(0xa000).unwrap(),
            HostPhysical::new(0xb000).unwrap(),
            HostPhysical::new(0xc000).unwrap(),
        )
        .unwrap()
    }

    fn configuration() -> MinimalVmcsConfiguration {
        let capabilities =
            EptCapabilities::new(EPT_VPID_CAP_PAGE_WALK_LENGTH_4 | EPT_VPID_CAP_MEMORY_TYPE_WB);
        let guest = guest();
        MinimalVmcsConfiguration {
            execution: VmcsExecutionState::toy_isolated(
                controls(),
                EptPointer::new(HostPhysical::new(0x8000).unwrap(), capabilities).unwrap(),
                interception_bitmaps(),
                guest,
                CrFixedBits::new(
                    VMX_CR0_PROTECTED_MODE_ENABLE | VMX_CR0_NUMERIC_ERROR | VMX_CR0_PAGING,
                    u64::MAX,
                ),
                CrFixedBits::new(VMX_CR4_PAE | VMX_CR4_VMXE, u64::MAX),
            )
            .unwrap(),
            host: host(),
            guest,
        }
    }

    fn recorded(executor: &MockVmxInstructions, field: VmcsField) -> Option<u64> {
        executor.writes[..executor.write_count]
            .iter()
            .flatten()
            .find_map(|&(candidate, value)| (candidate == field.raw()).then_some(value))
    }

    #[test]
    fn toy_long_mode_guest_uses_checked_segment_state() {
        let guest = guest();

        assert_eq!(guest.segments.cs.access_rights, 0xa09b);
        assert_eq!(guest.segments.ss.access_rights, 0xc093);
        assert_eq!(guest.segments.ldtr.access_rights, 0x10000);
        assert_eq!(guest.segments.tr.access_rights, 0x008b);
        assert_eq!(guest.pat, VmxPat::toy_guest());
        assert_ne!(guest.cr0 & VMX_CR0_TASK_SWITCHED, 0);
        assert_eq!(guest.cr0 & VMX_CR0_EMULATION, 0);
        assert_ne!(guest.cr4 & VMX_CR4_OSFXSR, 0);
        assert!(guest.validate().is_ok());
    }

    #[test]
    fn host_state_rejects_selector_with_rpl_or_ldt_bits() {
        let mut state = host();
        state.selectors.tr = 0x1b;

        assert_eq!(
            state.validate().unwrap_err().kind,
            VmxErrorKind::InvalidGuestState
        );
    }

    #[test]
    fn pat_validation_accepts_only_architectural_memory_types() {
        assert_eq!(
            VmxPat::new(VMX_TOY_GUEST_PAT_RAW).unwrap(),
            VmxPat::toy_guest()
        );
        for byte in 0..8 {
            let invalid = VMX_TOY_GUEST_PAT_RAW & !(0xff_u64 << (byte * 8)) | (2_u64 << (byte * 8));
            assert_eq!(
                VmxPat::new(invalid).unwrap_err().kind,
                VmxErrorKind::InvalidVmcsField
            );
        }

        assert_eq!(
            VmxPat::new(0x0007_0406_0007_0400)
                .unwrap()
                .validate_owned_host_mappings()
                .unwrap_err()
                .kind,
            VmxErrorKind::InvalidVmcsField
        );
    }

    #[test]
    fn complete_configuration_writes_ept_and_required_state_once() {
        let mut executor = MockVmxInstructions::default();

        unsafe { configuration().write_to(&mut executor) }.unwrap();

        assert_eq!(recorded(&executor, VmcsField::EPT_POINTER), Some(0x801e));
        assert_eq!(recorded(&executor, VmcsField::IO_BITMAP_A), Some(0xa000));
        assert_eq!(recorded(&executor, VmcsField::IO_BITMAP_B), Some(0xb000));
        assert_eq!(recorded(&executor, VmcsField::MSR_BITMAP), Some(0xc000));
        assert_eq!(
            recorded(&executor, VmcsField::GUEST_IA32_PAT),
            Some(VMX_TOY_GUEST_PAT_RAW)
        );
        assert_eq!(
            recorded(&executor, VmcsField::HOST_IA32_PAT),
            Some(host().pat.raw())
        );
        assert_eq!(
            recorded(&executor, VmcsField::EXCEPTION_BITMAP),
            Some(u64::from(VMX_TOY_EXCEPTION_BITMAP))
        );
        assert_eq!(
            recorded(&executor, VmcsField::VMX_PREEMPTION_TIMER_VALUE),
            Some(0)
        );
        assert_eq!(
            recorded(&executor, VmcsField::VIRTUAL_PROCESSOR_ID),
            Some(0)
        );
        assert_eq!(
            recorded(&executor, VmcsField::VMCS_LINK_POINTER),
            Some(u64::MAX)
        );
        assert_eq!(
            recorded(&executor, VmcsField::GUEST_CS_ACCESS_RIGHTS),
            Some(0xa09b)
        );
        assert_eq!(
            recorded(&executor, VmcsField::GUEST_IA32_EFER),
            Some(VMX_EFER_LME | VMX_EFER_LMA)
        );
        assert_eq!(
            recorded(&executor, VmcsField::CR4_GUEST_HOST_MASK),
            Some(VMX_CR4_PAE | VMX_CR4_OSFXSR | VMX_CR4_VMXE)
        );
        assert_eq!(
            recorded(&executor, VmcsField::CR4_READ_SHADOW),
            Some(VMX_CR4_PAE | VMX_CR4_OSFXSR)
        );
        assert_eq!(recorded(&executor, VmcsField::HOST_RIP), Some(host().rip));
        assert!(executor.write_count > 70);
    }

    #[test]
    fn configuration_rejects_vpid_dependency_without_a_vpid() {
        let mut config = configuration();
        config.execution.controls.secondary |= SECONDARY_ENABLE_VPID;

        assert_eq!(
            config.validate().unwrap_err().kind,
            VmxErrorKind::InvalidControlBits
        );
    }

    #[test]
    fn configuration_rejects_missing_preemption_or_bitmap_containment() {
        let mut config = configuration();
        config.execution.controls.pin_based &= !PIN_BASED_VMX_PREEMPTION_TIMER;
        assert_eq!(
            config.validate().unwrap_err().kind,
            VmxErrorKind::InvalidControlBits
        );

        let mut config = configuration();
        config.execution.controls.primary &= !PRIMARY_USE_IO_BITMAPS;
        assert_eq!(
            config.validate().unwrap_err().kind,
            VmxErrorKind::InvalidControlBits
        );

        let mut config = configuration();
        config.execution.controls.primary &= !PRIMARY_USE_MSR_BITMAPS;
        assert_eq!(
            config.validate().unwrap_err().kind,
            VmxErrorKind::InvalidControlBits
        );

        let mut config = configuration();
        config.execution.preemption_timer_value = 1;
        assert_eq!(
            config.validate().unwrap_err().kind,
            VmxErrorKind::InvalidControlBits
        );

        for (entry, exit) in [
            (ENTRY_LOAD_IA32_PAT, 0),
            (0, EXIT_SAVE_IA32_PAT),
            (0, EXIT_LOAD_IA32_PAT),
        ] {
            let mut config = configuration();
            config.execution.controls.entry &= !entry;
            config.execution.controls.exit &= !exit;
            assert_eq!(
                config.validate().unwrap_err().kind,
                VmxErrorKind::InvalidControlBits
            );
        }

        let mut config = configuration();
        config.execution.exception_bitmap &= !(1 << 7);
        assert_eq!(
            config.validate().unwrap_err().kind,
            VmxErrorKind::InvalidControlBits
        );
    }

    #[test]
    fn configuration_rejects_mutated_pat_or_fpu_guard_state() {
        let mut config = configuration();
        config.guest.cr0 &= !VMX_CR0_TASK_SWITCHED;
        assert_eq!(
            config.validate().unwrap_err().kind,
            VmxErrorKind::InvalidGuestState
        );

        let mut config = configuration();
        config.guest.cr0 |= VMX_CR0_EMULATION;
        assert_eq!(
            config.validate().unwrap_err().kind,
            VmxErrorKind::InvalidGuestState
        );

        let mut config = configuration();
        config.guest.cr4 &= !VMX_CR4_OSFXSR;
        assert_eq!(
            config.validate().unwrap_err().kind,
            VmxErrorKind::InvalidGuestState
        );

        let mut config = configuration();
        config.host.pat = VmxPat::toy_guest();
        assert_eq!(
            config.validate().unwrap_err().kind,
            VmxErrorKind::InvalidVmcsField
        );

        let mut config = configuration();
        config.host.pat = VmxPat::new(0x0007_0406_0007_0400).unwrap();
        assert_eq!(
            config.validate().unwrap_err().kind,
            VmxErrorKind::InvalidVmcsField
        );
    }

    #[test]
    fn generic_state_validation_does_not_impose_the_toy_isolation_profile() {
        let mut generic_guest = guest();
        generic_guest.cr0 &= !(VMX_CR0_MONITOR_COPROCESSOR | VMX_CR0_TASK_SWITCHED);
        generic_guest.cr4 &= !VMX_CR4_OSFXSR;
        assert!(generic_guest.validate().is_ok());

        let mut generic_host = host();
        generic_host.pat = VmxPat::new(0x0007_0406_0007_0400).unwrap();
        assert!(generic_host.validate().is_ok());
    }

    #[test]
    fn bitmap_addresses_are_nonzero_distinct_aligned_and_below_4g() {
        let valid = |raw| HostPhysical::new(raw).unwrap();
        assert!(
            VmcsInterceptionBitmaps::new(valid(0xffff_f000), valid(0x2000), valid(0x3000)).is_ok()
        );

        for result in [
            VmcsInterceptionBitmaps::new(HostPhysical::ZERO, valid(0x2000), valid(0x3000)),
            VmcsInterceptionBitmaps::new(valid(0x1001), valid(0x2000), valid(0x3000)),
            VmcsInterceptionBitmaps::new(valid(0x1000), valid(0x1000), valid(0x3000)),
            VmcsInterceptionBitmaps::new(valid(0x1000), valid(0x2000), valid(0x1_0000_0000)),
        ] {
            assert_eq!(result.unwrap_err().kind, VmxErrorKind::InvalidVmcsField);
        }
    }

    struct InterceptionReadback {
        configuration: MinimalVmcsConfiguration,
        corrupt_field: Option<u64>,
    }

    impl InterceptionReadback {
        fn new(configuration: MinimalVmcsConfiguration) -> Self {
            Self {
                configuration,
                corrupt_field: None,
            }
        }
    }

    impl VmxInstructionExecutor for InterceptionReadback {
        unsafe fn vmxon(&mut self, _region: HostPhysical) -> Result<(), VmxError> {
            unreachable!("readback test does not execute VMXON")
        }

        unsafe fn vmxoff(&mut self) -> Result<(), VmxError> {
            unreachable!("readback test does not execute VMXOFF")
        }

        unsafe fn vmptrld(&mut self, _vmcs: HostPhysical) -> Result<(), VmxError> {
            unreachable!("readback test does not execute VMPTRLD")
        }

        unsafe fn vmclear(&mut self, _vmcs: HostPhysical) -> Result<(), VmxError> {
            unreachable!("readback test does not execute VMCLEAR")
        }

        unsafe fn vmlaunch(&mut self) -> Result<(), VmxError> {
            unreachable!("readback test does not execute VMLAUNCH")
        }

        unsafe fn vmresume(&mut self) -> Result<(), VmxError> {
            unreachable!("readback test does not execute VMRESUME")
        }

        unsafe fn vmread(&mut self, field: u64) -> Result<u64, VmxError> {
            let state = self.configuration.execution;
            let expected = if field == VmcsField::PIN_BASED_CONTROLS.raw() {
                u64::from(state.controls.pin_based)
            } else if field == VmcsField::PRIMARY_PROCESSOR_CONTROLS.raw() {
                u64::from(state.controls.primary)
            } else if field == VmcsField::SECONDARY_PROCESSOR_CONTROLS.raw() {
                u64::from(state.controls.secondary)
            } else if field == VmcsField::EXIT_CONTROLS.raw() {
                u64::from(state.controls.exit)
            } else if field == VmcsField::ENTRY_CONTROLS.raw() {
                u64::from(state.controls.entry)
            } else if field == VmcsField::IO_BITMAP_A.raw() {
                state.interception_bitmaps.io_bitmap_a().get()
            } else if field == VmcsField::IO_BITMAP_B.raw() {
                state.interception_bitmaps.io_bitmap_b().get()
            } else if field == VmcsField::MSR_BITMAP.raw() {
                state.interception_bitmaps.msr_bitmap().get()
            } else if field == VmcsField::EXCEPTION_BITMAP.raw() {
                u64::from(state.exception_bitmap)
            } else if field == VmcsField::CR0_GUEST_HOST_MASK.raw() {
                state.cr0_guest_host_mask
            } else if field == VmcsField::CR4_GUEST_HOST_MASK.raw() {
                state.cr4_guest_host_mask
            } else if field == VmcsField::CR0_READ_SHADOW.raw() {
                state.cr0_read_shadow
            } else if field == VmcsField::CR4_READ_SHADOW.raw() {
                state.cr4_read_shadow
            } else if field == VmcsField::GUEST_CR0.raw() {
                self.configuration.guest.cr0
            } else if field == VmcsField::GUEST_CR4.raw() {
                self.configuration.guest.cr4
            } else if field == VmcsField::GUEST_IA32_PAT.raw() {
                self.configuration.guest.pat.raw()
            } else if field == VmcsField::HOST_IA32_PAT.raw() {
                self.configuration.host.pat.raw()
            } else {
                return Err(VmxError::new(
                    VmxErrorKind::InvalidVmcsField,
                    "readback test received an unexpected field",
                ));
            };
            Ok(if self.corrupt_field == Some(field) {
                expected ^ 1
            } else {
                expected
            })
        }

        unsafe fn vmwrite(&mut self, _field: u64, _value: u64) -> Result<(), VmxError> {
            unreachable!("readback test does not execute VMWRITE")
        }
    }

    #[test]
    fn interception_field_readback_is_exact_and_fails_closed() {
        let configuration = configuration();
        let mut executor = InterceptionReadback::new(configuration);
        unsafe { configuration.verify_isolation_fields(&mut executor) }.unwrap();

        executor.corrupt_field = Some(VmcsField::HOST_IA32_PAT.raw());
        assert_eq!(
            unsafe { configuration.verify_isolation_fields(&mut executor) }
                .unwrap_err()
                .kind,
            VmxErrorKind::InvalidVmcsField
        );
    }

    #[test]
    fn forced_unconditional_io_is_ignored_when_io_bitmaps_are_active() {
        let mut config = configuration();
        config.execution.controls.primary |=
            super::super::controls::PRIMARY_UNCONDITIONAL_IO_EXITING;

        assert!(config.validate().is_ok());
    }
}
