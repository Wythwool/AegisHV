use aegishv_arch_x86::vmx::ept::{
    EptCapabilities, EptLeafEntry4K, EptMemoryType, EptPermissions, EptPointer, EptTableEntry,
};
use aegishv_arch_x86::vmx::features::VmxErrorKind;
use aegishv_arch_x86::vmx::vmcs_config::VmcsDescriptorTableState;
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
pub const TYPE1_TOY_DEADLINE_PROBE_RIPS: [u64; 9] = [
    TYPE1_TOY_GUEST_RIP,
    TYPE1_TOY_GUEST_RIP + 2,
    TYPE1_TOY_GUEST_RIP + 4,
    TYPE1_TOY_GUEST_RIP + 10,
    TYPE1_TOY_GUEST_RIP + 15,
    TYPE1_TOY_GUEST_RIP + 17,
    TYPE1_TOY_GUEST_RIP + 19,
    TYPE1_TOY_GUEST_RIP + 21,
    TYPE1_TOY_GUEST_RIP + 23,
];
pub const TYPE1_TOY_DEADLINE_FALLBACK_RIP: u64 = TYPE1_TOY_CODE_GPA + 25;
pub const TYPE1_TOY_CONTINUATION_RIP: u64 = TYPE1_TOY_CODE_GPA + 26;
pub const TYPE1_TOY_IO_RIP: u64 = TYPE1_TOY_CODE_GPA + 28;
pub const TYPE1_TOY_IO_BITMAP_B_RIP: u64 = TYPE1_TOY_CODE_GPA + 34;
pub const TYPE1_TOY_CPUID_RIP: u64 = TYPE1_TOY_CODE_GPA + 39;
pub const TYPE1_TOY_RDMSR_RIP: u64 = TYPE1_TOY_CODE_GPA + 46;
pub const TYPE1_TOY_PAT_RDMSR_RIP: u64 = TYPE1_TOY_CODE_GPA + 53;
pub const TYPE1_TOY_X87_GUARD_RIP: u64 = TYPE1_TOY_CODE_GPA + 70;
pub const TYPE1_TOY_SIMD_GUARD_RIP: u64 = TYPE1_TOY_CODE_GPA + 72;
pub const TYPE1_TOY_UD2_RIP: u64 = TYPE1_TOY_CODE_GPA + 76;
pub const TYPE1_TOY_HLT_RIP: u64 = TYPE1_TOY_CODE_GPA + 78;
pub const TYPE1_TOY_PAT_MISMATCH_HLT_RIP: u64 = TYPE1_TOY_CODE_GPA + 79;
pub const TYPE1_TOY_RDMSR_INDEX: u32 = aegishv_arch_x86::vmx::toy_exit::TOY_RDMSR_IA32_EFER;
pub const TYPE1_TOY_PAT_INDEX: u32 = 0x0000_0277;
pub const TYPE1_TOY_GUEST_RSP: u64 = TYPE1_TOY_STACK_GPA + 0xff0;
pub const TYPE1_TOY_GUEST_CS: u16 = 0x08;
pub const TYPE1_TOY_GUEST_SS: u16 = 0x10;
pub const TYPE1_TOY_GUEST_RFLAGS: u64 = 0x02;
pub const TYPE1_TOY_HLT_EXIT_RFLAGS: u64 = 0x46;
pub const TYPE1_TOY_UD_HANDLER_RIP: u64 = TYPE1_TOY_CODE_GPA + 0x100;
pub const TYPE1_TOY_GDT_BASE: u64 = TYPE1_TOY_CODE_GPA + 0x200;
pub const TYPE1_TOY_GDT_LIMIT: u32 = TYPE1_TOY_GDT.len() as u32 - 1;
pub const TYPE1_TOY_IDT_BASE: u64 = TYPE1_TOY_CODE_GPA + 0x300;
pub const TYPE1_TOY_IDT_LIMIT: u32 = TYPE1_TOY_IDT.len() as u32 - 1;
pub const TYPE1_TOY_UD_HANDLER_COOKIE: u64 = 0x5544_494e_4a45_4354;
pub const TYPE1_TOY_DEADLINE_FALLBACK_TSC_TICKS: u32 = 1 << 27;
pub const TYPE1_TOY_DEADLINE_FALLBACK_ITERATIONS: u32 = 1 << 24;
pub const TYPE1_TOY_CODE: [u8; 80] = [
    0x0f, 0x31, 0x89, 0xc1, 0x81, 0xc1, 0x00, 0x00, 0x00, 0x08, 0xbb, 0x00, 0x00, 0x00, 0x01, 0x0f,
    0x31, 0x29, 0xc8, 0x79, 0x04, 0xff, 0xcb, 0x75, 0xf6, 0xf4, 0xb0, b'A', 0xe6, 0xe9, 0x66, 0xba,
    0x00, 0x80, 0xee, 0x31, 0xc0, 0x31, 0xc9, 0x0f, 0xa2, 0xb9, 0x80, 0x00, 0x00, 0xc0, 0x0f, 0x32,
    0xb9, 0x77, 0x02, 0x00, 0x00, 0x0f, 0x32, 0x3d, 0x06, 0x00, 0x01, 0x04, 0x75, 0x11, 0x81, 0xfa,
    0x05, 0x07, 0x06, 0x00, 0x75, 0x09, 0xd9, 0xd0, 0x66, 0x0f, 0x6f, 0xc0, 0x0f, 0x0b, 0xf4, 0xf4,
];

