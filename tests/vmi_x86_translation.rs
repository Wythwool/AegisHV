use aegishv::vmi::{
    AddressTranslator, GuestPhysical, GuestRegisters, GuestVirtual,
    SyntheticGuestPhysicalMemoryReader, TranslationError, VmId, VmiErrorKind,
};
use aegishv::vmi_x86::{X86PagingMode, X86_64FourLevelPageWalker, X86_64PageWalker};

const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const LARGE_PAGE: u64 = 1 << 7;
const LARGE_PAGE_PAT: u64 = 1 << 12;
const RESERVED_LARGE_PAGE_ADDRESS_BIT: u64 = 1 << 13;
const NX: u64 = 1 << 63;
const PAGE_4K: u64 = 4096;
const PAGE_2M: u64 = 2 * 1024 * 1024;
const PAGE_1G: u64 = 1024 * 1024 * 1024;

fn regs(cr3: u64) -> GuestRegisters {
    GuestRegisters {
        pc: 0,
        sp: 0,
        cr3_or_ttbr: Some(cr3),
        privilege: Some("kernel".to_string()),
    }
}

fn translate(
    memory: SyntheticGuestPhysicalMemoryReader,
    cr3: u64,
    gva: u64,
) -> Result<aegishv::vmi::TranslationResult, TranslationError> {
    X86_64FourLevelPageWalker::new(memory).translate(VmId(1), &regs(cr3), GuestVirtual(gva))
}

fn translate_with_mode(
    memory: SyntheticGuestPhysicalMemoryReader,
    cr3: u64,
    gva: u64,
    paging_mode: X86PagingMode,
) -> Result<aegishv::vmi::TranslationResult, TranslationError> {
    X86_64PageWalker::new(memory, paging_mode).translate(VmId(1), &regs(cr3), GuestVirtual(gva))
}

fn translate_la57(
    memory: SyntheticGuestPhysicalMemoryReader,
    cr3: u64,
    gva: u64,
) -> Result<aegishv::vmi::TranslationResult, TranslationError> {
    translate_with_mode(memory, cr3, gva, X86PagingMode::La57)
}

fn page_with_entry(index: u64, entry: u64) -> Vec<u8> {
    let mut page = vec![0u8; PAGE_4K as usize];
    let offset = usize::try_from(index * 8).expect("page-table entry offset fits usize");
    page[offset..offset + 8].copy_from_slice(&entry.to_le_bytes());
    page
}

fn entry_prefix(entry: u64, len: usize) -> Vec<u8> {
    entry.to_le_bytes()[..len].to_vec()
}

fn map_table(memory: &mut SyntheticGuestPhysicalMemoryReader, gpa: u64, index: u64, entry: u64) {
    memory
        .map_range(GuestPhysical(gpa), page_with_entry(index, entry))
        .expect("map synthetic page table");
}

fn pml5_index(addr: u64) -> u64 {
    (addr >> 48) & 0x1ff
}

fn pml4_index(addr: u64) -> u64 {
    (addr >> 39) & 0x1ff
}

fn pdpt_index(addr: u64) -> u64 {
    (addr >> 30) & 0x1ff
}

fn pd_index(addr: u64) -> u64 {
    (addr >> 21) & 0x1ff
}

fn pt_index(addr: u64) -> u64 {
    (addr >> 12) & 0x1ff
}

fn four_k_memory(gva: u64, pte: u64) -> SyntheticGuestPhysicalMemoryReader {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(
        &mut memory,
        0x1000,
        pml4_index(gva),
        0x2000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x2000,
        pdpt_index(gva),
        0x3000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x3000,
        pd_index(gva),
        0x4000 | PRESENT | WRITABLE | USER,
    );
    map_table(&mut memory, 0x4000, pt_index(gva), pte);
    memory
}

fn la57_four_k_memory(gva: u64, pte: u64) -> SyntheticGuestPhysicalMemoryReader {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(
        &mut memory,
        0x1000,
        pml5_index(gva),
        0x2000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x2000,
        pml4_index(gva),
        0x3000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x3000,
        pdpt_index(gva),
        0x4000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x4000,
        pd_index(gva),
        0x5000 | PRESENT | WRITABLE | USER,
    );
    map_table(&mut memory, 0x5000, pt_index(gva), pte);
    memory
}

