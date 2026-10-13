use aegishv_hypervisor_core::error::{CoreError, CoreErrorKind};
use aegishv_hypervisor_core::ids::HostPhysical;

pub const PAGE_SIZE_4K: u64 = 4096;
pub const PAGE_SIZE_2M: u64 = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapMode {
    Identity,
    DirectOffset(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PagePermissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub global: bool,
}

impl PagePermissions {
    pub const READ_ONLY: Self = Self {
        read: true,
        write: false,
        execute: false,
        global: false,
    };

    pub const READ_WRITE: Self = Self {
        read: true,
        write: true,
        execute: false,
        global: false,
    };

    pub const READ_EXECUTE: Self = Self {
        read: true,
        write: false,
        execute: true,
        global: true,
    };

    pub const fn is_wx(self) -> bool {
        self.write && self.execute
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MappingDescriptor {
    pub virtual_address: u64,
    pub physical_address: HostPhysical,
    pub length: u64,
    pub permissions: PagePermissions,
}

impl MappingDescriptor {
    const fn empty() -> Self {
        Self {
            virtual_address: 0,
            physical_address: HostPhysical::ZERO,
            length: 0,
            permissions: PagePermissions::READ_ONLY,
        }
    }
}

pub struct PageTablePlan<const N: usize> {
    mappings: [MappingDescriptor; N],
    len: usize,
}

impl<const N: usize> PageTablePlan<N> {
    pub const fn new() -> Self {
        Self {
            mappings: [MappingDescriptor::empty(); N],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn mappings(&self) -> &[MappingDescriptor] {
        &self.mappings[..self.len]
    }

    pub fn map_region(
        &mut self,
        mode: MapMode,
        physical_address: HostPhysical,
        length: u64,
        permissions: PagePermissions,
    ) -> Result<(), CoreError> {
        if self.len >= N {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "page-table mapping plan is full",
            ));
        }
        if length == 0 || physical_address.get() % PAGE_SIZE_4K != 0 || length % PAGE_SIZE_4K != 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "page-table mapping requires non-empty 4K-aligned physical range",
            ));
        }
        if permissions.is_wx() {
            return Err(CoreError::new(
                CoreErrorKind::PermissionViolation,
                "host page-table mapping would be writable and executable",
            ));
        }

        let virtual_address = match mode {
            MapMode::Identity => physical_address.get(),
            MapMode::DirectOffset(offset) => {
                physical_address
                    .get()
                    .checked_add(offset)
                    .ok_or(CoreError::new(
                        CoreErrorKind::InvalidAddress,
                        "direct-map virtual address overflowed",
                    ))?
            }
        };

        self.mappings[self.len] = MappingDescriptor {
            virtual_address,
            physical_address,
            length,
            permissions,
        };
        self.len += 1;
        Ok(())
    }
}

impl<const N: usize> Default for PageTablePlan<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_mapping_uses_physical_address_as_virtual_address() {
        let mut plan = PageTablePlan::<2>::new();

        plan.map_region(
            MapMode::Identity,
            HostPhysical::new(0x2000).unwrap(),
            PAGE_SIZE_4K,
            PagePermissions::READ_EXECUTE,
        )
        .unwrap();

        assert_eq!(plan.mappings()[0].virtual_address, 0x2000);
        assert_eq!(
            plan.mappings()[0].permissions,
            PagePermissions::READ_EXECUTE
        );
    }

    #[test]
    fn direct_mapping_applies_checked_offset() {
        let mut plan = PageTablePlan::<2>::new();

        plan.map_region(
            MapMode::DirectOffset(0xffff_8000_0000_0000),
            HostPhysical::new(0x4000).unwrap(),
            PAGE_SIZE_4K,
            PagePermissions::READ_WRITE,
        )
        .unwrap();

        assert_eq!(plan.mappings()[0].virtual_address, 0xffff_8000_0000_4000);
    }

    #[test]
    fn page_table_plan_rejects_writable_executable_mapping() {
        let mut plan = PageTablePlan::<2>::new();
        let wx = PagePermissions {
            read: true,
            write: true,
            execute: true,
            global: false,
        };

        assert_eq!(
            plan.map_region(
                MapMode::Identity,
                HostPhysical::new(0x1000).unwrap(),
                PAGE_SIZE_4K,
                wx,
            )
            .unwrap_err()
            .kind,
            CoreErrorKind::PermissionViolation
        );
    }
}
