use aegishv::vmi::{
    AddressTranslator, GuestPhysical, GuestRegisters, GuestVirtual,
    SyntheticGuestPhysicalMemoryReader, TranslationError, VmId, VmiErrorKind,
};
use aegishv::vmi_arm64::{
    translate_arm64_stage1, Arm64Granule, Arm64Stage1Context, Arm64Stage1Translator, Arm64Tcr,
};

const TABLE: u64 = 0b11;
const PAGE: u64 = 0b11;
const BLOCK: u64 = 0b01;
const AF: u64 = 1 << 10;
const AP_USER: u64 = 1 << 6;
const AP_READ_ONLY: u64 = 1 << 7;
const PXN: u64 = 1 << 53;
const UXN: u64 = 1 << 54;
const PXN_TABLE: u64 = 1 << 59;
const UXN_TABLE: u64 = 1 << 60;
const AP_TABLE_NO_USER: u64 = 1 << 61;
const AP_TABLE_READ_ONLY: u64 = 1 << 62;
const PAGE_4K: u64 = 4096;
const PAGE_16K: u64 = 16 * 1024;
const PAGE_64K: u64 = 64 * 1024;
const BLOCK_2M: u64 = 2 * 1024 * 1024;
const BLOCK_32M: u64 = 32 * 1024 * 1024;
const BLOCK_1G: u64 = 1024 * 1024 * 1024;
const BLOCK_512M: u64 = 512 * 1024 * 1024;
const BLOCK_64G: u64 = 64 * 1024 * 1024 * 1024;
const RESERVED_BLOCK_BIT: u64 = 1 << 13;
const RESERVED_16K_OUTPUT_BIT: u64 = 1 << 12;
const RESERVED_OUTPUT_HIGH_BIT: u64 = 1 << 48;

fn context() -> Arm64Stage1Context {
    Arm64Stage1Context::four_k(Some(0x1000), Some(0x8000), 16, 16)
}

fn context_16k() -> Arm64Stage1Context {
    Arm64Stage1Context::sixteen_k(Some(0x4000), Some(0x1_0000), 17, 17)
}

fn context_64k() -> Arm64Stage1Context {
    Arm64Stage1Context::sixty_four_k(Some(0x1_0000), Some(0x4_0000), 22, 22)
}

fn regs() -> GuestRegisters {
    GuestRegisters {
        pc: 0,
        sp: 0,
        cr3_or_ttbr: None,
        privilege: Some("kernel".to_string()),
    }
}

fn translate(
    memory: SyntheticGuestPhysicalMemoryReader,
    ctx: Arm64Stage1Context,
    va: u64,
) -> Result<aegishv::vmi::TranslationResult, TranslationError> {
    translate_arm64_stage1(&memory, VmId(1), &ctx, GuestVirtual(va))
}

fn descriptor_table(index: u64, descriptor: u64, table_size: u64) -> Vec<u8> {
    let mut page = vec![0u8; table_size as usize];
    let offset = usize::try_from(index * 8).expect("descriptor offset fits usize");
    page[offset..offset + 8].copy_from_slice(&descriptor.to_le_bytes());
    page
}

fn descriptor_prefix(descriptor: u64, len: usize) -> Vec<u8> {
    descriptor.to_le_bytes()[..len].to_vec()
}

fn map_table(
    memory: &mut SyntheticGuestPhysicalMemoryReader,
    gpa: u64,
    index: u64,
    descriptor: u64,
) {
    map_granule_table(memory, gpa, index, descriptor, PAGE_4K);
}

fn map_granule_table(
    memory: &mut SyntheticGuestPhysicalMemoryReader,
    gpa: u64,
    index: u64,
    descriptor: u64,
    table_size: u64,
) {
    memory
        .map_range(
            GuestPhysical(gpa),
            descriptor_table(index, descriptor, table_size),
        )
        .expect("map synthetic ARM64 translation table");
}

fn table_desc(base: u64) -> u64 {
    base | TABLE
}

fn page_desc(base: u64) -> u64 {
    base | AF | AP_USER | PAGE
}

fn block_desc(base: u64) -> u64 {
    base | AF | AP_USER | BLOCK
}