const TYPE1_TOY_CODE_OFFSET: usize = 0;
const TYPE1_TOY_UD_HANDLER_OFFSET: usize = 0x100;
const TYPE1_TOY_GDT_OFFSET: usize = 0x200;
const TYPE1_TOY_IDT_OFFSET: usize = 0x300;
const TYPE1_TOY_UD_HANDLER: [u8; 17] = [
    0x49, 0xbf, 0x54, 0x43, 0x45, 0x4a, 0x4e, 0x49, 0x44, 0x55, 0x48, 0x83, 0x04, 0x24, 0x02, 0x48,
    0xcf,
];
const TYPE1_TOY_GDT: [u8; 40] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // null
    0xff, 0xff, 0x00, 0x00, 0x00, 0x9b, 0xaf, 0x00, // 64-bit code, accessed
    0xff, 0xff, 0x00, 0x00, 0x00, 0x93, 0xcf, 0x00, // data, accessed
    0x67, 0x00, 0x00, 0x00, 0x00, 0x8b, 0x00, 0x00, // busy 64-bit TSS, low
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // busy 64-bit TSS, high
];
const TYPE1_TOY_IDT: [u8; 112] = build_type1_toy_idt();

const fn build_type1_toy_idt() -> [u8; 112] {
    let mut idt = [0; 112];
    let gate = 6 * 16;
    idt[gate] = TYPE1_TOY_UD_HANDLER_RIP as u8;
    idt[gate + 1] = (TYPE1_TOY_UD_HANDLER_RIP >> 8) as u8;
    idt[gate + 2] = TYPE1_TOY_GUEST_CS as u8;
    idt[gate + 3] = (TYPE1_TOY_GUEST_CS >> 8) as u8;
    idt[gate + 4] = 0;
    idt[gate + 5] = 0x8e;
    idt[gate + 6] = (TYPE1_TOY_UD_HANDLER_RIP >> 16) as u8;
    idt[gate + 7] = (TYPE1_TOY_UD_HANDLER_RIP >> 24) as u8;
    idt[gate + 8] = (TYPE1_TOY_UD_HANDLER_RIP >> 32) as u8;
    idt[gate + 9] = (TYPE1_TOY_UD_HANDLER_RIP >> 40) as u8;
    idt[gate + 10] = (TYPE1_TOY_UD_HANDLER_RIP >> 48) as u8;
    idt[gate + 11] = (TYPE1_TOY_UD_HANDLER_RIP >> 56) as u8;
    idt
}

const PAGE_SIZE: u64 = 4096;
const PAGE_ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const PAGE_PRESENT: u64 = 1 << 0;
const PAGE_WRITABLE: u64 = 1 << 1;
const BUILD_WRITE_COUNT: usize = 14;
const MSR_BITMAP_PAT_READ_BYTE: usize = (TYPE1_TOY_PAT_INDEX as usize) / 8;
const MSR_BITMAP_PAT_READ_BIT: u8 = 1 << (TYPE1_TOY_PAT_INDEX % 8);
const VMX_IO_INTERCEPTION_BITMAP: [u8; PAGE_SIZE as usize] = [0xff; PAGE_SIZE as usize];
const VMX_MSR_INTERCEPTION_BITMAP: [u8; PAGE_SIZE as usize] = build_msr_interception_bitmap();

