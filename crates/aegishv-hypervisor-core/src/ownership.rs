use crate::error::{CoreError, CoreErrorKind};
use crate::ids::{DeviceId, HostPhysical, VmId};

pub const OWNER_PAGE_SIZE: u64 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageOwner {
    Free,
    Reserved,
    Hypervisor,
    Vm(VmId),
    Device(DeviceId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageRange {
    pub start: HostPhysical,
    pub length: u64,
}

impl PageRange {
    pub fn new(start: HostPhysical, length: u64) -> Result<Self, CoreError> {
        if length == 0 || start.get() % OWNER_PAGE_SIZE != 0 || length % OWNER_PAGE_SIZE != 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "page ownership ranges must be non-empty and 4K aligned",
            ));
        }
        start.checked_add(length)?;
        Ok(Self { start, length })
    }

    pub fn end(self) -> Result<u64, CoreError> {
        self.start.checked_add(self.length).map(|end| end.get())
    }

    fn contains(self, address: u64) -> Result<bool, CoreError> {
        Ok(self.start.get() <= address && address < self.end()?)
    }

    fn overlaps(self, other: Self) -> Result<bool, CoreError> {
        Ok(self.start.get() < other.end()? && other.start.get() < self.end()?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageOwnership {
    pub range: PageRange,
    pub owner: PageOwner,
}

pub struct PageOwnershipTable<const N: usize> {
    entries: [Option<PageOwnership>; N],
    len: usize,
}

impl<const N: usize> PageOwnershipTable<N> {
    pub const fn new() -> Self {
        Self {
            entries: [None; N],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn entries(&self) -> impl Iterator<Item = PageOwnership> + '_ {
        self.entries[..self.len].iter().filter_map(|entry| *entry)
    }

    pub fn assign(&mut self, range: PageRange, owner: PageOwner) -> Result<(), CoreError> {
        if self.len >= N {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "page ownership table is full",
            ));
        }
        for entry in self.entries() {
            if entry.range.overlaps(range)? {
                return Err(CoreError::new(
                    CoreErrorKind::Overlap,
                    "page ownership ranges must not overlap",
                ));
            }
        }
        self.entries[self.len] = Some(PageOwnership { range, owner });
        self.len += 1;
        Ok(())
    }

    pub fn owner_at(&self, address: HostPhysical) -> Result<Option<PageOwner>, CoreError> {
        for entry in self.entries() {
            if entry.range.contains(address.get())? {
                return Ok(Some(entry.owner));
            }
        }
        Ok(None)
    }

    pub fn validate_guest_host_mapping(
        &self,
        vm_id: VmId,
        host_start: HostPhysical,
        length: u64,
    ) -> Result<(), CoreError> {
        let requested = PageRange::new(host_start, length)?;
        let end = requested.end()?;
        let mut cursor = host_start.get();
        while cursor < end {
            let mut covered_until = None;
            for entry in self.entries() {
                if entry.range.contains(cursor)? {
                    if entry.owner != PageOwner::Vm(vm_id) {
                        return Err(CoreError::new(
                            CoreErrorKind::PermissionViolation,
                            "guest mapping references a page not owned by that VM",
                        ));
                    }
                    covered_until = Some(entry.range.end()?.min(end));
                    break;
                }
            }
            match covered_until {
                Some(next) if next > cursor => cursor = next,
                _ => {
                    return Err(CoreError::new(
                        CoreErrorKind::PermissionViolation,
                        "guest mapping references unowned host pages",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl<const N: usize> Default for PageOwnershipTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_mapping_accepts_only_pages_owned_by_that_vm() {
        let vm = VmId::new(7).unwrap();
        let mut table = PageOwnershipTable::<4>::new();
        table
            .assign(
                PageRange::new(HostPhysical::new(0x4000).unwrap(), 0x2000).unwrap(),
                PageOwner::Vm(vm),
            )
            .unwrap();

        table
            .validate_guest_host_mapping(vm, HostPhysical::new(0x4000).unwrap(), 0x2000)
            .unwrap();
    }

    #[test]
    fn guest_mapping_rejects_hypervisor_owned_pages() {
        let vm = VmId::new(7).unwrap();
        let mut table = PageOwnershipTable::<4>::new();
        table
            .assign(
                PageRange::new(HostPhysical::new(0x8000).unwrap(), 0x1000).unwrap(),
                PageOwner::Hypervisor,
            )
            .unwrap();

        let err = table
            .validate_guest_host_mapping(vm, HostPhysical::new(0x8000).unwrap(), 0x1000)
            .unwrap_err();

        assert_eq!(err.kind, CoreErrorKind::PermissionViolation);
    }

    #[test]
    fn ownership_table_rejects_overlap_and_unaligned_ranges() {
        let mut table = PageOwnershipTable::<2>::new();
        table
            .assign(
                PageRange::new(HostPhysical::new(0x1000).unwrap(), 0x2000).unwrap(),
                PageOwner::Reserved,
            )
            .unwrap();

        assert_eq!(
            table
                .assign(
                    PageRange::new(HostPhysical::new(0x2000).unwrap(), 0x1000).unwrap(),
                    PageOwner::Free,
                )
                .unwrap_err()
                .kind,
            CoreErrorKind::Overlap
        );
        assert_eq!(
            PageRange::new(HostPhysical::new(0x1234).unwrap(), 0x1000)
                .unwrap_err()
                .kind,
            CoreErrorKind::InvalidAddress
        );
    }
}
