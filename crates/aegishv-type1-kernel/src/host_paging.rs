//! Bounded host page-table construction for the linked Type-1 kernel window.
//!
//! This module only prepares and validates a four-level page-table image. It
//! does not write physical memory or load CR3. The caller remains responsible
//! for materializing every returned table page before activating the root.

use core::fmt;

pub const HOST_PAGE_SIZE_4K: u64 = 4096;
pub const HOST_KERNEL_WINDOW_SIZE: u64 = 2 * 1024 * 1024;
pub const HOST_PAGE_TABLE_ENTRY_COUNT: usize = 512;
pub const HOST_PAGE_TABLE_PAGE_COUNT: usize = 4;

pub const HOST_PTE_PRESENT: u64 = 1 << 0;
pub const HOST_PTE_WRITABLE: u64 = 1 << 1;
pub const HOST_PTE_USER: u64 = 1 << 2;
pub const HOST_PTE_ACCESSED: u64 = 1 << 5;
pub const HOST_PTE_DIRTY: u64 = 1 << 6;
pub const HOST_PTE_LARGE: u64 = 1 << 7;
pub const HOST_PTE_NO_EXECUTE: u64 = 1 << 63;

const HOST_PTE_ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const HOST_TABLE_FLAGS: u64 = HOST_PTE_PRESENT | HOST_PTE_WRITABLE;
const HOST_TABLE_ALLOWED_BITS: u64 = HOST_PTE_ADDRESS_MASK | HOST_TABLE_FLAGS | HOST_PTE_ACCESSED;
const HOST_LEAF_ALLOWED_BITS: u64 = HOST_PTE_ADDRESS_MASK
    | HOST_PTE_PRESENT
    | HOST_PTE_WRITABLE
    | HOST_PTE_ACCESSED
    | HOST_PTE_DIRTY
    | HOST_PTE_NO_EXECUTE;