fn l0_index(va: u64) -> u64 {
    (va >> 39) & 0x1ff
}

fn l1_index(va: u64) -> u64 {
    (va >> 30) & 0x1ff
}

fn l2_index(va: u64) -> u64 {
    (va >> 21) & 0x1ff
}

fn l3_index(va: u64) -> u64 {
    (va >> 12) & 0x1ff
}

fn ttbr1_input(va: u64) -> u64 {
    ttbr1_input_bits(va, 48)
}

fn ttbr1_input_bits(va: u64, va_bits: u8) -> u64 {
    va & ((1u64 << u32::from(va_bits)) - 1)
}

fn granule_index(va: u64, level: u8, granule: Arm64Granule) -> u64 {
    let (shift, mask) = match (granule, level) {
        (Arm64Granule::Size4K, 0) => (39, 0x1ff),
        (Arm64Granule::Size4K, 1) => (30, 0x1ff),
        (Arm64Granule::Size4K, 2) => (21, 0x1ff),
        (Arm64Granule::Size4K, 3) => (12, 0x1ff),
        (Arm64Granule::Size16K, 1) => (36, 0x7ff),
        (Arm64Granule::Size16K, 2) => (25, 0x7ff),
        (Arm64Granule::Size16K, 3) => (14, 0x7ff),
        (Arm64Granule::Size64K, 2) => (29, 0x1fff),
        (Arm64Granule::Size64K, 3) => (16, 0x1fff),
        _ => panic!("invalid ARM64 stage-1 test level"),
    };
    (va >> shift) & mask
}

fn ttbr0_page_memory(va: u64, leaf: u64) -> SyntheticGuestPhysicalMemoryReader {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut memory, 0x1000, l0_index(va), table_desc(0x2000));
    map_table(&mut memory, 0x2000, l1_index(va), table_desc(0x3000));
    map_table(&mut memory, 0x3000, l2_index(va), table_desc(0x4000));
    map_table(&mut memory, 0x4000, l3_index(va), leaf);
    memory
}

fn ttbr1_page_memory(va: u64, leaf: u64) -> SyntheticGuestPhysicalMemoryReader {
    let input = ttbr1_input(va);
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut memory, 0x8000, l0_index(input), table_desc(0x9000));
    map_table(&mut memory, 0x9000, l1_index(input), table_desc(0xa000));
    map_table(&mut memory, 0xa000, l2_index(input), table_desc(0xb000));
    map_table(&mut memory, 0xb000, l3_index(input), leaf);
    memory
}

fn ttbr0_16k_page_memory(va: u64, leaf: u64) -> SyntheticGuestPhysicalMemoryReader {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut memory,
        0x4000,
        granule_index(va, 1, Arm64Granule::Size16K),
        table_desc(0x8000),
        PAGE_16K,
    );
    map_granule_table(
        &mut memory,
        0x8000,
        granule_index(va, 2, Arm64Granule::Size16K),
        table_desc(0xc000),
        PAGE_16K,
    );
    map_granule_table(
        &mut memory,
        0xc000,
        granule_index(va, 3, Arm64Granule::Size16K),
        leaf,
        PAGE_16K,
    );
    memory
}

fn ttbr1_16k_page_memory(va: u64, leaf: u64) -> SyntheticGuestPhysicalMemoryReader {
    let input = ttbr1_input_bits(va, 47);
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut memory,
        0x1_0000,
        granule_index(input, 1, Arm64Granule::Size16K),
        table_desc(0x1_4000),
        PAGE_16K,
    );
    map_granule_table(
        &mut memory,
        0x1_4000,
        granule_index(input, 2, Arm64Granule::Size16K),
        table_desc(0x1_8000),
        PAGE_16K,
    );
    map_granule_table(
        &mut memory,
        0x1_8000,
        granule_index(input, 3, Arm64Granule::Size16K),
        leaf,
        PAGE_16K,
    );
    memory
}

fn ttbr0_64k_page_memory(va: u64, leaf: u64) -> SyntheticGuestPhysicalMemoryReader {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut memory,
        0x1_0000,
        granule_index(va, 2, Arm64Granule::Size64K),
        table_desc(0x2_0000),
        PAGE_64K,
    );
    map_granule_table(
        &mut memory,
        0x2_0000,
        granule_index(va, 3, Arm64Granule::Size64K),
        leaf,
        PAGE_64K,
    );
    memory
}

