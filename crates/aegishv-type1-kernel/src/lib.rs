#![no_std]

pub mod early_memory;
pub mod toy_guest;

pub use early_memory::{
    allocate_type1_runtime_memory, Type1EarlyMemoryError, Type1RuntimeMemoryAllocation,
    Type1ToyGuestHostPages, TYPE1_MAX_MEMORY_MAP_ENTRIES, TYPE1_RUNTIME_MAX_PHYSICAL_EXCLUSIVE,
    TYPE1_RUNTIME_MIN_PHYSICAL,
};
pub use toy_guest::{
    materialize_type1_toy_guest, Type1PageTableWrite, Type1PhysicalPageWriter,
    Type1ToyGuestBuildPlan, Type1ToyGuestError, TYPE1_TOY_CODE, TYPE1_TOY_CODE_GPA,
    TYPE1_TOY_CPUID_RIP, TYPE1_TOY_GUEST_PML4_GPA, TYPE1_TOY_GUEST_RIP, TYPE1_TOY_GUEST_RSP,
    TYPE1_TOY_HLT_RIP, TYPE1_TOY_STACK_GPA,
};

use aegishv_arch_x86::svm::features::EferValue;
use aegishv_arch_x86::svm::features::{SvmCpuidExt1, SvmCpuidLeaf, SvmErrorKind, SvmFeatureSet};
use aegishv_arch_x86::svm::runtime::SvmRuntime;
use aegishv_arch_x86::vmx::features::{CpuidLeaf1, FeatureControlMsr, VmxErrorKind, VmxFeatureSet};
use aegishv_arch_x86::vmx::instructions::VmxInstructionExecutor;
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
pub const SERIAL_RUNTIME_PREFLIGHT_OK_MARKER: &str = "aegishv:type1:runtime-preflight-ok";
pub const SERIAL_RUNTIME_PREFLIGHT_ERROR_MARKER: &str = "aegishv:type1:runtime-preflight-error";
pub const SERIAL_RUNTIME_ENABLE_OK_MARKER: &str = "aegishv:type1:runtime-enable-ok";
pub const SERIAL_RUNTIME_ENABLE_ERROR_MARKER: &str = "aegishv:type1:runtime-enable-error";
pub const SERIAL_RUNTIME_REGIONS_OK_MARKER: &str = "aegishv:type1:runtime-regions-ok";
pub const SERIAL_RUNTIME_REGIONS_ERROR_MARKER: &str = "aegishv:type1:runtime-regions-error";
pub const SERIAL_RUNTIME_VMXON_OK_MARKER: &str = "aegishv:type1:vmxon-cycle-ok";
pub const SERIAL_RUNTIME_VMXON_ERROR_MARKER: &str = "aegishv:type1:vmxon-cycle-error";
pub const SERIAL_RUNTIME_VMXON_SKIPPED_MARKER: &str = "aegishv:type1:vmxon-cycle-skipped";
pub const SERIAL_RUNTIME_VMCS_LOAD_OK_MARKER: &str = "aegishv:type1:vmcs-load-ok";
pub const SERIAL_RUNTIME_VMCS_LOAD_ERROR_MARKER: &str = "aegishv:type1:vmcs-load-error";
pub const SERIAL_RUNTIME_VMCS_LOAD_SKIPPED_MARKER: &str = "aegishv:type1:vmcs-load-skipped";
pub const SERIAL_VMX_GUEST_CPUID_EXIT_OK_MARKER: &str = "aegishv:type1:guest-cpuid-exit-ok";
pub const SERIAL_VMX_GUEST_HLT_EXIT_OK_MARKER: &str = "aegishv:type1:guest-hlt-exit-ok";
pub const SERIAL_VMX_GUEST_EXIT_ERROR_MARKER: &str = "aegishv:type1:guest-exit-error";
pub const SERIAL_VMX_GUEST_RESUME_ERROR_MARKER: &str = "aegishv:type1:guest-resume-error";
pub const SERIAL_VMX_GUEST_RUN_OK_MARKER: &str = "aegishv:type1:guest-run-ok";
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
pub const TYPE1_RUNTIME_PAGE_SIZE: u64 = 4096;
#[cfg(test)]
pub const TYPE1_RUNTIME_REGION_BASE_OFFSET: u64 = 0x80_000;
#[cfg(test)]
pub const TYPE1_VMXON_REGION_OFFSET: u64 = TYPE1_RUNTIME_REGION_BASE_OFFSET;
#[cfg(test)]
pub const TYPE1_VMCS_REGION_OFFSET: u64 =
    TYPE1_RUNTIME_REGION_BASE_OFFSET + TYPE1_RUNTIME_PAGE_SIZE;
#[cfg(test)]
pub const TYPE1_SVM_VMCB_REGION_OFFSET: u64 =
    TYPE1_RUNTIME_REGION_BASE_OFFSET + (2 * TYPE1_RUNTIME_PAGE_SIZE);
pub const CPUID_VENDOR_LEAF: u32 = 0;
pub const CPUID_FEATURE_LEAF: u32 = 1;
pub const CPUID_EXTENDED_LIMIT_LEAF: u32 = 0x8000_0000;
pub const CPUID_EXTENDED_FEATURE_LEAF: u32 = 0x8000_0001;
pub const CPUID_SVM_FEATURE_LEAF: u32 = 0x8000_000a;
pub const IA32_FEATURE_CONTROL_MSR: u32 = 0x0000_003a;
pub const IA32_VMX_CR0_FIXED0_MSR: u32 = 0x0000_0486;
pub const IA32_VMX_CR0_FIXED1_MSR: u32 = 0x0000_0487;
pub const IA32_VMX_CR4_FIXED0_MSR: u32 = 0x0000_0488;
pub const IA32_VMX_CR4_FIXED1_MSR: u32 = 0x0000_0489;
pub const IA32_VMX_BASIC_MSR: u32 = 0x0000_0480;
pub const IA32_EFER_MSR: u32 = 0xc000_0080;
pub const TYPE1_CR4_VMXE: u64 = 1 << 13;
pub const TYPE1_VMX_BASIC_REVISION_MASK: u64 = 0x7fff_ffff;

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
pub enum Type1CpuVendor {
    Unknown,
    Intel,
    Amd,
}

