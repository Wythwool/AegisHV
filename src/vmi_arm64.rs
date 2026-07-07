use crate::vmi::{
    AddressTranslator, GuestMemoryReader, GuestPhysical, GuestRegisters, GuestVirtual,
    MemoryReadError, TranslationError, TranslationResult, VmId,
};

const DESCRIPTOR_SIZE: u64 = 8;
const PAGE_4K: u64 = 4096;
const PAGE_16K: u64 = 16 * 1024;
const PAGE_64K: u64 = 64 * 1024;
const DESC_TYPE_MASK: u64 = 0b11;
const DESC_INVALID_MASK: u64 = 0b1;
const DESC_BLOCK: u64 = 0b01;
const DESC_TABLE_OR_PAGE: u64 = 0b11;
const OUTPUT_ADDR_48_MASK: u64 = 0x0000_ffff_ffff_ffff;
const OUTPUT_ADDR_HIGH_RESERVED: u64 = 0x000f_0000_0000_0000;
const AP_USER: u64 = 1 << 6;
const AP_READ_ONLY: u64 = 1 << 7;
const PXN: u64 = 1 << 53;
const UXN: u64 = 1 << 54;
const PXN_TABLE: u64 = 1 << 59;
const UXN_TABLE: u64 = 1 << 60;
const AP_TABLE_NO_USER: u64 = 1 << 61;
const AP_TABLE_READ_ONLY: u64 = 1 << 62;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm64Granule {
    Size4K,
    Size16K,
    Size64K,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arm64Tcr {
    pub t0sz: u8,
    pub t1sz: u8,
    pub granule: Arm64Granule,
}

impl Arm64Tcr {
    pub fn four_k(t0sz: u8, t1sz: u8) -> Self {
        Self {
            t0sz,
            t1sz,
            granule: Arm64Granule::Size4K,
        }
    }

    pub fn sixteen_k(t0sz: u8, t1sz: u8) -> Self {
        Self {
            t0sz,
            t1sz,
            granule: Arm64Granule::Size16K,
        }
    }

    pub fn sixty_four_k(t0sz: u8, t1sz: u8) -> Self {
        Self {
            t0sz,
            t1sz,
            granule: Arm64Granule::Size64K,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arm64Stage1Context {
    pub ttbr0: Option<u64>,
    pub ttbr1: Option<u64>,
    pub tcr: Arm64Tcr,
}

impl Arm64Stage1Context {
    pub fn four_k(ttbr0: Option<u64>, ttbr1: Option<u64>, t0sz: u8, t1sz: u8) -> Self {
        Self {
            ttbr0,
            ttbr1,
            tcr: Arm64Tcr::four_k(t0sz, t1sz),
        }
    }

    pub fn sixteen_k(ttbr0: Option<u64>, ttbr1: Option<u64>, t0sz: u8, t1sz: u8) -> Self {
        Self {
            ttbr0,
            ttbr1,
            tcr: Arm64Tcr::sixteen_k(t0sz, t1sz),
        }
    }

    pub fn sixty_four_k(ttbr0: Option<u64>, ttbr1: Option<u64>, t0sz: u8, t1sz: u8) -> Self {
        Self {
            ttbr0,
            ttbr1,
            tcr: Arm64Tcr::sixty_four_k(t0sz, t1sz),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Arm64Stage1Translator<M> {
    memory: M,
    context: Option<Arm64Stage1Context>,
}

impl<M> Arm64Stage1Translator<M> {
    pub fn new(memory: M, context: Option<Arm64Stage1Context>) -> Self {
        Self { memory, context }
    }

    pub fn memory(&self) -> &M {
        &self.memory
    }

    pub fn context(&self) -> Option<Arm64Stage1Context> {
        self.context
    }

    pub fn into_memory(self) -> M {
        self.memory
    }
}

impl<M> AddressTranslator for Arm64Stage1Translator<M>
where
    M: GuestMemoryReader,
{
    fn translate(
        &self,
        vm: VmId,
        _regs: &GuestRegisters,
        gva: GuestVirtual,
    ) -> Result<TranslationResult, TranslationError> {
        let context = self
            .context
            .as_ref()
            .ok_or(TranslationError::MissingContext {
                field: "arm64_stage1_context",
            })?;
        translate_arm64_stage1(&self.memory, vm, context, gva)
    }
}

pub fn translate_arm64_stage1(
    memory: &dyn GuestMemoryReader,
    vm: VmId,
    context: &Arm64Stage1Context,
    va: GuestVirtual,
) -> Result<TranslationResult, TranslationError> {
    let config = Arm64GranuleConfig::for_granule(context.tcr.granule);
    let selected = select_ttbr(context, va, config)?;
    walk_stage1(memory, vm, selected, va, config)
}

fn walk_stage1(
    memory: &dyn GuestMemoryReader,
    vm: VmId,
    selected: SelectedTtbr,
    va: GuestVirtual,
    config: Arm64GranuleConfig,
) -> Result<TranslationResult, TranslationError> {
    let mut table_base = selected.ttbr & config.aligned_output_mask(config.page_size);
    let mut perms = Arm64Permissions::default();

    for level in selected.start_level..=3 {
        let descriptor = read_descriptor(
            memory,
            vm,
            table_base,
            table_index(selected.input_va, level, config),
            level_name(level),
        )?;
        validate_present(level_name(level), descriptor, va)?;

        match descriptor & DESC_TYPE_MASK {
            DESC_BLOCK => {
                return translate_block(level, descriptor, selected.input_va, va, perms, config);
            }
            DESC_TABLE_OR_PAGE if level == 3 => {
                return translate_page(descriptor, selected.input_va, perms, config);
            }
            DESC_TABLE_OR_PAGE => {
                validate_table_descriptor(level_name(level), descriptor, config)?;
                perms.apply_table(descriptor);
                table_base = descriptor & config.aligned_output_mask(config.page_size);
            }
            _ => {
                return Err(malformed(
                    level_name(level),
                    "reserved descriptor encoding is not valid for stage-1 translation",
                ));
            }
        }
    }

    Err(TranslationError::TranslationFailed {
        gva: va,
        detail: "ARM64 stage-1 walk ended without a leaf descriptor".to_string(),
    })
}

#[derive(Debug, Clone, Copy)]
struct SelectedTtbr {
    ttbr: u64,
    input_va: u64,
    start_level: u8,
}

#[derive(Debug, Clone, Copy)]
struct Arm64GranuleConfig {
    page_size: u64,
    offset_bits: u8,
    index_bits: u8,
    min_start_level: u8,
    max_va_bits: u8,
}

impl Arm64GranuleConfig {
    fn for_granule(granule: Arm64Granule) -> Self {
        match granule {
            Arm64Granule::Size4K => Self {
                page_size: PAGE_4K,
                offset_bits: 12,
                index_bits: 9,
                min_start_level: 0,
                max_va_bits: 48,
            },
            Arm64Granule::Size16K => Self {
                page_size: PAGE_16K,
                offset_bits: 14,
                index_bits: 11,
                min_start_level: 1,
                max_va_bits: 47,
            },
            Arm64Granule::Size64K => Self {
                page_size: PAGE_64K,
                offset_bits: 16,
                index_bits: 13,
                min_start_level: 2,
                max_va_bits: 42,
            },
        }
    }

    fn start_level(self, va_bits: u8) -> Result<u8, TranslationError> {
        for level in self.min_start_level..=3 {
            if va_bits <= self.level_coverage_bits(level) {
                return Ok(level);
            }
        }

        Err(TranslationError::Unsupported {
            backend: "arm64-stage1-offline",
            operation: "translate_unsupported_tcr_size",
        })
    }

    fn level_shift(self, level: u8) -> u8 {
        self.offset_bits + self.index_bits * (3 - level)
    }

    fn level_coverage_bits(self, level: u8) -> u8 {
        self.level_shift(level) + self.index_bits
    }

    fn block_size(self, level: u8) -> Option<u64> {
        match (self.page_size, level) {
            (PAGE_4K, 1) => Some(1u64 << self.level_shift(level)),
            (PAGE_4K, 2) => Some(1u64 << self.level_shift(level)),
            (PAGE_16K, 1) => Some(1u64 << self.level_shift(level)),
            (PAGE_16K, 2) => Some(1u64 << self.level_shift(level)),
            (PAGE_64K, 2) => Some(1u64 << self.level_shift(level)),
            _ => None,
        }
    }

    fn index_mask(self) -> u64 {
        (1u64 << self.index_bits) - 1
    }

    fn aligned_output_mask(self, alignment: u64) -> u64 {
        OUTPUT_ADDR_48_MASK & !(alignment - 1)
    }

    fn low_output_reserved_mask(self, alignment: u64) -> u64 {
        (alignment - 1) & !0xfff
    }

    fn table_low_reserved_mask(self) -> u64 {
        (self.page_size - 1) & !DESC_TYPE_MASK
    }
}

fn select_ttbr(
    context: &Arm64Stage1Context,
    va: GuestVirtual,
    config: Arm64GranuleConfig,
) -> Result<SelectedTtbr, TranslationError> {
    let t0_bits = va_bits_from_tsz(context.tcr.t0sz, config)?;
    let t1_bits = va_bits_from_tsz(context.tcr.t1sz, config)?;

    if va.0 < range_size(t0_bits) {
        let ttbr = context
            .ttbr0
            .ok_or(TranslationError::MissingContext { field: "ttbr0" })?;
        return Ok(SelectedTtbr {
            ttbr,
            input_va: va.0,
            start_level: config.start_level(t0_bits)?,
        });
    }

    let ttbr1_base = u64::MAX - (range_size(t1_bits) - 1);
    if va.0 >= ttbr1_base {
        let ttbr = context
            .ttbr1
            .ok_or(TranslationError::MissingContext { field: "ttbr1" })?;
        return Ok(SelectedTtbr {
            ttbr,
            input_va: va.0 & input_mask(t1_bits),
            start_level: config.start_level(t1_bits)?,
        });
    }

    Err(TranslationError::InvalidAddress { gva: va })
}

fn va_bits_from_tsz(tsz: u8, config: Arm64GranuleConfig) -> Result<u8, TranslationError> {
    let Some(va_bits) = 64u8.checked_sub(tsz) else {
        return Err(TranslationError::Unsupported {
            backend: "arm64-stage1-offline",
            operation: "translate_unsupported_tcr_size",
        });
    };

    if va_bits < config.offset_bits || va_bits > config.max_va_bits {
        return Err(TranslationError::Unsupported {
            backend: "arm64-stage1-offline",
            operation: "translate_unsupported_tcr_size",
        });
    }

    Ok(va_bits)
}

fn range_size(va_bits: u8) -> u64 {
    1u64 << u32::from(va_bits)
}

fn input_mask(va_bits: u8) -> u64 {
    range_size(va_bits) - 1
}

fn read_descriptor(
    memory: &dyn GuestMemoryReader,
    vm: VmId,
    table_base: u64,
    index: u64,
    level: &'static str,
) -> Result<u64, TranslationError> {
    let gpa = table_base
        .checked_add(index * DESCRIPTOR_SIZE)
        .ok_or_else(|| malformed(level, "translation-table descriptor address overflowed"))?;
    let mut bytes = [0u8; 8];
    let read = memory
        .read_physical(vm, GuestPhysical(gpa), &mut bytes)
        .map_err(|err| table_memory_error(GuestPhysical(gpa), level, err))?;
    if read != bytes.len() {
        return Err(TranslationError::MissingMemory {
            gpa: GuestPhysical(gpa),
            detail: format!(
                "short read while reading {level}: read {read} of {} bytes",
                bytes.len()
            ),
        });
    }
    Ok(u64::from_le_bytes(bytes))
}

fn table_memory_error(
    gpa: GuestPhysical,
    level: &'static str,
    err: MemoryReadError,
) -> TranslationError {
    match err {
        MemoryReadError::Unsupported { backend, operation } => {
            TranslationError::Unsupported { backend, operation }
        }
        MemoryReadError::Degraded { reason } => TranslationError::Degraded {
            reason: format!("cannot read {level}: {reason}"),
        },
        MemoryReadError::InvalidAddress { .. }
        | MemoryReadError::InvalidRange { .. }
        | MemoryReadError::MissingMemory { .. }
        | MemoryReadError::Unmapped { .. } => TranslationError::MissingMemory {
            gpa,
            detail: format!("cannot read {level}: {err}"),
        },
        MemoryReadError::Malformed { detail } => TranslationError::MalformedPageTables {
            level,
            detail: format!("memory source is malformed: {detail}"),
        },
        MemoryReadError::InconsistentSnapshot { detail } => {
            TranslationError::InconsistentSnapshot {
                detail: format!("cannot read {level}: {detail}"),
            }
        }
        MemoryReadError::PermissionDenied { operation, detail } => {
            TranslationError::PermissionDenied {
                operation,
                detail: format!("cannot read {level}: {detail}"),
            }
        }
        MemoryReadError::TemporarilyUnavailable { resource, detail } => {
            TranslationError::TemporarilyUnavailable {
                resource,
                detail: format!("cannot read {level}: {detail}"),
            }
        }
        MemoryReadError::Backend { detail } => TranslationError::Backend {
            detail: format!("cannot read {level}: {detail}"),
        },
    }
}

fn validate_present(
    level: &'static str,
    descriptor: u64,
    va: GuestVirtual,
) -> Result<(), TranslationError> {
    if descriptor & DESC_INVALID_MASK == 0 {
        return Err(TranslationError::NotPresent { level, gva: va });
    }
    Ok(())
}

fn validate_table_descriptor(
    level: &'static str,
    descriptor: u64,
    config: Arm64GranuleConfig,
) -> Result<(), TranslationError> {
    reject_output_high_bits(level, descriptor)?;
    if descriptor & config.table_low_reserved_mask() != 0 {
        return Err(malformed(
            level,
            "table descriptor has output-address bits set below the granule size",
        ));
    }
    Ok(())
}

fn translate_block(
    level: u8,
    descriptor: u64,
    input_va: u64,
    va: GuestVirtual,
    mut perms: Arm64Permissions,
    config: Arm64GranuleConfig,
) -> Result<TranslationResult, TranslationError> {
    let Some(block_size) = config.block_size(level) else {
        return match level {
            3 => Err(malformed(
                level_name(level),
                "block descriptors are not valid at final level",
            )),
            0..=2 => Err(malformed(
                level_name(level),
                "block descriptors are not valid at this level for the selected granule",
            )),
            _ => Err(TranslationError::TranslationFailed {
                gva: va,
                detail: "ARM64 stage-1 walk reached an unknown descriptor level".to_string(),
            }),
        };
    };

    validate_block_descriptor(
        level_name(level),
        descriptor,
        config.low_output_reserved_mask(block_size),
    )?;
    perms.apply_leaf(descriptor);
    Ok(result(
        GuestPhysical(
            (descriptor & config.aligned_output_mask(block_size)) | (input_va & (block_size - 1)),
        ),
        block_size,
        perms,
    ))
}

fn translate_page(
    descriptor: u64,
    input_va: u64,
    mut perms: Arm64Permissions,
    config: Arm64GranuleConfig,
) -> Result<TranslationResult, TranslationError> {
    validate_page_descriptor("l3", descriptor, config)?;
    perms.apply_leaf(descriptor);
    Ok(result(
        GuestPhysical(
            (descriptor & config.aligned_output_mask(config.page_size))
                | (input_va & (config.page_size - 1)),
        ),
        config.page_size,
        perms,
    ))
}

fn validate_page_descriptor(
    level: &'static str,
    descriptor: u64,
    config: Arm64GranuleConfig,
) -> Result<(), TranslationError> {
    reject_output_high_bits(level, descriptor)?;
    if descriptor & config.low_output_reserved_mask(config.page_size) != 0 {
        return Err(malformed(
            level,
            "reserved page output-address bits are set",
        ));
    }
    Ok(())
}

fn validate_block_descriptor(
    level: &'static str,
    descriptor: u64,
    low_reserved_mask: u64,
) -> Result<(), TranslationError> {
    reject_output_high_bits(level, descriptor)?;
    if descriptor & low_reserved_mask != 0 {
        return Err(malformed(
            level,
            "reserved block output-address bits are set",
        ));
    }
    Ok(())
}

fn reject_output_high_bits(level: &'static str, descriptor: u64) -> Result<(), TranslationError> {
    if descriptor & OUTPUT_ADDR_HIGH_RESERVED != 0 {
        return Err(malformed(level, "reserved output-address bits are set"));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct Arm64Permissions {
    writable: bool,
    user: bool,
    executable: bool,
}

impl Default for Arm64Permissions {
    fn default() -> Self {
        Self {
            writable: true,
            user: true,
            executable: true,
        }
    }
}

impl Arm64Permissions {
    fn apply_table(&mut self, descriptor: u64) {
        self.writable &= descriptor & AP_TABLE_READ_ONLY == 0;
        self.user &= descriptor & AP_TABLE_NO_USER == 0;
        self.executable &= descriptor & (UXN_TABLE | PXN_TABLE) == 0;
    }

    fn apply_leaf(&mut self, descriptor: u64) {
        self.writable &= descriptor & AP_READ_ONLY == 0;
        self.user &= descriptor & AP_USER != 0;
        self.executable &= descriptor & (UXN | PXN) == 0;
    }
}

fn result(gpa: GuestPhysical, page_size: u64, perms: Arm64Permissions) -> TranslationResult {
    TranslationResult {
        gpa,
        readable: true,
        writable: perms.writable,
        executable: perms.executable,
        user: perms.user,
        page_size,
    }
}

fn malformed(level: &'static str, detail: &'static str) -> TranslationError {
    TranslationError::MalformedPageTables {
        level,
        detail: detail.to_string(),
    }
}

fn table_index(input_va: u64, level: u8, config: Arm64GranuleConfig) -> u64 {
    (input_va >> config.level_shift(level)) & config.index_mask()
}

fn level_name(level: u8) -> &'static str {
    match level {
        0 => "l0",
        1 => "l1",
        2 => "l2",
        3 => "l3",
        _ => "unknown",
    }
}