fn ttbr1_64k_page_memory(va: u64, leaf: u64) -> SyntheticGuestPhysicalMemoryReader {
    let input = ttbr1_input_bits(va, 42);
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut memory,
        0x4_0000,
        granule_index(input, 2, Arm64Granule::Size64K),
        table_desc(0x5_0000),
        PAGE_64K,
    );
    map_granule_table(
        &mut memory,
        0x5_0000,
        granule_index(input, 3, Arm64Granule::Size64K),
        leaf,
        PAGE_64K,
    );
    memory
}

#[test]
fn arm64_stage1_translates_ttbr0_lower_4k_pages() {
    let va = 0x0000_0000_0040_1234;
    let memory = ttbr0_page_memory(va, page_desc(0x9000));

    let result = translate(memory, context(), va).expect("TTBR0 4K translation");

    assert_eq!(result.gpa, GuestPhysical(0x9234));
    assert_eq!(result.page_size, PAGE_4K);
    assert!(result.readable);
    assert!(result.writable);
    assert!(result.user);
    assert!(result.executable);
}

#[test]
fn arm64_stage1_translates_ttbr1_upper_4k_pages() {
    let va = 0xffff_0000_0000_3456;
    let memory = ttbr1_page_memory(va, page_desc(0xc000));

    let result = translate(memory, context(), va).expect("TTBR1 4K translation");

    assert_eq!(result.gpa, GuestPhysical(0xc456));
    assert_eq!(result.page_size, PAGE_4K);
}

#[test]
fn arm64_stage1_translates_ttbr0_lower_16k_pages() {
    let va = 0x0000_0000_0041_2345;
    let leaf = page_desc(0x2_0000);
    let memory = ttbr0_16k_page_memory(va, leaf);

    let result = translate(memory, context_16k(), va).expect("TTBR0 16K translation");

    assert_eq!(result.gpa, GuestPhysical(0x2_0000 | (va & (PAGE_16K - 1))));
    assert_eq!(result.page_size, PAGE_16K);
    assert!(result.readable);
    assert!(result.writable);
    assert!(result.user);
    assert!(result.executable);
}

#[test]
fn arm64_stage1_translates_ttbr1_upper_16k_pages() {
    let va = 0xffff_8000_0001_2345;
    let leaf = page_desc(0x2_4000);
    let memory = ttbr1_16k_page_memory(va, leaf);

    let result = translate(memory, context_16k(), va).expect("TTBR1 16K translation");
    let input = ttbr1_input_bits(va, 47);

    assert_eq!(
        result.gpa,
        GuestPhysical(0x2_4000 | (input & (PAGE_16K - 1)))
    );
    assert_eq!(result.page_size, PAGE_16K);
}

#[test]
fn arm64_stage1_translates_ttbr0_lower_64k_pages() {
    let va = 0x0000_0000_0012_3456;
    let leaf = page_desc(0x8_0000);
    let memory = ttbr0_64k_page_memory(va, leaf);

    let result = translate(memory, context_64k(), va).expect("TTBR0 64K translation");

    assert_eq!(result.gpa, GuestPhysical(0x8_0000 | (va & (PAGE_64K - 1))));
    assert_eq!(result.page_size, PAGE_64K);
}

#[test]
fn arm64_stage1_translates_ttbr1_upper_64k_pages() {
    let va = 0xffff_fc00_0001_3456;
    let leaf = page_desc(0x9_0000);
    let memory = ttbr1_64k_page_memory(va, leaf);

    let result = translate(memory, context_64k(), va).expect("TTBR1 64K translation");
    let input = ttbr1_input_bits(va, 42);

    assert_eq!(
        result.gpa,
        GuestPhysical(0x9_0000 | (input & (PAGE_64K - 1)))
    );
    assert_eq!(result.page_size, PAGE_64K);
}