#[test]
fn x86_64_walker_translates_4k_pages_and_masks_cr3_low_bits() {
    let gva = 0x0000_0000_0040_1234;
    let memory = four_k_memory(gva, 0x9000 | PRESENT | WRITABLE | USER);

    let result = translate(memory, 0x1000 | 0xabc, gva).expect("4K translation");

    assert_eq!(result.gpa, GuestPhysical(0x9234));
    assert_eq!(result.page_size, PAGE_4K);
    assert!(result.readable);
    assert!(result.writable);
    assert!(result.user);
    assert!(result.executable);
}

#[test]
fn x86_64_walker_translates_2m_large_pages() {
    let gva = 0x0000_0000_1234_5678;
    let large_base = 0x0000_0000_2400_0000;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(
        &mut memory,
        0x1000,
        pml4_index(gva),
        0x2000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x2000,
        pdpt_index(gva),
        0x3000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x3000,
        pd_index(gva),
        large_base | PRESENT | WRITABLE | USER | LARGE_PAGE,
    );

    let result = translate(memory, 0x1000, gva).expect("2M translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(large_base | (gva & (PAGE_2M - 1)))
    );
    assert_eq!(result.page_size, PAGE_2M);
    assert!(result.writable);
    assert!(result.user);
    assert!(result.executable);
}

#[test]
fn x86_64_walker_translates_1g_large_pages() {
    let gva = 0x0000_0000_4abc_def0;
    let large_base = 0x0000_0001_8000_0000;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(
        &mut memory,
        0x1000,
        pml4_index(gva),
        0x2000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x2000,
        pdpt_index(gva),
        large_base | PRESENT | WRITABLE | USER | LARGE_PAGE,
    );

    let result = translate(memory, 0x1000, gva).expect("1G translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(large_base | (gva & (PAGE_1G - 1)))
    );
    assert_eq!(result.page_size, PAGE_1G);
    assert!(result.writable);
    assert!(result.user);
    assert!(result.executable);
}

#[test]
fn x86_64_walker_accepts_pat_bit_for_2m_large_pages_without_leaking_it_into_gpa() {
    let gva = 0x0000_0000_1224_0678;
    let large_base = 0x0000_0000_2400_0000;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(
        &mut memory,
        0x1000,
        pml4_index(gva),
        0x2000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x2000,
        pdpt_index(gva),
        0x3000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x3000,
        pd_index(gva),
        large_base | LARGE_PAGE_PAT | PRESENT | WRITABLE | USER | LARGE_PAGE,
    );

    let result = translate(memory, 0x1000, gva).expect("2M PAT translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(large_base | (gva & (PAGE_2M - 1)))
    );
    assert_eq!(result.gpa.0 & LARGE_PAGE_PAT, 0);
    assert_eq!(result.page_size, PAGE_2M);
}

#[test]
fn x86_64_walker_accepts_pat_bit_for_1g_large_pages_without_leaking_it_into_gpa() {
    let gva = 0x0000_0000_4abc_0ef0;
    let large_base = 0x0000_0001_8000_0000;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(
        &mut memory,
        0x1000,
        pml4_index(gva),
        0x2000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x2000,
        pdpt_index(gva),
        large_base | LARGE_PAGE_PAT | PRESENT | WRITABLE | USER | LARGE_PAGE,
    );

    let result = translate(memory, 0x1000, gva).expect("1G PAT translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(large_base | (gva & (PAGE_1G - 1)))
    );
    assert_eq!(result.gpa.0 & LARGE_PAGE_PAT, 0);
    assert_eq!(result.page_size, PAGE_1G);
}

#[test]
fn x86_64_walker_rejects_non_canonical_48_bit_addresses() {
    let memory = SyntheticGuestPhysicalMemoryReader::new();
    let err = translate(memory, 0x1000, 0x0000_8000_0000_0000)
        .expect_err("non-canonical and LA57-style addresses must not translate in 4-level mode");

    assert_eq!(err.kind(), VmiErrorKind::InvalidAddress);
    assert!(matches!(
        err,
        TranslationError::InvalidAddress {
            gva: GuestVirtual(0x0000_8000_0000_0000)
        }
    ));
}

