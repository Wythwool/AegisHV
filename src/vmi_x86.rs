use crate::vmi::{
    AddressTranslator, GuestMemoryReader, GuestPhysical, GuestRegisters, GuestVirtual,
    MemoryReadError, TranslationError, TranslationResult, VmId,
};

const ENTRY_SIZE: u64 = 8;
const PAGE_4K: u64 = 4096;
const PAGE_2M: u64 = 2 * 1024 * 1024;
const PAGE_1G: u64 = 1024 * 1024 * 1024;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const LARGE_PAGE: u64 = 1 << 7;
const NX: u64 = 1 << 63;
const CR3_BASE_MASK: u64 = 0x000f_ffff_ffff_f000;
const TABLE_BASE_MASK: u64 = 0x000f_ffff_ffff_f000;
const PAGE_2M_BASE_MASK: u64 = 0x000f_ffff_ffe0_0000;
const PAGE_1G_BASE_MASK: u64 = 0x000f_ffff_c000_0000;
const RESERVED_HIGH_BITS: u64 = 0x7ff0_0000_0000_0000;
const RESERVED_2M_LOW_BITS: u64 = 0x0000_0000_001f_e000;
const RESERVED_1G_LOW_BITS: u64 = 0x0000_0000_3fff_e000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86PagingMode {
    FourLevel,
    La57,
}

#[derive(Debug, Clone)]
pub struct X86_64PageWalker<M> {
    memory: M,
    paging_mode: X86PagingMode,
}

impl<M> X86_64PageWalker<M> {
    pub fn new(memory: M, paging_mode: X86PagingMode) -> Self {
        Self {
            memory,
            paging_mode,
        }
    }

    pub fn four_level(memory: M) -> Self {
        Self::new(memory, X86PagingMode::FourLevel)
    }

    pub fn la57(memory: M) -> Self {
        Self::new(memory, X86PagingMode::La57)
    }

    pub fn paging_mode(&self) -> X86PagingMode {
        self.paging_mode
    }

    pub fn memory(&self) -> &M {
        &self.memory
    }

    pub fn into_memory(self) -> M {
        self.memory
    }
}

impl<M> AddressTranslator for X86_64PageWalker<M>
where
    M: GuestMemoryReader,
{
    fn translate(
        &self,
        vm: VmId,
        regs: &GuestRegisters,
        gva: GuestVirtual,
    ) -> Result<TranslationResult, TranslationError> {
        translate_x86_64(&self.memory, vm, regs, gva, self.paging_mode)
    }
}

#[derive(Debug, Clone)]
pub struct X86_64FourLevelPageWalker<M> {
    memory: M,
}

impl<M> X86_64FourLevelPageWalker<M> {
    pub fn new(memory: M) -> Self {
        Self { memory }
    }

    pub fn memory(&self) -> &M {
        &self.memory
    }

    pub fn into_memory(self) -> M {
        self.memory
    }
}

impl<M> AddressTranslator for X86_64FourLevelPageWalker<M>
where
    M: GuestMemoryReader,
{
    fn translate(
        &self,
        vm: VmId,
        regs: &GuestRegisters,
        gva: GuestVirtual,
    ) -> Result<TranslationResult, TranslationError> {
        translate_x86_64_4level(&self.memory, vm, regs, gva)
    }
}

pub fn translate_x86_64_4level(
    memory: &dyn GuestMemoryReader,
    vm: VmId,
    regs: &GuestRegisters,
    gva: GuestVirtual,
) -> Result<TranslationResult, TranslationError> {
    translate_x86_64(memory, vm, regs, gva, X86PagingMode::FourLevel)
}

pub fn translate_x86_64_la57(
    memory: &dyn GuestMemoryReader,
    vm: VmId,
    regs: &GuestRegisters,
    gva: GuestVirtual,
) -> Result<TranslationResult, TranslationError> {
    translate_x86_64(memory, vm, regs, gva, X86PagingMode::La57)
}