#[test]
fn arm64_stage1_translates_1g_block_descriptors() {
    let va = 0x0000_0000_4abc_def0;
    let block_base = 0x0000_0001_8000_0000;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut memory, 0x1000, l0_index(va), table_desc(0x2000));
    map_table(&mut memory, 0x2000, l1_index(va), block_desc(block_base));

    let result = translate(memory, context(), va).expect("1G block translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(block_base | (va & (BLOCK_1G - 1)))
    );
    assert_eq!(result.page_size, BLOCK_1G);
}

#[test]
fn arm64_stage1_translates_2m_block_descriptors() {
    let va = 0x0000_0000_1234_5678;
    let block_base = 0x0000_0000_2400_0000;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut memory, 0x1000, l0_index(va), table_desc(0x2000));
    map_table(&mut memory, 0x2000, l1_index(va), table_desc(0x3000));
    map_table(&mut memory, 0x3000, l2_index(va), block_desc(block_base));

    let result = translate(memory, context(), va).expect("2M block translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(block_base | (va & (BLOCK_2M - 1)))
    );
    assert_eq!(result.page_size, BLOCK_2M);
}

#[test]
fn arm64_stage1_translates_16k_block_descriptors() {
    let l1_va = 0x0000_0012_3456_789a;
    let l1_base = 0x0000_0100_0000_0000;
    let mut l1_memory = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut l1_memory,
        0x4000,
        granule_index(l1_va, 1, Arm64Granule::Size16K),
        block_desc(l1_base),
        PAGE_16K,
    );

    let l1_result = translate(l1_memory, context_16k(), l1_va).expect("16K L1 block translation");

    assert_eq!(
        l1_result.gpa,
        GuestPhysical(l1_base | (l1_va & (BLOCK_64G - 1)))
    );
    assert_eq!(l1_result.page_size, BLOCK_64G);

    let l2_va = 0x0000_0000_1234_5678;
    let l2_base = 0x0000_0000_0800_0000;
    let mut l2_memory = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut l2_memory,
        0x4000,
        granule_index(l2_va, 1, Arm64Granule::Size16K),
        table_desc(0x8000),
        PAGE_16K,
    );
    map_granule_table(
        &mut l2_memory,
        0x8000,
        granule_index(l2_va, 2, Arm64Granule::Size16K),
        block_desc(l2_base),
        PAGE_16K,
    );

    let l2_result = translate(l2_memory, context_16k(), l2_va).expect("16K L2 block translation");

    assert_eq!(
        l2_result.gpa,
        GuestPhysical(l2_base | (l2_va & (BLOCK_32M - 1)))
    );
    assert_eq!(l2_result.page_size, BLOCK_32M);
}

#[test]
fn arm64_stage1_translates_64k_block_descriptors() {
    let va = 0x0000_0000_1234_5678;
    let block_base = 0x0000_0000_2000_0000;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut memory,
        0x1_0000,
        granule_index(va, 2, Arm64Granule::Size64K),
        block_desc(block_base),
        PAGE_64K,
    );

    let result = translate(memory, context_64k(), va).expect("64K L2 block translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(block_base | (va & (BLOCK_512M - 1)))
    );
    assert_eq!(result.page_size, BLOCK_512M);
}

#[test]
fn arm64_stage1_rejects_va_outside_configured_tcr_ranges() {
    let ctx = Arm64Stage1Context::four_k(Some(0x1000), Some(0x8000), 32, 32);
    let memory = SyntheticGuestPhysicalMemoryReader::new();

    let err =
        translate(memory, ctx, 0x0000_0001_0000_0000).expect_err("VA in TTBR gap must be rejected");

    assert_eq!(err.kind(), VmiErrorKind::InvalidAddress);
    assert!(matches!(err, TranslationError::InvalidAddress { .. }));
}

#[test]
fn arm64_stage1_rejects_va_outside_16k_and_64k_tcr_ranges() {
    let cases = [
        Arm64Stage1Context::sixteen_k(Some(0x4000), Some(0x1_0000), 40, 40),
        Arm64Stage1Context::sixty_four_k(Some(0x1_0000), Some(0x4_0000), 40, 40),
    ];

    for ctx in cases {
        let err = translate(SyntheticGuestPhysicalMemoryReader::new(), ctx, 0x0100_0000)
            .expect_err("VA outside the configured TTBR ranges must fail");

        assert_eq!(err.kind(), VmiErrorKind::InvalidAddress);
        assert!(matches!(err, TranslationError::InvalidAddress { .. }));
    }
}

