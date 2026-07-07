use aegishv_hypervisor_core::memory::{MemoryRegion, MemoryRegionKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootProtocol {
    Limine,
    Uefi,
    Multiboot2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootMemoryKind {
    Usable,
    Reserved,
    Acpi,
    Mmio,
    Bad,
    BootloaderReclaimable,
    KernelAndModules,
}

impl BootMemoryKind {
    pub const fn to_core_kind(self) -> MemoryRegionKind {
        match self {
            Self::Usable | Self::BootloaderReclaimable => MemoryRegionKind::Usable,
            Self::Acpi => MemoryRegionKind::Acpi,
            Self::Mmio => MemoryRegionKind::Mmio,
            Self::Bad => MemoryRegionKind::Bad,
            Self::Reserved | Self::KernelAndModules => MemoryRegionKind::Reserved,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootMemoryRegion {
    pub base: u64,
    pub length: u64,
    pub kind: BootMemoryKind,
}

impl BootMemoryRegion {
    pub const fn new(base: u64, length: u64, kind: BootMemoryKind) -> Self {
        Self { base, length, kind }
    }

    pub fn to_core_region(self) -> MemoryRegion {
        MemoryRegion::new(self.base, self.length, self.kind.to_core_kind())
    }

    pub fn end(self) -> Result<u64, BootValidationError> {
        self.base
            .checked_add(self.length)
            .ok_or(BootValidationError::MemoryRegionOverflow)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootFramebuffer {
    pub address: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bits_per_pixel: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootModule<'a> {
    pub name: &'a str,
    pub address: u64,
    pub length: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootHandoff<'a> {
    pub protocol: BootProtocol,
    pub bootloader_name: &'a str,
    pub command_line: &'a str,
    pub kernel_base: u64,
    pub kernel_length: u64,
    pub stack_base: u64,
    pub stack_length: u64,
    pub memory_regions: &'a [BootMemoryRegion],
    pub modules: &'a [BootModule<'a>],
    pub rsdp_address: Option<u64>,
    pub framebuffer: Option<BootFramebuffer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootValidationError {
    MissingBootloaderName,
    KernelRangeInvalid,
    StackRangeInvalid,
    StackNotPageAligned,
    MissingMemoryMap,
    MemoryRegionInvalid,
    MemoryRegionOverflow,
    MemoryRegionOverlap,
    MissingUsableMemory,
    ModuleRangeInvalid,
    ModuleOverlapsKernel,
    FramebufferInvalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootValidationReport {
    pub protocol: BootProtocol,
    pub usable_regions: usize,
    pub usable_bytes: u64,
    pub module_count: usize,
    pub has_rsdp: bool,
    pub has_framebuffer: bool,
}

pub fn validate_boot_handoff(
    handoff: &BootHandoff<'_>,
) -> Result<BootValidationReport, BootValidationError> {
    if handoff.bootloader_name.trim().is_empty() {
        return Err(BootValidationError::MissingBootloaderName);
    }
    validate_range(handoff.kernel_base, handoff.kernel_length)
        .map_err(|_| BootValidationError::KernelRangeInvalid)?;
    validate_range(handoff.stack_base, handoff.stack_length)
        .map_err(|_| BootValidationError::StackRangeInvalid)?;
    if handoff.stack_base % 4096 != 0 || handoff.stack_length % 4096 != 0 {
        return Err(BootValidationError::StackNotPageAligned);
    }
    if handoff.memory_regions.is_empty() {
        return Err(BootValidationError::MissingMemoryMap);
    }

    let kernel_end = handoff.kernel_base + handoff.kernel_length;
    let mut usable_regions = 0usize;
    let mut usable_bytes = 0u64;

    for (index, region) in handoff.memory_regions.iter().enumerate() {
        let end = validate_region(*region)?;
        for previous in handoff.memory_regions.iter().take(index) {
            let previous_end = previous.end()?;
            let overlaps = region.base < previous_end && previous.base < end;
            if overlaps {
                return Err(BootValidationError::MemoryRegionOverlap);
            }
        }
        if matches!(
            region.kind,
            BootMemoryKind::Usable | BootMemoryKind::BootloaderReclaimable
        ) {
            usable_regions += 1;
            usable_bytes = usable_bytes.saturating_add(region.length);
        }
    }

    if usable_regions == 0 {
        return Err(BootValidationError::MissingUsableMemory);
    }

    for module in handoff.modules {
        validate_range(module.address, module.length)
            .map_err(|_| BootValidationError::ModuleRangeInvalid)?;
        let module_end = module.address + module.length;
        if module.address < kernel_end && handoff.kernel_base < module_end {
            return Err(BootValidationError::ModuleOverlapsKernel);
        }
    }

    if let Some(framebuffer) = handoff.framebuffer {
        validate_framebuffer(framebuffer)?;
    }

    Ok(BootValidationReport {
        protocol: handoff.protocol,
        usable_regions,
        usable_bytes,
        module_count: handoff.modules.len(),
        has_rsdp: handoff.rsdp_address.is_some(),
        has_framebuffer: handoff.framebuffer.is_some(),
    })
}

fn validate_region(region: BootMemoryRegion) -> Result<u64, BootValidationError> {
    if region.length == 0 || region.base % 4096 != 0 || region.length % 4096 != 0 {
        return Err(BootValidationError::MemoryRegionInvalid);
    }
    region.end()
}

fn validate_range(base: u64, length: u64) -> Result<u64, ()> {
    if length == 0 {
        return Err(());
    }
    base.checked_add(length).ok_or(())
}

fn validate_framebuffer(framebuffer: BootFramebuffer) -> Result<(), BootValidationError> {
    if framebuffer.address == 0
        || framebuffer.width == 0
        || framebuffer.height == 0
        || framebuffer.pitch == 0
        || framebuffer.bits_per_pixel == 0
    {
        return Err(BootValidationError::FramebufferInvalid);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_handoff<'a>(
        memory_regions: &'a [BootMemoryRegion],
        modules: &'a [BootModule<'a>],
    ) -> BootHandoff<'a> {
        BootHandoff {
            protocol: BootProtocol::Limine,
            bootloader_name: "limine",
            command_line: "serial=on",
            kernel_base: 0x20_0000,
            kernel_length: 0x40_000,
            stack_base: 0x80_0000,
            stack_length: 0x4000,
            memory_regions,
            modules,
            rsdp_address: Some(0xf0000),
            framebuffer: None,
        }
    }

    #[test]
    fn valid_boot_handoff_reports_usable_memory_and_modules() {
        let regions = [
            BootMemoryRegion::new(0x20_0000, 0x100_000, BootMemoryKind::KernelAndModules),
            BootMemoryRegion::new(0x40_0000, 0x200_000, BootMemoryKind::Usable),
        ];
        let modules = [BootModule {
            name: "toy-guest",
            address: 0x60_0000,
            length: 0x1000,
        }];

        let report = validate_boot_handoff(&valid_handoff(&regions, &modules)).unwrap();

        assert_eq!(report.protocol, BootProtocol::Limine);
        assert_eq!(report.usable_regions, 1);
        assert_eq!(report.usable_bytes, 0x200_000);
        assert_eq!(report.module_count, 1);
        assert!(report.has_rsdp);
    }

    #[test]
    fn handoff_rejects_overlapping_regions_and_modules_inside_kernel() {
        let overlap = [
            BootMemoryRegion::new(0x20_0000, 0x2000, BootMemoryKind::Reserved),
            BootMemoryRegion::new(0x20_1000, 0x2000, BootMemoryKind::Usable),
        ];
        assert_eq!(
            validate_boot_handoff(&valid_handoff(&overlap, &[])).unwrap_err(),
            BootValidationError::MemoryRegionOverlap
        );

        let regions = [BootMemoryRegion::new(
            0x40_0000,
            0x200_000,
            BootMemoryKind::Usable,
        )];
        let bad_module = [BootModule {
            name: "bad",
            address: 0x20_1000,
            length: 0x1000,
        }];
        assert_eq!(
            validate_boot_handoff(&valid_handoff(&regions, &bad_module)).unwrap_err(),
            BootValidationError::ModuleOverlapsKernel
        );
    }
}
