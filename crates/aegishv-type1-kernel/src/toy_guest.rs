use aegishv_arch_x86::vmx::ept::{
    EptCapabilities, EptLeafEntry4K, EptMemoryType, EptPermissions, EptPointer, EptTableEntry,
};
use aegishv_arch_x86::vmx::features::VmxErrorKind;
use aegishv_hypervisor_core::error::{CoreError, CoreErrorKind};
use aegishv_hypervisor_core::ids::{GuestPhysical, GuestVirtual, HostPhysical};

use crate::Type1ToyGuestHostPages;

pub const TYPE1_TOY_CODE_GPA: u64 = 0x1000;
pub const TYPE1_TOY_STACK_GPA: u64 = 0x2000;
pub const TYPE1_TOY_GUEST_PML4_GPA: u64 = 0x3000;
pub const TYPE1_TOY_GUEST_PDPT_GPA: u64 = 0x4000;
pub const TYPE1_TOY_GUEST_PD_GPA: u64 = 0x5000;
pub const TYPE1_TOY_GUEST_PT_GPA: u64 = 0x6000;
pub const TYPE1_TOY_GUEST_RIP: u64 = TYPE1_TOY_CODE_GPA;
pub const TYPE1_TOY_CPUID_RIP: u64 = TYPE1_TOY_CODE_GPA + 5;
pub const TYPE1_TOY_HLT_RIP: u64 = TYPE1_TOY_CODE_GPA + 7;
pub const TYPE1_TOY_GUEST_RSP: u64 = TYPE1_TOY_STACK_GPA + 0xff0;
pub const TYPE1_TOY_CODE: [u8; 8] = [0xb8, 0, 0, 0, 0, 0x0f, 0xa2, 0xf4];

