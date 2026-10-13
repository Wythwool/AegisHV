use core::mem::{align_of, size_of};

use aegishv_hypervisor_core::ids::HostPhysical;

use super::features::{SvmError, SvmErrorKind};

pub const VMCB_SIZE: usize = 4096;
pub const VMCB_CONTROL_SIZE: usize = 1024;
pub const VMCB_STATE_SIZE: usize = 1024;
pub const VMCB_ALIGNMENT: u64 = 4096;

pub const VMCB_INTERCEPT_CR_READ_OFFSET: usize = 0x000;
pub const VMCB_INTERCEPT_CR_WRITE_OFFSET: usize = 0x002;
pub const VMCB_INTERCEPT_MISC1_OFFSET: usize = 0x00c;
pub const VMCB_IOPM_BASE_PA_OFFSET: usize = 0x040;
pub const VMCB_MSRPM_BASE_PA_OFFSET: usize = 0x048;
pub const VMCB_ASID_OFFSET: usize = 0x058;
pub const VMCB_TLB_CONTROL_OFFSET: usize = 0x05c;
pub const VMCB_EXIT_CODE_OFFSET: usize = 0x070;
pub const VMCB_EXIT_INFO1_OFFSET: usize = 0x078;
pub const VMCB_EXIT_INFO2_OFFSET: usize = 0x080;
pub const VMCB_NP_ENABLE_OFFSET: usize = 0x090;
pub const VMCB_N_CR3_OFFSET: usize = 0x0b0;

pub const VMCB_STATE_EFER_OFFSET: usize = 0x0d0;
pub const VMCB_STATE_CR0_OFFSET: usize = 0x148;
pub const VMCB_STATE_CR3_OFFSET: usize = 0x158;
pub const VMCB_STATE_CR4_OFFSET: usize = 0x160;
pub const VMCB_STATE_RSP_OFFSET: usize = 0x1d8;
pub const VMCB_STATE_RIP_OFFSET: usize = 0x1f8;
pub const VMCB_STATE_RFLAGS_OFFSET: usize = 0x200;

pub const INTERCEPT_CPUID: u64 = 1 << 18;
pub const INTERCEPT_HLT: u64 = 1 << 24;
pub const INTERCEPT_PAUSE: u64 = 1 << 25;
pub const INTERCEPT_IOIO: u64 = 1 << 27;
pub const INTERCEPT_MSR: u64 = 1 << 28;

#[repr(C)]
#[derive(Clone, PartialEq, Eq)]
pub struct VmcbControlArea {
    bytes: [u8; VMCB_CONTROL_SIZE],
}

impl VmcbControlArea {
    pub const fn zeroed() -> Self {
        Self {
            bytes: [0; VMCB_CONTROL_SIZE],
        }
    }

    pub fn bytes(&self) -> &[u8; VMCB_CONTROL_SIZE] {
        &self.bytes
    }

    pub fn set_intercept_cr_read(&mut self, mask: u16) {
        self.write_u16(VMCB_INTERCEPT_CR_READ_OFFSET, mask);
    }

    pub fn intercept_cr_read(&self) -> u16 {
        self.read_u16(VMCB_INTERCEPT_CR_READ_OFFSET)
    }

    pub fn set_intercept_cr_write(&mut self, mask: u16) {
        self.write_u16(VMCB_INTERCEPT_CR_WRITE_OFFSET, mask);
    }

    pub fn intercept_cr_write(&self) -> u16 {
        self.read_u16(VMCB_INTERCEPT_CR_WRITE_OFFSET)
    }

    pub fn set_misc_intercepts(&mut self, mask: u64) {
        self.write_u64(VMCB_INTERCEPT_MISC1_OFFSET, mask);
    }

    pub fn misc_intercepts(&self) -> u64 {
        self.read_u64(VMCB_INTERCEPT_MISC1_OFFSET)
    }

    pub fn set_io_permission_map(&mut self, address: HostPhysical) {
        self.write_u64(VMCB_IOPM_BASE_PA_OFFSET, address.get());
    }

    pub fn set_msr_permission_map(&mut self, address: HostPhysical) {
        self.write_u64(VMCB_MSRPM_BASE_PA_OFFSET, address.get());
    }

