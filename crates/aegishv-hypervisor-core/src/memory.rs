use crate::error::{CoreError, CoreErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryRegionKind {
    Usable,
    Reserved,
    Mmio,
    Acpi,
    Bad,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryRegion {
    pub base: u64,
    pub length: u64,
    pub kind: MemoryRegionKind,
}

impl MemoryRegion {
    pub const fn new(base: u64, length: u64, kind: MemoryRegionKind) -> Self {
        Self { base, length, kind }
    }

    pub const fn empty() -> Self {
        Self {
            base: 0,
            length: 0,
            kind: MemoryRegionKind::Reserved,
        }
    }

    pub fn end(self) -> Result<u64, CoreError> {
        self.base.checked_add(self.length).ok_or(CoreError::new(
            CoreErrorKind::InvalidMemoryMap,
            "memory region end address overflowed",
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryMap<const N: usize> {
    regions: [MemoryRegion; N],
    len: usize,
}

impl<const N: usize> MemoryMap<N> {
    pub fn from_entries(entries: &[MemoryRegion]) -> Result<Self, CoreError> {
        if entries.len() > N {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "firmware memory map has more entries than the fixed table can hold",
            ));
        }

        let mut sorted = [MemoryRegion::empty(); N];
        let mut len = 0;
        for &entry in entries {
            validate_region(entry)?;
            insert_sorted(&mut sorted, &mut len, entry)?;
        }

        let mut merged = [MemoryRegion::empty(); N];
        let mut merged_len = 0;
        for region in sorted.iter().take(len).copied() {
            merge_region(&mut merged, &mut merged_len, region)?;
        }

        Ok(Self {
            regions: merged,
            len: merged_len,
        })
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn regions(&self) -> &[MemoryRegion] {
        &self.regions[..self.len]
    }

    pub fn usable_regions(&self) -> impl Iterator<Item = MemoryRegion> + '_ {
        self.regions()
            .iter()
            .copied()
            .filter(|region| region.kind == MemoryRegionKind::Usable)
    }
}

fn validate_region(region: MemoryRegion) -> Result<(), CoreError> {
    if region.length == 0 {
        return Err(CoreError::new(
            CoreErrorKind::InvalidMemoryMap,
            "firmware memory map contains a zero-length region",
        ));
    }
    region.end()?;
    Ok(())
}

fn insert_sorted<const N: usize>(
    regions: &mut [MemoryRegion; N],
    len: &mut usize,
    region: MemoryRegion,
) -> Result<(), CoreError> {
    if *len >= N {
        return Err(CoreError::new(
            CoreErrorKind::CapacityExceeded,
            "firmware memory map sort table is full",
        ));
    }

    let mut index = *len;
    while index > 0 && regions[index - 1].base > region.base {
        regions[index] = regions[index - 1];
        index -= 1;
    }
    regions[index] = region;
    *len += 1;
    Ok(())
}

fn merge_region<const N: usize>(
    regions: &mut [MemoryRegion; N],
    len: &mut usize,
    region: MemoryRegion,
) -> Result<(), CoreError> {
    if *len == 0 {
        regions[0] = region;
        *len = 1;
        return Ok(());
    }

    let last = regions[*len - 1];
    let last_end = last.end()?;
    if region.base < last_end && region.kind != last.kind {
        return Err(CoreError::new(
            CoreErrorKind::Overlap,
            "firmware memory map has overlapping regions with different types",
        ));
    }

    if region.base <= last_end && region.kind == last.kind {
        let region_end = region.end()?;
        let merged_end = if region_end > last_end {
            region_end
        } else {
            last_end
        };
        regions[*len - 1].length = merged_end - last.base;
        return Ok(());
    }

    if *len >= N {
        return Err(CoreError::new(
            CoreErrorKind::CapacityExceeded,
            "firmware memory map merge table is full",
        ));
    }
    regions[*len] = region;
    *len += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_map_sorts_and_merges_adjacent_regions() {
        let entries = [
            MemoryRegion::new(0x3000, 0x1000, MemoryRegionKind::Reserved),
            MemoryRegion::new(0x1000, 0x1000, MemoryRegionKind::Usable),
            MemoryRegion::new(0x2000, 0x1000, MemoryRegionKind::Usable),
        ];

        let map = MemoryMap::<4>::from_entries(&entries).unwrap();

        assert_eq!(
            map.regions(),
            &[
                MemoryRegion::new(0x1000, 0x2000, MemoryRegionKind::Usable),
                MemoryRegion::new(0x3000, 0x1000, MemoryRegionKind::Reserved),
            ]
        );
    }

    #[test]
    fn memory_map_rejects_overlapping_different_region_types() {
        let entries = [
            MemoryRegion::new(0x1000, 0x3000, MemoryRegionKind::Usable),
            MemoryRegion::new(0x2000, 0x1000, MemoryRegionKind::Mmio),
        ];

        assert_eq!(
            MemoryMap::<4>::from_entries(&entries).unwrap_err().kind,
            CoreErrorKind::Overlap
        );
    }

    #[test]
    fn memory_map_rejects_malformed_region() {
        let entries = [MemoryRegion::new(0x1000, 0, MemoryRegionKind::Bad)];

        assert_eq!(
            MemoryMap::<4>::from_entries(&entries).unwrap_err().kind,
            CoreErrorKind::InvalidMemoryMap
        );
    }
}
