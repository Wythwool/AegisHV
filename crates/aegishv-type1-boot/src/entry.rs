use crate::handoff::{validate_boot_handoff, BootHandoff, BootValidationError};
use crate::layout::{validate_link_layout, LinkLayout, LinkLayoutError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootEntryState {
    FirmwareHandoff,
    LayoutChecked,
    MemoryChecked,
    ReadyForArchitectureInit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootEntryPlan {
    pub state: BootEntryState,
    pub cpu_count_hint: u16,
    pub module_count: usize,
    pub usable_bytes: u64,
    pub stack_top: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootEntryError {
    Layout(LinkLayoutError),
    Handoff(BootValidationError),
    StackTopOverflow,
}

pub fn plan_primary_entry(
    layout: LinkLayout,
    handoff: &BootHandoff<'_>,
    cpu_count_hint: u16,
) -> Result<BootEntryPlan, BootEntryError> {
    validate_link_layout(layout).map_err(BootEntryError::Layout)?;
    let report = validate_boot_handoff(handoff).map_err(BootEntryError::Handoff)?;
    let stack_top = handoff
        .stack_base
        .checked_add(handoff.stack_length)
        .ok_or(BootEntryError::StackTopOverflow)?;

    Ok(BootEntryPlan {
        state: BootEntryState::ReadyForArchitectureInit,
        cpu_count_hint: cpu_count_hint.max(1),
        module_count: report.module_count,
        usable_bytes: report.usable_bytes,
        stack_top,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handoff::{BootMemoryKind, BootMemoryRegion, BootProtocol};

    #[test]
    fn primary_entry_plan_validates_layout_handoff_and_stack_top() {
        let regions = [BootMemoryRegion::new(
            0x40_0000,
            0x200_000,
            BootMemoryKind::Usable,
        )];
        let handoff = BootHandoff {
            protocol: BootProtocol::Limine,
            bootloader_name: "limine",
            command_line: "",
            kernel_base: 0x20_0000,
            kernel_length: 0x40_000,
            stack_base: 0x80_0000,
            stack_length: 0x4000,
            memory_regions: &regions,
            modules: &[],
            rsdp_address: None,
            framebuffer: None,
        };

        let plan = plan_primary_entry(LinkLayout::planned_x86_64(), &handoff, 0).unwrap();

        assert_eq!(plan.state, BootEntryState::ReadyForArchitectureInit);
        assert_eq!(plan.cpu_count_hint, 1);
        assert_eq!(plan.stack_top, 0x80_4000);
    }
}
