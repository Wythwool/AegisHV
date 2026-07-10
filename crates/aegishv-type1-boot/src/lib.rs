#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![deny(unsafe_code)]

pub mod entry;
pub mod handoff;
pub mod image;
pub mod layout;
pub mod limine;

pub const TYPE1_BOOT_ABI_VERSION: u16 = 1;
pub const TYPE1_BOOT_MAGIC: u64 = 0x4145_4749_5348_5631;

pub use entry::{plan_primary_entry, BootEntryPlan, BootEntryState};
pub use handoff::{
    validate_boot_handoff, BootFramebuffer, BootHandoff, BootMemoryKind, BootMemoryRegion,
    BootModule, BootProtocol, BootValidationError, BootValidationReport,
};
pub use image::{
    validate_boot_image_plan, BootImageFormat, BootImagePlan, BootImagePlanError, QemuEvidencePlan,
};
pub use layout::{validate_link_layout, LinkLayout, LinkLayoutError};
pub use limine::{LimineMemmapEntry, LimineMemoryKind, LimineMemoryMapError, LimineUsableMemory};
