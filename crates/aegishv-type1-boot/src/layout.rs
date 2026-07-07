pub const KERNEL_PHYSICAL_BASE: u64 = 0x20_0000;
pub const KERNEL_VIRTUAL_BASE: u64 = 0xffff_8000_0020_0000;
pub const BOOT_STACK_SIZE: u64 = 64 * 1024;
pub const EARLY_HEAP_SIZE: u64 = 2 * 1024 * 1024;
pub const AP_TRAMPOLINE_PAGE: u64 = 0x7000;
pub const SERIAL_COM1_PORT: u16 = 0x3f8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkLayout {
    pub physical_base: u64,
    pub virtual_base: u64,
    pub text_start: u64,
    pub text_end: u64,
    pub rodata_end: u64,
    pub data_end: u64,
    pub bss_end: u64,
    pub boot_stack_size: u64,
}

impl LinkLayout {
    pub const fn planned_x86_64() -> Self {
        Self {
            physical_base: KERNEL_PHYSICAL_BASE,
            virtual_base: KERNEL_VIRTUAL_BASE,
            text_start: KERNEL_VIRTUAL_BASE,
            text_end: KERNEL_VIRTUAL_BASE + 0x20_000,
            rodata_end: KERNEL_VIRTUAL_BASE + 0x30_000,
            data_end: KERNEL_VIRTUAL_BASE + 0x40_000,
            bss_end: KERNEL_VIRTUAL_BASE + 0x50_000,
            boot_stack_size: BOOT_STACK_SIZE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkLayoutError {
    PhysicalBaseTooLow,
    VirtualBaseNotCanonicalHighHalf,
    SectionsOutOfOrder,
    SectionNotPageAligned,
    StackTooSmall,
    StackNotPageAligned,
}

pub fn validate_link_layout(layout: LinkLayout) -> Result<(), LinkLayoutError> {
    if layout.physical_base < 0x20_0000 {
        return Err(LinkLayoutError::PhysicalBaseTooLow);
    }
    if layout.virtual_base < 0xffff_8000_0000_0000 {
        return Err(LinkLayoutError::VirtualBaseNotCanonicalHighHalf);
    }
    if !(layout.text_start <= layout.text_end
        && layout.text_end <= layout.rodata_end
        && layout.rodata_end <= layout.data_end
        && layout.data_end <= layout.bss_end)
    {
        return Err(LinkLayoutError::SectionsOutOfOrder);
    }
    for value in [
        layout.physical_base,
        layout.virtual_base,
        layout.text_start,
        layout.text_end,
        layout.rodata_end,
        layout.data_end,
        layout.bss_end,
    ] {
        if value % 4096 != 0 {
            return Err(LinkLayoutError::SectionNotPageAligned);
        }
    }
    if layout.boot_stack_size < 16 * 1024 {
        return Err(LinkLayoutError::StackTooSmall);
    }
    if layout.boot_stack_size % 4096 != 0 {
        return Err(LinkLayoutError::StackNotPageAligned);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planned_x86_layout_passes_basic_link_constraints() {
        validate_link_layout(LinkLayout::planned_x86_64()).unwrap();
    }

    #[test]
    fn layout_rejects_low_physical_base_and_misaligned_stack() {
        let mut layout = LinkLayout::planned_x86_64();
        layout.physical_base = 0x100000;
        assert_eq!(
            validate_link_layout(layout).unwrap_err(),
            LinkLayoutError::PhysicalBaseTooLow
        );

        let mut layout = LinkLayout::planned_x86_64();
        layout.boot_stack_size = 17 * 1024;
        assert_eq!(
            validate_link_layout(layout).unwrap_err(),
            LinkLayoutError::StackNotPageAligned
        );
    }
}
