use super::{
    BackendError, GuestMemoryReader, GuestPhysical, MemoryReadError, TranslationError, VmId,
};

#[derive(Debug, Clone, Default)]
pub struct SyntheticGuestPhysicalMemoryReader {
    ranges: Vec<SyntheticMemoryRange>,
    partial_reads: bool,
}

#[derive(Debug, Clone)]
struct SyntheticMemoryRange {
    start: u64,
    end: u64,
    access: SyntheticMemoryAccess,
}

#[derive(Debug, Clone)]
enum SyntheticMemoryAccess {
    Mapped(Vec<u8>),
    PermissionDenied { detail: String },
    TemporarilyUnavailable { detail: String },
}

impl SyntheticGuestPhysicalMemoryReader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_partial_reads(mut self, allow: bool) -> Self {
        self.partial_reads = allow;
        self
    }

    pub fn set_partial_reads(&mut self, allow: bool) {
        self.partial_reads = allow;
    }

    pub fn map_range(
        &mut self,
        gpa: GuestPhysical,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<(), MemoryReadError> {
        let bytes = bytes.into();
        let end = checked_memory_range(gpa, bytes.len())?;
        self.insert_range(SyntheticMemoryRange {
            start: gpa.0,
            end,
            access: SyntheticMemoryAccess::Mapped(bytes),
        })
    }

    pub fn deny_range(
        &mut self,
        gpa: GuestPhysical,
        len: usize,
        detail: impl Into<String>,
    ) -> Result<(), MemoryReadError> {
        let end = checked_memory_range(gpa, len)?;
        self.insert_range(SyntheticMemoryRange {
            start: gpa.0,
            end,
            access: SyntheticMemoryAccess::PermissionDenied {
                detail: detail.into(),
            },
        })
    }

    pub fn mark_unavailable_range(
        &mut self,
        gpa: GuestPhysical,
        len: usize,
        detail: impl Into<String>,
    ) -> Result<(), MemoryReadError> {
        let end = checked_memory_range(gpa, len)?;
        self.insert_range(SyntheticMemoryRange {
            start: gpa.0,
            end,
            access: SyntheticMemoryAccess::TemporarilyUnavailable {
                detail: detail.into(),
            },
        })
    }

    fn insert_range(&mut self, range: SyntheticMemoryRange) -> Result<(), MemoryReadError> {
        if self
            .ranges
            .iter()
            .any(|existing| ranges_overlap(range.start, range.end, existing.start, existing.end))
        {
            return Err(MemoryReadError::InvalidRange {
                gpa: GuestPhysical(range.start),
                len: range_len(range.start, range.end),
            });
        }

        self.ranges.push(range);
        self.ranges.sort_by_key(|range| range.start);
        Ok(())
    }

    fn validate_full_read(&self, gpa: GuestPhysical, len: usize) -> Result<(), MemoryReadError> {
        let mut cursor = gpa.0;
        let mut remaining = len;
        while remaining > 0 {
            let range = self
                .range_containing(cursor)
                .ok_or(MemoryReadError::Unmapped {
                    gpa: GuestPhysical(cursor),
                    len: remaining,
                })?;

            match &range.access {
                SyntheticMemoryAccess::Mapped(bytes) => {
                    let offset = range_offset(range, cursor);
                    let readable = bytes.len().saturating_sub(offset).min(remaining);
                    cursor = advance_guest_physical(cursor, readable)?;
                    remaining -= readable;
                }
                SyntheticMemoryAccess::PermissionDenied { detail } => {
                    return Err(MemoryReadError::Backend(BackendError::PermissionDenied {
                        operation: "read_physical",
                        detail: detail.clone(),
                    }));
                }
                SyntheticMemoryAccess::TemporarilyUnavailable { detail } => {
                    return Err(MemoryReadError::Backend(
                        BackendError::TemporarilyUnavailable {
                            resource: "synthetic-memory-range",
                            detail: detail.clone(),
                        },
                    ));
                }
            }
        }
        Ok(())
    }

    fn copy_mapped_prefix(
        &self,
        gpa: GuestPhysical,
        buf: &mut [u8],
    ) -> Result<usize, MemoryReadError> {
        let mut cursor = gpa.0;
        let mut copied = 0;
        while copied < buf.len() {
            let Some(range) = self.range_containing(cursor) else {
                if self.partial_reads && copied > 0 {
                    return Ok(copied);
                }
                return Err(MemoryReadError::Unmapped {
                    gpa: GuestPhysical(cursor),
                    len: buf.len() - copied,
                });
            };

            match &range.access {
                SyntheticMemoryAccess::Mapped(bytes) => {
                    let offset = range_offset(range, cursor);
                    let readable = bytes.len().saturating_sub(offset).min(buf.len() - copied);
                    buf[copied..copied + readable]
                        .copy_from_slice(&bytes[offset..offset + readable]);
                    cursor = advance_guest_physical(cursor, readable)?;
                    copied += readable;
                }
                SyntheticMemoryAccess::PermissionDenied { detail } => {
                    if self.partial_reads && copied > 0 {
                        return Ok(copied);
                    }
                    return Err(MemoryReadError::Backend(BackendError::PermissionDenied {
                        operation: "read_physical",
                        detail: detail.clone(),
                    }));
                }
                SyntheticMemoryAccess::TemporarilyUnavailable { detail } => {
                    if self.partial_reads && copied > 0 {
                        return Ok(copied);
                    }
                    return Err(MemoryReadError::Backend(
                        BackendError::TemporarilyUnavailable {
                            resource: "synthetic-memory-range",
                            detail: detail.clone(),
                        },
                    ));
                }
            }
        }
        Ok(copied)
    }

    fn range_containing(&self, addr: u64) -> Option<&SyntheticMemoryRange> {
        self.ranges
            .iter()
            .find(|range| range.start <= addr && addr < range.end)
    }
}

