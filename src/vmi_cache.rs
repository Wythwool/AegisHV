use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

use crate::vmi::{
    GuestPhysical, GuestVirtual, TranslationError, TranslationResult, VmId, VmiErrorKind,
};

const X86_CR3_BASE_MASK: u64 = !0xfff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslationCacheError {
    InvalidCapacity { max_entries: usize },
    InvalidPageSize { page_size: u64 },
}

impl TranslationCacheError {
    pub fn kind(&self) -> VmiErrorKind {
        match self {
            Self::InvalidCapacity { .. } | Self::InvalidPageSize { .. } => {
                VmiErrorKind::InvalidInput
            }
        }
    }
}

impl fmt::Display for TranslationCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapacity { max_entries } => {
                write!(
                    f,
                    "translation cache capacity must be greater than zero, got {max_entries}"
                )
            }
            Self::InvalidPageSize { page_size } => {
                write!(
                    f,
                    "translation cache page size must be a non-zero power of two, got {page_size}"
                )
            }
        }
    }
}

impl Error for TranslationCacheError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arm64CacheGranule {
    Size4K,
    Size16K,
    Size64K,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TranslationMode {
    X86_64FourLevel,
    X86_64La57,
    Arm64Stage1 { granule: Arm64CacheGranule },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressSpaceRoot {
    X86Cr3 { base: u64 },
    Arm64Ttbr0 { base: u64, asid: Option<u16> },
    Arm64Ttbr1 { base: u64, asid: Option<u16> },
    Explicit { id: u64, asid: Option<u16> },
}

impl AddressSpaceRoot {
    pub fn x86_cr3(cr3: u64) -> Self {
        Self::X86Cr3 {
            base: cr3 & X86_CR3_BASE_MASK,
        }
    }

    pub fn arm64_ttbr0(base: u64, asid: Option<u16>) -> Self {
        Self::Arm64Ttbr0 { base, asid }
    }

    pub fn arm64_ttbr1(base: u64, asid: Option<u16>) -> Self {
        Self::Arm64Ttbr1 { base, asid }
    }

    pub fn explicit(id: u64, asid: Option<u16>) -> Self {
        Self::Explicit { id, asid }
    }

    fn asid(self) -> Option<u16> {
        match self {
            Self::X86Cr3 { .. } => None,
            Self::Arm64Ttbr0 { asid, .. }
            | Self::Arm64Ttbr1 { asid, .. }
            | Self::Explicit { asid, .. } => asid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TranslationAccess {
    pub user: bool,
    pub write: bool,
    pub execute: bool,
}

impl TranslationAccess {
    pub const fn kernel_read() -> Self {
        Self {
            user: false,
            write: false,
            execute: false,
        }
    }

    pub const fn kernel_write() -> Self {
        Self {
            user: false,
            write: true,
            execute: false,
        }
    }

    pub const fn kernel_execute() -> Self {
        Self {
            user: false,
            write: false,
            execute: true,
        }
    }

    pub const fn user_read() -> Self {
        Self {
            user: true,
            write: false,
            execute: false,
        }
    }

    pub const fn user_write() -> Self {
        Self {
            user: true,
            write: true,
            execute: false,
        }
    }

    pub const fn user_execute() -> Self {
        Self {
            user: true,
            write: false,
            execute: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TranslationCacheKey {
    pub vm: VmId,
    pub mode: TranslationMode,
    pub root: AddressSpaceRoot,
    pub virtual_page: u64,
    pub page_size: u64,
    pub access: TranslationAccess,
}

impl TranslationCacheKey {
    pub fn for_gva(
        vm: VmId,
        mode: TranslationMode,
        root: AddressSpaceRoot,
        gva: GuestVirtual,
        page_size: u64,
        access: TranslationAccess,
    ) -> Result<Self, TranslationCacheError> {
        validate_page_size(page_size)?;
        Ok(Self {
            vm,
            mode,
            root,
            virtual_page: gva.0 / page_size,
            page_size,
            access,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslationCacheValue {
    pub physical_page: GuestPhysical,
    pub page_size: u64,
    pub readable: bool,
    pub writable: bool,
    pub user: bool,
    pub executable: bool,
}

impl TranslationCacheValue {
    pub fn new(
        physical_page: GuestPhysical,
        page_size: u64,
        readable: bool,
        writable: bool,
        user: bool,
        executable: bool,
    ) -> Result<Self, TranslationCacheError> {
        validate_page_size(page_size)?;
        Ok(Self {
            physical_page: GuestPhysical(physical_page.0 & !(page_size - 1)),
            page_size,
            readable,
            writable,
            user,
            executable,
        })
    }

    pub fn from_translation(result: &TranslationResult) -> Result<Self, TranslationCacheError> {
        Self::new(
            result.gpa,
            result.page_size,
            result.readable,
            result.writable,
            result.user,
            result.executable,
        )
    }

    pub fn to_result(self, gva: GuestVirtual) -> TranslationResult {
        let offset = gva.0 & (self.page_size - 1);
        TranslationResult {
            gpa: GuestPhysical(self.physical_page.0 | offset),
            readable: self.readable,
            writable: self.writable,
            executable: self.executable,
            user: self.user,
            page_size: self.page_size,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TranslationCacheEntry {
    key: TranslationCacheKey,
    value: TranslationCacheValue,
}

#[derive(Debug, Clone)]
pub struct TranslationCache {
    max_entries: usize,
    entries: VecDeque<TranslationCacheEntry>,
}

impl TranslationCache {
    pub fn new(max_entries: usize) -> Result<Self, TranslationCacheError> {
        if max_entries == 0 {
            return Err(TranslationCacheError::InvalidCapacity { max_entries });
        }
        Ok(Self {
            max_entries,
            entries: VecDeque::new(),
        })
    }

    pub fn capacity(&self) -> usize {
        self.max_entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn lookup(&self, key: &TranslationCacheKey) -> Option<TranslationCacheValue> {
        self.entries
            .iter()
            .find(|entry| entry.key == *key)
            .map(|entry| entry.value)
    }

    pub fn lookup_result(
        &self,
        key: &TranslationCacheKey,
        gva: GuestVirtual,
    ) -> Option<TranslationResult> {
        if gva.0.checked_div(key.page_size)? != key.virtual_page {
            return None;
        }

        self.lookup(key)
            .filter(|value| value.page_size == key.page_size)
            .map(|value| value.to_result(gva))
    }

    pub fn insert(&mut self, key: TranslationCacheKey, value: TranslationCacheValue) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
            entry.value = value;
            return;
        }

        if self.entries.len() == self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(TranslationCacheEntry { key, value });
    }

    pub fn insert_translation_result(
        &mut self,
        key: TranslationCacheKey,
        result: Result<TranslationResult, TranslationError>,
    ) -> Result<(), TranslationError> {
        let result = result?;
        if result.page_size != key.page_size {
            return Err(cache_page_size_mismatch(key, result.page_size));
        }
        let value = TranslationCacheValue::from_translation(&result)
            .map_err(|err| cache_record_error(key, err))?;
        self.insert(key, value);
        Ok(())
    }

    pub fn invalidate_vmid(&mut self, vm: VmId) -> usize {
        self.retain_count(|entry| entry.key.vm != vm)
    }

    pub fn invalidate_root(&mut self, vm: VmId, root: AddressSpaceRoot) -> usize {
        self.retain_count(|entry| entry.key.vm != vm || entry.key.root != root)
    }

    pub fn invalidate_cr3(&mut self, vm: VmId, cr3: u64) -> usize {
        let root = AddressSpaceRoot::x86_cr3(cr3);
        self.retain_count(|entry| entry.key.vm != vm || entry.key.root != root)
    }

    pub fn invalidate_asid(&mut self, vm: VmId, asid: u16) -> usize {
        self.retain_count(|entry| entry.key.vm != vm || entry.key.root.asid() != Some(asid))
    }

    pub fn flush(&mut self) -> usize {
        let removed = self.entries.len();
        self.entries.clear();
        removed
    }

    fn retain_count<F>(&mut self, mut keep: F) -> usize
    where
        F: FnMut(&TranslationCacheEntry) -> bool,
    {
        let before = self.entries.len();
        self.entries.retain(|entry| keep(entry));
        before - self.entries.len()
    }
}

fn validate_page_size(page_size: u64) -> Result<(), TranslationCacheError> {
    if page_size == 0 || !page_size.is_power_of_two() {
        return Err(TranslationCacheError::InvalidPageSize { page_size });
    }
    Ok(())
}

fn cache_record_error(key: TranslationCacheKey, err: TranslationCacheError) -> TranslationError {
    TranslationError::TranslationFailed {
        gva: GuestVirtual(key.virtual_page.saturating_mul(key.page_size)),
        detail: err.to_string(),
    }
}

fn cache_page_size_mismatch(key: TranslationCacheKey, result_page_size: u64) -> TranslationError {
    TranslationError::TranslationFailed {
        gva: GuestVirtual(key.virtual_page.saturating_mul(key.page_size)),
        detail: format!(
            "translation cache key page size {} does not match translation result page size {}",
            key.page_size, result_page_size
        ),
    }
}
