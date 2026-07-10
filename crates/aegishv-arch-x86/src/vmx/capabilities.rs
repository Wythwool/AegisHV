use super::controls::{
    VmxControlFields, VmxControlGroup, VmxControlMsr, VmxControlMsrs, VmxControlRequest,
};
use super::ept::EptCapabilities;
use super::features::{CrFixedBits, VmxError, VmxErrorKind};

pub const IA32_VMX_BASIC_MSR: u32 = 0x480;
pub const IA32_VMX_PINBASED_CTLS_MSR: u32 = 0x481;
pub const IA32_VMX_PROCBASED_CTLS_MSR: u32 = 0x482;
pub const IA32_VMX_EXIT_CTLS_MSR: u32 = 0x483;
pub const IA32_VMX_ENTRY_CTLS_MSR: u32 = 0x484;
pub const IA32_VMX_CR0_FIXED0_MSR: u32 = 0x486;
pub const IA32_VMX_CR0_FIXED1_MSR: u32 = 0x487;
pub const IA32_VMX_CR4_FIXED0_MSR: u32 = 0x488;
pub const IA32_VMX_CR4_FIXED1_MSR: u32 = 0x489;
pub const IA32_VMX_PROCBASED_CTLS2_MSR: u32 = 0x48b;
pub const IA32_VMX_EPT_VPID_CAP_MSR: u32 = 0x48c;
pub const IA32_VMX_TRUE_PINBASED_CTLS_MSR: u32 = 0x48d;
pub const IA32_VMX_TRUE_PROCBASED_CTLS_MSR: u32 = 0x48e;
pub const IA32_VMX_TRUE_EXIT_CTLS_MSR: u32 = 0x48f;
pub const IA32_VMX_TRUE_ENTRY_CTLS_MSR: u32 = 0x490;