#[test]
fn x86_64_paging_mode_selection_keeps_la57_out_of_4level_callers() {
    let gva = 0x0001_0000_0040_1234;
    let memory = la57_four_k_memory(gva, 0x9000 | PRESENT | WRITABLE | USER);

    let err = translate_with_mode(memory.clone(), 0x1000, gva, X86PagingMode::FourLevel)
        .expect_err("4-level mode must reject LA57-only canonical addresses");
    assert_eq!(err.kind(), VmiErrorKind::InvalidAddress);

    let walker = X86_64PageWalker::new(memory, X86PagingMode::La57);
    assert_eq!(walker.paging_mode(), X86PagingMode::La57);
    let result = walker
        .translate(VmId(1), &regs(0x1000), GuestVirtual(gva))
        .expect("LA57 mode translates the same address");

    assert_eq!(result.gpa, GuestPhysical(0x9234));
    assert_eq!(result.page_size, PAGE_4K);
}

#[test]
fn x86_64_la57_walker_rejects_non_canonical_57_bit_addresses() {
    let memory = SyntheticGuestPhysicalMemoryReader::new();
    let err = translate_la57(memory, 0x1000, 0x0100_0000_0000_0000)
        .expect_err("LA57 mode must reject addresses without bit 56 sign extension");

    assert_eq!(err.kind(), VmiErrorKind::InvalidAddress);
    assert!(matches!(
        err,
        TranslationError::InvalidAddress {
            gva: GuestVirtual(0x0100_0000_0000_0000)
        }
    ));
}

#[test]
fn x86_64_la57_walker_translates_4k_pages_and_masks_cr3_low_bits() {
    let gva = 0x0001_0000_0040_1234;
    let memory = la57_four_k_memory(gva, 0x9000 | PRESENT | WRITABLE | USER);

    let result = translate_la57(memory, 0x1000 | 0xabc, gva).expect("LA57 4K translation");

    assert_eq!(result.gpa, GuestPhysical(0x9234));
    assert_eq!(result.page_size, PAGE_4K);
    assert!(result.readable);
    assert!(result.writable);
    assert!(result.user);
    assert!(result.executable);
}

#[test]
fn x86_64_la57_walker_translates_2m_large_pages() {
    let gva = 0x0001_0000_1234_5678;
    let large_base = 0x0000_0000_2600_0000;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(
        &mut memory,
        0x1000,
        pml5_index(gva),
        0x2000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x2000,
        pml4_index(gva),
        0x3000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x3000,
        pdpt_index(gva),
        0x4000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x4000,
        pd_index(gva),
        large_base | PRESENT | WRITABLE | USER | LARGE_PAGE,
    );

    let result = translate_la57(memory, 0x1000, gva).expect("LA57 2M translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(large_base | (gva & (PAGE_2M - 1)))
    );
    assert_eq!(result.page_size, PAGE_2M);
    assert!(result.writable);
    assert!(result.user);
    assert!(result.executable);
}

#[test]
fn x86_64_la57_walker_translates_1g_large_pages() {
    let gva = 0x0001_0000_4abc_def0;
    let large_base = 0x0000_0002_8000_0000;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(
        &mut memory,
        0x1000,
        pml5_index(gva),
        0x2000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x2000,
        pml4_index(gva),
        0x3000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x3000,
        pdpt_index(gva),
        large_base | PRESENT | WRITABLE | USER | LARGE_PAGE,
    );

    let result = translate_la57(memory, 0x1000, gva).expect("LA57 1G translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(large_base | (gva & (PAGE_1G - 1)))
    );
    assert_eq!(result.page_size, PAGE_1G);
    assert!(result.writable);
    assert!(result.user);
    assert!(result.executable);
}