pub fn translate_x86_64(
    memory: &dyn GuestMemoryReader,
    vm: VmId,
    regs: &GuestRegisters,
    gva: GuestVirtual,
    paging_mode: X86PagingMode,
) -> Result<TranslationResult, TranslationError> {
    if !is_canonical(gva.0, paging_mode) {
        return Err(TranslationError::InvalidAddress { gva });
    }

    let cr3 = regs.cr3_or_ttbr.ok_or(TranslationError::MissingContext {
        field: "cr3_or_ttbr",
    })?;
    let mut table_base = cr3 & CR3_BASE_MASK;
    let mut perms = X86PagePermissions::default();

    if paging_mode == X86PagingMode::La57 {
        let pml5e = read_entry(memory, vm, table_base, pml5_index(gva.0), "pml5e")?;
        validate_present("pml5e", pml5e, gva)?;
        reject_reserved_high_bits("pml5e", pml5e)?;
        if pml5e & LARGE_PAGE != 0 {
            return Err(malformed(
                "pml5e",
                "large-page bit is reserved in PML5 entries",
            ));
        }
        perms.apply(pml5e);
        table_base = pml5e & TABLE_BASE_MASK;
    }

    let pml4e = read_entry(memory, vm, table_base, pml4_index(gva.0), "pml4e")?;
    validate_present("pml4e", pml4e, gva)?;
    reject_reserved_high_bits("pml4e", pml4e)?;
    if pml4e & LARGE_PAGE != 0 {
        return Err(malformed(
            "pml4e",
            "large-page bit is reserved in PML4 entries",
        ));
    }
    perms.apply(pml4e);

    let pdpt_base = pml4e & TABLE_BASE_MASK;
    let pdpte = read_entry(memory, vm, pdpt_base, pdpt_index(gva.0), "pdpte")?;
    validate_present("pdpte", pdpte, gva)?;
    reject_reserved_high_bits("pdpte", pdpte)?;
    perms.apply(pdpte);
    if pdpte & LARGE_PAGE != 0 {
        reject_reserved_large_page_bits("pdpte", pdpte, RESERVED_1G_LOW_BITS)?;
        return Ok(result(
            GuestPhysical((pdpte & PAGE_1G_BASE_MASK) | (gva.0 & (PAGE_1G - 1))),
            PAGE_1G,
            perms,
        ));
    }

    let pd_base = pdpte & TABLE_BASE_MASK;
    let pde = read_entry(memory, vm, pd_base, pd_index(gva.0), "pde")?;
    validate_present("pde", pde, gva)?;
    reject_reserved_high_bits("pde", pde)?;
    perms.apply(pde);
    if pde & LARGE_PAGE != 0 {
        reject_reserved_large_page_bits("pde", pde, RESERVED_2M_LOW_BITS)?;
        return Ok(result(
            GuestPhysical((pde & PAGE_2M_BASE_MASK) | (gva.0 & (PAGE_2M - 1))),
            PAGE_2M,
            perms,
        ));
    }

    let pt_base = pde & TABLE_BASE_MASK;
    let pte = read_entry(memory, vm, pt_base, pt_index(gva.0), "pte")?;
    validate_present("pte", pte, gva)?;
    reject_reserved_high_bits("pte", pte)?;
    perms.apply(pte);

    Ok(result(
        GuestPhysical((pte & TABLE_BASE_MASK) | (gva.0 & (PAGE_4K - 1))),
        PAGE_4K,
        perms,
    ))
}

#[derive(Debug, Clone, Copy)]
struct X86PagePermissions {
    writable: bool,
    user: bool,
    executable: bool,
}

impl Default for X86PagePermissions {
    fn default() -> Self {
        Self {
            writable: true,
            user: true,
            executable: true,
        }
    }
}

impl X86PagePermissions {
    fn apply(&mut self, entry: u64) {
        self.writable &= entry & WRITABLE != 0;
        self.user &= entry & USER != 0;
        self.executable &= entry & NX == 0;
    }
}

fn result(gpa: GuestPhysical, page_size: u64, perms: X86PagePermissions) -> TranslationResult {
    TranslationResult {
        gpa,
        readable: true,
        writable: perms.writable,
        executable: perms.executable,
        user: perms.user,
        page_size,
    }
}

fn read_entry(
    memory: &dyn GuestMemoryReader,
    vm: VmId,
    table_base: u64,
    index: u64,
    level: &'static str,
) -> Result<u64, TranslationError> {
    let gpa = table_base
        .checked_add(index * ENTRY_SIZE)
        .ok_or_else(|| malformed(level, "page-table entry address overflowed"))?;
    let mut bytes = [0u8; 8];
    let read = memory
        .read_physical(vm, GuestPhysical(gpa), &mut bytes)
        .map_err(|err| page_table_memory_error(GuestPhysical(gpa), level, err))?;
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

fn page_table_memory_error(
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
    entry: u64,
    gva: GuestVirtual,
) -> Result<(), TranslationError> {
    if entry & PRESENT == 0 {
        return Err(TranslationError::NotPresent { level, gva });
    }
    Ok(())
}

fn reject_reserved_high_bits(level: &'static str, entry: u64) -> Result<(), TranslationError> {
    if entry & RESERVED_HIGH_BITS != 0 {
        return Err(malformed(level, "reserved physical-address bits are set"));
    }
    Ok(())
}

fn reject_reserved_large_page_bits(
    level: &'static str,
    entry: u64,
    mask: u64,
) -> Result<(), TranslationError> {
    if entry & mask != 0 {
        return Err(malformed(level, "reserved large-page address bits are set"));
    }
    Ok(())
}

fn malformed(level: &'static str, detail: &'static str) -> TranslationError {
    TranslationError::MalformedPageTables {
        level,
        detail: detail.to_string(),
    }
}

fn is_canonical(addr: u64, paging_mode: X86PagingMode) -> bool {
    match paging_mode {
        X86PagingMode::FourLevel => is_canonical_with_sign_bit(addr, 47),
        X86PagingMode::La57 => is_canonical_with_sign_bit(addr, 56),
    }
}

fn is_canonical_with_sign_bit(addr: u64, sign_bit: u8) -> bool {
    let sign = (addr >> sign_bit) & 1;
    let high = addr >> (u32::from(sign_bit) + 1);
    let high_bits = 63 - u32::from(sign_bit);
    let expected = if sign == 0 {
        0
    } else {
        (1u64 << high_bits) - 1
    };
    high == expected
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