impl GuestMemoryReader for SyntheticGuestPhysicalMemoryReader {
    fn read_physical(
        &self,
        _vm: VmId,
        gpa: GuestPhysical,
        buf: &mut [u8],
    ) -> Result<usize, MemoryReadError> {
        checked_memory_range(gpa, buf.len())?;
        if !self.partial_reads {
            self.validate_full_read(gpa, buf.len())?;
        }
        self.copy_mapped_prefix(gpa, buf)
    }
}

fn checked_memory_range(gpa: GuestPhysical, len: usize) -> Result<u64, MemoryReadError> {
    if len == 0 {
        return Err(MemoryReadError::InvalidRange { gpa, len });
    }

    let len_u64 = u64::try_from(len).map_err(|_| MemoryReadError::InvalidAddress { gpa, len })?;
    gpa.0
        .checked_add(len_u64)
        .ok_or(MemoryReadError::InvalidAddress { gpa, len })
}

fn advance_guest_physical(gpa: u64, len: usize) -> Result<u64, MemoryReadError> {
    let len_u64 = u64::try_from(len).map_err(|_| MemoryReadError::InvalidAddress {
        gpa: GuestPhysical(gpa),
        len,
    })?;
    gpa.checked_add(len_u64)
        .ok_or(MemoryReadError::InvalidAddress {
            gpa: GuestPhysical(gpa),
            len,
        })
}

fn range_offset(range: &SyntheticMemoryRange, addr: u64) -> usize {
    usize::try_from(addr - range.start).expect("synthetic memory range offset fits usize")
}

fn range_len(start: u64, end: u64) -> usize {
    usize::try_from(end - start).expect("synthetic memory range length came from usize")
}

fn ranges_overlap(left_start: u64, left_end: u64, right_start: u64, right_end: u64) -> bool {
    left_start < right_end && right_start < left_end
}

pub(crate) fn read_table_entry(
    memory: &dyn GuestMemoryReader,
    vm: VmId,
    table_base: u64,
    index: u64,
    level: &'static str,
) -> Result<u64, TranslationError> {
    let gpa = index
        .checked_mul(8)
        .and_then(|offset| table_base.checked_add(offset))
        .map(GuestPhysical)
        .ok_or_else(|| TranslationError::MalformedPageTables {
            level,
            detail: "page-table entry address overflowed".to_string(),
        })?;
    let mut bytes = [0u8; 8];
    let read = memory
        .read_physical(vm, gpa, &mut bytes)
        .map_err(|err| match err {
            MemoryReadError::Backend(err) => {
                TranslationError::Backend(err.with_context(&format!("cannot read {level}")))
            }
            MemoryReadError::Malformed { detail } => TranslationError::MalformedPageTables {
                level,
                detail: format!("memory source is malformed: {detail}"),
            },
            _ => TranslationError::MissingMemory {
                gpa,
                detail: format!("cannot read {level}: {err}"),
            },
        })?;
    if read != bytes.len() {
        return Err(TranslationError::MissingMemory {
            gpa,
            detail: format!(
                "short read while reading {level}: read {read} of {} bytes",
                bytes.len()
            ),
        });
    }
    Ok(u64::from_le_bytes(bytes))
}
