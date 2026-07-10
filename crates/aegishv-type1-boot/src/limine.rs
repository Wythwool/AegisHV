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
    MappedReserved,
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
            Self::MappedReserved => BootMemoryKind::Reserved,
        }
    }

    pub const fn from_raw(raw: u64) -> Result<Self, LimineMemoryMapError> {
        match raw {
            0 => Ok(Self::Usable),
            1 => Ok(Self::Reserved),
            2 => Ok(Self::AcpiReclaimable),
            3 => Ok(Self::AcpiNvs),
            4 => Ok(Self::BadMemory),
            5 => Ok(Self::BootloaderReclaimable),
            6 => Ok(Self::ExecutableAndModules),
            7 => Ok(Self::Framebuffer),
            8 => Ok(Self::MappedReserved),
            _ => Err(LimineMemoryMapError::UnknownMemoryKind),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimineMemmapEntry {
    pub base: u64,
    pub length: u64,
    pub kind: u64,
}

impl LimineMemmapEntry {
    pub const fn new(base: u64, length: u64, kind: u64) -> Self {
        Self { base, length, kind }
    }

    pub const fn empty() -> Self {
        Self::new(0, 0, 1)
    }

    fn end(self) -> Result<u64, LimineMemoryMapError> {
        self.base
            .checked_add(self.length)
            .ok_or(LimineMemoryMapError::AddressOverflow)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimineMemoryMapError {
    UnknownMemoryKind,
    AddressOverflow,
    InvalidUsableRegion,
    UsableRegionOverlap,
    CapacityExceeded,
}

#[derive(Debug)]
pub struct LimineUsableMemory<const N: usize> {
    regions: [BootMemoryRegion; N],
    len: usize,
}

impl<const N: usize> LimineUsableMemory<N> {
    pub fn from_entries(entries: &[LimineMemmapEntry]) -> Result<Self, LimineMemoryMapError> {
        let mut usable = Self {
            regions: [BootMemoryRegion::new(0, 0, BootMemoryKind::Reserved); N],
            len: 0,
        };

        for (index, entry) in entries.iter().copied().enumerate() {
            let kind = LimineMemoryKind::from_raw(entry.kind)?;
            let end = entry.end()?;
            if kind != LimineMemoryKind::Usable {
                continue;
            }
            if entry.length == 0 || entry.base % 4096 != 0 || entry.length % 4096 != 0 {
                return Err(LimineMemoryMapError::InvalidUsableRegion);
            }
            for (other_index, other) in entries.iter().copied().enumerate() {
                if other_index == index {
                    continue;
                }
                LimineMemoryKind::from_raw(other.kind)?;
                let other_end = other.end()?;
                if entry.base < other_end && other.base < end {
                    return Err(LimineMemoryMapError::UsableRegionOverlap);
                }
            }
            if usable.len >= N {
                return Err(LimineMemoryMapError::CapacityExceeded);
            }
            usable.regions[usable.len] =
                BootMemoryRegion::new(entry.base, entry.length, BootMemoryKind::Usable);
            usable.len += 1;
        }

        Ok(usable)
    }

    pub fn regions(&self) -> &[BootMemoryRegion] {
        &self.regions[..self.len]
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
        assert_eq!(
            LimineMemoryKind::from_raw(8).unwrap(),
            LimineMemoryKind::MappedReserved
        );
    }

    #[test]
    fn limine_memmap_entry_matches_protocol_layout() {
        assert_eq!(core::mem::size_of::<LimineMemmapEntry>(), 24);
        assert_eq!(core::mem::align_of::<LimineMemmapEntry>(), 8);
    }

    #[test]
    fn usable_projection_accepts_overlapping_unaligned_reserved_entries() {
        let entries = [
            LimineMemmapEntry::new(0x10_0000, 0x20_0000, 0),
            LimineMemmapEntry::new(0x101, 0x333, 1),
            LimineMemmapEntry::new(0x200, 0x100, 8),
        ];

        let usable = LimineUsableMemory::<4>::from_entries(&entries).unwrap();

        assert_eq!(usable.regions().len(), 1);
        assert_eq!(usable.regions()[0].base, 0x10_0000);
    }

    #[test]
    fn usable_projection_rejects_overlap_and_unknown_types() {
        let overlap = [
            LimineMemmapEntry::new(0x10_0000, 0x20_0000, 0),
            LimineMemmapEntry::new(0x20_0000, 0x1000, 1),
        ];
        assert_eq!(
            LimineUsableMemory::<4>::from_entries(&overlap).unwrap_err(),
            LimineMemoryMapError::UsableRegionOverlap
        );

        let unknown = [LimineMemmapEntry::new(0x10_0000, 0x1000, 9)];
        assert_eq!(
            LimineUsableMemory::<4>::from_entries(&unknown).unwrap_err(),
            LimineMemoryMapError::UnknownMemoryKind
        );
    }

    #[test]
    fn bootloader_reclaimable_and_mapped_reserved_are_never_usable() {
        let entries = [
            LimineMemmapEntry::new(0x10_0000, 0x1000, 5),
            LimineMemmapEntry::new(0x20_0000, 0x1000, 8),
        ];

        let usable = LimineUsableMemory::<4>::from_entries(&entries).unwrap();

        assert!(usable.regions().is_empty());
    }
}