#[test]
fn x86_64_la57_walker_accepts_pat_bit_for_2m_large_pages_without_leaking_it_into_gpa() {
    let gva = 0x0001_0000_1224_0678;
    let large_base = 0x0000_0000_2600_0000;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(
        &mut memory,
        0x1000,
        pml5_index(gva),
        0x2000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x2000,
        pml4_index(gva),
        0x3000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x3000,
        pdpt_index(gva),
        0x4000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x4000,
        pd_index(gva),
        large_base | LARGE_PAGE_PAT | PRESENT | WRITABLE | USER | LARGE_PAGE,
    );

    let result = translate_la57(memory, 0x1000, gva).expect("LA57 2M PAT translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(large_base | (gva & (PAGE_2M - 1)))
    );
    assert_eq!(result.gpa.0 & LARGE_PAGE_PAT, 0);
    assert_eq!(result.page_size, PAGE_2M);
}

#[test]
fn x86_64_la57_walker_accepts_pat_bit_for_1g_large_pages_without_leaking_it_into_gpa() {
    let gva = 0x0001_0000_4abc_0ef0;
    let large_base = 0x0000_0002_8000_0000;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(
        &mut memory,
        0x1000,
        pml5_index(gva),
        0x2000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x2000,
        pml4_index(gva),
        0x3000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x3000,
        pdpt_index(gva),
        large_base | LARGE_PAGE_PAT | PRESENT | WRITABLE | USER | LARGE_PAGE,
    );

    let result = translate_la57(memory, 0x1000, gva).expect("LA57 1G PAT translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(large_base | (gva & (PAGE_1G - 1)))
    );
    assert_eq!(result.gpa.0 & LARGE_PAGE_PAT, 0);
    assert_eq!(result.page_size, PAGE_1G);
}

#[test]
fn x86_64_walker_reports_not_present_entries_at_each_level() {
    for (level, memory) in [
        ("pml4e", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new();
            map_table(&mut memory, 0x1000, 0, 0);
            memory
        }),
        ("pdpte", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new();
            map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT);
            map_table(&mut memory, 0x2000, 0, 0);
            memory
        }),
        ("pde", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new();
            map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT);
            map_table(&mut memory, 0x2000, 0, 0x3000 | PRESENT);
            map_table(&mut memory, 0x3000, 0, 0);
            memory
        }),
        ("pte", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new();
            map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT);
            map_table(&mut memory, 0x2000, 0, 0x3000 | PRESENT);
            map_table(&mut memory, 0x3000, 0, 0x4000 | PRESENT);
            map_table(&mut memory, 0x4000, 0, 0);
            memory
        }),
    ] {
        let err = translate(memory, 0x1000, 0).expect_err("not-present entry must fail");
        assert_eq!(err.kind(), VmiErrorKind::TranslationFailure, "{level}");
        assert!(matches!(err, TranslationError::NotPresent { level: got, .. } if got == level));
    }
}

#[test]
fn x86_64_la57_walker_reports_not_present_entries_at_each_level() {
    for (level, memory) in [
        ("pml5e", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new();
            map_table(&mut memory, 0x1000, 0, 0);
            memory
        }),
        ("pml4e", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new();
            map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT);
            map_table(&mut memory, 0x2000, 0, 0);
            memory
        }),
        ("pdpte", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new();
            map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT);
            map_table(&mut memory, 0x2000, 0, 0x3000 | PRESENT);
            map_table(&mut memory, 0x3000, 0, 0);
            memory
        }),
        ("pde", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new();
            map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT);
            map_table(&mut memory, 0x2000, 0, 0x3000 | PRESENT);
            map_table(&mut memory, 0x3000, 0, 0x4000 | PRESENT);
            map_table(&mut memory, 0x4000, 0, 0);
            memory
        }),
        ("pte", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new();
            map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT);
            map_table(&mut memory, 0x2000, 0, 0x3000 | PRESENT);
            map_table(&mut memory, 0x3000, 0, 0x4000 | PRESENT);
            map_table(&mut memory, 0x4000, 0, 0x5000 | PRESENT);
            map_table(&mut memory, 0x5000, 0, 0);
            memory
        }),
    ] {
        let err = translate_la57(memory, 0x1000, 0).expect_err("not-present entry must fail");
        assert_eq!(err.kind(), VmiErrorKind::TranslationFailure, "{level}");
        assert!(matches!(err, TranslationError::NotPresent { level: got, .. } if got == level));
    }
}

