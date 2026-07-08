#![no_std]

use aegishv_arch_x86::svm::features::SvmErrorKind;
use aegishv_arch_x86::svm::runtime::SvmRuntime;
use aegishv_arch_x86::vmx::features::VmxErrorKind;
use aegishv_arch_x86::vmx::region::{VmxRevisionId, VmxonRegion};
use aegishv_arch_x86::vmx::runtime::VmxRuntime;
use aegishv_arch_x86::vmx::vmcs::VmcsRegion;
use aegishv_hypervisor_core::ids::HostPhysical;

pub const SERIAL_READY_MARKER: &str = "aegishv:type1:halt";
pub const SERIAL_PANIC_MARKER: &str = "aegishv:type1:panic";
pub const SERIAL_LIMINE_MISSING_MARKER: &str = "aegishv:type1:limine-missing";
pub const SERIAL_RUNTIME_BACKEND_NONE_MARKER: &str = "aegishv:type1:backend-none";
pub const SERIAL_RUNTIME_BACKEND_VMX_MARKER: &str = "aegishv:type1:backend-vmx";
pub const SERIAL_RUNTIME_BACKEND_SVM_MARKER: &str = "aegishv:type1:backend-svm";
pub const SERIAL_RUNTIME_PLAN_ERROR_MARKER: &str = "aegishv:type1:runtime-plan-error";
pub const SERIAL_LIMINE_BASE_REVISION_MARKER: &str = "aegishv:type1:limine-base-revision";
pub const SERIAL_LIMINE_HHDM_MISSING_MARKER: &str = "aegishv:type1:limine-hhdm-missing";
pub const SERIAL_LIMINE_HHDM_REVISION_MARKER: &str = "aegishv:type1:limine-hhdm-revision";
pub const SERIAL_LIMINE_HHDM_OFFSET_MARKER: &str = "aegishv:type1:limine-hhdm-offset";
pub const SERIAL_LIMINE_MEMMAP_MISSING_MARKER: &str = "aegishv:type1:limine-memmap-missing";
pub const SERIAL_LIMINE_MEMMAP_REVISION_MARKER: &str = "aegishv:type1:limine-memmap-revision";
pub const SERIAL_LIMINE_MEMMAP_EMPTY_MARKER: &str = "aegishv:type1:limine-memmap-empty";
pub const SERIAL_LIMINE_MEMMAP_ENTRIES_MARKER: &str = "aegishv:type1:limine-memmap-entries";
pub const SERIAL_LIMINE_EXECUTABLE_MISSING_MARKER: &str = "aegishv:type1:limine-executable-missing";
pub const SERIAL_LIMINE_EXECUTABLE_REVISION_MARKER: &str =
    "aegishv:type1:limine-executable-revision";
pub const SERIAL_LIMINE_EXECUTABLE_EMPTY_MARKER: &str = "aegishv:type1:limine-executable-empty";
pub const SERIAL_LIMINE_EXECUTABLE_PHYSICAL_MARKER: &str =
    "aegishv:type1:limine-executable-physical";
pub const SERIAL_LIMINE_EXECUTABLE_VIRTUAL_MARKER: &str = "aegishv:type1:limine-executable-virtual";
pub const LIMINE_BASE_REVISION: u64 = 6;
pub const LIMINE_REQUEST_COUNT: usize = 6;
pub const LIMINE_RESPONSE_REVISION_OFFSET: usize = 0;
pub const LIMINE_HHDM_OFFSET_OFFSET: usize = 8;
pub const LIMINE_MEMMAP_ENTRY_COUNT_OFFSET: usize = 8;
pub const LIMINE_MEMMAP_ENTRIES_OFFSET: usize = 16;
pub const LIMINE_EXECUTABLE_PHYSICAL_BASE_OFFSET: usize = 8;
pub const LIMINE_EXECUTABLE_VIRTUAL_BASE_OFFSET: usize = 16;
pub const TYPE1_RUNTIME_REGION_BASE_OFFSET: u64 = 0x80_000;
pub const TYPE1_RUNTIME_PAGE_SIZE: u64 = 4096;
pub const TYPE1_VMXON_REGION_OFFSET: u64 = TYPE1_RUNTIME_REGION_BASE_OFFSET;
pub const TYPE1_VMCS_REGION_OFFSET: u64 =
    TYPE1_RUNTIME_REGION_BASE_OFFSET + TYPE1_RUNTIME_PAGE_SIZE;