#[test]
fn arm64_stage1_requires_explicit_context_and_selected_ttbr() {
    let translator = Arm64Stage1Translator::new(SyntheticGuestPhysicalMemoryReader::new(), None);
    let err = translator
        .translate(VmId(1), &regs(), GuestVirtual(0))
        .expect_err("missing ARM64 context must fail");
    assert_eq!(err.kind(), VmiErrorKind::InvalidInput);
    assert!(err.to_string().contains("arm64_stage1_context"));

    let ctx = Arm64Stage1Context::four_k(None, Some(0x8000), 16, 16);
    let err = translate(SyntheticGuestPhysicalMemoryReader::new(), ctx, 0)
        .expect_err("selected TTBR0 must be present");
    assert_eq!(err.kind(), VmiErrorKind::InvalidInput);
    assert!(err.to_string().contains("ttbr0"));
}

#[test]
fn arm64_stage1_rejects_tcr_sizes_not_supported_by_selected_granule() {
    let cases = [
        Arm64Stage1Context {
            ttbr0: Some(0x4000),
            ttbr1: Some(0x1_0000),
            tcr: Arm64Tcr {
                t0sz: 16,
                t1sz: 17,
                granule: Arm64Granule::Size16K,
            },
        },
        Arm64Stage1Context {
            ttbr0: Some(0x1_0000),
            ttbr1: Some(0x4_0000),
            tcr: Arm64Tcr {
                t0sz: 21,
                t1sz: 22,
                granule: Arm64Granule::Size64K,
            },
        },
    ];

    for ctx in cases {
        let err = translate(SyntheticGuestPhysicalMemoryReader::new(), ctx, 0)
            .expect_err("unsupported TCR size must fail before table reads");

        assert_eq!(err.kind(), VmiErrorKind::UnsupportedBackend);
    }
}

#[test]
fn arm64_stage1_rejects_invalid_descriptors_as_not_present() {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut memory, 0x1000, 0, 0);

    let err = translate(memory, context(), 0).expect_err("invalid descriptor must fail");

    assert_eq!(err.kind(), VmiErrorKind::TranslationFailure);
    assert!(matches!(
        &err,
        TranslationError::NotPresent { level: "l0", .. }
    ));
    let rendered = err.to_string();
    assert!(rendered.contains("translation entry is not present"));
    assert!(rendered.contains("l0"));
    assert!(!rendered.contains("x86_64"));
}

#[test]
fn arm64_stage1_rejects_invalid_descriptors_for_16k_and_64k_with_neutral_text() {
    let cases = [
        (context_16k(), 0x4000, PAGE_16K, "l1"),
        (context_64k(), 0x1_0000, PAGE_64K, "l2"),
    ];

    for (ctx, root, table_size, level) in cases {
        let mut memory = SyntheticGuestPhysicalMemoryReader::new();
        map_granule_table(&mut memory, root, 0, 0, table_size);

        let err = translate(memory, ctx, 0).expect_err("invalid descriptor must fail");

        assert_eq!(err.kind(), VmiErrorKind::TranslationFailure);
        assert!(matches!(&err, TranslationError::NotPresent { .. }));
        let rendered = err.to_string();
        assert!(rendered.contains("translation entry is not present"));
        assert!(rendered.contains(level), "{rendered}");
        assert!(!rendered.contains("x86_64"));
    }
}

#[test]
fn arm64_stage1_rejects_block_descriptors_at_invalid_levels() {
    let mut level0 = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut level0, 0x1000, 0, block_desc(0));

    let err = translate(level0, context(), 0).expect_err("level 0 block must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "l0", .. }
    ));

    let mut level3 = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut level3, 0x1000, 0, table_desc(0x2000));
    map_table(&mut level3, 0x2000, 0, table_desc(0x3000));
    map_table(&mut level3, 0x3000, 0, table_desc(0x4000));
    map_table(&mut level3, 0x4000, 0, block_desc(0x9000));

    let err = translate(level3, context(), 0).expect_err("level 3 block must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "l3", .. }
    ));
}