const PAGE_SIZE: u64 = 4096;
const PAGE_ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const PAGE_PRESENT: u64 = 1 << 0;
const PAGE_WRITABLE: u64 = 1 << 1;
const BUILD_WRITE_COUNT: usize = 14;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1ToyGuestError {
    Vmx(VmxErrorKind),
    Core(CoreErrorKind),
    ScrubFailed(CoreErrorKind),
    InvalidHostPageLayout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1PageTableWrite {
    pub host_page: HostPhysical,
    pub index: u16,
    pub value: u64,
}

impl Type1PageTableWrite {
    const fn new(host_page: HostPhysical, index: u16, value: u64) -> Self {
        Self {
            host_page,
            index,
            value,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1ToyGuestBuildPlan {
    pub pages: Type1ToyGuestHostPages,
    pub ept_pointer: EptPointer,
    pub guest_cr3: GuestPhysical,
    pub rip: GuestVirtual,
    pub rsp: u64,
    pub writes: [Type1PageTableWrite; BUILD_WRITE_COUNT],
}

impl Type1ToyGuestBuildPlan {
    pub fn new(
        pages: Type1ToyGuestHostPages,
        capabilities: EptCapabilities,
    ) -> Result<Self, Type1ToyGuestError> {
        validate_host_pages(pages)?;
        let ept_pointer = EptPointer::new(pages.ept_pml4, capabilities).map_err(map_vmx_error)?;
        let ept_pml4 = EptTableEntry::new(pages.ept_pdpt)
            .map_err(map_vmx_error)?
            .raw();
        let ept_pdpt = EptTableEntry::new(pages.ept_pd)
            .map_err(map_vmx_error)?
            .raw();
        let ept_pd = EptTableEntry::new(pages.ept_pt)
            .map_err(map_vmx_error)?
            .raw();
        let code_leaf = EptLeafEntry4K::new(
            pages.code,
            EptPermissions::READ_EXECUTE,
            EptMemoryType::WriteBack,
            capabilities,
        )
        .map_err(map_vmx_error)?
        .raw();
        let stack_leaf = read_write_leaf(pages.stack, capabilities)?;
        let guest_pml4_leaf = read_write_leaf(pages.guest_pml4, capabilities)?;
        let guest_pdpt_leaf = read_write_leaf(pages.guest_pdpt, capabilities)?;
        let guest_pd_leaf = read_write_leaf(pages.guest_pd, capabilities)?;
        let guest_pt_leaf = read_write_leaf(pages.guest_pt, capabilities)?;

        let guest_pml4 = guest_page_table_entry(TYPE1_TOY_GUEST_PDPT_GPA, true)?;
        let guest_pdpt = guest_page_table_entry(TYPE1_TOY_GUEST_PD_GPA, true)?;
        let guest_pd = guest_page_table_entry(TYPE1_TOY_GUEST_PT_GPA, true)?;
        let guest_code = guest_page_table_entry(TYPE1_TOY_CODE_GPA, false)?;
        let guest_stack = guest_page_table_entry(TYPE1_TOY_STACK_GPA, true)?;

        Ok(Self {
            pages,
            ept_pointer,
            guest_cr3: GuestPhysical::new(TYPE1_TOY_GUEST_PML4_GPA)
                .map_err(|err| Type1ToyGuestError::Core(err.kind))?,
            rip: GuestVirtual::new(TYPE1_TOY_GUEST_RIP),
            rsp: TYPE1_TOY_GUEST_RSP,
            writes: [
                Type1PageTableWrite::new(pages.ept_pml4, 0, ept_pml4),
                Type1PageTableWrite::new(pages.ept_pdpt, 0, ept_pdpt),
                Type1PageTableWrite::new(pages.ept_pd, 0, ept_pd),
                Type1PageTableWrite::new(pages.ept_pt, 1, code_leaf),
                Type1PageTableWrite::new(pages.ept_pt, 2, stack_leaf),
                Type1PageTableWrite::new(pages.ept_pt, 3, guest_pml4_leaf),
                Type1PageTableWrite::new(pages.ept_pt, 4, guest_pdpt_leaf),
                Type1PageTableWrite::new(pages.ept_pt, 5, guest_pd_leaf),
                Type1PageTableWrite::new(pages.ept_pt, 6, guest_pt_leaf),
                Type1PageTableWrite::new(pages.guest_pml4, 0, guest_pml4),
                Type1PageTableWrite::new(pages.guest_pdpt, 0, guest_pdpt),
                Type1PageTableWrite::new(pages.guest_pd, 0, guest_pd),
                Type1PageTableWrite::new(pages.guest_pt, 1, guest_code),
                Type1PageTableWrite::new(pages.guest_pt, 2, guest_stack),
            ],
        })
    }
}

pub trait Type1PhysicalPageWriter {
    fn zero_page(&mut self, page: HostPhysical) -> Result<(), CoreError>;
    fn write_u64(&mut self, page: HostPhysical, index: u16, value: u64) -> Result<(), CoreError>;
    fn write_bytes(
        &mut self,
        page: HostPhysical,
        offset: usize,
        bytes: &[u8],
    ) -> Result<(), CoreError>;
}

pub fn materialize_type1_toy_guest(
    plan: &Type1ToyGuestBuildPlan,
    writer: &mut impl Type1PhysicalPageWriter,
) -> Result<(), Type1ToyGuestError> {
    let pages = plan.pages.all();
    for page in pages {
        if let Err(error) = writer.zero_page(page) {
            scrub_pages(&pages, writer)?;
            return Err(Type1ToyGuestError::Core(error.kind));
        }
    }
    for write in plan.writes {
        if let Err(error) = writer.write_u64(write.host_page, write.index, write.value) {
            scrub_pages(&pages, writer)?;
            return Err(Type1ToyGuestError::Core(error.kind));
        }
    }
    if let Err(error) = writer.write_bytes(plan.pages.code, 0, &TYPE1_TOY_CODE) {
        scrub_pages(&pages, writer)?;
        return Err(Type1ToyGuestError::Core(error.kind));
    }
    Ok(())
}

fn read_write_leaf(
    page: HostPhysical,
    capabilities: EptCapabilities,
) -> Result<u64, Type1ToyGuestError> {
    EptLeafEntry4K::new(
        page,
        EptPermissions::READ_WRITE,
        EptMemoryType::WriteBack,
        capabilities,
    )
    .map_err(map_vmx_error)
    .map(EptLeafEntry4K::raw)
}

fn guest_page_table_entry(address: u64, writable: bool) -> Result<u64, Type1ToyGuestError> {
    if address == 0 || address % PAGE_SIZE != 0 || address & !PAGE_ADDRESS_MASK != 0 {
        return Err(Type1ToyGuestError::InvalidHostPageLayout);
    }
    Ok(address | PAGE_PRESENT | if writable { PAGE_WRITABLE } else { 0 })
}

fn validate_host_pages(pages: Type1ToyGuestHostPages) -> Result<(), Type1ToyGuestError> {
    let all = pages.all();
    for (index, page) in all.iter().enumerate() {
        if page.get() == 0
            || page.get() % PAGE_SIZE != 0
            || page.get() & !PAGE_ADDRESS_MASK != 0
            || all[..index].contains(page)
        {
            return Err(Type1ToyGuestError::InvalidHostPageLayout);
        }
    }
    Ok(())
}

fn scrub_pages(
    pages: &[HostPhysical; 10],
    writer: &mut impl Type1PhysicalPageWriter,
) -> Result<(), Type1ToyGuestError> {
    for page in pages.iter().copied() {
        writer
            .zero_page(page)
            .map_err(|error| Type1ToyGuestError::ScrubFailed(error.kind))?;
    }
    Ok(())
}

const fn map_vmx_error(error: aegishv_arch_x86::vmx::VmxError) -> Type1ToyGuestError {
    Type1ToyGuestError::Vmx(error.kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aegishv_arch_x86::vmx::ept::{
        EPT_VPID_CAP_MEMORY_TYPE_WB, EPT_VPID_CAP_PAGE_WALK_LENGTH_4,
    };

    fn host(raw: u64) -> HostPhysical {
        HostPhysical::new(raw).unwrap()
    }

    fn pages() -> Type1ToyGuestHostPages {
        Type1ToyGuestHostPages {
            code: host(0x10_0000),
            stack: host(0x10_1000),
            guest_pml4: host(0x10_2000),
            guest_pdpt: host(0x10_3000),
            guest_pd: host(0x10_4000),
            guest_pt: host(0x10_5000),
            ept_pml4: host(0x10_6000),
            ept_pdpt: host(0x10_7000),
            ept_pd: host(0x10_8000),
            ept_pt: host(0x10_9000),
        }
    }

    fn capabilities() -> EptCapabilities {
        EptCapabilities::new(EPT_VPID_CAP_PAGE_WALK_LENGTH_4 | EPT_VPID_CAP_MEMORY_TYPE_WB)
    }

    fn write_for(plan: &Type1ToyGuestBuildPlan, page: HostPhysical, index: u16) -> u64 {
        plan.writes
            .iter()
            .find(|write| write.host_page == page && write.index == index)
            .map(|write| write.value)
            .unwrap()
    }

    #[test]
    fn toy_guest_plan_builds_two_stage_translation_without_wx() {
        let pages = pages();
        let plan = Type1ToyGuestBuildPlan::new(pages, capabilities()).unwrap();

        assert_eq!(plan.ept_pointer.raw(), pages.ept_pml4.get() | 0x1e);
        assert_eq!(plan.guest_cr3.get(), TYPE1_TOY_GUEST_PML4_GPA);
        assert_eq!(plan.rip.get(), TYPE1_TOY_GUEST_RIP);
        assert_eq!(plan.rsp, TYPE1_TOY_GUEST_RSP);
        assert_eq!(write_for(&plan, pages.guest_pml4, 0), 0x4003);
        assert_eq!(write_for(&plan, pages.guest_pdpt, 0), 0x5003);
        assert_eq!(write_for(&plan, pages.guest_pd, 0), 0x6003);
        assert_eq!(write_for(&plan, pages.guest_pt, 1), 0x1001);
        assert_eq!(write_for(&plan, pages.guest_pt, 2), 0x2003);
        assert_eq!(write_for(&plan, pages.ept_pt, 1), pages.code.get() | 0x35);
        assert_eq!(write_for(&plan, pages.ept_pt, 2), pages.stack.get() | 0x33);
    }

    #[test]
    fn toy_guest_plan_rejects_duplicate_host_pages() {
        let mut pages = pages();
        pages.ept_pt = pages.code;

        assert_eq!(
            Type1ToyGuestBuildPlan::new(pages, capabilities()).unwrap_err(),
            Type1ToyGuestError::InvalidHostPageLayout
        );
    }

    struct RecordingWriter {
        pages: Type1ToyGuestHostPages,
        zeroed: [bool; 10],
        writes: usize,
        code: [u8; TYPE1_TOY_CODE.len()],
    }

    impl RecordingWriter {
        fn new(pages: Type1ToyGuestHostPages) -> Self {
            Self {
                pages,
                zeroed: [false; 10],
                writes: 0,
                code: [0; TYPE1_TOY_CODE.len()],
            }
        }

        fn page_index(&self, page: HostPhysical) -> Option<usize> {
            self.pages
                .all()
                .iter()
                .position(|candidate| *candidate == page)
        }
    }

    impl Type1PhysicalPageWriter for RecordingWriter {
        fn zero_page(&mut self, page: HostPhysical) -> Result<(), CoreError> {
            let index = self.page_index(page).ok_or(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "test writer received an unknown page",
            ))?;
            self.zeroed[index] = true;
            Ok(())
        }

        fn write_u64(
            &mut self,
            page: HostPhysical,
            index: u16,
            _value: u64,
        ) -> Result<(), CoreError> {
            let page_index = self.page_index(page).ok_or(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "test writer received an unknown page",
            ))?;
            if !self.zeroed[page_index] || index >= 512 {
                return Err(CoreError::new(
                    CoreErrorKind::InvalidState,
                    "test writer requires zero-before-write",
                ));
            }
            self.writes += 1;
            Ok(())
        }

        fn write_bytes(
            &mut self,
            page: HostPhysical,
            offset: usize,
            bytes: &[u8],
        ) -> Result<(), CoreError> {
            let page_index = self.page_index(page).ok_or(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "test writer received an unknown page",
            ))?;
            if !self.zeroed[page_index] || offset != 0 || bytes.len() != self.code.len() {
                return Err(CoreError::new(
                    CoreErrorKind::InvalidState,
                    "test writer rejected payload placement",
                ));
            }
            self.code.copy_from_slice(bytes);
            Ok(())
        }
    }

    #[test]
    fn materializer_zeroes_every_page_before_writing() {
        let plan = Type1ToyGuestBuildPlan::new(pages(), capabilities()).unwrap();
        let mut writer = RecordingWriter::new(plan.pages);

        materialize_type1_toy_guest(&plan, &mut writer).unwrap();

        assert!(writer.zeroed.iter().all(|value| *value));
        assert_eq!(writer.writes, BUILD_WRITE_COUNT);
        assert_eq!(writer.code, TYPE1_TOY_CODE);
    }
}