impl Type1CpuVendor {
    pub const fn from_cpuid0(ebx: u32, ecx: u32, edx: u32) -> Self {
        match (ebx, ecx, edx) {
            (0x756e_6547, 0x6c65_746e, 0x4965_6e69) => Self::Intel,
            (0x6874_7541, 0x444d_4163, 0x6974_6e65) => Self::Amd,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1CpuSnapshot {
    pub vendor: Type1CpuVendor,
    pub cpuid_leaf1_ecx: u32,
    pub feature_control_msr: u64,
    pub max_extended_leaf: u32,
    pub cpuid_ext1_ecx: u32,
    pub svm_leaf_ebx: u32,
    pub svm_leaf_edx: u32,
}

impl Type1CpuSnapshot {
    pub const fn empty() -> Self {
        Self {
            vendor: Type1CpuVendor::Unknown,
            cpuid_leaf1_ecx: 0,
            feature_control_msr: 0,
            max_extended_leaf: 0,
            cpuid_ext1_ecx: 0,
            svm_leaf_ebx: 0,
            svm_leaf_edx: 0,
        }
    }

    pub const fn from_raw(
        vendor: Type1CpuVendor,
        cpuid_leaf1_ecx: u32,
        feature_control_msr: u64,
        max_extended_leaf: u32,
        cpuid_ext1_ecx: u32,
        svm_leaf_ebx: u32,
        svm_leaf_edx: u32,
    ) -> Self {
        Self {
            vendor,
            cpuid_leaf1_ecx,
            feature_control_msr,
            max_extended_leaf,
            cpuid_ext1_ecx,
            svm_leaf_ebx,
            svm_leaf_edx,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1CapabilityReport {
    pub vendor: Type1CpuVendor,
    pub capabilities: Type1ArchCapabilities,
    pub vmx_error: Option<VmxErrorKind>,
    pub svm_error: Option<SvmErrorKind>,
    pub svm_leaf_available: bool,
}

pub fn type1_capabilities_from_snapshot(snapshot: Type1CpuSnapshot) -> Type1CapabilityReport {
    let mut report = Type1CapabilityReport {
        vendor: snapshot.vendor,
        capabilities: Type1ArchCapabilities::none(),
        vmx_error: None,
        svm_error: None,
        svm_leaf_available: snapshot.max_extended_leaf >= CPUID_SVM_FEATURE_LEAF,
    };

    match snapshot.vendor {
        Type1CpuVendor::Intel => {
            match VmxFeatureSet::from_registers(
                CpuidLeaf1 {
                    ecx: snapshot.cpuid_leaf1_ecx,
                },
                FeatureControlMsr::new(snapshot.feature_control_msr),
            )
            .validate_non_smx()
            {
                Ok(_) => report.capabilities.intel_vmx = true,
                Err(err) => report.vmx_error = Some(err.kind),
            }
        }
        Type1CpuVendor::Amd => {
            if report.svm_leaf_available {
                match SvmFeatureSet::from_cpuid(
                    SvmCpuidExt1 {
                        ecx: snapshot.cpuid_ext1_ecx,
                    },
                    SvmCpuidLeaf {
                        ebx: snapshot.svm_leaf_ebx,
                        edx: snapshot.svm_leaf_edx,
                    },
                )
                .validate_for_npt_lab()
                {
                    Ok(_) => report.capabilities.amd_svm = true,
                    Err(err) => report.svm_error = Some(err.kind),
                }
            } else {
                report.svm_error = Some(SvmErrorKind::MissingCpuidBit);
            }
        }
        Type1CpuVendor::Unknown => {}
    }

    report
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
    #[cfg(test)]
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
    MissingVmxBasic,
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

#[cfg(test)]
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

pub const fn plan_type1_runtime_with_memory(
    handoff: LimineMinimalHandoff,
    request: Type1BackendRequest,
    capabilities: Type1ArchCapabilities,
    memory: Type1RuntimeMemoryPlan,
) -> Result<Type1RuntimePlan, Type1RuntimePlanError> {
    let status = limine_minimal_handoff_status(handoff);
    if !status.is_ready() {
        return Err(Type1RuntimePlanError::Handoff(status));
    }
    let backend = match select_type1_runtime_backend(request, capabilities) {
        Ok(backend) => backend,
        Err(err) => return Err(err),
    };
    match backend {
        Type1RuntimeBackend::None => {}
        Type1RuntimeBackend::IntelVmx => {
            if memory.runtime_base != memory.vmxon_physical
                || memory.vmxon_physical == 0
                || memory.vmcs_physical == 0
                || memory.vmxon_physical == memory.vmcs_physical
            {
                return Err(Type1RuntimePlanError::InvalidRuntimeAddress);
            }
            if memory.vmxon_physical % TYPE1_RUNTIME_PAGE_SIZE != 0
                || memory.vmcs_physical % TYPE1_RUNTIME_PAGE_SIZE != 0
            {
                return Err(Type1RuntimePlanError::RuntimeAddressMisaligned);
            }
        }
        Type1RuntimeBackend::AmdSvm => {
            if memory.runtime_base != memory.svm_vmcb_physical || memory.svm_vmcb_physical == 0 {
                return Err(Type1RuntimePlanError::InvalidRuntimeAddress);
            }
            if memory.svm_vmcb_physical % TYPE1_RUNTIME_PAGE_SIZE != 0 {
                return Err(Type1RuntimePlanError::RuntimeAddressMisaligned);
            }
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1VmxBasic {
    pub raw: u64,
}

impl Type1VmxBasic {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    pub const fn vmcs_revision_id(self) -> u32 {
        (self.raw & TYPE1_VMX_BASIC_REVISION_MASK) as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1RuntimeRegionMaterialization {
    pub backend: Type1RuntimeBackend,
    pub vmxon_physical: u64,
    pub vmcs_physical: u64,
    pub svm_vmcb_physical: u64,
    pub vmxon_revision: Option<u32>,
    pub vmcs_revision: Option<u32>,
    pub zeroed_page_count: u8,
}

impl Type1RuntimeRegionMaterialization {
    pub const fn uses_vmx_pages(self) -> bool {
        matches!(self.backend, Type1RuntimeBackend::IntelVmx)
    }

    pub const fn uses_svm_page(self) -> bool {
        matches!(self.backend, Type1RuntimeBackend::AmdSvm)
    }
}

pub fn plan_type1_runtime_regions(
    plan: Type1RuntimePlan,
    vmx_basic: Option<Type1VmxBasic>,
) -> Result<Type1RuntimeRegionMaterialization, Type1RuntimePlanError> {
    match plan.backend {
        Type1RuntimeBackend::None => Ok(Type1RuntimeRegionMaterialization {
            backend: Type1RuntimeBackend::None,
            vmxon_physical: plan.memory.vmxon_physical,
            vmcs_physical: plan.memory.vmcs_physical,
            svm_vmcb_physical: plan.memory.svm_vmcb_physical,
            vmxon_revision: None,
            vmcs_revision: None,
            zeroed_page_count: 0,
        }),
        Type1RuntimeBackend::IntelVmx => {
            let revision_id = match vmx_basic {
                Some(value) => value.vmcs_revision_id(),
                None => return Err(Type1RuntimePlanError::MissingVmxBasic),
            };
            let _runtime = build_vmx_runtime(plan, revision_id)?;
            Ok(Type1RuntimeRegionMaterialization {
                backend: Type1RuntimeBackend::IntelVmx,
                vmxon_physical: plan.memory.vmxon_physical,
                vmcs_physical: plan.memory.vmcs_physical,
                svm_vmcb_physical: plan.memory.svm_vmcb_physical,
                vmxon_revision: Some(revision_id),
                vmcs_revision: Some(revision_id),
                zeroed_page_count: 2,
            })
        }
        Type1RuntimeBackend::AmdSvm => {
            let _runtime = build_svm_runtime(plan)?;
            Ok(Type1RuntimeRegionMaterialization {
                backend: Type1RuntimeBackend::AmdSvm,
                vmxon_physical: plan.memory.vmxon_physical,
                vmcs_physical: plan.memory.vmcs_physical,
                svm_vmcb_physical: plan.memory.svm_vmcb_physical,
                vmxon_revision: None,
                vmcs_revision: None,
                zeroed_page_count: 1,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1VmxonCyclePlan {
    pub vmxon_physical: HostPhysical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1VmxonCycleStatus {
    Skipped,
    EnteredAndLeft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1VmxonCycleError {
    MissingVmxonRegion,
    InvalidRuntimeAddress,
    Vmxon(VmxErrorKind),
    Vmxoff(VmxErrorKind),
}

pub fn plan_type1_vmxon_cycle(
    regions: Type1RuntimeRegionMaterialization,
) -> Result<Option<Type1VmxonCyclePlan>, Type1VmxonCycleError> {
    if regions.backend != Type1RuntimeBackend::IntelVmx {
        return Ok(None);
    }
    if regions.vmxon_revision.is_none() {
        return Err(Type1VmxonCycleError::MissingVmxonRegion);
    }
    if regions.vmxon_physical == 0 || regions.vmxon_physical % TYPE1_RUNTIME_PAGE_SIZE != 0 {
        return Err(Type1VmxonCycleError::InvalidRuntimeAddress);
    }
    let vmxon_physical = HostPhysical::new(regions.vmxon_physical)
        .map_err(|_| Type1VmxonCycleError::InvalidRuntimeAddress)?;
    Ok(Some(Type1VmxonCyclePlan { vmxon_physical }))
}

/// # Safety
///
/// The caller must run this only after the current CPU passed VMX capability,
/// control-register, and VMXON-region checks. The executor must operate on the
/// same CPU that owns the VMXON region, and the caller must not have a live VMCS.
pub unsafe fn run_type1_vmxon_cycle_with<E: VmxInstructionExecutor>(
    regions: Type1RuntimeRegionMaterialization,
    executor: &mut E,
) -> Result<Type1VmxonCycleStatus, Type1VmxonCycleError> {
    let plan = match plan_type1_vmxon_cycle(regions)? {
        Some(plan) => plan,
        None => return Ok(Type1VmxonCycleStatus::Skipped),
    };
    unsafe { executor.vmxon(plan.vmxon_physical) }
        .map_err(|err| Type1VmxonCycleError::Vmxon(err.kind))?;
    unsafe { executor.vmxoff() }.map_err(|err| Type1VmxonCycleError::Vmxoff(err.kind))?;
    Ok(Type1VmxonCycleStatus::EnteredAndLeft)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1VmcsLoadCyclePlan {
    pub vmxon_physical: HostPhysical,
    pub vmcs_physical: HostPhysical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1VmcsLoadCycleStatus {
    Skipped,
    LoadedAndLeft,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1VmcsLoadCycleError {
    MissingVmxRegion,
    InvalidRuntimeAddress,
    Vmxon(VmxErrorKind),
    Vmclear(VmxErrorKind),
    Vmptrld(VmxErrorKind),
    Vmxoff(VmxErrorKind),
}

pub fn plan_type1_vmcs_load_cycle(
    regions: Type1RuntimeRegionMaterialization,
) -> Result<Option<Type1VmcsLoadCyclePlan>, Type1VmcsLoadCycleError> {
    if regions.backend != Type1RuntimeBackend::IntelVmx {
        return Ok(None);
    }
    if regions.vmxon_revision.is_none() || regions.vmcs_revision.is_none() {
        return Err(Type1VmcsLoadCycleError::MissingVmxRegion);
    }
    if regions.vmxon_physical == 0
        || regions.vmcs_physical == 0
        || regions.vmxon_physical % TYPE1_RUNTIME_PAGE_SIZE != 0
        || regions.vmcs_physical % TYPE1_RUNTIME_PAGE_SIZE != 0
    {
        return Err(Type1VmcsLoadCycleError::InvalidRuntimeAddress);
    }
    let vmxon_physical = HostPhysical::new(regions.vmxon_physical)
        .map_err(|_| Type1VmcsLoadCycleError::InvalidRuntimeAddress)?;
    let vmcs_physical = HostPhysical::new(regions.vmcs_physical)
        .map_err(|_| Type1VmcsLoadCycleError::InvalidRuntimeAddress)?;
    Ok(Some(Type1VmcsLoadCyclePlan {
        vmxon_physical,
        vmcs_physical,
    }))
}

/// # Safety
///
/// The caller must run this only after VMX capability and control-register
/// checks passed and after the VMXON and VMCS pages were initialized with the
/// current CPU's VMCS revision id. This routine deliberately stops before any
/// VMCS field writes or guest entry.
pub unsafe fn run_type1_vmcs_load_cycle_with<E: VmxInstructionExecutor>(
    regions: Type1RuntimeRegionMaterialization,
    executor: &mut E,
) -> Result<Type1VmcsLoadCycleStatus, Type1VmcsLoadCycleError> {
    let plan = match plan_type1_vmcs_load_cycle(regions)? {
        Some(plan) => plan,
        None => return Ok(Type1VmcsLoadCycleStatus::Skipped),
    };
    unsafe { executor.vmxon(plan.vmxon_physical) }
        .map_err(|err| Type1VmcsLoadCycleError::Vmxon(err.kind))?;
    if let Err(err) = unsafe { executor.vmclear(plan.vmcs_physical) } {
        if let Err(off_err) = unsafe { executor.vmxoff() } {
            return Err(Type1VmcsLoadCycleError::Vmxoff(off_err.kind));
        }
        return Err(Type1VmcsLoadCycleError::Vmclear(err.kind));
    }
    if let Err(err) = unsafe { executor.vmptrld(plan.vmcs_physical) } {
        if let Err(off_err) = unsafe { executor.vmxoff() } {
            return Err(Type1VmcsLoadCycleError::Vmxoff(off_err.kind));
        }
        return Err(Type1VmcsLoadCycleError::Vmptrld(err.kind));
    }
    unsafe { executor.vmxoff() }.map_err(|err| Type1VmcsLoadCycleError::Vmxoff(err.kind))?;
    Ok(Type1VmcsLoadCycleStatus::LoadedAndLeft)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1ControlSnapshot {
    pub cr0: u64,
    pub cr4: u64,
    pub efer: u64,
    pub vmx_cr0_fixed0: u64,
    pub vmx_cr0_fixed1: u64,
    pub vmx_cr4_fixed0: u64,
    pub vmx_cr4_fixed1: u64,
}

impl Type1ControlSnapshot {
    pub const fn empty() -> Self {
        Self {
            cr0: 0,
            cr4: 0,
            efer: 0,
            vmx_cr0_fixed0: 0,
            vmx_cr0_fixed1: u64::MAX,
            vmx_cr4_fixed0: 0,
            vmx_cr4_fixed1: u64::MAX,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1RuntimePreflight {
    pub backend: Type1RuntimeBackend,
    pub cr0_before: u64,
    pub cr0_after: u64,
    pub cr4_before: u64,
    pub cr4_after: u64,
    pub efer_before: u64,
    pub efer_after: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1HostControlRegister {
    Cr0,
    Cr4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1RuntimePreflightError {
    BackendMismatch,
    InconsistentVmxFixedBits {
        register: Type1HostControlRegister,
        bits: u64,
    },
    ActiveHostControlBitsForbidden {
        register: Type1HostControlRegister,
        bits: u64,
    },
    RequiredHostControlBitsForbidden {
        register: Type1HostControlRegister,
        bits: u64,
    },
    Svm(SvmErrorKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1RuntimeEnablePlan {
    pub backend: Type1RuntimeBackend,
    pub cr0: Option<u64>,
    pub cr4: Option<u64>,
    pub efer: Option<u64>,
}

impl Type1RuntimeEnablePlan {
    pub const fn has_writes(self) -> bool {
        self.cr0.is_some() || self.cr4.is_some() || self.efer.is_some()
    }
}

pub fn plan_type1_runtime_preflight(
    plan: Type1RuntimePlan,
    controls: Type1ControlSnapshot,
) -> Result<Type1RuntimePreflight, Type1RuntimePreflightError> {
    match plan.backend {
        Type1RuntimeBackend::None => Ok(Type1RuntimePreflight {
            backend: Type1RuntimeBackend::None,
            cr0_before: controls.cr0,
            cr0_after: controls.cr0,
            cr4_before: controls.cr4,
            cr4_after: controls.cr4,
            efer_before: controls.efer,
            efer_after: controls.efer,
        }),
        Type1RuntimeBackend::IntelVmx => plan_vmx_preflight(controls),
        Type1RuntimeBackend::AmdSvm => Ok(plan_svm_preflight(controls)),
    }
}

fn plan_vmx_preflight(
    controls: Type1ControlSnapshot,
) -> Result<Type1RuntimePreflight, Type1RuntimePreflightError> {
    let cr0_fixed_conflict = controls.vmx_cr0_fixed0 & !controls.vmx_cr0_fixed1;
    if cr0_fixed_conflict != 0 {
        return Err(Type1RuntimePreflightError::InconsistentVmxFixedBits {
            register: Type1HostControlRegister::Cr0,
            bits: cr0_fixed_conflict,
        });
    }
    let cr4_fixed_conflict = controls.vmx_cr4_fixed0 & !controls.vmx_cr4_fixed1;
    if cr4_fixed_conflict != 0 {
        return Err(Type1RuntimePreflightError::InconsistentVmxFixedBits {
            register: Type1HostControlRegister::Cr4,
            bits: cr4_fixed_conflict,
        });
    }
    let active_cr0_forbidden = controls.cr0 & !controls.vmx_cr0_fixed1;
    if active_cr0_forbidden != 0 {
        return Err(Type1RuntimePreflightError::ActiveHostControlBitsForbidden {
            register: Type1HostControlRegister::Cr0,
            bits: active_cr0_forbidden,
        });
    }
    let active_cr4_forbidden = controls.cr4 & !controls.vmx_cr4_fixed1;
    if active_cr4_forbidden != 0 {
        return Err(Type1RuntimePreflightError::ActiveHostControlBitsForbidden {
            register: Type1HostControlRegister::Cr4,
            bits: active_cr4_forbidden,
        });
    }
    if controls.vmx_cr4_fixed1 & TYPE1_CR4_VMXE == 0 {
        return Err(
            Type1RuntimePreflightError::RequiredHostControlBitsForbidden {
                register: Type1HostControlRegister::Cr4,
                bits: TYPE1_CR4_VMXE,
            },
        );
    }
    let cr0_after = controls.cr0 | controls.vmx_cr0_fixed0;
    let cr4_after = controls.cr4 | TYPE1_CR4_VMXE | controls.vmx_cr4_fixed0;
    Ok(Type1RuntimePreflight {
        backend: Type1RuntimeBackend::IntelVmx,
        cr0_before: controls.cr0,
        cr0_after,
        cr4_before: controls.cr4,
        cr4_after,
        efer_before: controls.efer,
        efer_after: controls.efer,
    })
}

fn plan_svm_preflight(controls: Type1ControlSnapshot) -> Type1RuntimePreflight {
    let efer_after = EferValue::new(controls.efer).with_svme().raw();
    Type1RuntimePreflight {
        backend: Type1RuntimeBackend::AmdSvm,
        cr0_before: controls.cr0,
        cr0_after: controls.cr0,
        cr4_before: controls.cr4,
        cr4_after: controls.cr4,
        efer_before: controls.efer,
        efer_after,
    }
}

pub const fn plan_type1_runtime_enable(preflight: Type1RuntimePreflight) -> Type1RuntimeEnablePlan {
    Type1RuntimeEnablePlan {
        backend: preflight.backend,
        cr0: if preflight.cr0_after != preflight.cr0_before {
            Some(preflight.cr0_after)
        } else {
            None
        },
        cr4: if preflight.cr4_after != preflight.cr4_before {
            Some(preflight.cr4_after)
        } else {
            None
        },
        efer: if preflight.efer_after != preflight.efer_before {
            Some(preflight.efer_after)
        } else {
            None
        },
    }
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
    use aegishv_arch_x86::vmx::features::VmxError;

    #[derive(Default)]
    struct MockVmxonCycleExecutor {
        vmxon_region: Option<HostPhysical>,
        current_vmcs: Option<HostPhysical>,
        cleared_vmcs: Option<HostPhysical>,
        fail_vmxon: bool,
        fail_vmxoff: bool,
        fail_vmclear: bool,
        fail_vmptrld: bool,
    }

    impl MockVmxonCycleExecutor {
        fn fail_vmxon(&mut self) {
            self.fail_vmxon = true;
        }

        fn fail_vmxoff(&mut self) {
            self.fail_vmxoff = true;
        }

        fn fail_vmclear(&mut self) {
            self.fail_vmclear = true;
        }

        fn fail_vmptrld(&mut self) {
            self.fail_vmptrld = true;
        }
    }

    impl VmxInstructionExecutor for MockVmxonCycleExecutor {
        unsafe fn vmxon(&mut self, region: HostPhysical) -> Result<(), VmxError> {
            if self.fail_vmxon {
                self.fail_vmxon = false;
                return Err(VmxError::new(
                    VmxErrorKind::InstructionFailed,
                    "mock VMXON failed",
                ));
            }
            self.vmxon_region = Some(region);
            Ok(())
        }

        unsafe fn vmxoff(&mut self) -> Result<(), VmxError> {
            if self.fail_vmxoff {
                self.fail_vmxoff = false;
                return Err(VmxError::new(
                    VmxErrorKind::InstructionFailed,
                    "mock VMXOFF failed",
                ));
            }
            self.vmxon_region = None;
            self.current_vmcs = None;
            Ok(())
        }

        unsafe fn vmclear(&mut self, vmcs: HostPhysical) -> Result<(), VmxError> {
            if self.fail_vmclear {
                self.fail_vmclear = false;
                return Err(VmxError::new(
                    VmxErrorKind::InstructionFailed,
                    "mock VMCLEAR failed",
                ));
            }
            self.cleared_vmcs = Some(vmcs);
            Ok(())
        }

        unsafe fn vmptrld(&mut self, vmcs: HostPhysical) -> Result<(), VmxError> {
            if self.fail_vmptrld {
                self.fail_vmptrld = false;
                return Err(VmxError::new(
                    VmxErrorKind::InstructionFailed,
                    "mock VMPTRLD failed",
                ));
            }
            self.current_vmcs = Some(vmcs);
            Ok(())
        }

        unsafe fn vmlaunch(&mut self) -> Result<(), VmxError> {
            Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "mock VMXON cycle does not launch guests",
            ))
        }

        unsafe fn vmresume(&mut self) -> Result<(), VmxError> {
            Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "mock VMXON cycle does not resume guests",
            ))
        }

        unsafe fn vmread(&mut self, _field: u64) -> Result<u64, VmxError> {
            Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "mock VMXON cycle does not read VMCS fields",
            ))
        }

        unsafe fn vmwrite(&mut self, _field: u64, _value: u64) -> Result<(), VmxError> {
            Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "mock VMXON cycle does not write VMCS fields",
            ))
        }
    }

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
        assert!(marker_line(SERIAL_RUNTIME_VMXON_OK_MARKER, &mut out).is_some());
        assert!(marker_line(SERIAL_RUNTIME_VMXON_ERROR_MARKER, &mut out).is_some());
        assert!(marker_line(SERIAL_RUNTIME_VMXON_SKIPPED_MARKER, &mut out).is_some());
        assert!(marker_line(SERIAL_RUNTIME_VMCS_LOAD_OK_MARKER, &mut out).is_some());
        assert!(marker_line(SERIAL_RUNTIME_VMCS_LOAD_ERROR_MARKER, &mut out).is_some());
        assert!(marker_line(SERIAL_RUNTIME_VMCS_LOAD_SKIPPED_MARKER, &mut out).is_some());
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
    fn cpu_vendor_id_recognizes_x86_vendors() {
        assert_eq!(
            Type1CpuVendor::from_cpuid0(0x756e_6547, 0x6c65_746e, 0x4965_6e69),
            Type1CpuVendor::Intel
        );
        assert_eq!(
            Type1CpuVendor::from_cpuid0(0x6874_7541, 0x444d_4163, 0x6974_6e65),
            Type1CpuVendor::Amd
        );
        assert_eq!(
            Type1CpuVendor::from_cpuid0(0, 0, 0),
            Type1CpuVendor::Unknown
        );
    }

    #[test]
    fn cpu_snapshot_accepts_locked_intel_vmx() {
        let report = type1_capabilities_from_snapshot(Type1CpuSnapshot::from_raw(
            Type1CpuVendor::Intel,
            aegishv_arch_x86::vmx::features::CPUID_LEAF1_ECX_VMX,
            aegishv_arch_x86::vmx::features::IA32_FEATURE_CONTROL_LOCK
                | aegishv_arch_x86::vmx::features::IA32_FEATURE_CONTROL_VMX_OUTSIDE_SMX,
            CPUID_EXTENDED_LIMIT_LEAF,
            0,
            0,
            0,
        ));

        assert_eq!(report.vendor, Type1CpuVendor::Intel);
        assert_eq!(report.capabilities, Type1ArchCapabilities::intel_vmx());
        assert_eq!(report.vmx_error, None);
        assert_eq!(report.svm_error, None);
    }

    #[test]
    fn cpu_snapshot_reports_intel_vmx_feature_error() {
        let report = type1_capabilities_from_snapshot(Type1CpuSnapshot::from_raw(
            Type1CpuVendor::Intel,
            aegishv_arch_x86::vmx::features::CPUID_LEAF1_ECX_VMX,
            aegishv_arch_x86::vmx::features::IA32_FEATURE_CONTROL_LOCK,
            CPUID_EXTENDED_LIMIT_LEAF,
            0,
            0,
            0,
        ));

        assert_eq!(report.capabilities, Type1ArchCapabilities::none());
        assert_eq!(
            report.vmx_error,
            Some(aegishv_arch_x86::vmx::features::VmxErrorKind::VmxDisabledOutsideSmx)
        );
    }

    #[test]
    fn cpu_snapshot_accepts_amd_svm_with_npt() {
        let report = type1_capabilities_from_snapshot(Type1CpuSnapshot::from_raw(
            Type1CpuVendor::Amd,
            aegishv_arch_x86::vmx::features::CPUID_LEAF1_ECX_VMX,
            0,
            CPUID_SVM_FEATURE_LEAF,
            aegishv_arch_x86::svm::features::CPUID_EXT1_ECX_SVM,
            16,
            aegishv_arch_x86::svm::features::CPUID_SVM_EDX_NPT,
        ));

        assert_eq!(report.vendor, Type1CpuVendor::Amd);
        assert_eq!(report.capabilities, Type1ArchCapabilities::amd_svm());
        assert_eq!(report.vmx_error, None);
        assert_eq!(report.svm_error, None);
        assert!(report.svm_leaf_available);
    }

    #[test]
    fn cpu_snapshot_uses_vendor_to_avoid_mixed_backends() {
        let report = type1_capabilities_from_snapshot(Type1CpuSnapshot::from_raw(
            Type1CpuVendor::Amd,
            aegishv_arch_x86::vmx::features::CPUID_LEAF1_ECX_VMX,
            aegishv_arch_x86::vmx::features::IA32_FEATURE_CONTROL_LOCK
                | aegishv_arch_x86::vmx::features::IA32_FEATURE_CONTROL_VMX_OUTSIDE_SMX,
            CPUID_SVM_FEATURE_LEAF,
            aegishv_arch_x86::svm::features::CPUID_EXT1_ECX_SVM,
            16,
            aegishv_arch_x86::svm::features::CPUID_SVM_EDX_NPT,
        ));

        assert_eq!(report.capabilities, Type1ArchCapabilities::amd_svm());
        assert_eq!(report.vmx_error, None);
    }

    #[test]
    fn cpu_snapshot_rejects_missing_svm_leaf() {
        let report = type1_capabilities_from_snapshot(Type1CpuSnapshot::from_raw(
            Type1CpuVendor::Amd,
            0,
            0,
            CPUID_EXTENDED_FEATURE_LEAF,
            aegishv_arch_x86::svm::features::CPUID_EXT1_ECX_SVM,
            16,
            aegishv_arch_x86::svm::features::CPUID_SVM_EDX_NPT,
        ));

        assert_eq!(report.capabilities, Type1ArchCapabilities::none());
        assert!(!report.svm_leaf_available);
        assert_eq!(
            report.svm_error,
            Some(aegishv_arch_x86::svm::features::SvmErrorKind::MissingCpuidBit)
        );
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
    fn runtime_plan_accepts_distinct_allocator_owned_vmx_pages() {
        let plan = plan_type1_runtime_with_memory(
            ready_handoff(),
            Type1BackendRequest::Auto,
            Type1ArchCapabilities::intel_vmx(),
            Type1RuntimeMemoryPlan {
                runtime_base: 0x40_0000,
                vmxon_physical: 0x40_0000,
                vmcs_physical: 0x40_1000,
                svm_vmcb_physical: 0,
            },
        )
        .unwrap();

        assert_eq!(plan.backend, Type1RuntimeBackend::IntelVmx);
        assert_eq!(plan.memory.vmxon_physical, 0x40_0000);
        assert_eq!(plan.memory.vmcs_physical, 0x40_1000);
        assert_eq!(plan.memory.svm_vmcb_physical, 0);
    }

    #[test]
    fn runtime_plan_rejects_duplicate_or_misaligned_vmx_pages() {
        for memory in [
            Type1RuntimeMemoryPlan {
                runtime_base: 0x40_0000,
                vmxon_physical: 0x40_0000,
                vmcs_physical: 0x40_0000,
                svm_vmcb_physical: 0,
            },
            Type1RuntimeMemoryPlan {
                runtime_base: 0x40_0001,
                vmxon_physical: 0x40_0001,
                vmcs_physical: 0x40_1000,
                svm_vmcb_physical: 0,
            },
        ] {
            assert!(plan_type1_runtime_with_memory(
                ready_handoff(),
                Type1BackendRequest::IntelVmx,
                Type1ArchCapabilities::intel_vmx(),
                memory,
            )
            .is_err());
        }
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

    #[test]
    fn runtime_region_plan_materializes_vmx_pages_from_basic_msr() {
        let plan = plan_type1_runtime(
            ready_handoff(),
            Type1BackendRequest::IntelVmx,
            Type1ArchCapabilities::intel_vmx(),
        )
        .unwrap();

        let regions =
            plan_type1_runtime_regions(plan, Some(Type1VmxBasic::new(0x8000_0033))).unwrap();

        assert_eq!(regions.backend, Type1RuntimeBackend::IntelVmx);
        assert!(regions.uses_vmx_pages());
        assert!(!regions.uses_svm_page());
        assert_eq!(regions.vmxon_physical, 0x28_0000);
        assert_eq!(regions.vmcs_physical, 0x28_1000);
        assert_eq!(regions.vmxon_revision, Some(0x33));
        assert_eq!(regions.vmcs_revision, Some(0x33));
        assert_eq!(regions.zeroed_page_count, 2);
    }

    #[test]
    fn runtime_region_plan_rejects_missing_or_bad_vmx_revision() {
        let plan = plan_type1_runtime(
            ready_handoff(),
            Type1BackendRequest::IntelVmx,
            Type1ArchCapabilities::intel_vmx(),
        )
        .unwrap();

        assert_eq!(
            plan_type1_runtime_regions(plan, None).unwrap_err(),
            Type1RuntimePlanError::MissingVmxBasic
        );
        assert_eq!(
            plan_type1_runtime_regions(plan, Some(Type1VmxBasic::new(0))).unwrap_err(),
            Type1RuntimePlanError::Vmx(VmxErrorKind::InvalidRevisionId)
        );
    }

    #[test]
    fn runtime_region_plan_materializes_svm_vmcb_page() {
        let plan = plan_type1_runtime(
            ready_handoff(),
            Type1BackendRequest::AmdSvm,
            Type1ArchCapabilities::amd_svm(),
        )
        .unwrap();

        let regions = plan_type1_runtime_regions(plan, None).unwrap();

        assert_eq!(regions.backend, Type1RuntimeBackend::AmdSvm);
        assert!(!regions.uses_vmx_pages());
        assert!(regions.uses_svm_page());
        assert_eq!(regions.svm_vmcb_physical, 0x28_2000);
        assert_eq!(regions.vmxon_revision, None);
        assert_eq!(regions.vmcs_revision, None);
        assert_eq!(regions.zeroed_page_count, 1);
    }

    #[test]
    fn runtime_region_plan_leaves_no_backend_without_writes() {
        let plan = Type1RuntimePlan {
            backend: Type1RuntimeBackend::None,
            memory: Type1RuntimeMemoryPlan::from_executable_base(
                aegishv_type1_boot::layout::KERNEL_PHYSICAL_BASE,
            )
            .unwrap(),
        };

        let regions = plan_type1_runtime_regions(plan, None).unwrap();

        assert_eq!(regions.backend, Type1RuntimeBackend::None);
        assert!(!regions.uses_vmx_pages());
        assert!(!regions.uses_svm_page());
        assert_eq!(regions.zeroed_page_count, 0);
    }

    #[test]
    fn vmxon_cycle_enters_and_leaves_with_mock_executor() {
        let regions = plan_type1_runtime_regions(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::IntelVmx,
                Type1ArchCapabilities::intel_vmx(),
            )
            .unwrap(),
            Some(Type1VmxBasic::new(0x33)),
        )
        .unwrap();
        let plan = plan_type1_vmxon_cycle(regions).unwrap().unwrap();
        let mut executor = MockVmxonCycleExecutor::default();

        let status = unsafe { run_type1_vmxon_cycle_with(regions, &mut executor) }.unwrap();

        assert_eq!(plan.vmxon_physical.get(), 0x28_0000);
        assert_eq!(status, Type1VmxonCycleStatus::EnteredAndLeft);
        assert!(executor.vmxon_region.is_none());
        assert!(executor.current_vmcs.is_none());
    }

    #[test]
    fn vmxon_cycle_reports_vmxon_failure_without_vmxoff() {
        let regions = plan_type1_runtime_regions(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::IntelVmx,
                Type1ArchCapabilities::intel_vmx(),
            )
            .unwrap(),
            Some(Type1VmxBasic::new(0x33)),
        )
        .unwrap();
        let mut executor = MockVmxonCycleExecutor::default();
        executor.fail_vmxon();

        let err = unsafe { run_type1_vmxon_cycle_with(regions, &mut executor) }.unwrap_err();

        assert_eq!(
            err,
            Type1VmxonCycleError::Vmxon(VmxErrorKind::InstructionFailed)
        );
        assert!(executor.vmxon_region.is_none());
    }

    #[test]
    fn vmxon_cycle_reports_vmxoff_failure_after_enter() {
        let regions = plan_type1_runtime_regions(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::IntelVmx,
                Type1ArchCapabilities::intel_vmx(),
            )
            .unwrap(),
            Some(Type1VmxBasic::new(0x33)),
        )
        .unwrap();
        let mut executor = MockVmxonCycleExecutor::default();
        executor.fail_vmxoff();

        let err = unsafe { run_type1_vmxon_cycle_with(regions, &mut executor) }.unwrap_err();

        assert_eq!(
            err,
            Type1VmxonCycleError::Vmxoff(VmxErrorKind::InstructionFailed)
        );
        assert_eq!(executor.vmxon_region.unwrap().get(), 0x28_0000);
    }

    #[test]
    fn vmxon_cycle_skips_non_vmx_backends() {
        let svm_regions = plan_type1_runtime_regions(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::AmdSvm,
                Type1ArchCapabilities::amd_svm(),
            )
            .unwrap(),
            None,
        )
        .unwrap();
        let none_regions = Type1RuntimeRegionMaterialization {
            backend: Type1RuntimeBackend::None,
            vmxon_physical: 0x28_0000,
            vmcs_physical: 0x28_1000,
            svm_vmcb_physical: 0x28_2000,
            vmxon_revision: None,
            vmcs_revision: None,
            zeroed_page_count: 0,
        };
        let mut executor = MockVmxonCycleExecutor::default();

        assert_eq!(
            unsafe { run_type1_vmxon_cycle_with(svm_regions, &mut executor) }.unwrap(),
            Type1VmxonCycleStatus::Skipped
        );
        assert_eq!(
            unsafe { run_type1_vmxon_cycle_with(none_regions, &mut executor) }.unwrap(),
            Type1VmxonCycleStatus::Skipped
        );
        assert!(executor.vmxon_region.is_none());
    }

    #[test]
    fn vmcs_load_cycle_clears_loads_and_leaves_with_mock_executor() {
        let regions = plan_type1_runtime_regions(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::IntelVmx,
                Type1ArchCapabilities::intel_vmx(),
            )
            .unwrap(),
            Some(Type1VmxBasic::new(0x33)),
        )
        .unwrap();
        let plan = plan_type1_vmcs_load_cycle(regions).unwrap().unwrap();
        let mut executor = MockVmxonCycleExecutor::default();

        let status = unsafe { run_type1_vmcs_load_cycle_with(regions, &mut executor) }.unwrap();

        assert_eq!(plan.vmxon_physical.get(), 0x28_0000);
        assert_eq!(plan.vmcs_physical.get(), 0x28_1000);
        assert_eq!(status, Type1VmcsLoadCycleStatus::LoadedAndLeft);
        assert_eq!(executor.cleared_vmcs.unwrap().get(), 0x28_1000);
        assert!(executor.current_vmcs.is_none());
        assert!(executor.vmxon_region.is_none());
    }

    #[test]
    fn vmcs_load_cycle_skips_non_vmx_backend() {
        let regions = plan_type1_runtime_regions(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::AmdSvm,
                Type1ArchCapabilities::amd_svm(),
            )
            .unwrap(),
            None,
        )
        .unwrap();
        let mut executor = MockVmxonCycleExecutor::default();

        assert_eq!(
            unsafe { run_type1_vmcs_load_cycle_with(regions, &mut executor) }.unwrap(),
            Type1VmcsLoadCycleStatus::Skipped
        );
        assert!(executor.vmxon_region.is_none());
    }

    #[test]
    fn vmcs_load_cycle_rolls_back_after_vmclear_failure() {
        let regions = plan_type1_runtime_regions(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::IntelVmx,
                Type1ArchCapabilities::intel_vmx(),
            )
            .unwrap(),
            Some(Type1VmxBasic::new(0x33)),
        )
        .unwrap();
        let mut executor = MockVmxonCycleExecutor::default();
        executor.fail_vmclear();

        let err = unsafe { run_type1_vmcs_load_cycle_with(regions, &mut executor) }.unwrap_err();

        assert_eq!(
            err,
            Type1VmcsLoadCycleError::Vmclear(VmxErrorKind::InstructionFailed)
        );
        assert!(executor.cleared_vmcs.is_none());
        assert!(executor.current_vmcs.is_none());
        assert!(executor.vmxon_region.is_none());
    }

    #[test]
    fn vmcs_load_cycle_rolls_back_after_vmptrld_failure() {
        let regions = plan_type1_runtime_regions(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::IntelVmx,
                Type1ArchCapabilities::intel_vmx(),
            )
            .unwrap(),
            Some(Type1VmxBasic::new(0x33)),
        )
        .unwrap();
        let mut executor = MockVmxonCycleExecutor::default();
        executor.fail_vmptrld();

        let err = unsafe { run_type1_vmcs_load_cycle_with(regions, &mut executor) }.unwrap_err();

        assert_eq!(
            err,
            Type1VmcsLoadCycleError::Vmptrld(VmxErrorKind::InstructionFailed)
        );
        assert_eq!(executor.cleared_vmcs.unwrap().get(), 0x28_1000);
        assert!(executor.current_vmcs.is_none());
        assert!(executor.vmxon_region.is_none());
    }

    #[test]
    fn vmcs_load_cycle_reports_vmxoff_failure_after_load() {
        let regions = plan_type1_runtime_regions(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::IntelVmx,
                Type1ArchCapabilities::intel_vmx(),
            )
            .unwrap(),
            Some(Type1VmxBasic::new(0x33)),
        )
        .unwrap();
        let mut executor = MockVmxonCycleExecutor::default();
        executor.fail_vmxoff();

        let err = unsafe { run_type1_vmcs_load_cycle_with(regions, &mut executor) }.unwrap_err();

        assert_eq!(
            err,
            Type1VmcsLoadCycleError::Vmxoff(VmxErrorKind::InstructionFailed)
        );
        assert_eq!(executor.vmxon_region.unwrap().get(), 0x28_0000);
        assert_eq!(executor.current_vmcs.unwrap().get(), 0x28_1000);
    }

    #[test]
    fn runtime_preflight_keeps_no_backend_registers_unchanged() {
        let preflight = plan_type1_runtime_preflight(
            Type1RuntimePlan {
                backend: Type1RuntimeBackend::None,
                memory: Type1RuntimeMemoryPlan::from_executable_base(
                    aegishv_type1_boot::layout::KERNEL_PHYSICAL_BASE,
                )
                .unwrap(),
            },
            Type1ControlSnapshot {
                cr0: 0x8000_0011,
                cr4: 0x20,
                efer: 0x500,
                ..Type1ControlSnapshot::empty()
            },
        )
        .unwrap();

        assert_eq!(preflight.backend, Type1RuntimeBackend::None);
        assert_eq!(preflight.cr0_after, 0x8000_0011);
        assert_eq!(preflight.cr4_after, 0x20);
        assert_eq!(preflight.efer_after, 0x500);
    }

    #[test]
    fn runtime_preflight_sets_vmx_fixed_bits_and_vmxe() {
        let preflight = plan_type1_runtime_preflight(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::IntelVmx,
                Type1ArchCapabilities::intel_vmx(),
            )
            .unwrap(),
            Type1ControlSnapshot {
                cr0: 0x8000_0011,
                cr4: 0,
                efer: 0x500,
                vmx_cr0_fixed0: 0x8000_0031,
                vmx_cr0_fixed1: u64::MAX,
                vmx_cr4_fixed0: 0,
                vmx_cr4_fixed1: u64::MAX,
            },
        )
        .unwrap();

        assert_eq!(preflight.backend, Type1RuntimeBackend::IntelVmx);
        assert_eq!(preflight.cr0_after & 0x8000_0031, 0x8000_0031);
        assert_ne!(preflight.cr4_before & TYPE1_CR4_VMXE, TYPE1_CR4_VMXE);
        assert_eq!(preflight.cr4_after & TYPE1_CR4_VMXE, TYPE1_CR4_VMXE);
        assert_eq!(preflight.efer_after, 0x500);
    }

    #[test]
    fn runtime_preflight_rejects_vmx_when_vmxe_is_forbidden() {
        let err = plan_type1_runtime_preflight(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::IntelVmx,
                Type1ArchCapabilities::intel_vmx(),
            )
            .unwrap(),
            Type1ControlSnapshot {
                cr0: 0x8000_0011,
                cr4: 0,
                efer: 0,
                vmx_cr0_fixed0: 0,
                vmx_cr0_fixed1: u64::MAX,
                vmx_cr4_fixed0: 0,
                vmx_cr4_fixed1: !TYPE1_CR4_VMXE,
            },
        )
        .unwrap_err();

        assert_eq!(
            err,
            Type1RuntimePreflightError::RequiredHostControlBitsForbidden {
                register: Type1HostControlRegister::Cr4,
                bits: TYPE1_CR4_VMXE,
            }
        );
    }

    #[test]
    fn runtime_preflight_refuses_to_clear_active_control_bits() {
        let err = plan_type1_runtime_preflight(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::IntelVmx,
                Type1ArchCapabilities::intel_vmx(),
            )
            .unwrap(),
            Type1ControlSnapshot {
                cr0: 0x8000_0011,
                cr4: 1 << 12,
                efer: 0x500,
                vmx_cr0_fixed0: 0,
                vmx_cr0_fixed1: u64::MAX,
                vmx_cr4_fixed0: 0,
                vmx_cr4_fixed1: !(1 << 12),
            },
        )
        .unwrap_err();

        assert_eq!(
            err,
            Type1RuntimePreflightError::ActiveHostControlBitsForbidden {
                register: Type1HostControlRegister::Cr4,
                bits: 1 << 12,
            }
        );
    }

    #[test]
    fn runtime_preflight_enables_svm_svme_bit() {
        let preflight = plan_type1_runtime_preflight(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::AmdSvm,
                Type1ArchCapabilities::amd_svm(),
            )
            .unwrap(),
            Type1ControlSnapshot {
                cr0: 0x8000_0011,
                cr4: 0x20,
                efer: 0x500,
                ..Type1ControlSnapshot::empty()
            },
        )
        .unwrap();

        assert_eq!(preflight.backend, Type1RuntimeBackend::AmdSvm);
        assert_eq!(preflight.cr0_after, 0x8000_0011);
        assert_eq!(
            preflight.efer_after & aegishv_arch_x86::svm::features::EFER_SVME,
            aegishv_arch_x86::svm::features::EFER_SVME
        );
    }

    #[test]
    fn runtime_enable_plan_has_no_writes_for_no_backend() {
        let preflight = plan_type1_runtime_preflight(
            Type1RuntimePlan {
                backend: Type1RuntimeBackend::None,
                memory: Type1RuntimeMemoryPlan::from_executable_base(
                    aegishv_type1_boot::layout::KERNEL_PHYSICAL_BASE,
                )
                .unwrap(),
            },
            Type1ControlSnapshot {
                cr0: 0x8000_0011,
                cr4: 0x20,
                efer: 0x500,
                ..Type1ControlSnapshot::empty()
            },
        )
        .unwrap();

        let enable = plan_type1_runtime_enable(preflight);

        assert_eq!(enable.backend, Type1RuntimeBackend::None);
        assert!(!enable.has_writes());
    }

    #[test]
    fn runtime_enable_plan_records_vmx_control_register_writes() {
        let preflight = plan_type1_runtime_preflight(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::IntelVmx,
                Type1ArchCapabilities::intel_vmx(),
            )
            .unwrap(),
            Type1ControlSnapshot {
                cr0: 0x8000_0011,
                cr4: 0,
                efer: 0x500,
                vmx_cr0_fixed0: 0x8000_0031,
                vmx_cr0_fixed1: u64::MAX,
                vmx_cr4_fixed0: 0,
                vmx_cr4_fixed1: u64::MAX,
            },
        )
        .unwrap();

        let enable = plan_type1_runtime_enable(preflight);

        assert_eq!(enable.backend, Type1RuntimeBackend::IntelVmx);
        assert_eq!(enable.cr0, Some(0x8000_0031));
        assert_eq!(enable.cr4, Some(TYPE1_CR4_VMXE));
        assert_eq!(enable.efer, None);
    }

    #[test]
    fn runtime_enable_plan_records_svm_efer_write() {
        let preflight = plan_type1_runtime_preflight(
            plan_type1_runtime(
                ready_handoff(),
                Type1BackendRequest::AmdSvm,
                Type1ArchCapabilities::amd_svm(),
            )
            .unwrap(),
            Type1ControlSnapshot {
                cr0: 0x8000_0011,
                cr4: 0x20,
                efer: 0x500,
                ..Type1ControlSnapshot::empty()
            },
        )
        .unwrap();

        let enable = plan_type1_runtime_enable(preflight);

        assert_eq!(enable.backend, Type1RuntimeBackend::AmdSvm);
        assert_eq!(enable.cr0, None);
        assert_eq!(enable.cr4, None);
        assert_eq!(
            enable.efer,
            Some(0x500 | aegishv_arch_x86::svm::features::EFER_SVME)
        );
    }
}