pub const TYPE1_SVM_VMCB_REGION_OFFSET: u64 =
    TYPE1_RUNTIME_REGION_BASE_OFFSET + (2 * TYPE1_RUNTIME_PAGE_SIZE);

pub const LIMINE_REQUESTS_START_MARKER: [u64; 4] = [
    0xf6b8_f4b3_9de7_d1ae,
    0xfab9_1a69_40fc_b9cf,
    0x785c_6ed0_15d3_e316,
    0x181e_920a_7852_b9d9,
];
pub const LIMINE_REQUESTS_END_MARKER: [u64; 2] = [0xadc0_e053_1bb1_0d03, 0x9572_709f_3176_4c62];

const LIMINE_COMMON_MAGIC: [u64; 2] = [0xc7b1_dd30_df4c_8b88, 0x0a82_e883_a194_f07b];

pub const LIMINE_BOOTLOADER_INFO_REQUEST_ID: [u64; 4] = [
    LIMINE_COMMON_MAGIC[0],
    LIMINE_COMMON_MAGIC[1],
    0xf550_38d8_e2a1_202f,
    0x2794_26fc_f5f5_9740,
];
pub const LIMINE_EXECUTABLE_CMDLINE_REQUEST_ID: [u64; 4] = [
    LIMINE_COMMON_MAGIC[0],
    LIMINE_COMMON_MAGIC[1],
    0x4b16_1536_e598_651e,
    0xb390_ad4a_2f1f_303a,
];
pub const LIMINE_HHDM_REQUEST_ID: [u64; 4] = [
    LIMINE_COMMON_MAGIC[0],
    LIMINE_COMMON_MAGIC[1],
    0x48dc_f1cb_8ad2_b852,
    0x6398_4e95_9a98_244b,
];
pub const LIMINE_MEMMAP_REQUEST_ID: [u64; 4] = [
    LIMINE_COMMON_MAGIC[0],
    LIMINE_COMMON_MAGIC[1],
    0x67cf_3d9d_378a_806f,
    0xe304_acdf_c50c_3c62,
];
pub const LIMINE_RSDP_REQUEST_ID: [u64; 4] = [
    LIMINE_COMMON_MAGIC[0],
    LIMINE_COMMON_MAGIC[1],
    0xc5e7_7b6b_397e_7b43,
    0x2763_7845_accd_cf3c,
];
pub const LIMINE_EXECUTABLE_ADDRESS_REQUEST_ID: [u64; 4] = [
    LIMINE_COMMON_MAGIC[0],
    LIMINE_COMMON_MAGIC[1],
    0x71ba_7686_3cc5_5f63,
    0xb264_4a48_c516_a487,
];

pub const LIMINE_BOOT_REQUEST_IDS: [[u64; 4]; LIMINE_REQUEST_COUNT] = [
    LIMINE_BOOTLOADER_INFO_REQUEST_ID,
    LIMINE_EXECUTABLE_CMDLINE_REQUEST_ID,
    LIMINE_HHDM_REQUEST_ID,
    LIMINE_MEMMAP_REQUEST_ID,
    LIMINE_RSDP_REQUEST_ID,
    LIMINE_EXECUTABLE_ADDRESS_REQUEST_ID,
];

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: u64,
}