    pub fn set_guest_asid(&mut self, asid: u32) {
        self.write_u32(VMCB_ASID_OFFSET, asid);
    }

    pub fn guest_asid(&self) -> u32 {
        self.read_u32(VMCB_ASID_OFFSET)
    }

    pub fn set_tlb_control(&mut self, value: u32) {
        self.write_u32(VMCB_TLB_CONTROL_OFFSET, value);
    }

    pub fn set_exit_code(&mut self, value: u64) {
        self.write_u64(VMCB_EXIT_CODE_OFFSET, value);
    }

    pub fn exit_code(&self) -> u64 {
        self.read_u64(VMCB_EXIT_CODE_OFFSET)
    }

    pub fn set_exit_info(&mut self, info1: u64, info2: u64) {
        self.write_u64(VMCB_EXIT_INFO1_OFFSET, info1);
        self.write_u64(VMCB_EXIT_INFO2_OFFSET, info2);
    }

    pub fn exit_info1(&self) -> u64 {
        self.read_u64(VMCB_EXIT_INFO1_OFFSET)
    }

    pub fn exit_info2(&self) -> u64 {
        self.read_u64(VMCB_EXIT_INFO2_OFFSET)
    }

    pub fn set_nested_paging(&mut self, enabled: bool, root: HostPhysical) {
        self.write_u64(VMCB_NP_ENABLE_OFFSET, u64::from(enabled));
        self.write_u64(VMCB_N_CR3_OFFSET, root.get());
    }

    pub fn nested_paging_enabled(&self) -> bool {
        self.read_u64(VMCB_NP_ENABLE_OFFSET) & 1 != 0
    }

    pub fn nested_cr3(&self) -> u64 {
        self.read_u64(VMCB_N_CR3_OFFSET)
    }

    fn read_u16(&self, offset: usize) -> u16 {
        let bytes = [self.bytes[offset], self.bytes[offset + 1]];
        u16::from_le_bytes(bytes)
    }

    fn read_u32(&self, offset: usize) -> u32 {
        let bytes = [
            self.bytes[offset],
            self.bytes[offset + 1],
            self.bytes[offset + 2],
            self.bytes[offset + 3],
        ];
        u32::from_le_bytes(bytes)
    }

    fn read_u64(&self, offset: usize) -> u64 {
        let bytes = [
            self.bytes[offset],
            self.bytes[offset + 1],
            self.bytes[offset + 2],
            self.bytes[offset + 3],
            self.bytes[offset + 4],
            self.bytes[offset + 5],
            self.bytes[offset + 6],
            self.bytes[offset + 7],
        ];
        u64::from_le_bytes(bytes)
    }