#[test]
fn arm64_stage1_rejects_page_descriptors_at_invalid_levels() {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut memory, 0x1000, 0, table_desc(0x2000));
    map_table(&mut memory, 0x2000, 0, 0x3000 | AF | TABLE);

    let err =
        translate(memory, context(), 0).expect_err("page-shaped level 1 descriptor must fail");

    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "l1", .. }
    ));
}

#[test]
fn arm64_stage1_rejects_invalid_block_and_page_levels_for_large_granules() {
    let mut final_block_16k = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut final_block_16k,
        0x4000,
        0,
        table_desc(0x8000),
        PAGE_16K,
    );
    map_granule_table(
        &mut final_block_16k,
        0x8000,
        0,
        table_desc(0xc000),
        PAGE_16K,
    );
    map_granule_table(
        &mut final_block_16k,
        0xc000,
        0,
        block_desc(0x2_0000),
        PAGE_16K,
    );

    let err =
        translate(final_block_16k, context_16k(), 0).expect_err("16K final-level block must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "l3", .. }
    ));

    let mut page_at_l2_64k = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut page_at_l2_64k,
        0x1_0000,
        0,
        page_desc(0x8_0000),
        PAGE_64K,
    );

    let err = translate(page_at_l2_64k, context_64k(), 0)
        .expect_err("64K page-shaped level 2 descriptor must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "l2", .. }
    ));
}

#[test]
fn arm64_stage1_rejects_reserved_block_output_address_bits() {
    let mut one_g = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut one_g, 0x1000, 0, table_desc(0x2000));
    map_table(
        &mut one_g,
        0x2000,
        0,
        block_desc(0x8000_0000 | RESERVED_BLOCK_BIT),
    );

    let err = translate(one_g, context(), 0).expect_err("1G reserved address bit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "l1", .. }
    ));

    let mut two_m = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut two_m, 0x1000, 0, table_desc(0x2000));
    map_table(&mut two_m, 0x2000, 0, table_desc(0x3000));
    map_table(
        &mut two_m,
        0x3000,
        0,
        block_desc(0x2000_0000 | RESERVED_BLOCK_BIT),
    );

    let err = translate(two_m, context(), 0).expect_err("2M reserved address bit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "l2", .. }
    ));
}

#[test]
fn arm64_stage1_rejects_reserved_output_bits_for_16k_and_64k_descriptors() {
    let va_16k = 0x0000_0000_0041_2345;
    let memory = ttbr0_16k_page_memory(va_16k, page_desc(0x2_0000 | RESERVED_16K_OUTPUT_BIT));
    let err =
        translate(memory, context_16k(), va_16k).expect_err("16K page reserved bit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "l3", .. }
    ));

    let va_64k = 0x0000_0000_0012_3456;
    let memory = ttbr0_64k_page_memory(va_64k, page_desc(0x8_0000 | RESERVED_16K_OUTPUT_BIT));
    let err =
        translate(memory, context_64k(), va_64k).expect_err("64K page reserved bit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "l3", .. }
    ));

    let mut block_16k = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut block_16k,
        0x4000,
        0,
        block_desc(0x0000_0100_0000_0000 | RESERVED_BLOCK_BIT),
        PAGE_16K,
    );
    let err = translate(block_16k, context_16k(), 0)
        .expect_err("16K block reserved output bit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "l1", .. }
    ));

    let mut block_64k = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut block_64k,
        0x1_0000,
        0,
        block_desc(0x2000_0000 | RESERVED_BLOCK_BIT),
        PAGE_64K,
    );
    let err = translate(block_64k, context_64k(), 0)
        .expect_err("64K block reserved output bit must fail");
    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "l2", .. }
    ));
}

#[test]
fn arm64_stage1_rejects_reserved_high_output_address_bits() {
    let va = 0x0000_0000_0040_1234;
    let memory = ttbr0_page_memory(va, RESERVED_OUTPUT_HIGH_BIT | page_desc(0x9000));

    let err = translate(memory, context(), va).expect_err("reserved high output bit must fail");

    assert_eq!(err.kind(), VmiErrorKind::Malformed);
    assert!(matches!(
        err,
        TranslationError::MalformedPageTables { level: "l3", .. }
    ));
}