#[test]
fn x86_64_walker_reports_unmapped_page_table_memory() {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT);

    let err = translate(memory, 0x1000, 0).expect_err("missing PDPT memory must fail");

    assert_eq!(err.kind(), VmiErrorKind::MissingMemory);
    assert!(matches!(
        err,
        TranslationError::MissingMemory {
            gpa: GuestPhysical(0x2000),
            ..
        }
    ));
}

#[test]
fn x86_64_la57_walker_reports_unmapped_pml5_memory() {
    let memory = SyntheticGuestPhysicalMemoryReader::new();

    let err = translate_la57(memory, 0x1000, 0).expect_err("missing PML5 memory must fail");

    assert_eq!(err.kind(), VmiErrorKind::MissingMemory);
    assert!(matches!(
        err,
        TranslationError::MissingMemory {
            gpa: GuestPhysical(0x1000),
            ..
        }
    ));
}

#[test]
fn x86_64_walker_rejects_partial_page_table_entry_reads_at_each_level() {
    for (level, memory) in [
        ("pml4e", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new().with_partial_reads(true);
            memory
                .map_range(
                    GuestPhysical(0x1000),
                    entry_prefix(0x2000 | PRESENT | WRITABLE | USER, 4),
                )
                .expect("map partial PML4 entry");
            map_table(&mut memory, 0x2000, 0, 0x3000 | PRESENT | WRITABLE | USER);
            map_table(&mut memory, 0x3000, 0, 0x4000 | PRESENT | WRITABLE | USER);
            map_table(&mut memory, 0x4000, 0, 0x9000 | PRESENT | WRITABLE | USER);
            memory
        }),
        ("pdpte", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new().with_partial_reads(true);
            map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT | WRITABLE | USER);
            memory
                .map_range(
                    GuestPhysical(0x2000),
                    entry_prefix(0x3000 | PRESENT | WRITABLE | USER, 4),
                )
                .expect("map partial PDPT entry");
            map_table(&mut memory, 0x3000, 0, 0x4000 | PRESENT | WRITABLE | USER);
            map_table(&mut memory, 0x4000, 0, 0x9000 | PRESENT | WRITABLE | USER);
            memory
        }),
        ("pde", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new().with_partial_reads(true);
            map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT | WRITABLE | USER);
            map_table(&mut memory, 0x2000, 0, 0x3000 | PRESENT | WRITABLE | USER);
            memory
                .map_range(
                    GuestPhysical(0x3000),
                    entry_prefix(0x4000 | PRESENT | WRITABLE | USER, 4),
                )
                .expect("map partial PD entry");
            map_table(&mut memory, 0x4000, 0, 0x9000 | PRESENT | WRITABLE | USER);
            memory
        }),
        ("pte", {
            let mut memory = SyntheticGuestPhysicalMemoryReader::new().with_partial_reads(true);
            map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT | WRITABLE | USER);
            map_table(&mut memory, 0x2000, 0, 0x3000 | PRESENT | WRITABLE | USER);
            map_table(&mut memory, 0x3000, 0, 0x4000 | PRESENT | WRITABLE | USER);
            memory
                .map_range(
                    GuestPhysical(0x4000),
                    entry_prefix(0x9000 | PRESENT | WRITABLE | USER, 4),
                )
                .expect("map partial PT entry");
            memory
        }),
    ] {
        let err = translate(memory, 0x1000, 0).expect_err("partial entry read must fail");

        assert_eq!(err.kind(), VmiErrorKind::MissingMemory, "{level}");
        assert!(matches!(&err, TranslationError::MissingMemory { .. }));
        let rendered = err.to_string();
        assert!(rendered.contains(level), "{rendered}");
        assert!(rendered.contains("read 4 of 8 bytes"), "{rendered}");
    }
}

#[test]
fn x86_64_la57_walker_rejects_partial_pml5e_reads() {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new().with_partial_reads(true);
    memory
        .map_range(
            GuestPhysical(0x1000),
            entry_prefix(0x2000 | PRESENT | WRITABLE | USER, 4),
        )
        .expect("map partial PML5 entry");

    let err = translate_la57(memory, 0x1000, 0).expect_err("partial PML5 read must fail");

    assert_eq!(err.kind(), VmiErrorKind::MissingMemory);
    assert!(matches!(&err, TranslationError::MissingMemory { .. }));
    let rendered = err.to_string();
    assert!(rendered.contains("pml5e"), "{rendered}");
    assert!(rendered.contains("read 4 of 8 bytes"), "{rendered}");
}