    fn write_u16(&mut self, offset: usize, value: u16) {
        self.bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u32(&mut self, offset: usize, value: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u64(&mut self, offset: usize, value: u64) {
        self.bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
}

#[repr(C)]
#[derive(Clone, PartialEq, Eq)]
pub struct VmcbStateSaveArea {
    bytes: [u8; VMCB_STATE_SIZE],
}

impl VmcbStateSaveArea {
    pub const fn zeroed() -> Self {
        Self {
            bytes: [0; VMCB_STATE_SIZE],
        }
    }

    pub fn set_efer(&mut self, value: u64) {
        self.write_u64(VMCB_STATE_EFER_OFFSET, value);
    }

    pub fn efer(&self) -> u64 {
        self.read_u64(VMCB_STATE_EFER_OFFSET)
    }

    pub fn set_cr0(&mut self, value: u64) {
        self.write_u64(VMCB_STATE_CR0_OFFSET, value);
    }

    pub fn set_cr3(&mut self, value: u64) {
        self.write_u64(VMCB_STATE_CR3_OFFSET, value);
    }

    pub fn set_cr4(&mut self, value: u64) {
        self.write_u64(VMCB_STATE_CR4_OFFSET, value);
    }

    pub fn set_rsp(&mut self, value: u64) {
        self.write_u64(VMCB_STATE_RSP_OFFSET, value);
    }

    pub fn set_rip(&mut self, value: u64) {
        self.write_u64(VMCB_STATE_RIP_OFFSET, value);
    }

    pub fn rip(&self) -> u64 {
        self.read_u64(VMCB_STATE_RIP_OFFSET)
    }

    pub fn set_rflags(&mut self, value: u64) {
        self.write_u64(VMCB_STATE_RFLAGS_OFFSET, value);
    }

    fn read_u64(&self, offset: usize) -> u64 {
        let bytes = [
            self.bytes[offset],
            self.bytes[offset + 1],
            self.bytes[offset + 2],
            self.bytes[offset + 3],
            self.bytes[offset + 4],
            self.bytes[offset + 5],
            self.bytes[offset + 6],
            self.bytes[offset + 7],
        ];
        u64::from_le_bytes(bytes)
    }

    fn write_u64(&mut self, offset: usize, value: u64) {
        self.bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
}

#[repr(C, align(4096))]
#[derive(Clone, PartialEq, Eq)]
pub struct Vmcb {
    pub control: VmcbControlArea,
    pub state: VmcbStateSaveArea,
    reserved: [u8; VMCB_SIZE - VMCB_CONTROL_SIZE - VMCB_STATE_SIZE],
}

impl Vmcb {
    pub const fn zeroed() -> Self {
        Self {
            control: VmcbControlArea::zeroed(),
            state: VmcbStateSaveArea::zeroed(),
            reserved: [0; VMCB_SIZE - VMCB_CONTROL_SIZE - VMCB_STATE_SIZE],
        }
    }

    pub fn validate_physical_address(address: HostPhysical) -> Result<HostPhysical, SvmError> {
        if address.get() == 0 || address.get() % VMCB_ALIGNMENT != 0 {
            return Err(SvmError::new(
                SvmErrorKind::InvalidVmcbAddress,
                "VMCB physical address must be non-zero and 4K-aligned",
            ));
        }
        Ok(address)
    }

    pub fn require_hlt_intercept(&self) -> Result<(), SvmError> {
        if self.control.misc_intercepts() & INTERCEPT_HLT == 0 {
            return Err(SvmError::new(
                SvmErrorKind::InvalidIntercept,
                "toy SVM lab VMCB must intercept HLT",
            ));
        }
        Ok(())
    }
}

pub const fn vmcb_layout() -> (usize, usize, usize) {
    (
        size_of::<Vmcb>(),
        align_of::<Vmcb>(),
        size_of::<VmcbControlArea>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vmcb_layout_is_one_aligned_page() {
        assert_eq!(size_of::<Vmcb>(), VMCB_SIZE);
        assert_eq!(align_of::<Vmcb>(), VMCB_ALIGNMENT as usize);
        assert_eq!(size_of::<VmcbControlArea>(), VMCB_CONTROL_SIZE);
        assert_eq!(size_of::<VmcbStateSaveArea>(), VMCB_STATE_SIZE);
    }

    #[test]
    fn control_area_accessors_write_known_offsets() {
        let mut control = VmcbControlArea::zeroed();
        control.set_intercept_cr_read(1 << 3);
        control.set_misc_intercepts(INTERCEPT_CPUID | INTERCEPT_HLT);
        control.set_guest_asid(7);
        control.set_exit_info(0x11, 0x22);

        assert_eq!(control.intercept_cr_read(), 1 << 3);
        assert_ne!(control.misc_intercepts() & INTERCEPT_CPUID, 0);
        assert_eq!(control.guest_asid(), 7);
        assert_eq!(control.exit_info1(), 0x11);
        assert_eq!(control.exit_info2(), 0x22);
    }

    #[test]
    fn vmcb_address_validation_rejects_zero_and_misaligned() {
        assert_eq!(
            Vmcb::validate_physical_address(HostPhysical::ZERO)
                .unwrap_err()
                .kind,
            SvmErrorKind::InvalidVmcbAddress
        );
        assert_eq!(
            Vmcb::validate_physical_address(HostPhysical::new(0x2100).unwrap())
                .unwrap_err()
                .kind,
            SvmErrorKind::InvalidVmcbAddress
        );
    }

    #[test]
    fn hlt_intercept_is_required_for_toy_lab() {
        let mut vmcb = Vmcb::zeroed();
        assert_eq!(
            vmcb.require_hlt_intercept().unwrap_err().kind,
            SvmErrorKind::InvalidIntercept
        );
        vmcb.control.set_misc_intercepts(INTERCEPT_HLT);
        vmcb.require_hlt_intercept().unwrap();
    }
}