const fn build_msr_interception_bitmap() -> [u8; PAGE_SIZE as usize] {
    let mut bitmap = [0xff; PAGE_SIZE as usize];
    bitmap[MSR_BITMAP_PAT_READ_BYTE] &= !MSR_BITMAP_PAT_READ_BIT;
    bitmap
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1ToyGuestError {
    Vmx(VmxErrorKind),
    Core(CoreErrorKind),
    ScrubFailed(CoreErrorKind),
    GuestImageVerificationFailed,
    BitmapVerificationFailed,
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
    pub gdtr: VmcsDescriptorTableState,
    pub idtr: VmcsDescriptorTableState,
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
            gdtr: VmcsDescriptorTableState::new(TYPE1_TOY_GDT_BASE, TYPE1_TOY_GDT_LIMIT),
            idtr: VmcsDescriptorTableState::new(TYPE1_TOY_IDT_BASE, TYPE1_TOY_IDT_LIMIT),
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
    fn read_u8(&mut self, page: HostPhysical, offset: usize) -> Result<u8, CoreError>;
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
    for (offset, bytes) in [
        (TYPE1_TOY_CODE_OFFSET, TYPE1_TOY_CODE.as_slice()),
        (TYPE1_TOY_UD_HANDLER_OFFSET, TYPE1_TOY_UD_HANDLER.as_slice()),
        (TYPE1_TOY_GDT_OFFSET, TYPE1_TOY_GDT.as_slice()),
        (TYPE1_TOY_IDT_OFFSET, TYPE1_TOY_IDT.as_slice()),
    ] {
        if let Err(error) = writer.write_bytes(plan.pages.code, offset, bytes) {
            scrub_pages(&pages, writer)?;
            return Err(Type1ToyGuestError::Core(error.kind));
        }
    }
    for offset in 0..PAGE_SIZE as usize {
        let byte = match writer.read_u8(plan.pages.code, offset) {
            Ok(byte) => byte,
            Err(error) => {
                scrub_pages(&pages, writer)?;
                return Err(Type1ToyGuestError::Core(error.kind));
            }
        };
        if byte != expected_type1_toy_code_page_byte(offset) {
            scrub_pages(&pages, writer)?;
            return Err(Type1ToyGuestError::GuestImageVerificationFailed);
        }
    }
    for (page, bitmap) in [
        (plan.pages.io_bitmap_a, &VMX_IO_INTERCEPTION_BITMAP),
        (plan.pages.io_bitmap_b, &VMX_IO_INTERCEPTION_BITMAP),
        (plan.pages.msr_bitmap, &VMX_MSR_INTERCEPTION_BITMAP),
    ] {
        if let Err(error) = writer.write_bytes(page, 0, bitmap) {
            scrub_pages(&pages, writer)?;
            return Err(Type1ToyGuestError::Core(error.kind));
        }
    }
    for page in plan.pages.interception_bitmaps() {
        for offset in 0..PAGE_SIZE as usize {
            let byte = match writer.read_u8(page, offset) {
                Ok(byte) => byte,
                Err(error) => {
                    scrub_pages(&pages, writer)?;
                    return Err(Type1ToyGuestError::Core(error.kind));
                }
            };
            let expected = if page == plan.pages.msr_bitmap {
                VMX_MSR_INTERCEPTION_BITMAP[offset]
            } else {
                VMX_IO_INTERCEPTION_BITMAP[offset]
            };
            if byte != expected {
                scrub_pages(&pages, writer)?;
                return Err(Type1ToyGuestError::BitmapVerificationFailed);
            }
        }
    }
    Ok(())
}

fn expected_type1_toy_code_page_byte(offset: usize) -> u8 {
    for (start, bytes) in [
        (TYPE1_TOY_CODE_OFFSET, TYPE1_TOY_CODE.as_slice()),
        (TYPE1_TOY_UD_HANDLER_OFFSET, TYPE1_TOY_UD_HANDLER.as_slice()),
        (TYPE1_TOY_GDT_OFFSET, TYPE1_TOY_GDT.as_slice()),
        (TYPE1_TOY_IDT_OFFSET, TYPE1_TOY_IDT.as_slice()),
    ] {
        if let Some(index) = offset.checked_sub(start) {
            if let Some(byte) = bytes.get(index) {
                return *byte;
            }
        }
    }
    0
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
    pages: &[HostPhysical; 13],
    writer: &mut impl Type1PhysicalPageWriter,
) -> Result<(), Type1ToyGuestError> {
    let mut first_error = None;
    for page in pages.iter().copied() {
        if let Err(error) = writer.zero_page(page) {
            if first_error.is_none() {
                first_error = Some(error.kind);
            }
        }
    }
    if let Some(kind) = first_error {
        return Err(Type1ToyGuestError::ScrubFailed(kind));
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
            io_bitmap_a: host(0x10_a000),
            io_bitmap_b: host(0x10_b000),
            msr_bitmap: host(0x10_c000),
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
        assert_eq!(
            plan.gdtr,
            VmcsDescriptorTableState::new(TYPE1_TOY_GDT_BASE, TYPE1_TOY_GDT_LIMIT)
        );
        assert_eq!(
            plan.idtr,
            VmcsDescriptorTableState::new(TYPE1_TOY_IDT_BASE, TYPE1_TOY_IDT_LIMIT)
        );
        assert_eq!(write_for(&plan, pages.guest_pml4, 0), 0x4003);
        assert_eq!(write_for(&plan, pages.guest_pdpt, 0), 0x5003);
        assert_eq!(write_for(&plan, pages.guest_pd, 0), 0x6003);
        assert_eq!(write_for(&plan, pages.guest_pt, 1), 0x1001);
        assert_eq!(write_for(&plan, pages.guest_pt, 2), 0x2003);
        assert_eq!(write_for(&plan, pages.ept_pt, 1), pages.code.get() | 0x35);
        assert_eq!(write_for(&plan, pages.ept_pt, 2), pages.stack.get() | 0x33);
    }

    #[test]
    fn toy_guest_payload_matches_the_bounded_exit_contract() {
        assert_eq!(
            TYPE1_TOY_CODE,
            [
                0x0f, 0x31, 0x89, 0xc1, 0x81, 0xc1, 0x00, 0x00, 0x00, 0x08, 0xbb, 0x00, 0x00, 0x00,
                0x01, 0x0f, 0x31, 0x29, 0xc8, 0x79, 0x04, 0xff, 0xcb, 0x75, 0xf6, 0xf4, 0xb0, b'A',
                0xe6, 0xe9, 0x66, 0xba, 0x00, 0x80, 0xee, 0x31, 0xc0, 0x31, 0xc9, 0x0f, 0xa2, 0xb9,
                0x80, 0x00, 0x00, 0xc0, 0x0f, 0x32, 0xb9, 0x77, 0x02, 0x00, 0x00, 0x0f, 0x32, 0x3d,
                0x06, 0x00, 0x01, 0x04, 0x75, 0x11, 0x81, 0xfa, 0x05, 0x07, 0x06, 0x00, 0x75, 0x09,
                0xd9, 0xd0, 0x66, 0x0f, 0x6f, 0xc0, 0x0f, 0x0b, 0xf4, 0xf4
            ]
        );
        assert_eq!(TYPE1_TOY_CODE.len(), 80);
        assert_eq!(TYPE1_TOY_DEADLINE_FALLBACK_TSC_TICKS, 1 << 27);
        let encoded_fallback = u32::from_le_bytes(TYPE1_TOY_CODE[6..10].try_into().unwrap());
        assert_eq!(encoded_fallback, TYPE1_TOY_DEADLINE_FALLBACK_TSC_TICKS);
        assert!(
            u64::from(encoded_fallback)
                > aegishv_arch_x86::vmx::capabilities::VMX_TOY_GUEST_BUDGET_TSC_TICKS
        );
        assert!(encoded_fallback < (1 << 31));
        let encoded_iterations = u32::from_le_bytes(TYPE1_TOY_CODE[11..15].try_into().unwrap());
        assert_eq!(encoded_iterations, TYPE1_TOY_DEADLINE_FALLBACK_ITERATIONS);
        assert_ne!(encoded_iterations, 0);
        assert_eq!(
            TYPE1_TOY_DEADLINE_PROBE_RIPS,
            [0x1000, 0x1002, 0x1004, 0x100a, 0x100f, 0x1011, 0x1013, 0x1015, 0x1017]
        );
        assert!(!TYPE1_TOY_DEADLINE_PROBE_RIPS.contains(&TYPE1_TOY_DEADLINE_FALLBACK_RIP));
        assert_eq!(TYPE1_TOY_DEADLINE_FALLBACK_RIP, TYPE1_TOY_GUEST_RIP + 25);
        assert_eq!(TYPE1_TOY_CONTINUATION_RIP, TYPE1_TOY_GUEST_RIP + 26);
        assert_eq!(TYPE1_TOY_IO_RIP, TYPE1_TOY_GUEST_RIP + 28);
        assert_eq!(TYPE1_TOY_IO_BITMAP_B_RIP, TYPE1_TOY_GUEST_RIP + 34);
        assert_eq!(TYPE1_TOY_CPUID_RIP, TYPE1_TOY_GUEST_RIP + 39);
        assert_eq!(TYPE1_TOY_RDMSR_RIP, TYPE1_TOY_GUEST_RIP + 46);
        assert_eq!(TYPE1_TOY_PAT_RDMSR_RIP, TYPE1_TOY_GUEST_RIP + 53);
        assert_eq!(TYPE1_TOY_X87_GUARD_RIP, TYPE1_TOY_GUEST_RIP + 70);
        assert_eq!(TYPE1_TOY_SIMD_GUARD_RIP, TYPE1_TOY_GUEST_RIP + 72);
        assert_eq!(TYPE1_TOY_UD2_RIP, TYPE1_TOY_GUEST_RIP + 76);
        assert_eq!(TYPE1_TOY_HLT_RIP, TYPE1_TOY_GUEST_RIP + 78);
        assert_eq!(TYPE1_TOY_PAT_MISMATCH_HLT_RIP, TYPE1_TOY_GUEST_RIP + 79);
        assert_eq!(TYPE1_TOY_RDMSR_INDEX, 0xc000_0080);
        assert_eq!(TYPE1_TOY_PAT_INDEX, 0x277);
        assert_eq!(
            u32::from_le_bytes(TYPE1_TOY_CODE[49..53].try_into().unwrap()),
            TYPE1_TOY_PAT_INDEX
        );
        let pat_low = u32::from_le_bytes(TYPE1_TOY_CODE[56..60].try_into().unwrap());
        let pat_high = u32::from_le_bytes(TYPE1_TOY_CODE[64..68].try_into().unwrap());
        assert_eq!(
            u64::from(pat_low) | (u64::from(pat_high) << 32),
            aegishv_arch_x86::vmx::vmcs_config::VMX_TOY_GUEST_PAT_RAW
        );
        assert_eq!(TYPE1_TOY_CODE[60], 0x75);
        assert_eq!(60 + 2 + TYPE1_TOY_CODE[61] as usize, 79);
        assert_eq!(TYPE1_TOY_CODE[68], 0x75);
        assert_eq!(68 + 2 + TYPE1_TOY_CODE[69] as usize, 79);
        assert_eq!(&TYPE1_TOY_CODE[76..78], &[0x0f, 0x0b]);
        assert_eq!(TYPE1_TOY_CODE[78], 0xf4);
        assert_eq!(TYPE1_TOY_CODE[79], 0xf4);
        // The equal high PAT compare establishes PF and ZF. The fixed IRETQ
        // route reaches HLT, whose configured VM exit saves RF clear.
        assert_eq!(TYPE1_TOY_GUEST_RFLAGS, 0x02);
        assert_eq!(TYPE1_TOY_HLT_EXIT_RFLAGS, 0x46);
        let frame_size = 5 * core::mem::size_of::<u64>() as u64;
        let frame_low = TYPE1_TOY_GUEST_RSP - frame_size;
        assert_eq!(frame_size, 40);
        assert_eq!(frame_low, TYPE1_TOY_STACK_GPA + 0xfc8);
        assert!((TYPE1_TOY_STACK_GPA..TYPE1_TOY_STACK_GPA + PAGE_SIZE).contains(&frame_low));
    }

    #[test]
    fn toy_guest_exception_tables_and_handler_are_fixed_and_read_only() {
        assert_eq!(TYPE1_TOY_UD_HANDLER_OFFSET, 0x100);
        assert_eq!(TYPE1_TOY_GDT_OFFSET, 0x200);
        assert_eq!(TYPE1_TOY_IDT_OFFSET, 0x300);
        assert_eq!(TYPE1_TOY_UD_HANDLER_RIP, 0x1100);
        assert_eq!(TYPE1_TOY_GDT_BASE, 0x1200);
        assert_eq!(TYPE1_TOY_IDT_BASE, 0x1300);
        assert_eq!(TYPE1_TOY_GDT_LIMIT, 0x27);
        assert_eq!(TYPE1_TOY_IDT_LIMIT, 0x6f);
        assert_eq!(TYPE1_TOY_GUEST_CS, 0x08);
        assert_eq!(TYPE1_TOY_GUEST_SS, 0x10);
        assert_eq!(
            TYPE1_TOY_UD_HANDLER,
            [
                0x49, 0xbf, 0x54, 0x43, 0x45, 0x4a, 0x4e, 0x49, 0x44, 0x55, 0x48, 0x83, 0x04, 0x24,
                0x02, 0x48, 0xcf
            ]
        );
        assert_eq!(
            u64::from_le_bytes(TYPE1_TOY_UD_HANDLER[2..10].try_into().unwrap()),
            TYPE1_TOY_UD_HANDLER_COOKIE
        );
        assert_eq!(&TYPE1_TOY_GDT[8..16], &[0xff, 0xff, 0, 0, 0, 0x9b, 0xaf, 0]);
        assert_eq!(
            &TYPE1_TOY_GDT[16..24],
            &[0xff, 0xff, 0, 0, 0, 0x93, 0xcf, 0]
        );
        assert_eq!(
            &TYPE1_TOY_GDT[24..40],
            &[0x67, 0, 0, 0, 0, 0x8b, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_ne!(TYPE1_TOY_GDT[13] & 1, 0);
        assert_ne!(TYPE1_TOY_GDT[21] & 1, 0);
        assert!(TYPE1_TOY_IDT[..6 * 16].iter().all(|byte| *byte == 0));
        assert_eq!(
            &TYPE1_TOY_IDT[6 * 16..7 * 16],
            &[0x00, 0x11, 0x08, 0x00, 0x00, 0x8e, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0]
        );
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
        zeroed: [bool; 13],
        writes: usize,
        code: [u8; PAGE_SIZE as usize],
        code_reads: usize,
        bitmaps: [[u8; PAGE_SIZE as usize]; 3],
        bitmap_reads: [usize; 3],
        corrupt_read: Option<(HostPhysical, usize, u8)>,
    }

    impl RecordingWriter {
        fn new(pages: Type1ToyGuestHostPages) -> Self {
            Self {
                pages,
                zeroed: [false; 13],
                writes: 0,
                code: [0; PAGE_SIZE as usize],
                code_reads: 0,
                bitmaps: [[0; PAGE_SIZE as usize]; 3],
                bitmap_reads: [0; 3],
                corrupt_read: None,
            }
        }

        fn page_index(&self, page: HostPhysical) -> Option<usize> {
            self.pages
                .all()
                .iter()
                .position(|candidate| *candidate == page)
        }

        fn bitmap_index(&self, page: HostPhysical) -> Option<usize> {
            self.pages
                .interception_bitmaps()
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
            if page == self.pages.code {
                self.code.fill(0);
            }
            if let Some(index) = self.bitmap_index(page) {
                self.bitmaps[index].fill(0);
            }
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
            if !self.zeroed[page_index] {
                return Err(CoreError::new(
                    CoreErrorKind::InvalidState,
                    "test writer requires zero-before-write",
                ));
            }
            let end = offset.checked_add(bytes.len()).ok_or(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "test writer rejected an overflowing byte range",
            ))?;
            if page == self.pages.code && end <= self.code.len() {
                self.code[offset..end].copy_from_slice(bytes);
            } else if let Some(index) = self.bitmap_index(page) {
                if offset != 0 || bytes.len() != PAGE_SIZE as usize {
                    return Err(CoreError::new(
                        CoreErrorKind::InvalidArgument,
                        "test writer rejected bitmap size",
                    ));
                }
                self.bitmaps[index].copy_from_slice(bytes);
            } else {
                return Err(CoreError::new(
                    CoreErrorKind::InvalidAddress,
                    "test writer rejected byte destination",
                ));
            }
            Ok(())
        }

        fn read_u8(&mut self, page: HostPhysical, offset: usize) -> Result<u8, CoreError> {
            if let Some((corrupt_page, corrupt_offset, value)) = self.corrupt_read {
                if (page, offset) == (corrupt_page, corrupt_offset) {
                    return Ok(value);
                }
            }
            if page == self.pages.code {
                let byte = self.code.get(offset).copied().ok_or(CoreError::new(
                    CoreErrorKind::InvalidArgument,
                    "test writer rejected code byte offset",
                ))?;
                self.code_reads += 1;
                return Ok(byte);
            }
            if let Some(index) = self.bitmap_index(page) {
                let byte = self.bitmaps[index]
                    .get(offset)
                    .copied()
                    .ok_or(CoreError::new(
                        CoreErrorKind::InvalidArgument,
                        "test writer rejected bitmap byte offset",
                    ))?;
                self.bitmap_reads[index] += 1;
                return Ok(byte);
            }
            Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "test writer rejected byte source",
            ))
        }
    }

    #[test]
    fn materializer_zeroes_every_page_before_writing() {
        let plan = Type1ToyGuestBuildPlan::new(pages(), capabilities()).unwrap();
        let mut writer = RecordingWriter::new(plan.pages);

        materialize_type1_toy_guest(&plan, &mut writer).unwrap();

        assert!(writer.zeroed.iter().all(|value| *value));
        assert_eq!(writer.writes, BUILD_WRITE_COUNT);
        assert_eq!(&writer.code[..TYPE1_TOY_CODE.len()], &TYPE1_TOY_CODE);
        assert_eq!(
            &writer.code[TYPE1_TOY_UD_HANDLER_OFFSET
                ..TYPE1_TOY_UD_HANDLER_OFFSET + TYPE1_TOY_UD_HANDLER.len()],
            &TYPE1_TOY_UD_HANDLER
        );
        assert_eq!(
            &writer.code[TYPE1_TOY_GDT_OFFSET..TYPE1_TOY_GDT_OFFSET + TYPE1_TOY_GDT.len()],
            &TYPE1_TOY_GDT
        );
        assert_eq!(
            &writer.code[TYPE1_TOY_IDT_OFFSET..TYPE1_TOY_IDT_OFFSET + TYPE1_TOY_IDT.len()],
            &TYPE1_TOY_IDT
        );
        assert!(
            writer.code[TYPE1_TOY_CODE.len()..TYPE1_TOY_UD_HANDLER_OFFSET]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(writer.code_reads, PAGE_SIZE as usize);
        assert!(writer.bitmaps[0].iter().all(|byte| *byte == 0xff));
        assert!(writer.bitmaps[1].iter().all(|byte| *byte == 0xff));
        assert_eq!(writer.bitmaps[2], VMX_MSR_INTERCEPTION_BITMAP);
        assert_eq!(writer.bitmap_reads, [PAGE_SIZE as usize; 3]);
        assert_eq!(writer.bitmaps[2][0x4e], 0x7f);
        assert_eq!(writer.bitmaps[2][0x410], 0xff);
        assert_eq!(writer.bitmaps[2][0x84e], 0xff);
        assert_eq!(
            writer.bitmaps[2]
                .iter()
                .map(|byte| byte.count_zeros())
                .sum::<u32>(),
            1
        );
    }

    #[test]
    fn materializer_scrubs_after_guest_handler_readback_mismatch() {
        let plan = Type1ToyGuestBuildPlan::new(pages(), capabilities()).unwrap();
        let mut writer = RecordingWriter::new(plan.pages);
        writer.corrupt_read = Some((
            plan.pages.code,
            TYPE1_TOY_UD_HANDLER_OFFSET + TYPE1_TOY_UD_HANDLER.len() - 1,
            0x90,
        ));

        assert_eq!(
            materialize_type1_toy_guest(&plan, &mut writer).unwrap_err(),
            Type1ToyGuestError::GuestImageVerificationFailed
        );
        assert_eq!(writer.code, [0; PAGE_SIZE as usize]);
        assert!(writer
            .bitmaps
            .iter()
            .all(|bitmap| bitmap.iter().all(|byte| *byte == 0)));
    }

    #[test]
    fn materializer_rejects_a_nonaccessed_guest_code_descriptor() {
        let plan = Type1ToyGuestBuildPlan::new(pages(), capabilities()).unwrap();
        let mut writer = RecordingWriter::new(plan.pages);
        writer.corrupt_read = Some((plan.pages.code, TYPE1_TOY_GDT_OFFSET + 13, 0x9a));

        assert_eq!(
            materialize_type1_toy_guest(&plan, &mut writer).unwrap_err(),
            Type1ToyGuestError::GuestImageVerificationFailed
        );
        assert_eq!(writer.code, [0; PAGE_SIZE as usize]);
    }

    #[test]
    fn materializer_rejects_a_nonpresent_ud_interrupt_gate() {
        let plan = Type1ToyGuestBuildPlan::new(pages(), capabilities()).unwrap();
        let mut writer = RecordingWriter::new(plan.pages);
        writer.corrupt_read = Some((plan.pages.code, TYPE1_TOY_IDT_OFFSET + 6 * 16 + 5, 0x0e));

        assert_eq!(
            materialize_type1_toy_guest(&plan, &mut writer).unwrap_err(),
            Type1ToyGuestError::GuestImageVerificationFailed
        );
        assert_eq!(writer.code, [0; PAGE_SIZE as usize]);
    }

    #[test]
    fn materializer_rejects_nonzero_data_between_fixed_guest_objects() {
        let plan = Type1ToyGuestBuildPlan::new(pages(), capabilities()).unwrap();
        let mut writer = RecordingWriter::new(plan.pages);
        writer.corrupt_read = Some((plan.pages.code, TYPE1_TOY_CODE.len(), 0x90));

        assert_eq!(
            materialize_type1_toy_guest(&plan, &mut writer).unwrap_err(),
            Type1ToyGuestError::GuestImageVerificationFailed
        );
        assert_eq!(writer.code, [0; PAGE_SIZE as usize]);
    }

    #[test]
    fn materializer_scrubs_all_pages_after_bitmap_readback_mismatch() {
        let plan = Type1ToyGuestBuildPlan::new(pages(), capabilities()).unwrap();
        let mut writer = RecordingWriter::new(plan.pages);
        writer.corrupt_read = Some((plan.pages.msr_bitmap, PAGE_SIZE as usize - 1, 0));

        assert_eq!(
            materialize_type1_toy_guest(&plan, &mut writer).unwrap_err(),
            Type1ToyGuestError::BitmapVerificationFailed
        );
        assert_eq!(writer.code, [0; PAGE_SIZE as usize]);
        assert!(writer
            .bitmaps
            .iter()
            .all(|bitmap| bitmap.iter().all(|byte| *byte == 0)));
    }

    #[test]
    fn materializer_rejects_a_trapped_pat_read_bit_on_readback() {
        let plan = Type1ToyGuestBuildPlan::new(pages(), capabilities()).unwrap();
        let mut writer = RecordingWriter::new(plan.pages);
        writer.corrupt_read = Some((plan.pages.msr_bitmap, MSR_BITMAP_PAT_READ_BYTE, 0xff));

        assert_eq!(
            materialize_type1_toy_guest(&plan, &mut writer).unwrap_err(),
            Type1ToyGuestError::BitmapVerificationFailed
        );
        assert_eq!(writer.code, [0; PAGE_SIZE as usize]);
        assert!(writer
            .bitmaps
            .iter()
            .all(|bitmap| bitmap.iter().all(|byte| *byte == 0)));
    }

    struct RejectingScrubWriter;

    impl Type1PhysicalPageWriter for RejectingScrubWriter {
        fn zero_page(&mut self, _page: HostPhysical) -> Result<(), CoreError> {
            Err(CoreError::new(
                CoreErrorKind::ZeroingFailed,
                "test scrub failure",
            ))
        }

        fn write_u64(
            &mut self,
            _page: HostPhysical,
            _index: u16,
            _value: u64,
        ) -> Result<(), CoreError> {
            Err(CoreError::new(
                CoreErrorKind::InvalidState,
                "write followed a failed scrub",
            ))
        }

        fn write_bytes(
            &mut self,
            _page: HostPhysical,
            _offset: usize,
            _bytes: &[u8],
        ) -> Result<(), CoreError> {
            Err(CoreError::new(
                CoreErrorKind::InvalidState,
                "payload followed a failed scrub",
            ))
        }

        fn read_u8(&mut self, _page: HostPhysical, _offset: usize) -> Result<u8, CoreError> {
            Err(CoreError::new(
                CoreErrorKind::InvalidState,
                "read followed a failed scrub",
            ))
        }
    }

    #[test]
    fn materializer_propagates_page_scrub_failure() {
        let plan = Type1ToyGuestBuildPlan::new(pages(), capabilities()).unwrap();

        assert_eq!(
            materialize_type1_toy_guest(&plan, &mut RejectingScrubWriter).unwrap_err(),
            Type1ToyGuestError::ScrubFailed(CoreErrorKind::ZeroingFailed)
        );
    }
}