#[test]
fn x86_64_walker_maps_denied_page_table_memory_to_permission_denied() {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT);
    memory
        .deny_range(GuestPhysical(0x2000), 8, "PDPT range is MMIO-like")
        .expect("deny synthetic page-table memory");

    let err = translate(memory, 0x1000, 0).expect_err("denied PDPT memory must fail");

    assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);
    assert!(matches!(
        &err,
        TranslationError::PermissionDenied {
            operation: "read_physical",
            ..
        }
    ));
    assert!(err.to_string().contains("pdpte"));
}

#[test]
fn x86_64_la57_walker_maps_denied_pml5_memory_to_permission_denied() {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    memory
        .deny_range(GuestPhysical(0x1000), 8, "PML5 range is MMIO-like")
        .expect("deny synthetic PML5 memory");

    let err = translate_la57(memory, 0x1000, 0).expect_err("denied PML5 memory must fail");

    assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);
    assert!(matches!(
        &err,
        TranslationError::PermissionDenied {
            operation: "read_physical",
            ..
        }
    ));
    assert!(err.to_string().contains("pml5e"));
}

#[test]
fn x86_64_walker_maps_unavailable_page_table_memory_to_temporarily_unavailable() {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut memory, 0x1000, 0, 0x2000 | PRESENT);
    memory
        .mark_unavailable_range(GuestPhysical(0x2000), 8, "snapshot chunk is missing")
        .expect("mark synthetic page-table memory unavailable");

    let err = translate(memory, 0x1000, 0).expect_err("unavailable PDPT memory must fail");

    assert_eq!(err.kind(), VmiErrorKind::TemporarilyUnavailable);
    assert!(matches!(
        &err,
        TranslationError::TemporarilyUnavailable {
            resource: "synthetic-memory-range",
            ..
        }
    ));
    assert!(err.to_string().contains("pdpte"));
}

#[test]
fn x86_64_la57_walker_maps_unavailable_pml5_memory_to_temporarily_unavailable() {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    memory
        .mark_unavailable_range(GuestPhysical(0x1000), 8, "snapshot PML5 chunk is missing")
        .expect("mark synthetic PML5 memory unavailable");

    let err = translate_la57(memory, 0x1000, 0).expect_err("unavailable PML5 memory must fail");

    assert_eq!(err.kind(), VmiErrorKind::TemporarilyUnavailable);
    assert!(matches!(
        &err,
        TranslationError::TemporarilyUnavailable {
            resource: "synthetic-memory-range",
            ..
        }
    ));
    assert!(err.to_string().contains("pml5e"));
}

#[test]
fn x86_64_walker_rejects_reserved_large_page_address_bits() {
    let gva = 0x0000_0000_1234_5678;
    let mut two_m = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut two_m, 0x1000, pml4_index(gva), 0x2000 | PRESENT);
    map_table(&mut two_m, 0x2000, pdpt_index(gva), 0x3000 | PRESENT);
    map_table(
        &mut two_m,
        0x3000,
        pd_index(gva),
        0x2000_0000 | RESERVED_LARGE_PAGE_ADDRESS_BIT | PRESENT | LARGE_PAGE,
    );

    let err = translate(two_m, 0x1000, gva).expect_err("reserved 2M address bit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "pde", .. }
    ));

    let mut one_g = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut one_g, 0x1000, pml4_index(gva), 0x2000 | PRESENT);
    map_table(
        &mut one_g,
        0x2000,
        pdpt_index(gva),
        0x8000_0000 | RESERVED_LARGE_PAGE_ADDRESS_BIT | PRESENT | LARGE_PAGE,
    );

    let err = translate(one_g, 0x1000, gva).expect_err("reserved 1G address bit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "pdpte", .. }
    ));
}

