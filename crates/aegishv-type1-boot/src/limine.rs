use crate::handoff::{BootMemoryKind, BootMemoryRegion, BootProtocol};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimineMemoryKind {
    Usable,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    BadMemory,
    BootloaderReclaimable,
    ExecutableAndModules,
    Framebuffer,
}

impl LimineMemoryKind {
    pub const fn to_boot_kind(self) -> BootMemoryKind {
        match self {
            Self::Usable => BootMemoryKind::Usable,
            Self::Reserved => BootMemoryKind::Reserved,
            Self::AcpiReclaimable | Self::AcpiNvs => BootMemoryKind::Acpi,
            Self::BadMemory => BootMemoryKind::Bad,
            Self::BootloaderReclaimable => BootMemoryKind::BootloaderReclaimable,
            Self::ExecutableAndModules => BootMemoryKind::KernelAndModules,
            Self::Framebuffer => BootMemoryKind::Mmio,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineMemoryEntry {
    pub base: u64,
    pub length: u64,
    pub kind: LimineMemoryKind,
}

impl LimineMemoryEntry {
    pub const fn new(base: u64, length: u64, kind: LimineMemoryKind) -> Self {
        Self { base, length, kind }
    }

    pub const fn to_boot_region(self) -> BootMemoryRegion {
        BootMemoryRegion::new(self.base, self.length, self.kind.to_boot_kind())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineRequestPlan {
    pub protocol: BootProtocol,
    pub needs_memory_map: bool,
    pub needs_rsdp: bool,
    pub needs_hhdm: bool,
    pub needs_kernel_file: bool,
    pub needs_executable_address: bool,
}

impl LimineRequestPlan {
    pub const fn x86_64_first_boot() -> Self {
        Self {
            protocol: BootProtocol::Limine,
            needs_memory_map: true,
            needs_rsdp: true,
            needs_hhdm: true,
            needs_kernel_file: true,
            needs_executable_address: true,
        }
    }

    pub const fn is_minimal_handoff_complete(self) -> bool {
        self.needs_memory_map
            && self.needs_hhdm
            && self.needs_kernel_file
            && self.needs_executable_address
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limine_request_plan_requires_memory_map_hhdm_and_kernel_address() {
        let plan = LimineRequestPlan::x86_64_first_boot();

        assert!(plan.is_minimal_handoff_complete());
        assert!(plan.needs_rsdp);
    }

    #[test]
    fn limine_memory_kinds_map_to_internal_boot_kinds() {
        assert_eq!(
            LimineMemoryEntry::new(0x1000, 0x1000, LimineMemoryKind::BootloaderReclaimable)
                .to_boot_region()
                .kind,
            BootMemoryKind::BootloaderReclaimable
        );
        assert_eq!(
            LimineMemoryEntry::new(0x2000, 0x1000, LimineMemoryKind::ExecutableAndModules)
                .to_boot_region()
                .kind,
            BootMemoryKind::KernelAndModules
        );
    }
}