pub const VMX_BASIC_TRUE_CONTROLS: u64 = 1 << 55;
const VMX_BASIC_REVISION_MASK: u64 = 0x7fff_ffff;
const VMX_BASIC_VMCS_REGION_SIZE_SHIFT: u32 = 32;
const VMX_BASIC_VMCS_REGION_SIZE_MASK: u64 = 0x1fff;
const VMX_BASIC_MEMORY_TYPE_SHIFT: u32 = 50;
const VMX_BASIC_MEMORY_TYPE_MASK: u64 = 0xf;
const VMX_MEMORY_TYPE_WRITE_BACK: u64 = 6;
const VMX_REGION_PAGE_SIZE: u64 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmxCapabilitySnapshot {
    pub basic: u64,
    pub pin_based: u64,
    pub primary: u64,
    pub secondary: u64,
    pub exit: u64,
    pub entry: u64,
    pub ept_vpid: u64,
    pub cr0_fixed: CrFixedBits,
    pub cr4_fixed: CrFixedBits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmxToyCapabilities {
    pub controls: VmxControlFields,
    pub ept: EptCapabilities,
    pub cr0_fixed: CrFixedBits,
    pub cr4_fixed: CrFixedBits,
}

impl VmxCapabilitySnapshot {
    pub const fn uses_true_controls(basic: u64) -> bool {
        basic & VMX_BASIC_TRUE_CONTROLS != 0
    }

    pub const fn control_allows(raw: u64, bit: u32) -> bool {
        (raw >> 32) as u32 & bit != 0
    }

    pub fn prepare_toy_guest(self) -> Result<VmxToyCapabilities, VmxError> {
        self.validate_basic()?;
        if !self.cr0_fixed.validate(self.cr0_fixed.fixed0)
            || !self.cr4_fixed.validate(self.cr4_fixed.fixed0)
        {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "VMX control-register fixed MSRs are internally inconsistent",
            ));
        }
        let controls = VmxControlMsrs {
            pin_based: VmxControlMsr::from_raw(VmxControlGroup::PinBased, self.pin_based),
            primary: VmxControlMsr::from_raw(VmxControlGroup::PrimaryProcessor, self.primary),
            secondary: VmxControlMsr::from_raw(VmxControlGroup::SecondaryProcessor, self.secondary),
            exit: VmxControlMsr::from_raw(VmxControlGroup::Exit, self.exit),
            entry: VmxControlMsr::from_raw(VmxControlGroup::Entry, self.entry),
        }
        .build(VmxControlRequest::toy_hlt_guest())?;
        let ept = EptCapabilities::new(self.ept_vpid);
        ept.validate_4_level_write_back()?;
        Ok(VmxToyCapabilities {
            controls,
            ept,
            cr0_fixed: self.cr0_fixed,
            cr4_fixed: self.cr4_fixed,
        })
    }

    fn validate_basic(self) -> Result<(), VmxError> {
        if self.basic & VMX_BASIC_REVISION_MASK == 0 {
            return Err(VmxError::new(
                VmxErrorKind::InvalidRevisionId,
                "IA32_VMX_BASIC exposes revision id zero",
            ));
        }
        if !Self::uses_true_controls(self.basic) {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "toy VMX entry requires the true control MSRs",
            ));
        }
        let region_size =
            (self.basic >> VMX_BASIC_VMCS_REGION_SIZE_SHIFT) & VMX_BASIC_VMCS_REGION_SIZE_MASK;
        if region_size == 0 || region_size > VMX_REGION_PAGE_SIZE {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "VMX regions do not fit in one 4K runtime page",
            ));
        }
        let memory_type = (self.basic >> VMX_BASIC_MEMORY_TYPE_SHIFT) & VMX_BASIC_MEMORY_TYPE_MASK;
        if memory_type != VMX_MEMORY_TYPE_WRITE_BACK {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "VMX regions require a memory type other than write-back",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vmx::controls::{ENTRY_IA32E_MODE_GUEST, PRIMARY_HLT_EXITING, SECONDARY_ENABLE_EPT};
    use crate::vmx::ept::{EPT_VPID_CAP_MEMORY_TYPE_WB, EPT_VPID_CAP_PAGE_WALK_LENGTH_4};

    fn permissive_snapshot() -> VmxCapabilitySnapshot {
        VmxCapabilitySnapshot {
            basic: (4096 << VMX_BASIC_VMCS_REGION_SIZE_SHIFT)
                | (VMX_MEMORY_TYPE_WRITE_BACK << VMX_BASIC_MEMORY_TYPE_SHIFT)
                | VMX_BASIC_TRUE_CONTROLS
                | 1,
            pin_based: u64::from(u32::MAX) << 32,
            primary: u64::from(u32::MAX) << 32,
            secondary: u64::from(u32::MAX) << 32,
            exit: u64::from(u32::MAX) << 32,
            entry: u64::from(u32::MAX) << 32,
            ept_vpid: EPT_VPID_CAP_PAGE_WALK_LENGTH_4 | EPT_VPID_CAP_MEMORY_TYPE_WB,
            cr0_fixed: CrFixedBits::new(0x21, u64::MAX),
            cr4_fixed: CrFixedBits::new(0x2020, u64::MAX),
        }
    }

    #[test]
    fn basic_msr_selects_true_control_family() {
        assert!(VmxCapabilitySnapshot::uses_true_controls(
            permissive_snapshot().basic
        ));
        assert!(!VmxCapabilitySnapshot::uses_true_controls(0));
        assert!(VmxCapabilitySnapshot::control_allows(
            u64::from(1_u32 << 31) << 32,
            1 << 31
        ));
    }

    #[test]
    fn snapshot_prepares_the_exact_toy_guest_contract() {
        let prepared = permissive_snapshot().prepare_toy_guest().unwrap();

        assert_ne!(prepared.controls.primary & PRIMARY_HLT_EXITING, 0);
        assert_ne!(prepared.controls.secondary & SECONDARY_ENABLE_EPT, 0);
        assert_ne!(prepared.controls.entry & ENTRY_IA32E_MODE_GUEST, 0);
        assert!(prepared.ept.supports_4_level_walk());
        assert_eq!(prepared.cr4_fixed.fixed0, 0x2020);
    }

    #[test]
    fn snapshot_rejects_bad_region_and_ept_capabilities() {
        let mut snapshot = permissive_snapshot();
        snapshot.basic &= !(VMX_BASIC_VMCS_REGION_SIZE_MASK << VMX_BASIC_VMCS_REGION_SIZE_SHIFT);
        assert_eq!(
            snapshot.prepare_toy_guest().unwrap_err().kind,
            VmxErrorKind::UnsupportedCapability
        );

        let mut snapshot = permissive_snapshot();
        snapshot.ept_vpid = 0;
        assert_eq!(
            snapshot.prepare_toy_guest().unwrap_err().kind,
            VmxErrorKind::UnsupportedCapability
        );
    }
}