impl LimineRequest {
    pub const fn new(id: [u64; 4]) -> Self {
        Self {
            id,
            revision: 0,
            response: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineHhdmResponse {
    pub revision: u64,
    pub offset: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineMemmapResponse {
    pub revision: u64,
    pub entry_count: u64,
    pub entries: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineExecutableAddressResponse {
    pub revision: u64,
    pub physical_base: u64,
    pub virtual_base: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineMinimalHandoff {
    pub base_revision_value: u64,
    pub hhdm_response: u64,
    pub hhdm_revision: u64,
    pub hhdm_offset: u64,
    pub memmap_response: u64,
    pub memmap_revision: u64,
    pub memmap_entry_count: u64,
    pub memmap_entries: u64,
    pub executable_address_response: u64,
    pub executable_address_revision: u64,
    pub executable_physical_base: u64,
    pub executable_virtual_base: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimineHandoffStatus {
    Ready,
    BaseRevisionUnsupported,
    HhdmResponseMissing,
    HhdmRevisionUnsupported,
    HhdmOffsetMissing,
    MemmapResponseMissing,
    MemmapRevisionUnsupported,
    MemmapEmpty,
    MemmapEntriesMissing,
    ExecutableAddressResponseMissing,
    ExecutableAddressRevisionUnsupported,
    ExecutableAddressEmpty,
    ExecutablePhysicalBaseMismatch,
    ExecutableVirtualBaseMismatch,
}

impl LimineHandoffStatus {
    pub const fn is_ready(self) -> bool {
        matches!(self, Self::Ready)
    }

    pub const fn serial_marker(self) -> &'static str {
        match self {
            Self::Ready => SERIAL_READY_MARKER,
            Self::BaseRevisionUnsupported => SERIAL_LIMINE_BASE_REVISION_MARKER,
            Self::HhdmResponseMissing => SERIAL_LIMINE_HHDM_MISSING_MARKER,
            Self::HhdmRevisionUnsupported => SERIAL_LIMINE_HHDM_REVISION_MARKER,
            Self::HhdmOffsetMissing => SERIAL_LIMINE_HHDM_OFFSET_MARKER,
            Self::MemmapResponseMissing => SERIAL_LIMINE_MEMMAP_MISSING_MARKER,
            Self::MemmapRevisionUnsupported => SERIAL_LIMINE_MEMMAP_REVISION_MARKER,
            Self::MemmapEmpty => SERIAL_LIMINE_MEMMAP_EMPTY_MARKER,
            Self::MemmapEntriesMissing => SERIAL_LIMINE_MEMMAP_ENTRIES_MARKER,
            Self::ExecutableAddressResponseMissing => SERIAL_LIMINE_EXECUTABLE_MISSING_MARKER,
            Self::ExecutableAddressRevisionUnsupported => SERIAL_LIMINE_EXECUTABLE_REVISION_MARKER,
            Self::ExecutableAddressEmpty => SERIAL_LIMINE_EXECUTABLE_EMPTY_MARKER,
            Self::ExecutablePhysicalBaseMismatch => SERIAL_LIMINE_EXECUTABLE_PHYSICAL_MARKER,
            Self::ExecutableVirtualBaseMismatch => SERIAL_LIMINE_EXECUTABLE_VIRTUAL_MARKER,
        }
    }
}

pub const fn limine_minimal_handoff_status(handoff: LimineMinimalHandoff) -> LimineHandoffStatus {
    if handoff.base_revision_value != 0 {
        return LimineHandoffStatus::BaseRevisionUnsupported;
    }
    if handoff.hhdm_response == 0 {
        return LimineHandoffStatus::HhdmResponseMissing;
    }
    if handoff.hhdm_revision != 0 {
        return LimineHandoffStatus::HhdmRevisionUnsupported;
    }
    if handoff.hhdm_offset == 0 {
        return LimineHandoffStatus::HhdmOffsetMissing;
    }
    if handoff.memmap_response == 0 {
        return LimineHandoffStatus::MemmapResponseMissing;
    }
    if handoff.memmap_revision != 0 {
        return LimineHandoffStatus::MemmapRevisionUnsupported;
    }
    if handoff.memmap_entry_count == 0 {
        return LimineHandoffStatus::MemmapEmpty;
    }
    if handoff.memmap_entries == 0 {
        return LimineHandoffStatus::MemmapEntriesMissing;
    }
    if handoff.executable_address_response == 0 {
        return LimineHandoffStatus::ExecutableAddressResponseMissing;
    }
    if handoff.executable_address_revision != 0 {
        return LimineHandoffStatus::ExecutableAddressRevisionUnsupported;
    }
    if handoff.executable_physical_base == 0 || handoff.executable_virtual_base == 0 {
        return LimineHandoffStatus::ExecutableAddressEmpty;
    }
    if handoff.executable_physical_base != aegishv_type1_boot::layout::KERNEL_PHYSICAL_BASE {
        return LimineHandoffStatus::ExecutablePhysicalBaseMismatch;
    }
    if handoff.executable_virtual_base != aegishv_type1_boot::layout::KERNEL_VIRTUAL_BASE {
        return LimineHandoffStatus::ExecutableVirtualBaseMismatch;
    }
    LimineHandoffStatus::Ready
}

pub const fn limine_base_revision_tag() -> [u64; 3] {
    [
        0xf956_2b2d_5c95_a6c8,
        0x6a7b_3849_4453_6bdc,
        LIMINE_BASE_REVISION,
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1ArchCapabilities {
    pub intel_vmx: bool,
    pub amd_svm: bool,
}

impl Type1ArchCapabilities {
    pub const fn none() -> Self {
        Self {
            intel_vmx: false,
            amd_svm: false,
        }
    }

    pub const fn intel_vmx() -> Self {
        Self {
            intel_vmx: true,
            amd_svm: false,
        }
    }

    pub const fn amd_svm() -> Self {
        Self {
            intel_vmx: false,
            amd_svm: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1BackendRequest {
    Auto,
    IntelVmx,
    AmdSvm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1RuntimeBackend {
    None,
    IntelVmx,
    AmdSvm,
}

impl Type1RuntimeBackend {
    pub const fn serial_marker(self) -> &'static str {
        match self {
            Self::None => SERIAL_RUNTIME_BACKEND_NONE_MARKER,
            Self::IntelVmx => SERIAL_RUNTIME_BACKEND_VMX_MARKER,
            Self::AmdSvm => SERIAL_RUNTIME_BACKEND_SVM_MARKER,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1RuntimeMemoryPlan {
    pub runtime_base: u64,
    pub vmxon_physical: u64,
    pub vmcs_physical: u64,
    pub svm_vmcb_physical: u64,
}

impl Type1RuntimeMemoryPlan {
    pub const fn from_executable_base(
        executable_physical_base: u64,
    ) -> Result<Self, Type1RuntimePlanError> {
        let runtime_base =
            match executable_physical_base.checked_add(TYPE1_RUNTIME_REGION_BASE_OFFSET) {
                Some(value) => value,
                None => return Err(Type1RuntimePlanError::RuntimeAddressOverflow),
            };
        let vmxon_physical = match executable_physical_base.checked_add(TYPE1_VMXON_REGION_OFFSET) {
            Some(value) => value,
            None => return Err(Type1RuntimePlanError::RuntimeAddressOverflow),
        };
        let vmcs_physical = match executable_physical_base.checked_add(TYPE1_VMCS_REGION_OFFSET) {
            Some(value) => value,
            None => return Err(Type1RuntimePlanError::RuntimeAddressOverflow),
        };
        let svm_vmcb_physical =
            match executable_physical_base.checked_add(TYPE1_SVM_VMCB_REGION_OFFSET) {
                Some(value) => value,
                None => return Err(Type1RuntimePlanError::RuntimeAddressOverflow),
            };
        if runtime_base % TYPE1_RUNTIME_PAGE_SIZE != 0
            || vmxon_physical % TYPE1_RUNTIME_PAGE_SIZE != 0
            || vmcs_physical % TYPE1_RUNTIME_PAGE_SIZE != 0
            || svm_vmcb_physical % TYPE1_RUNTIME_PAGE_SIZE != 0
        {
            return Err(Type1RuntimePlanError::RuntimeAddressMisaligned);
        }
        Ok(Self {
            runtime_base,
            vmxon_physical,
            vmcs_physical,
            svm_vmcb_physical,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1RuntimePlan {
    pub backend: Type1RuntimeBackend,
    pub memory: Type1RuntimeMemoryPlan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1RuntimePlanError {
    Handoff(LimineHandoffStatus),
    MixedVendorCapabilities,
    MissingIntelVmx,
    MissingAmdSvm,
    RuntimeAddressOverflow,
    RuntimeAddressMisaligned,
    InvalidRuntimeAddress,
    BackendMismatch,
    Vmx(VmxErrorKind),
    Svm(SvmErrorKind),
}

pub const fn select_type1_runtime_backend(
    request: Type1BackendRequest,
    capabilities: Type1ArchCapabilities,
) -> Result<Type1RuntimeBackend, Type1RuntimePlanError> {
    match request {
        Type1BackendRequest::Auto => match (capabilities.intel_vmx, capabilities.amd_svm) {
            (false, false) => Ok(Type1RuntimeBackend::None),
            (true, false) => Ok(Type1RuntimeBackend::IntelVmx),
            (false, true) => Ok(Type1RuntimeBackend::AmdSvm),
            (true, true) => Err(Type1RuntimePlanError::MixedVendorCapabilities),
        },
        Type1BackendRequest::IntelVmx => {
            if capabilities.intel_vmx {
                Ok(Type1RuntimeBackend::IntelVmx)
            } else {
                Err(Type1RuntimePlanError::MissingIntelVmx)
            }
        }
        Type1BackendRequest::AmdSvm => {
            if capabilities.amd_svm {
                Ok(Type1RuntimeBackend::AmdSvm)
            } else {
                Err(Type1RuntimePlanError::MissingAmdSvm)
            }
        }
    }
}

pub const fn plan_type1_runtime(
    handoff: LimineMinimalHandoff,
    request: Type1BackendRequest,
    capabilities: Type1ArchCapabilities,
) -> Result<Type1RuntimePlan, Type1RuntimePlanError> {
    let status = limine_minimal_handoff_status(handoff);
    if !status.is_ready() {
        return Err(Type1RuntimePlanError::Handoff(status));
    }
    let memory =
        match Type1RuntimeMemoryPlan::from_executable_base(handoff.executable_physical_base) {
            Ok(plan) => plan,
            Err(err) => return Err(err),
        };
    let backend = match select_type1_runtime_backend(request, capabilities) {
        Ok(backend) => backend,
        Err(err) => return Err(err),
    };
    Ok(Type1RuntimePlan { backend, memory })
}

pub fn build_vmx_runtime(
    plan: Type1RuntimePlan,
    revision_id: u32,
) -> Result<VmxRuntime, Type1RuntimePlanError> {
    if plan.backend != Type1RuntimeBackend::IntelVmx {
        return Err(Type1RuntimePlanError::BackendMismatch);
    }
    let revision =
        VmxRevisionId::new(revision_id).map_err(|err| Type1RuntimePlanError::Vmx(err.kind))?;
    let vmxon = VmxonRegion::new(host_physical(plan.memory.vmxon_physical)?, revision)
        .map_err(|err| Type1RuntimePlanError::Vmx(err.kind))?;
    let vmcs = VmcsRegion::allocate(host_physical(plan.memory.vmcs_physical)?, revision)
        .map_err(|err| Type1RuntimePlanError::Vmx(err.kind))?;
    VmxRuntime::new(vmxon, vmcs).map_err(|err| Type1RuntimePlanError::Vmx(err.kind))
}

pub fn build_svm_runtime(plan: Type1RuntimePlan) -> Result<SvmRuntime, Type1RuntimePlanError> {
    if plan.backend != Type1RuntimeBackend::AmdSvm {
        return Err(Type1RuntimePlanError::BackendMismatch);
    }
    SvmRuntime::new(host_physical(plan.memory.svm_vmcb_physical)?)
        .map_err(|err| Type1RuntimePlanError::Svm(err.kind))
}

fn host_physical(raw: u64) -> Result<HostPhysical, Type1RuntimePlanError> {
    HostPhysical::new(raw).map_err(|_| Type1RuntimePlanError::InvalidRuntimeAddress)
}

pub const fn serial_ready_marker() -> &'static str {
    SERIAL_READY_MARKER
}

pub const fn serial_panic_marker() -> &'static str {
    SERIAL_PANIC_MARKER
}

pub const fn serial_limine_missing_marker() -> &'static str {
    SERIAL_LIMINE_MISSING_MARKER
}

pub fn marker_line(marker: &str, out: &mut [u8]) -> Option<usize> {
    let bytes = marker.as_bytes();
    if out.len() < bytes.len() + 1 {
        return None;
    }
    let mut index = 0;
    while index < bytes.len() {
        out[index] = bytes[index];
        index += 1;
    }
    out[index] = b'\n';
    Some(index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_line_appends_newline_without_allocation() {
        let mut out = [0u8; 32];
        let len = marker_line(SERIAL_READY_MARKER, &mut out).unwrap();

        assert_eq!(&out[..len], b"aegishv:type1:halt\n");
    }

    #[test]
    fn marker_line_rejects_short_buffer() {
        let mut out = [0u8; 4];

        assert_eq!(marker_line(SERIAL_READY_MARKER, &mut out), None);
    }

    #[test]
    fn marker_line_supports_limine_missing_marker() {
        let mut out = [0u8; 40];
        let len = marker_line(SERIAL_LIMINE_MISSING_MARKER, &mut out).unwrap();

        assert_eq!(&out[..len], b"aegishv:type1:limine-missing\n");
    }

    #[test]
    fn runtime_backend_markers_fit_serial_line_buffer() {
        let mut out = [0u8; 64];
        let len = marker_line(SERIAL_RUNTIME_BACKEND_NONE_MARKER, &mut out).unwrap();

        assert_eq!(&out[..len], b"aegishv:type1:backend-none\n");
        assert_eq!(
            Type1RuntimeBackend::IntelVmx.serial_marker(),
            "aegishv:type1:backend-vmx"
        );
        assert_eq!(
            Type1RuntimeBackend::AmdSvm.serial_marker(),
            "aegishv:type1:backend-svm"
        );
    }

    #[test]
    fn handoff_statuses_have_stable_serial_markers() {
        assert_eq!(
            LimineHandoffStatus::Ready.serial_marker(),
            "aegishv:type1:halt"
        );
        assert_eq!(
            LimineHandoffStatus::BaseRevisionUnsupported.serial_marker(),
            "aegishv:type1:limine-base-revision"
        );
        assert_eq!(
            LimineHandoffStatus::HhdmResponseMissing.serial_marker(),
            "aegishv:type1:limine-hhdm-missing"
        );
        assert_eq!(
            LimineHandoffStatus::HhdmRevisionUnsupported.serial_marker(),
            "aegishv:type1:limine-hhdm-revision"
        );
        assert_eq!(
            LimineHandoffStatus::HhdmOffsetMissing.serial_marker(),
            "aegishv:type1:limine-hhdm-offset"
        );
        assert_eq!(
            LimineHandoffStatus::MemmapResponseMissing.serial_marker(),
            "aegishv:type1:limine-memmap-missing"
        );
        assert_eq!(
            LimineHandoffStatus::MemmapRevisionUnsupported.serial_marker(),
            "aegishv:type1:limine-memmap-revision"
        );
        assert_eq!(
            LimineHandoffStatus::MemmapEmpty.serial_marker(),
            "aegishv:type1:limine-memmap-empty"
        );
        assert_eq!(
            LimineHandoffStatus::MemmapEntriesMissing.serial_marker(),
            "aegishv:type1:limine-memmap-entries"
        );
        assert_eq!(
            LimineHandoffStatus::ExecutableAddressResponseMissing.serial_marker(),
            "aegishv:type1:limine-executable-missing"
        );
        assert_eq!(
            LimineHandoffStatus::ExecutableAddressRevisionUnsupported.serial_marker(),
            "aegishv:type1:limine-executable-revision"
        );
        assert_eq!(
            LimineHandoffStatus::ExecutableAddressEmpty.serial_marker(),
            "aegishv:type1:limine-executable-empty"
        );
        assert_eq!(
            LimineHandoffStatus::ExecutablePhysicalBaseMismatch.serial_marker(),
            "aegishv:type1:limine-executable-physical"
        );
        assert_eq!(
            LimineHandoffStatus::ExecutableVirtualBaseMismatch.serial_marker(),
            "aegishv:type1:limine-executable-virtual"
        );
    }

    #[test]
    fn limine_request_ids_cover_minimal_boot_handoff_inputs() {
        assert_eq!(LIMINE_REQUEST_COUNT, 6);
        assert_eq!(
            LIMINE_MEMMAP_REQUEST_ID,
            [
                0xc7b1_dd30_df4c_8b88,
                0x0a82_e883_a194_f07b,
                0x67cf_3d9d_378a_806f,
                0xe304_acdf_c50c_3c62
            ]
        );
        assert!(LIMINE_BOOT_REQUEST_IDS.contains(&LIMINE_HHDM_REQUEST_ID));
        assert!(LIMINE_BOOT_REQUEST_IDS.contains(&LIMINE_EXECUTABLE_ADDRESS_REQUEST_ID));
    }

    #[test]
    fn limine_base_revision_tag_uses_current_revision() {
        let tag = limine_base_revision_tag();

        assert_eq!(tag[0], 0xf956_2b2d_5c95_a6c8);
        assert_eq!(tag[1], 0x6a7b_3849_4453_6bdc);
        assert_eq!(tag[2], LIMINE_BASE_REVISION);
    }

    #[test]
    fn generic_limine_request_starts_with_id_revision_and_response() {
        let request = LimineRequest::new(LIMINE_RSDP_REQUEST_ID);

        assert_eq!(request.id, LIMINE_RSDP_REQUEST_ID);
        assert_eq!(request.revision, 0);
        assert_eq!(request.response, 0);
        assert_eq!(core::mem::size_of::<LimineRequest>(), 48);
        assert_eq!(core::mem::align_of::<LimineRequest>(), 8);
    }

    #[test]
    fn limine_response_structs_match_expected_offsets() {
        assert_eq!(
            LIMINE_RESPONSE_REVISION_OFFSET,
            core::mem::offset_of!(LimineHhdmResponse, revision)
        );
        assert_eq!(
            LIMINE_HHDM_OFFSET_OFFSET,
            core::mem::offset_of!(LimineHhdmResponse, offset)
        );
        assert_eq!(
            LIMINE_MEMMAP_ENTRY_COUNT_OFFSET,
            core::mem::offset_of!(LimineMemmapResponse, entry_count)
        );
        assert_eq!(
            LIMINE_MEMMAP_ENTRIES_OFFSET,
            core::mem::offset_of!(LimineMemmapResponse, entries)
        );
        assert_eq!(
            LIMINE_EXECUTABLE_PHYSICAL_BASE_OFFSET,
            core::mem::offset_of!(LimineExecutableAddressResponse, physical_base)
        );
        assert_eq!(
            LIMINE_EXECUTABLE_VIRTUAL_BASE_OFFSET,
            core::mem::offset_of!(LimineExecutableAddressResponse, virtual_base)
        );
    }

    #[test]
    fn limine_handoff_status_requires_each_minimal_response() {
        const READY_HANDOFF: LimineMinimalHandoff = LimineMinimalHandoff {
            base_revision_value: 0,
            hhdm_response: 1,
            hhdm_revision: 0,
            hhdm_offset: 0xffff_8000_0000_0000,
            memmap_response: 1,
            memmap_revision: 0,
            memmap_entry_count: 1,
            memmap_entries: 0xffff_8000_0010_0000,
            executable_address_response: 1,
            executable_address_revision: 0,
            executable_physical_base: aegishv_type1_boot::layout::KERNEL_PHYSICAL_BASE,
            executable_virtual_base: 0xffff_ffff_8020_0000,
        };

        assert!(limine_minimal_handoff_status(READY_HANDOFF).is_ready());

        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                base_revision_value: 6,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::BaseRevisionUnsupported
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                hhdm_response: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::HhdmResponseMissing
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                hhdm_revision: 1,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::HhdmRevisionUnsupported
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                hhdm_offset: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::HhdmOffsetMissing
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                memmap_response: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::MemmapResponseMissing
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                memmap_revision: 1,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::MemmapRevisionUnsupported
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                memmap_entry_count: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::MemmapEmpty
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                memmap_entries: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::MemmapEntriesMissing
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                executable_address_response: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::ExecutableAddressResponseMissing
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                executable_address_revision: 1,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::ExecutableAddressRevisionUnsupported
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                executable_physical_base: 0,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::ExecutableAddressEmpty
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                executable_physical_base: 0x30_0000,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::ExecutablePhysicalBaseMismatch
        );
        assert_eq!(
            limine_minimal_handoff_status(LimineMinimalHandoff {
                executable_virtual_base: 0xffff_ffff_8030_0000,
                ..READY_HANDOFF
            }),
            LimineHandoffStatus::ExecutableVirtualBaseMismatch
        );
    }

    fn ready_handoff() -> LimineMinimalHandoff {
        LimineMinimalHandoff {
            base_revision_value: 0,
            hhdm_response: 1,
            hhdm_revision: 0,
            hhdm_offset: 0xffff_8000_0000_0000,
            memmap_response: 1,
            memmap_revision: 0,
            memmap_entry_count: 1,
            memmap_entries: 0xffff_8000_0010_0000,
            executable_address_response: 1,
            executable_address_revision: 0,
            executable_physical_base: aegishv_type1_boot::layout::KERNEL_PHYSICAL_BASE,
            executable_virtual_base: aegishv_type1_boot::layout::KERNEL_VIRTUAL_BASE,
        }
    }

    #[test]
    fn runtime_plan_selects_backend_from_capabilities() {
        assert_eq!(
            select_type1_runtime_backend(Type1BackendRequest::Auto, Type1ArchCapabilities::none())
                .unwrap(),
            Type1RuntimeBackend::None
        );
        assert_eq!(
            select_type1_runtime_backend(
                Type1BackendRequest::Auto,
                Type1ArchCapabilities::intel_vmx()
            )
            .unwrap(),
            Type1RuntimeBackend::IntelVmx
        );
        assert_eq!(
            select_type1_runtime_backend(
                Type1BackendRequest::Auto,
                Type1ArchCapabilities::amd_svm()
            )
            .unwrap(),
            Type1RuntimeBackend::AmdSvm
        );
        assert_eq!(
            select_type1_runtime_backend(
                Type1BackendRequest::Auto,
                Type1ArchCapabilities {
                    intel_vmx: true,
                    amd_svm: true
                }
            )
            .unwrap_err(),
            Type1RuntimePlanError::MixedVendorCapabilities
        );
    }

    #[test]
    fn runtime_memory_plan_uses_page_aligned_regions_after_kernel() {
        let plan = plan_type1_runtime(
            ready_handoff(),
            Type1BackendRequest::Auto,
            Type1ArchCapabilities::intel_vmx(),
        )
        .unwrap();

        assert_eq!(plan.backend, Type1RuntimeBackend::IntelVmx);
        assert_eq!(plan.memory.runtime_base, 0x28_0000);
        assert_eq!(plan.memory.vmxon_physical, 0x28_0000);
        assert_eq!(plan.memory.vmcs_physical, 0x28_1000);
        assert_eq!(plan.memory.svm_vmcb_physical, 0x28_2000);
    }

    #[test]
    fn runtime_plan_rejects_unready_handoff() {
        let err = plan_type1_runtime(
            LimineMinimalHandoff {
                hhdm_response: 0,
                ..ready_handoff()
            },
            Type1BackendRequest::Auto,
            Type1ArchCapabilities::intel_vmx(),
        )
        .unwrap_err();

        assert_eq!(
            err,
            Type1RuntimePlanError::Handoff(LimineHandoffStatus::HhdmResponseMissing)
        );
    }

    #[test]
    fn runtime_plan_builds_vmx_runtime_object() {
        let plan = plan_type1_runtime(
            ready_handoff(),
            Type1BackendRequest::IntelVmx,
            Type1ArchCapabilities::intel_vmx(),
        )
        .unwrap();

        let runtime = build_vmx_runtime(plan, 0x33).unwrap();

        assert_eq!(runtime.vmxon_physical_address().get(), 0x28_0000);
        assert_eq!(runtime.vmcs_physical_address().get(), 0x28_1000);
    }

    #[test]
    fn runtime_plan_builds_svm_runtime_object() {
        let plan = plan_type1_runtime(
            ready_handoff(),
            Type1BackendRequest::AmdSvm,
            Type1ArchCapabilities::amd_svm(),
        )
        .unwrap();

        let runtime = build_svm_runtime(plan).unwrap();

        assert_eq!(runtime.vmcb_physical_address().get(), 0x28_2000);
    }
}