const HOST_GUARD_BITMAP_WORDS: usize = HOST_PAGE_TABLE_ENTRY_COUNT / u64::BITS as usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostPagingError {
    NxNotEnabled,
    FiveLevelPagingActive,
    InvalidPhysicalAddressWidth,
    AddressOverflow,
    NonCanonicalAddress,
    KernelWindowUnaligned,
    KernelWindowNotHigherHalf,
    TableAddressZero,
    TableAddressUnaligned,
    TableAddressOutOfRange,
    DuplicateTablePhysicalAddress,
    DuplicateTableVirtualAddress,
    TableVirtualAddressOutsideWindow,
    EmptyMapping,
    MappingUnaligned,
    MappingOutsideWindow,
    MappingPhysicalAddressOutOfRange,
    WritableExecutableMapping,
    VirtualPageAlreadyMapped,
    PhysicalPageAliased,
    TablePhysicalAddressMappedAtWrongVirtualAddress,
    TableVirtualAddressMappedToWrongPhysicalAddress,
    TablePagePermissionsInvalid,
    TablePageMissingFromKernelMapping,
    GuardAddressUnaligned,
    GuardOverlapsMapping,
    NoMappedPages,
    MaterializedTableMismatch,
    CorruptHierarchy,
    CorruptLeaf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostPagingCapabilities {
    nx_enabled: bool,
    la57_active: bool,
    physical_address_bits: u8,
}

impl HostPagingCapabilities {
    pub const fn new(nx_enabled: bool, la57_active: bool, physical_address_bits: u8) -> Self {
        Self {
            nx_enabled,
            la57_active,
            physical_address_bits,
        }
    }

    pub const fn nx_enabled(self) -> bool {
        self.nx_enabled
    }

    pub const fn la57_active(self) -> bool {
        self.la57_active
    }

    pub const fn physical_address_bits(self) -> u8 {
        self.physical_address_bits
    }

    fn physical_limit(self) -> Result<u64, HostPagingError> {
        if !(12..=52).contains(&self.physical_address_bits) {
            return Err(HostPagingError::InvalidPhysicalAddressWidth);
        }
        Ok(1_u64 << self.physical_address_bits)
    }

    fn validate(self) -> Result<u64, HostPagingError> {
        if !self.nx_enabled {
            return Err(HostPagingError::NxNotEnabled);
        }
        if self.la57_active {
            return Err(HostPagingError::FiveLevelPagingActive);
        }
        self.physical_limit()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostPagePermissions {
    writable: bool,
    executable: bool,
}

impl HostPagePermissions {
    pub const READ_ONLY: Self = Self::new(false, false);
    pub const READ_WRITE: Self = Self::new(true, false);
    pub const READ_EXECUTE: Self = Self::new(false, true);

    pub const fn new(writable: bool, executable: bool) -> Self {
        Self {
            writable,
            executable,
        }
    }

    pub const fn writable(self) -> bool {
        self.writable
    }

    pub const fn executable(self) -> bool {
        self.executable
    }

    pub const fn is_writable_executable(self) -> bool {
        self.writable && self.executable
    }

    const fn leaf_flags(self) -> u64 {
        let writable = if self.writable { HOST_PTE_WRITABLE } else { 0 };
        let no_execute = if self.executable {
            0
        } else {
            HOST_PTE_NO_EXECUTE
        };
        HOST_PTE_PRESENT | writable | no_execute
    }

    const fn from_leaf_entry(entry: u64) -> Self {
        Self {
            writable: entry & HOST_PTE_WRITABLE != 0,
            executable: entry & HOST_PTE_NO_EXECUTE == 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostPageMapping {
    virtual_address: u64,
    physical_address: u64,
    length: u64,
    permissions: HostPagePermissions,
}

impl HostPageMapping {
    pub const fn new(
        virtual_address: u64,
        physical_address: u64,
        length: u64,
        permissions: HostPagePermissions,
    ) -> Self {
        Self {
            virtual_address,
            physical_address,
            length,
            permissions,
        }
    }

    pub const fn virtual_address(self) -> u64 {
        self.virtual_address
    }

    pub const fn physical_address(self) -> u64 {
        self.physical_address
    }

    pub const fn length(self) -> u64 {
        self.length
    }

    pub const fn permissions(self) -> HostPagePermissions {
        self.permissions
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostUnmappedPage {
    virtual_address: u64,
}

impl HostUnmappedPage {
    pub const NULL: Self = Self::new(0);

    pub const fn new(virtual_address: u64) -> Self {
        Self { virtual_address }
    }

    pub const fn virtual_address(self) -> u64 {
        self.virtual_address
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostPageTableLocation {
    physical_address: u64,
    virtual_address: u64,
}

impl HostPageTableLocation {
    pub const fn new(physical_address: u64, virtual_address: u64) -> Self {
        Self {
            physical_address,
            virtual_address,
        }
    }

    pub const fn physical_address(self) -> u64 {
        self.physical_address
    }

    pub const fn virtual_address(self) -> u64 {
        self.virtual_address
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum HostPageTableLevel {
    Pml4 = 0,
    Pdpt = 1,
    Pd = 2,
    Pt = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostPageTableLayout {
    locations: [HostPageTableLocation; HOST_PAGE_TABLE_PAGE_COUNT],
}

impl HostPageTableLayout {
    pub const fn new(
        pml4: HostPageTableLocation,
        pdpt: HostPageTableLocation,
        pd: HostPageTableLocation,
        pt: HostPageTableLocation,
    ) -> Self {
        Self {
            locations: [pml4, pdpt, pd, pt],
        }
    }

    pub fn contiguous(physical_base: u64, virtual_base: u64) -> Result<Self, HostPagingError> {
        let mut locations = [HostPageTableLocation::new(0, 0); HOST_PAGE_TABLE_PAGE_COUNT];
        let mut index = 0;
        while index < HOST_PAGE_TABLE_PAGE_COUNT {
            let offset = (index as u64)
                .checked_mul(HOST_PAGE_SIZE_4K)
                .ok_or(HostPagingError::AddressOverflow)?;
            locations[index] = HostPageTableLocation::new(
                physical_base
                    .checked_add(offset)
                    .ok_or(HostPagingError::AddressOverflow)?,
                virtual_base
                    .checked_add(offset)
                    .ok_or(HostPagingError::AddressOverflow)?,
            );
            index += 1;
        }
        Ok(Self { locations })
    }

    pub const fn location(self, level: HostPageTableLevel) -> HostPageTableLocation {
        self.locations[level as usize]
    }

    pub const fn root_physical_address(self) -> u64 {
        self.location(HostPageTableLevel::Pml4).physical_address()
    }

    pub const fn locations(self) -> [HostPageTableLocation; HOST_PAGE_TABLE_PAGE_COUNT] {
        self.locations
    }

    fn table_index_by_physical(self, physical_address: u64) -> Option<usize> {
        self.locations
            .iter()
            .position(|location| location.physical_address == physical_address)
    }

    fn table_index_by_virtual(self, virtual_address: u64) -> Option<usize> {
        self.locations
            .iter()
            .position(|location| location.virtual_address == virtual_address)
    }

    fn validate(
        self,
        kernel_virtual_base: u64,
        kernel_virtual_end: u64,
        physical_limit: u64,
    ) -> Result<(), HostPagingError> {
        let mut index = 0;
        while index < self.locations.len() {
            let location = self.locations[index];
            if location.physical_address == 0 {
                return Err(HostPagingError::TableAddressZero);
            }
            if !is_page_aligned(location.physical_address)
                || !is_page_aligned(location.virtual_address)
            {
                return Err(HostPagingError::TableAddressUnaligned);
            }
            if !is_canonical_48(location.virtual_address) {
                return Err(HostPagingError::NonCanonicalAddress);
            }
            let physical_end = location
                .physical_address
                .checked_add(HOST_PAGE_SIZE_4K)
                .ok_or(HostPagingError::AddressOverflow)?;
            if physical_end > physical_limit {
                return Err(HostPagingError::TableAddressOutOfRange);
            }
            if location.virtual_address < kernel_virtual_base
                || location.virtual_address >= kernel_virtual_end
            {
                return Err(HostPagingError::TableVirtualAddressOutsideWindow);
            }

            let mut previous = 0;
            while previous < index {
                if self.locations[previous].physical_address == location.physical_address {
                    return Err(HostPagingError::DuplicateTablePhysicalAddress);
                }
                if self.locations[previous].virtual_address == location.virtual_address {
                    return Err(HostPagingError::DuplicateTableVirtualAddress);
                }
                previous += 1;
            }
            index += 1;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
#[repr(C, align(4096))]
pub struct HostPageTable {
    entries: [u64; HOST_PAGE_TABLE_ENTRY_COUNT],
}

impl HostPageTable {
    const fn zeroed() -> Self {
        Self {
            entries: [0; HOST_PAGE_TABLE_ENTRY_COUNT],
        }
    }

    pub const fn entries(&self) -> &[u64; HOST_PAGE_TABLE_ENTRY_COUNT] {
        &self.entries
    }

    pub const fn from_entries(entries: [u64; HOST_PAGE_TABLE_ENTRY_COUNT]) -> Self {
        Self { entries }
    }
}

const _: [(); HOST_PAGE_SIZE_4K as usize] = [(); core::mem::size_of::<HostPageTable>()];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostPageWalk {
    virtual_address: u64,
    physical_address: u64,
    physical_page: u64,
    page_offset: u16,
    permissions: HostPagePermissions,
}

impl HostPageWalk {
    pub const fn virtual_address(self) -> u64 {
        self.virtual_address
    }

    pub const fn physical_address(self) -> u64 {
        self.physical_address
    }

    pub const fn physical_page(self) -> u64 {
        self.physical_page
    }

    pub const fn page_offset(self) -> u16 {
        self.page_offset
    }

    pub const fn permissions(self) -> HostPagePermissions {
        self.permissions
    }
}

pub struct HostPageTableImage {
    layout: HostPageTableLayout,
    kernel_virtual_base: u64,
    physical_limit: u64,
    mapped_page_count: u16,
    guard_bitmap: [u64; HOST_GUARD_BITMAP_WORDS],
    tables: [HostPageTable; HOST_PAGE_TABLE_PAGE_COUNT],
}

impl fmt::Debug for HostPageTableImage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostPageTableImage")
            .field("layout", &self.layout)
            .field("kernel_virtual_base", &self.kernel_virtual_base)
            .field("physical_limit", &self.physical_limit)
            .field("mapped_page_count", &self.mapped_page_count)
            .finish_non_exhaustive()
    }
}

impl HostPageTableImage {
    pub fn build(
        capabilities: HostPagingCapabilities,
        layout: HostPageTableLayout,
        kernel_virtual_base: u64,
        mappings: &[HostPageMapping],
        unmapped_pages: &[HostUnmappedPage],
    ) -> Result<Self, HostPagingError> {
        let physical_limit = capabilities.validate()?;
        let kernel_virtual_end = validate_kernel_window(kernel_virtual_base)?;
        layout.validate(kernel_virtual_base, kernel_virtual_end, physical_limit)?;

        let mut image = Self {
            layout,
            kernel_virtual_base,
            physical_limit,
            mapped_page_count: 0,
            guard_bitmap: [0; HOST_GUARD_BITMAP_WORDS],
            tables: [HostPageTable::zeroed(); HOST_PAGE_TABLE_PAGE_COUNT],
        };
        image.install_hierarchy();

        for guard in unmapped_pages {
            image.record_guard(*guard)?;
        }

        let mut mapped_tables = [false; HOST_PAGE_TABLE_PAGE_COUNT];
        for mapping in mappings {
            image.install_mapping(*mapping, &mut mapped_tables)?;
        }
        if image.mapped_page_count == 0 {
            return Err(HostPagingError::NoMappedPages);
        }
        if mapped_tables.iter().any(|mapped| !mapped) {
            return Err(HostPagingError::TablePageMissingFromKernelMapping);
        }

        image.validate()?;
        for guard in unmapped_pages {
            if image.walk(guard.virtual_address)?.is_some() {
                return Err(HostPagingError::GuardOverlapsMapping);
            }
        }
        Ok(image)
    }

    pub const fn layout(&self) -> HostPageTableLayout {
        self.layout
    }

    pub const fn root_physical_address(&self) -> u64 {
        self.layout.root_physical_address()
    }

    pub const fn kernel_virtual_base(&self) -> u64 {
        self.kernel_virtual_base
    }

    pub const fn mapped_page_count(&self) -> usize {
        self.mapped_page_count as usize
    }

    pub const fn table(&self, level: HostPageTableLevel) -> &HostPageTable {
        &self.tables[level as usize]
    }

    pub const fn tables(&self) -> &[HostPageTable; HOST_PAGE_TABLE_PAGE_COUNT] {
        &self.tables
    }

    pub fn walk(&self, virtual_address: u64) -> Result<Option<HostPageWalk>, HostPagingError> {
        self.walk_tables(&self.tables, virtual_address)
    }

    pub fn walk_materialized(
        &self,
        tables: &[HostPageTable; HOST_PAGE_TABLE_PAGE_COUNT],
        virtual_address: u64,
    ) -> Result<Option<HostPageWalk>, HostPagingError> {
        self.validate_materialized_tables(tables)?;
        self.walk_tables(tables, virtual_address)
    }

    pub fn is_unmapped(&self, virtual_address: u64) -> Result<bool, HostPagingError> {
        Ok(self.walk(virtual_address)?.is_none())
    }

    pub fn validate(&self) -> Result<(), HostPagingError> {
        self.validate_tables(&self.tables)
    }

    pub fn validate_materialized_tables(
        &self,
        tables: &[HostPageTable; HOST_PAGE_TABLE_PAGE_COUNT],
    ) -> Result<(), HostPagingError> {
        let mut table_index = 0;
        while table_index < HOST_PAGE_TABLE_PAGE_COUNT {
            let mutable_bits = if table_index == HostPageTableLevel::Pt as usize {
                HOST_PTE_ACCESSED | HOST_PTE_DIRTY
            } else {
                HOST_PTE_ACCESSED
            };
            let mut entry_index = 0;
            while entry_index < HOST_PAGE_TABLE_ENTRY_COUNT {
                let planned = self.tables[table_index].entries[entry_index];
                let materialized = tables[table_index].entries[entry_index];
                if (planned == 0 && materialized != 0)
                    || planned & !mutable_bits != materialized & !mutable_bits
                {
                    return Err(HostPagingError::MaterializedTableMismatch);
                }
                entry_index += 1;
            }
            table_index += 1;
        }
        self.validate_tables(tables)
    }

    fn walk_tables(
        &self,
        tables: &[HostPageTable; HOST_PAGE_TABLE_PAGE_COUNT],
        virtual_address: u64,
    ) -> Result<Option<HostPageWalk>, HostPagingError> {
        if !is_canonical_48(virtual_address) {
            return Err(HostPagingError::NonCanonicalAddress);
        }

        let pml4_entry =
            tables[HostPageTableLevel::Pml4 as usize].entries[pml4_index(virtual_address)];
        if pml4_entry == 0 {
            return Ok(None);
        }
        self.validate_table_entry(
            pml4_entry,
            self.layout
                .location(HostPageTableLevel::Pdpt)
                .physical_address,
        )?;

        let pdpt_entry =
            tables[HostPageTableLevel::Pdpt as usize].entries[pdpt_index(virtual_address)];
        if pdpt_entry == 0 {
            return Ok(None);
        }
        self.validate_table_entry(
            pdpt_entry,
            self.layout
                .location(HostPageTableLevel::Pd)
                .physical_address,
        )?;

        let pd_entry = tables[HostPageTableLevel::Pd as usize].entries[pd_index(virtual_address)];
        if pd_entry == 0 {
            return Ok(None);
        }
        self.validate_table_entry(
            pd_entry,
            self.layout
                .location(HostPageTableLevel::Pt)
                .physical_address,
        )?;

        let leaf = tables[HostPageTableLevel::Pt as usize].entries[pt_index(virtual_address)];
        if leaf == 0 {
            return Ok(None);
        }
        self.validate_leaf_entry(leaf)?;
        let physical_page = leaf & HOST_PTE_ADDRESS_MASK;
        let page_offset = (virtual_address & (HOST_PAGE_SIZE_4K - 1)) as u16;
        let physical_address = physical_page
            .checked_add(u64::from(page_offset))
            .ok_or(HostPagingError::CorruptLeaf)?;
        Ok(Some(HostPageWalk {
            virtual_address,
            physical_address,
            physical_page,
            page_offset,
            permissions: HostPagePermissions::from_leaf_entry(leaf),
        }))
    }

    fn validate_tables(
        &self,
        tables: &[HostPageTable; HOST_PAGE_TABLE_PAGE_COUNT],
    ) -> Result<(), HostPagingError> {
        let kernel_virtual_end = validate_kernel_window(self.kernel_virtual_base)?;
        self.layout.validate(
            self.kernel_virtual_base,
            kernel_virtual_end,
            self.physical_limit,
        )?;

        let pml4_slot = pml4_index(self.kernel_virtual_base);
        let pdpt_slot = pdpt_index(self.kernel_virtual_base);
        let pd_slot = pd_index(self.kernel_virtual_base);
        self.validate_single_hierarchy_entry(
            tables,
            HostPageTableLevel::Pml4,
            pml4_slot,
            self.layout.location(HostPageTableLevel::Pdpt),
        )?;
        self.validate_single_hierarchy_entry(
            tables,
            HostPageTableLevel::Pdpt,
            pdpt_slot,
            self.layout.location(HostPageTableLevel::Pd),
        )?;
        self.validate_single_hierarchy_entry(
            tables,
            HostPageTableLevel::Pd,
            pd_slot,
            self.layout.location(HostPageTableLevel::Pt),
        )?;

        let mut mapped_count = 0_usize;
        let mut index = 0;
        while index < HOST_PAGE_TABLE_ENTRY_COUNT {
            let entry = tables[HostPageTableLevel::Pt as usize].entries[index];
            if entry != 0 {
                self.validate_leaf_entry(entry)?;
                if self.guard_is_set(index) {
                    return Err(HostPagingError::GuardOverlapsMapping);
                }
                let physical_page = entry & HOST_PTE_ADDRESS_MASK;
                let mut previous = 0;
                while previous < index {
                    let previous_entry = tables[HostPageTableLevel::Pt as usize].entries[previous];
                    if previous_entry != 0
                        && previous_entry & HOST_PTE_ADDRESS_MASK == physical_page
                    {
                        return Err(HostPagingError::PhysicalPageAliased);
                    }
                    previous += 1;
                }
                mapped_count += 1;
            }
            index += 1;
        }
        if mapped_count == 0 {
            return Err(HostPagingError::NoMappedPages);
        }
        if mapped_count != usize::from(self.mapped_page_count) {
            return Err(HostPagingError::CorruptLeaf);
        }

        let mut table_index = 0;
        while table_index < HOST_PAGE_TABLE_PAGE_COUNT {
            let location = self.layout.locations[table_index];
            let walk = self
                .walk_tables(tables, location.virtual_address)?
                .ok_or(HostPagingError::TablePageMissingFromKernelMapping)?;
            if walk.physical_page != location.physical_address {
                return Err(HostPagingError::TableVirtualAddressMappedToWrongPhysicalAddress);
            }
            if walk.permissions != HostPagePermissions::READ_WRITE {
                return Err(HostPagingError::TablePagePermissionsInvalid);
            }
            table_index += 1;
        }
        Ok(())
    }

    fn install_hierarchy(&mut self) {
        let virtual_address = self.kernel_virtual_base;
        self.tables[HostPageTableLevel::Pml4 as usize].entries[pml4_index(virtual_address)] = self
            .layout
            .location(HostPageTableLevel::Pdpt)
            .physical_address
            | HOST_TABLE_FLAGS;
        self.tables[HostPageTableLevel::Pdpt as usize].entries[pdpt_index(virtual_address)] = self
            .layout
            .location(HostPageTableLevel::Pd)
            .physical_address
            | HOST_TABLE_FLAGS;
        self.tables[HostPageTableLevel::Pd as usize].entries[pd_index(virtual_address)] = self
            .layout
            .location(HostPageTableLevel::Pt)
            .physical_address
            | HOST_TABLE_FLAGS;
    }

    fn record_guard(&mut self, guard: HostUnmappedPage) -> Result<(), HostPagingError> {
        if !is_canonical_48(guard.virtual_address) {
            return Err(HostPagingError::NonCanonicalAddress);
        }
        if !is_page_aligned(guard.virtual_address) {
            return Err(HostPagingError::GuardAddressUnaligned);
        }
        if self.is_inside_kernel_window(guard.virtual_address) {
            let index = pt_index(guard.virtual_address);
            self.guard_bitmap[index / u64::BITS as usize] |= 1_u64 << (index % u64::BITS as usize);
        }
        Ok(())
    }

    fn install_mapping(
        &mut self,
        mapping: HostPageMapping,
        mapped_tables: &mut [bool; HOST_PAGE_TABLE_PAGE_COUNT],
    ) -> Result<(), HostPagingError> {
        if mapping.length == 0 {
            return Err(HostPagingError::EmptyMapping);
        }
        if !is_page_aligned(mapping.virtual_address)
            || !is_page_aligned(mapping.physical_address)
            || !is_page_aligned(mapping.length)
        {
            return Err(HostPagingError::MappingUnaligned);
        }
        if mapping.permissions.is_writable_executable() {
            return Err(HostPagingError::WritableExecutableMapping);
        }

        let virtual_end = mapping
            .virtual_address
            .checked_add(mapping.length)
            .ok_or(HostPagingError::AddressOverflow)?;
        let physical_end = mapping
            .physical_address
            .checked_add(mapping.length)
            .ok_or(HostPagingError::AddressOverflow)?;
        let kernel_virtual_end = self
            .kernel_virtual_base
            .checked_add(HOST_KERNEL_WINDOW_SIZE)
            .ok_or(HostPagingError::AddressOverflow)?;
        if !is_canonical_48(mapping.virtual_address)
            || !is_canonical_48(virtual_end - 1)
            || mapping.virtual_address < self.kernel_virtual_base
            || virtual_end > kernel_virtual_end
        {
            return Err(HostPagingError::MappingOutsideWindow);
        }
        if physical_end > self.physical_limit {
            return Err(HostPagingError::MappingPhysicalAddressOutOfRange);
        }

        let page_count = mapping.length / HOST_PAGE_SIZE_4K;
        let mut page = 0;
        while page < page_count {
            let offset = page * HOST_PAGE_SIZE_4K;
            let virtual_address = mapping.virtual_address + offset;
            let physical_address = mapping.physical_address + offset;
            let leaf_index = pt_index(virtual_address);
            if self.guard_is_set(leaf_index) {
                return Err(HostPagingError::GuardOverlapsMapping);
            }
            if self.tables[HostPageTableLevel::Pt as usize].entries[leaf_index] != 0 {
                return Err(HostPagingError::VirtualPageAlreadyMapped);
            }
            if self.physical_page_is_mapped(physical_address) {
                return Err(HostPagingError::PhysicalPageAliased);
            }

            let physical_table = self.layout.table_index_by_physical(physical_address);
            let virtual_table = self.layout.table_index_by_virtual(virtual_address);
            match (physical_table, virtual_table) {
                (Some(physical_index), Some(virtual_index)) if physical_index == virtual_index => {
                    if mapping.permissions != HostPagePermissions::READ_WRITE {
                        return Err(HostPagingError::TablePagePermissionsInvalid);
                    }
                    mapped_tables[physical_index] = true;
                }
                (Some(_), _) => {
                    return Err(HostPagingError::TablePhysicalAddressMappedAtWrongVirtualAddress)
                }
                (_, Some(_)) => {
                    return Err(HostPagingError::TableVirtualAddressMappedToWrongPhysicalAddress)
                }
                (None, None) => {}
            }

            self.tables[HostPageTableLevel::Pt as usize].entries[leaf_index] =
                physical_address | mapping.permissions.leaf_flags();
            self.mapped_page_count = self
                .mapped_page_count
                .checked_add(1)
                .ok_or(HostPagingError::AddressOverflow)?;
            page += 1;
        }
        Ok(())
    }

    fn is_inside_kernel_window(&self, virtual_address: u64) -> bool {
        virtual_address >= self.kernel_virtual_base
            && virtual_address - self.kernel_virtual_base < HOST_KERNEL_WINDOW_SIZE
    }

    fn guard_is_set(&self, leaf_index: usize) -> bool {
        self.guard_bitmap[leaf_index / u64::BITS as usize]
            & (1_u64 << (leaf_index % u64::BITS as usize))
            != 0
    }

    fn physical_page_is_mapped(&self, physical_address: u64) -> bool {
        self.tables[HostPageTableLevel::Pt as usize]
            .entries
            .iter()
            .any(|entry| *entry != 0 && *entry & HOST_PTE_ADDRESS_MASK == physical_address)
    }

    fn validate_table_entry(
        &self,
        entry: u64,
        expected_physical_address: u64,
    ) -> Result<(), HostPagingError> {
        if entry & !HOST_TABLE_ALLOWED_BITS != 0
            || entry & HOST_TABLE_FLAGS != HOST_TABLE_FLAGS
            || entry & HOST_PTE_ADDRESS_MASK != expected_physical_address
            || expected_physical_address >= self.physical_limit
        {
            return Err(HostPagingError::CorruptHierarchy);
        }
        Ok(())
    }

    fn validate_leaf_entry(&self, entry: u64) -> Result<(), HostPagingError> {
        if entry & !HOST_LEAF_ALLOWED_BITS != 0
            || entry & HOST_PTE_PRESENT == 0
            || entry & HOST_PTE_USER != 0
            || entry & HOST_PTE_LARGE != 0
            || entry & HOST_PTE_ADDRESS_MASK >= self.physical_limit
            || (entry & HOST_PTE_WRITABLE != 0 && entry & HOST_PTE_NO_EXECUTE == 0)
        {
            return Err(HostPagingError::CorruptLeaf);
        }
        Ok(())
    }

    fn validate_single_hierarchy_entry(
        &self,
        tables: &[HostPageTable; HOST_PAGE_TABLE_PAGE_COUNT],
        level: HostPageTableLevel,
        populated_index: usize,
        target: HostPageTableLocation,
    ) -> Result<(), HostPagingError> {
        let entries = &tables[level as usize].entries;
        let mut index = 0;
        while index < entries.len() {
            if index == populated_index {
                self.validate_table_entry(entries[index], target.physical_address)?;
            } else if entries[index] != 0 {
                return Err(HostPagingError::CorruptHierarchy);
            }
            index += 1;
        }
        Ok(())
    }
}

pub fn build_host_page_table_image(
    capabilities: HostPagingCapabilities,
    layout: HostPageTableLayout,
    kernel_virtual_base: u64,
    mappings: &[HostPageMapping],
    unmapped_pages: &[HostUnmappedPage],
) -> Result<HostPageTableImage, HostPagingError> {
    HostPageTableImage::build(
        capabilities,
        layout,
        kernel_virtual_base,
        mappings,
        unmapped_pages,
    )
}

const fn is_page_aligned(address: u64) -> bool {
    address & (HOST_PAGE_SIZE_4K - 1) == 0
}

const fn is_canonical_48(address: u64) -> bool {
    let high = address >> 48;
    let sign = (address >> 47) & 1;
    (sign == 0 && high == 0) || (sign == 1 && high == 0xffff)
}

fn validate_kernel_window(kernel_virtual_base: u64) -> Result<u64, HostPagingError> {
    if kernel_virtual_base & (HOST_KERNEL_WINDOW_SIZE - 1) != 0 {
        return Err(HostPagingError::KernelWindowUnaligned);
    }
    if !is_canonical_48(kernel_virtual_base) {
        return Err(HostPagingError::NonCanonicalAddress);
    }
    if kernel_virtual_base & (1_u64 << 47) == 0 {
        return Err(HostPagingError::KernelWindowNotHigherHalf);
    }
    let end = kernel_virtual_base
        .checked_add(HOST_KERNEL_WINDOW_SIZE)
        .ok_or(HostPagingError::AddressOverflow)?;
    if !is_canonical_48(end - 1) {
        return Err(HostPagingError::NonCanonicalAddress);
    }
    Ok(end)
}

const fn pml4_index(virtual_address: u64) -> usize {
    ((virtual_address >> 39) & 0x1ff) as usize
}

const fn pdpt_index(virtual_address: u64) -> usize {
    ((virtual_address >> 30) & 0x1ff) as usize
}

const fn pd_index(virtual_address: u64) -> usize {
    ((virtual_address >> 21) & 0x1ff) as usize
}

const fn pt_index(virtual_address: u64) -> usize {
    ((virtual_address >> 12) & 0x1ff) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    const KERNEL_BASE: u64 = 0xffff_ffff_8000_0000;
    const KERNEL_PHYSICAL_BASE: u64 = 0x20_0000;
    const TABLE_PHYSICAL_BASE: u64 = KERNEL_PHYSICAL_BASE + 0x1f_0000;
    const TABLE_VIRTUAL_BASE: u64 = KERNEL_BASE + 0x1f_0000;

    fn capabilities() -> HostPagingCapabilities {
        HostPagingCapabilities::new(true, false, 48)
    }

    fn layout() -> HostPageTableLayout {
        HostPageTableLayout::contiguous(TABLE_PHYSICAL_BASE, TABLE_VIRTUAL_BASE).unwrap()
    }

    fn table_mapping() -> HostPageMapping {
        HostPageMapping::new(
            TABLE_VIRTUAL_BASE,
            TABLE_PHYSICAL_BASE,
            HOST_PAGE_SIZE_4K * HOST_PAGE_TABLE_PAGE_COUNT as u64,
            HostPagePermissions::READ_WRITE,
        )
    }

    fn minimal_mappings() -> [HostPageMapping; 2] {
        [
            HostPageMapping::new(
                KERNEL_BASE,
                KERNEL_PHYSICAL_BASE,
                HOST_PAGE_SIZE_4K,
                HostPagePermissions::READ_EXECUTE,
            ),
            table_mapping(),
        ]
    }

    fn image() -> HostPageTableImage {
        HostPageTableImage::build(
            capabilities(),
            layout(),
            KERNEL_BASE,
            &minimal_mappings(),
            &[
                HostUnmappedPage::NULL,
                HostUnmappedPage::new(KERNEL_BASE + 0x1000),
            ],
        )
        .unwrap()
    }

    #[test]
    fn table_page_has_exact_hardware_page_layout() {
        assert_eq!(core::mem::size_of::<HostPageTable>(), 4096);
        assert_eq!(core::mem::align_of::<HostPageTable>(), 4096);
    }

    #[test]
    fn builds_relocated_supervisor_4k_mappings_and_self_walks() {
        let mappings = [
            HostPageMapping::new(
                KERNEL_BASE,
                KERNEL_PHYSICAL_BASE,
                HOST_PAGE_SIZE_4K * 2,
                HostPagePermissions::READ_EXECUTE,
            ),
            HostPageMapping::new(
                KERNEL_BASE + 0x4000,
                KERNEL_PHYSICAL_BASE + 0x40_000,
                HOST_PAGE_SIZE_4K,
                HostPagePermissions::READ_ONLY,
            ),
            HostPageMapping::new(
                KERNEL_BASE + 0x8000,
                KERNEL_PHYSICAL_BASE + 0x80_000,
                HOST_PAGE_SIZE_4K * 2,
                HostPagePermissions::READ_WRITE,
            ),
            table_mapping(),
        ];
        let built = HostPageTableImage::build(
            capabilities(),
            layout(),
            KERNEL_BASE,
            &mappings,
            &[
                HostUnmappedPage::NULL,
                HostUnmappedPage::new(KERNEL_BASE + 0x3000),
            ],
        )
        .unwrap();

        let text = built.walk(KERNEL_BASE + 0x123).unwrap().unwrap();
        assert_eq!(text.physical_address(), KERNEL_PHYSICAL_BASE + 0x123);
        assert_eq!(text.page_offset(), 0x123);
        assert_eq!(text.permissions(), HostPagePermissions::READ_EXECUTE);

        let rodata = built.walk(KERNEL_BASE + 0x4000).unwrap().unwrap();
        assert_eq!(rodata.physical_page(), KERNEL_PHYSICAL_BASE + 0x40_000);
        assert_eq!(rodata.permissions(), HostPagePermissions::READ_ONLY);

        let data = built.walk(KERNEL_BASE + 0x8000).unwrap().unwrap();
        assert_eq!(data.permissions(), HostPagePermissions::READ_WRITE);
        assert_eq!(built.mapped_page_count(), 9);
        assert_eq!(built.root_physical_address(), TABLE_PHYSICAL_BASE);
        assert!(built.validate().is_ok());

        let text_entry = built.table(HostPageTableLevel::Pt).entries()[0];
        assert_eq!(text_entry & HOST_PTE_USER, 0);
        assert_eq!(text_entry & HOST_PTE_LARGE, 0);
        assert_eq!(text_entry & HOST_PTE_NO_EXECUTE, 0);
        let data_entry = built.table(HostPageTableLevel::Pt).entries()[8];
        assert_ne!(data_entry & HOST_PTE_WRITABLE, 0);
        assert_ne!(data_entry & HOST_PTE_NO_EXECUTE, 0);
    }

    #[test]
    fn keeps_null_explicit_guard_and_mapping_gaps_unmapped() {
        let built = image();

        assert!(built.is_unmapped(0).unwrap());
        assert!(built.is_unmapped(KERNEL_BASE + 0x1000).unwrap());
        assert!(built.is_unmapped(KERNEL_BASE + 0x10_000).unwrap());
        assert!(built.is_unmapped(0xffff_8000_0000_0000).unwrap());
    }

    #[test]
    fn maps_each_live_table_once_as_supervisor_rw_nx() {
        let built = image();

        for location in built.layout().locations() {
            let walk = built.walk(location.virtual_address()).unwrap().unwrap();
            assert_eq!(walk.physical_page(), location.physical_address());
            assert_eq!(walk.permissions(), HostPagePermissions::READ_WRITE);
            let entry =
                built.table(HostPageTableLevel::Pt).entries()[pt_index(location.virtual_address())];
            assert_eq!(entry & HOST_PTE_USER, 0);
            assert_ne!(entry & HOST_PTE_WRITABLE, 0);
            assert_ne!(entry & HOST_PTE_NO_EXECUTE, 0);
        }
    }

    #[test]
    fn hierarchy_uses_relocated_table_physical_addresses() {
        let built = image();
        let pml4 = built.table(HostPageTableLevel::Pml4).entries()[pml4_index(KERNEL_BASE)];
        let pdpt = built.table(HostPageTableLevel::Pdpt).entries()[pdpt_index(KERNEL_BASE)];
        let pd = built.table(HostPageTableLevel::Pd).entries()[pd_index(KERNEL_BASE)];

        assert_eq!(
            pml4 & HOST_PTE_ADDRESS_MASK,
            TABLE_PHYSICAL_BASE + HOST_PAGE_SIZE_4K
        );
        assert_eq!(
            pdpt & HOST_PTE_ADDRESS_MASK,
            TABLE_PHYSICAL_BASE + 2 * HOST_PAGE_SIZE_4K
        );
        assert_eq!(
            pd & HOST_PTE_ADDRESS_MASK,
            TABLE_PHYSICAL_BASE + 3 * HOST_PAGE_SIZE_4K
        );
    }

    #[test]
    fn fills_the_bounded_2m_window_with_4k_leaves() {
        let mappings = [
            HostPageMapping::new(
                KERNEL_BASE,
                KERNEL_PHYSICAL_BASE,
                HOST_KERNEL_WINDOW_SIZE,
                HostPagePermissions::READ_ONLY,
            ),
            table_mapping(),
        ];

        assert_eq!(
            HostPageTableImage::build(capabilities(), layout(), KERNEL_BASE, &mappings, &[],)
                .unwrap_err(),
            HostPagingError::TablePagePermissionsInvalid
        );

        let built = HostPageTableImage::build(
            capabilities(),
            layout(),
            KERNEL_BASE,
            &[
                HostPageMapping::new(
                    KERNEL_BASE,
                    KERNEL_PHYSICAL_BASE,
                    TABLE_VIRTUAL_BASE - KERNEL_BASE,
                    HostPagePermissions::READ_ONLY,
                ),
                table_mapping(),
                HostPageMapping::new(
                    TABLE_VIRTUAL_BASE + 4 * HOST_PAGE_SIZE_4K,
                    TABLE_PHYSICAL_BASE + 4 * HOST_PAGE_SIZE_4K,
                    KERNEL_BASE + HOST_KERNEL_WINDOW_SIZE
                        - (TABLE_VIRTUAL_BASE + 4 * HOST_PAGE_SIZE_4K),
                    HostPagePermissions::READ_ONLY,
                ),
            ],
            &[],
        )
        .unwrap();
        assert_eq!(built.mapped_page_count(), 512);
        assert!(built
            .walk(KERNEL_BASE + HOST_KERNEL_WINDOW_SIZE - 1)
            .unwrap()
            .is_some());
    }

    #[test]
    fn rejects_missing_nx_and_active_la57() {
        assert_eq!(
            HostPageTableImage::build(
                HostPagingCapabilities::new(false, false, 48),
                layout(),
                KERNEL_BASE,
                &minimal_mappings(),
                &[],
            )
            .unwrap_err(),
            HostPagingError::NxNotEnabled
        );
        assert_eq!(
            HostPageTableImage::build(
                HostPagingCapabilities::new(true, true, 48),
                layout(),
                KERNEL_BASE,
                &minimal_mappings(),
                &[],
            )
            .unwrap_err(),
            HostPagingError::FiveLevelPagingActive
        );
    }

    #[test]
    fn rejects_bad_physical_width_and_out_of_range_addresses() {
        assert_eq!(
            HostPageTableImage::build(
                HostPagingCapabilities::new(true, false, 53),
                layout(),
                KERNEL_BASE,
                &minimal_mappings(),
                &[],
            )
            .unwrap_err(),
            HostPagingError::InvalidPhysicalAddressWidth
        );
        let high_layout = HostPageTableLayout::contiguous(
            (1_u64 << 32) - 3 * HOST_PAGE_SIZE_4K,
            TABLE_VIRTUAL_BASE,
        )
        .unwrap();
        assert_eq!(
            HostPageTableImage::build(
                HostPagingCapabilities::new(true, false, 32),
                high_layout,
                KERNEL_BASE,
                &minimal_mappings(),
                &[],
            )
            .unwrap_err(),
            HostPagingError::TableAddressOutOfRange
        );
    }

    #[test]
    fn rejects_invalid_kernel_windows() {
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                KERNEL_BASE + HOST_PAGE_SIZE_4K,
                &minimal_mappings(),
                &[],
            )
            .unwrap_err(),
            HostPagingError::KernelWindowUnaligned
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                0x4000_0000,
                &minimal_mappings(),
                &[],
            )
            .unwrap_err(),
            HostPagingError::KernelWindowNotHigherHalf
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                0x0000_8000_0000_0000,
                &minimal_mappings(),
                &[],
            )
            .unwrap_err(),
            HostPagingError::NonCanonicalAddress
        );
    }

    #[test]
    fn rejects_unaligned_duplicate_and_out_of_window_table_locations() {
        let bad_alignment = HostPageTableLayout::new(
            HostPageTableLocation::new(TABLE_PHYSICAL_BASE + 1, TABLE_VIRTUAL_BASE),
            layout().location(HostPageTableLevel::Pdpt),
            layout().location(HostPageTableLevel::Pd),
            layout().location(HostPageTableLevel::Pt),
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                bad_alignment,
                KERNEL_BASE,
                &minimal_mappings(),
                &[],
            )
            .unwrap_err(),
            HostPagingError::TableAddressUnaligned
        );

        let duplicate = HostPageTableLayout::new(
            layout().location(HostPageTableLevel::Pml4),
            layout().location(HostPageTableLevel::Pml4),
            layout().location(HostPageTableLevel::Pd),
            layout().location(HostPageTableLevel::Pt),
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                duplicate,
                KERNEL_BASE,
                &minimal_mappings(),
                &[],
            )
            .unwrap_err(),
            HostPagingError::DuplicateTablePhysicalAddress
        );

        let outside = HostPageTableLayout::new(
            HostPageTableLocation::new(TABLE_PHYSICAL_BASE, KERNEL_BASE - HOST_PAGE_SIZE_4K),
            layout().location(HostPageTableLevel::Pdpt),
            layout().location(HostPageTableLevel::Pd),
            layout().location(HostPageTableLevel::Pt),
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                outside,
                KERNEL_BASE,
                &minimal_mappings(),
                &[],
            )
            .unwrap_err(),
            HostPagingError::TableVirtualAddressOutsideWindow
        );
    }

    #[test]
    fn rejects_empty_unaligned_overflowing_and_outside_mappings() {
        let cases = [
            (
                HostPageMapping::new(
                    KERNEL_BASE,
                    KERNEL_PHYSICAL_BASE,
                    0,
                    HostPagePermissions::READ_ONLY,
                ),
                HostPagingError::EmptyMapping,
            ),
            (
                HostPageMapping::new(
                    KERNEL_BASE + 1,
                    KERNEL_PHYSICAL_BASE,
                    HOST_PAGE_SIZE_4K,
                    HostPagePermissions::READ_ONLY,
                ),
                HostPagingError::MappingUnaligned,
            ),
            (
                HostPageMapping::new(
                    KERNEL_BASE + HOST_KERNEL_WINDOW_SIZE,
                    KERNEL_PHYSICAL_BASE,
                    HOST_PAGE_SIZE_4K,
                    HostPagePermissions::READ_ONLY,
                ),
                HostPagingError::MappingOutsideWindow,
            ),
            (
                HostPageMapping::new(
                    KERNEL_BASE,
                    u64::MAX - (HOST_PAGE_SIZE_4K - 1),
                    HOST_PAGE_SIZE_4K,
                    HostPagePermissions::READ_ONLY,
                ),
                HostPagingError::AddressOverflow,
            ),
        ];
        for (mapping, expected) in cases {
            assert_eq!(
                HostPageTableImage::build(
                    capabilities(),
                    layout(),
                    KERNEL_BASE,
                    &[mapping, table_mapping()],
                    &[],
                )
                .unwrap_err(),
                expected
            );
        }
    }

    #[test]
    fn rejects_writable_executable_virtual_and_physical_aliases() {
        let wx = HostPageMapping::new(
            KERNEL_BASE,
            KERNEL_PHYSICAL_BASE,
            HOST_PAGE_SIZE_4K,
            HostPagePermissions::new(true, true),
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                KERNEL_BASE,
                &[wx, table_mapping()],
                &[],
            )
            .unwrap_err(),
            HostPagingError::WritableExecutableMapping
        );

        let original = minimal_mappings()[0];
        let duplicate_virtual = HostPageMapping::new(
            KERNEL_BASE,
            KERNEL_PHYSICAL_BASE + 0x10_000,
            HOST_PAGE_SIZE_4K,
            HostPagePermissions::READ_ONLY,
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                KERNEL_BASE,
                &[original, duplicate_virtual, table_mapping()],
                &[],
            )
            .unwrap_err(),
            HostPagingError::VirtualPageAlreadyMapped
        );

        let duplicate_physical = HostPageMapping::new(
            KERNEL_BASE + 0x1000,
            KERNEL_PHYSICAL_BASE,
            HOST_PAGE_SIZE_4K,
            HostPagePermissions::READ_ONLY,
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                KERNEL_BASE,
                &[original, duplicate_physical, table_mapping()],
                &[],
            )
            .unwrap_err(),
            HostPagingError::PhysicalPageAliased
        );
    }

    #[test]
    fn rejects_table_mapping_with_wrong_address_permissions_or_omission() {
        let wrong_virtual = HostPageMapping::new(
            TABLE_VIRTUAL_BASE - HOST_PAGE_SIZE_4K,
            TABLE_PHYSICAL_BASE,
            HOST_PAGE_SIZE_4K,
            HostPagePermissions::READ_WRITE,
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                KERNEL_BASE,
                &[minimal_mappings()[0], wrong_virtual],
                &[],
            )
            .unwrap_err(),
            HostPagingError::TablePhysicalAddressMappedAtWrongVirtualAddress
        );

        let wrong_physical = HostPageMapping::new(
            TABLE_VIRTUAL_BASE,
            TABLE_PHYSICAL_BASE - HOST_PAGE_SIZE_4K,
            HOST_PAGE_SIZE_4K,
            HostPagePermissions::READ_WRITE,
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                KERNEL_BASE,
                &[minimal_mappings()[0], wrong_physical],
                &[],
            )
            .unwrap_err(),
            HostPagingError::TableVirtualAddressMappedToWrongPhysicalAddress
        );

        let executable_tables = HostPageMapping::new(
            TABLE_VIRTUAL_BASE,
            TABLE_PHYSICAL_BASE,
            4 * HOST_PAGE_SIZE_4K,
            HostPagePermissions::READ_EXECUTE,
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                KERNEL_BASE,
                &[minimal_mappings()[0], executable_tables],
                &[],
            )
            .unwrap_err(),
            HostPagingError::TablePagePermissionsInvalid
        );

        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                KERNEL_BASE,
                &[minimal_mappings()[0]],
                &[],
            )
            .unwrap_err(),
            HostPagingError::TablePageMissingFromKernelMapping
        );
    }

    #[test]
    fn rejects_guard_conflicts_and_bad_guard_addresses() {
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                KERNEL_BASE,
                &minimal_mappings(),
                &[HostUnmappedPage::new(KERNEL_BASE)],
            )
            .unwrap_err(),
            HostPagingError::GuardOverlapsMapping
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                KERNEL_BASE,
                &minimal_mappings(),
                &[HostUnmappedPage::new(1)],
            )
            .unwrap_err(),
            HostPagingError::GuardAddressUnaligned
        );
        assert_eq!(
            HostPageTableImage::build(
                capabilities(),
                layout(),
                KERNEL_BASE,
                &minimal_mappings(),
                &[HostUnmappedPage::new(0x0000_8000_0000_0000)],
            )
            .unwrap_err(),
            HostPagingError::NonCanonicalAddress
        );
    }

    #[test]
    fn self_validation_rejects_hierarchy_and_leaf_corruption() {
        let mut hierarchy = image();
        hierarchy.tables[HostPageTableLevel::Pml4 as usize].entries[pml4_index(KERNEL_BASE)] |=
            HOST_PTE_USER;
        assert_eq!(
            hierarchy.validate().unwrap_err(),
            HostPagingError::CorruptHierarchy
        );

        let mut leaf = image();
        leaf.tables[HostPageTableLevel::Pt as usize].entries[pt_index(KERNEL_BASE)] |=
            HOST_PTE_WRITABLE;
        assert_eq!(leaf.validate().unwrap_err(), HostPagingError::CorruptLeaf);

        let mut aliased = image();
        aliased.tables[HostPageTableLevel::Pt as usize].entries[pt_index(KERNEL_BASE + 0x1000)] =
            aliased.tables[HostPageTableLevel::Pt as usize].entries[pt_index(KERNEL_BASE)];
        aliased.mapped_page_count += 1;
        assert_eq!(
            aliased.validate().unwrap_err(),
            HostPagingError::GuardOverlapsMapping
        );
    }

    #[test]
    fn validation_accepts_hardware_accessed_and_dirty_updates() {
        let mut built = image();
        built.tables[HostPageTableLevel::Pml4 as usize].entries[pml4_index(KERNEL_BASE)] |=
            HOST_PTE_ACCESSED;
        built.tables[HostPageTableLevel::Pdpt as usize].entries[pdpt_index(KERNEL_BASE)] |=
            HOST_PTE_ACCESSED;
        built.tables[HostPageTableLevel::Pd as usize].entries[pd_index(KERNEL_BASE)] |=
            HOST_PTE_ACCESSED;
        built.tables[HostPageTableLevel::Pt as usize].entries[pt_index(KERNEL_BASE)] |=
            HOST_PTE_ACCESSED;
        built.tables[HostPageTableLevel::Pt as usize].entries[pt_index(TABLE_VIRTUAL_BASE)] |=
            HOST_PTE_ACCESSED | HOST_PTE_DIRTY;

        assert!(built.validate().is_ok());
        assert_eq!(
            built.walk(KERNEL_BASE).unwrap().unwrap().permissions(),
            HostPagePermissions::READ_EXECUTE
        );
    }

    #[test]
    fn materialized_validation_accepts_ad_only_differences() {
        let planned = image();
        let mut materialized = *planned.tables();
        materialized[HostPageTableLevel::Pml4 as usize].entries[pml4_index(KERNEL_BASE)] |=
            HOST_PTE_ACCESSED;
        materialized[HostPageTableLevel::Pdpt as usize].entries[pdpt_index(KERNEL_BASE)] |=
            HOST_PTE_ACCESSED;
        materialized[HostPageTableLevel::Pd as usize].entries[pd_index(KERNEL_BASE)] |=
            HOST_PTE_ACCESSED;
        materialized[HostPageTableLevel::Pt as usize].entries[pt_index(KERNEL_BASE)] |=
            HOST_PTE_ACCESSED;
        materialized[HostPageTableLevel::Pt as usize].entries[pt_index(TABLE_VIRTUAL_BASE)] |=
            HOST_PTE_ACCESSED | HOST_PTE_DIRTY;

        assert!(planned.validate_materialized_tables(&materialized).is_ok());
        let table_walk = planned
            .walk_materialized(&materialized, TABLE_VIRTUAL_BASE)
            .unwrap()
            .unwrap();
        assert_eq!(table_walk.physical_page(), TABLE_PHYSICAL_BASE);
        assert_eq!(table_walk.permissions(), HostPagePermissions::READ_WRITE);
    }

    #[test]
    fn materialized_validation_rejects_non_hardware_differences() {
        let planned = image();

        let mut wrong_leaf = *planned.tables();
        wrong_leaf[HostPageTableLevel::Pt as usize].entries[pt_index(KERNEL_BASE)] ^=
            HOST_PTE_NO_EXECUTE;
        assert_eq!(
            planned
                .validate_materialized_tables(&wrong_leaf)
                .unwrap_err(),
            HostPagingError::MaterializedTableMismatch
        );

        let mut dirty_hierarchy = *planned.tables();
        dirty_hierarchy[HostPageTableLevel::Pml4 as usize].entries[pml4_index(KERNEL_BASE)] |=
            HOST_PTE_DIRTY;
        assert_eq!(
            planned
                .validate_materialized_tables(&dirty_hierarchy)
                .unwrap_err(),
            HostPagingError::MaterializedTableMismatch
        );

        let mut unexpected_leaf = *planned.tables();
        unexpected_leaf[HostPageTableLevel::Pt as usize].entries[pt_index(KERNEL_BASE + 0x1000)] =
            HOST_PTE_ACCESSED;
        assert_eq!(
            planned
                .walk_materialized(&unexpected_leaf, KERNEL_BASE)
                .unwrap_err(),
            HostPagingError::MaterializedTableMismatch
        );
    }

    #[test]
    fn walk_rejects_noncanonical_addresses() {
        assert_eq!(
            image().walk(0x0000_8000_0000_0000).unwrap_err(),
            HostPagingError::NonCanonicalAddress
        );
    }

    #[test]
    fn contiguous_layout_rejects_address_wrap() {
        assert_eq!(
            HostPageTableLayout::contiguous(u64::MAX - 0x1000, TABLE_VIRTUAL_BASE).unwrap_err(),
            HostPagingError::AddressOverflow
        );
        assert_eq!(
            HostPageTableLayout::contiguous(TABLE_PHYSICAL_BASE, u64::MAX - 0x1000).unwrap_err(),
            HostPagingError::AddressOverflow
        );
    }
}