#[test]
fn x86_64_la57_walker_rejects_reserved_large_page_address_bits() {
    let gva = 0x0001_0000_1234_5678;
    let mut two_m = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut two_m, 0x1000, pml5_index(gva), 0x2000 | PRESENT);
    map_table(&mut two_m, 0x2000, pml4_index(gva), 0x3000 | PRESENT);
    map_table(&mut two_m, 0x3000, pdpt_index(gva), 0x4000 | PRESENT);
    map_table(
        &mut two_m,
        0x4000,
        pd_index(gva),
        0x2000_0000 | RESERVED_LARGE_PAGE_ADDRESS_BIT | PRESENT | LARGE_PAGE,
    );

    let err = translate_la57(two_m, 0x1000, gva).expect_err("reserved 2M address bit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "pde", .. }
    ));

    let mut one_g = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut one_g, 0x1000, pml5_index(gva), 0x2000 | PRESENT);
    map_table(&mut one_g, 0x2000, pml4_index(gva), 0x3000 | PRESENT);
    map_table(
        &mut one_g,
        0x3000,
        pdpt_index(gva),
        0x8000_0000 | RESERVED_LARGE_PAGE_ADDRESS_BIT | PRESENT | LARGE_PAGE,
    );

    let err = translate_la57(one_g, 0x1000, gva).expect_err("reserved 1G address bit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "pdpte", .. }
    ));
}

#[test]
fn x86_64_la57_walker_rejects_large_page_bits_in_pml5e_and_pml4e() {
    let mut pml5_large = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut pml5_large, 0x1000, 0, 0x2000 | PRESENT | LARGE_PAGE);

    let err = translate_la57(pml5_large, 0x1000, 0).expect_err("PML5 large bit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "pml5e", .. }
    ));

    let mut pml4_large = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut pml4_large, 0x1000, 0, 0x2000 | PRESENT);
    map_table(&mut pml4_large, 0x2000, 0, 0x3000 | PRESENT | LARGE_PAGE);

    let err = translate_la57(pml4_large, 0x1000, 0).expect_err("PML4 large bit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "pml4e", .. }
    ));
}

#[test]
fn x86_64_walker_propagates_nx_writable_and_user_flags() {
    let gva = 0x0000_0000_0000_3456;
    let memory = four_k_memory(gva, 0x9000 | PRESENT | NX);

    let result = translate(memory, 0x1000, gva).expect("4K translation with restricted leaf");

    assert_eq!(result.gpa, GuestPhysical(0x9456));
    assert!(!result.writable);
    assert!(!result.user);
    assert!(!result.executable);
}

#[test]
fn x86_64_la57_walker_propagates_nx_writable_and_user_flags_across_five_levels() {
    let gva = 0x0001_0000_0000_3456;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut memory, 0x1000, pml5_index(gva), 0x2000 | PRESENT | NX);
    map_table(
        &mut memory,
        0x2000,
        pml4_index(gva),
        0x3000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x3000,
        pdpt_index(gva),
        0x4000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x4000,
        pd_index(gva),
        0x5000 | PRESENT | WRITABLE | USER,
    );
    map_table(
        &mut memory,
        0x5000,
        pt_index(gva),
        0x9000 | PRESENT | WRITABLE | USER,
    );

    let result = translate_la57(memory, 0x1000, gva).expect("LA57 translation with PML5 limits");

    assert_eq!(result.gpa, GuestPhysical(0x9456));
    assert!(!result.writable);
    assert!(!result.user);
    assert!(!result.executable);
}

#[test]
fn x86_64_walker_requires_cr3_context() {
    let memory = SyntheticGuestPhysicalMemoryReader::new();
    let regs = GuestRegisters {
        pc: 0,
        sp: 0,
        cr3_or_ttbr: None,
        privilege: None,
    };

    let err = X86_64FourLevelPageWalker::new(memory)
        .translate(VmId(1), &regs, GuestVirtual(0))
        .expect_err("missing CR3 must fail");

    assert_eq!(err.kind(), VmiErrorKind::InvalidInput);
    assert!(matches!(
        err,
        TranslationError::MissingContext {
            field: "cr3_or_ttbr"
        }
    ));
}