#[test]
fn arm64_stage1_requires_exact_descriptor_reads() {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new().with_partial_reads(true);
    memory
        .map_range(
            GuestPhysical(0x1000),
            descriptor_prefix(table_desc(0x2000), 4),
        )
        .expect("map partial ARM64 descriptor");

    let err = translate(memory, context(), 0).expect_err("partial descriptor must fail");

    assert_eq!(err.kind(), VmiErrorKind::MissingMemory);
    assert!(matches!(&err, TranslationError::MissingMemory { .. }));
    let rendered = err.to_string();
    assert!(rendered.contains("l0"), "{rendered}");
    assert!(rendered.contains("read 4 of 8 bytes"), "{rendered}");
}

#[test]
fn arm64_stage1_requires_exact_descriptor_reads_for_16k_and_64k() {
    let cases = [
        (context_16k(), 0x4000, "l1"),
        (context_64k(), 0x1_0000, "l2"),
    ];

    for (ctx, root, level) in cases {
        let mut memory = SyntheticGuestPhysicalMemoryReader::new().with_partial_reads(true);
        memory
            .map_range(
                GuestPhysical(root),
                descriptor_prefix(table_desc(root + PAGE_16K), 4),
            )
            .expect("map partial ARM64 descriptor");

        let err = translate(memory, ctx, 0).expect_err("partial descriptor must fail");

        assert_eq!(err.kind(), VmiErrorKind::MissingMemory);
        assert!(matches!(&err, TranslationError::MissingMemory { .. }));
        let rendered = err.to_string();
        assert!(rendered.contains(level), "{rendered}");
        assert!(rendered.contains("read 4 of 8 bytes"), "{rendered}");
    }
}

#[test]
fn arm64_stage1_reports_unmapped_translation_table_memory() {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(&mut memory, 0x1000, 0, table_desc(0x2000));

    let err = translate(memory, context(), 0).expect_err("missing level 1 table must fail");

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
fn arm64_stage1_maps_denied_translation_table_memory_to_permission_denied() {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    memory
        .deny_range(GuestPhysical(0x1000), 8, "root table is MMIO-like")
        .expect("deny ARM64 translation table memory");

    let err = translate(memory, context(), 0).expect_err("denied table memory must fail");

    assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);
    assert!(matches!(
        &err,
        TranslationError::PermissionDenied {
            operation: "read_physical",
            ..
        }
    ));
    assert!(err.to_string().contains("l0"));
}

#[test]
fn arm64_stage1_maps_unavailable_translation_table_memory_to_temporarily_unavailable() {
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    memory
        .mark_unavailable_range(GuestPhysical(0x1000), 8, "snapshot root table is missing")
        .expect("mark ARM64 translation table memory unavailable");

    let err = translate(memory, context(), 0).expect_err("unavailable table memory must fail");

    assert_eq!(err.kind(), VmiErrorKind::TemporarilyUnavailable);
    assert!(matches!(
        &err,
        TranslationError::TemporarilyUnavailable {
            resource: "synthetic-memory-range",
            ..
        }
    ));
    assert!(err.to_string().contains("l0"));
}

#[test]
fn arm64_stage1_maps_large_granule_table_memory_errors() {
    let cases = [
        (context_16k(), 0x4000, "l1"),
        (context_64k(), 0x1_0000, "l2"),
    ];

    for (ctx, root, level) in cases {
        let err = translate(SyntheticGuestPhysicalMemoryReader::new(), ctx, 0)
            .expect_err("unmapped root table must fail");
        assert_eq!(err.kind(), VmiErrorKind::MissingMemory);
        assert!(matches!(&err, TranslationError::MissingMemory { .. }));
        assert!(err.to_string().contains(level));

        let mut denied = SyntheticGuestPhysicalMemoryReader::new();
        denied
            .deny_range(GuestPhysical(root), 8, "root table is MMIO-like")
            .expect("deny ARM64 root table");
        let err = translate(denied, ctx, 0).expect_err("denied root table must fail");
        assert_eq!(err.kind(), VmiErrorKind::PermissionDenied);
        assert!(matches!(
            &err,
            TranslationError::PermissionDenied {
                operation: "read_physical",
                ..
            }
        ));
        assert!(err.to_string().contains(level));

        let mut unavailable = SyntheticGuestPhysicalMemoryReader::new();
        unavailable
            .mark_unavailable_range(GuestPhysical(root), 8, "snapshot root table is missing")
            .expect("mark ARM64 root table unavailable");
        let err = translate(unavailable, ctx, 0).expect_err("unavailable root table must fail");
        assert_eq!(err.kind(), VmiErrorKind::TemporarilyUnavailable);
        assert!(matches!(
            &err,
            TranslationError::TemporarilyUnavailable {
                resource: "synthetic-memory-range",
                ..
            }
        ));
        assert!(err.to_string().contains(level));
    }
}

#[test]
fn arm64_stage1_propagates_table_and_leaf_permissions() {
    let va = 0x0000_0000_0040_1234;
    let mut memory = SyntheticGuestPhysicalMemoryReader::new();
    map_table(
        &mut memory,
        0x1000,
        l0_index(va),
        table_desc(0x2000) | AP_TABLE_READ_ONLY | AP_TABLE_NO_USER | UXN_TABLE | PXN_TABLE,
    );
    map_table(&mut memory, 0x2000, l1_index(va), table_desc(0x3000));
    map_table(&mut memory, 0x3000, l2_index(va), table_desc(0x4000));
    map_table(
        &mut memory,
        0x4000,
        l3_index(va),
        page_desc(0x9000) | AP_READ_ONLY | UXN | PXN,
    );

    let result = translate(memory, context(), va).expect("permission-limited ARM64 translation");

    assert_eq!(result.gpa, GuestPhysical(0x9234));
    assert!(!result.writable);
    assert!(!result.user);
    assert!(!result.executable);
}

#[test]
fn arm64_stage1_propagates_permissions_for_16k_and_64k() {
    let va_16k = 0x0000_0000_0041_2345;
    let mut memory_16k = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut memory_16k,
        0x4000,
        granule_index(va_16k, 1, Arm64Granule::Size16K),
        table_desc(0x8000) | AP_TABLE_READ_ONLY | AP_TABLE_NO_USER | UXN_TABLE | PXN_TABLE,
        PAGE_16K,
    );
    map_granule_table(
        &mut memory_16k,
        0x8000,
        granule_index(va_16k, 2, Arm64Granule::Size16K),
        table_desc(0xc000),
        PAGE_16K,
    );
    map_granule_table(
        &mut memory_16k,
        0xc000,
        granule_index(va_16k, 3, Arm64Granule::Size16K),
        page_desc(0x2_0000) | AP_READ_ONLY | UXN | PXN,
        PAGE_16K,
    );

    let result = translate(memory_16k, context_16k(), va_16k).expect("16K permission translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(0x2_0000 | (va_16k & (PAGE_16K - 1)))
    );
    assert!(!result.writable);
    assert!(!result.user);
    assert!(!result.executable);

    let va_64k = 0x0000_0000_0012_3456;
    let mut memory_64k = SyntheticGuestPhysicalMemoryReader::new();
    map_granule_table(
        &mut memory_64k,
        0x1_0000,
        granule_index(va_64k, 2, Arm64Granule::Size64K),
        table_desc(0x2_0000) | AP_TABLE_READ_ONLY | AP_TABLE_NO_USER | UXN_TABLE | PXN_TABLE,
        PAGE_64K,
    );
    map_granule_table(
        &mut memory_64k,
        0x2_0000,
        granule_index(va_64k, 3, Arm64Granule::Size64K),
        page_desc(0x8_0000) | AP_READ_ONLY | UXN | PXN,
        PAGE_64K,
    );

    let result = translate(memory_64k, context_64k(), va_64k).expect("64K permission translation");

    assert_eq!(
        result.gpa,
        GuestPhysical(0x8_0000 | (va_64k & (PAGE_64K - 1)))
    );
    assert!(!result.writable);
    assert!(!result.user);
    assert!(!result.executable);
}
