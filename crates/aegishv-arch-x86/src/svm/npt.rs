use aegishv_hypervisor_core::ids::{GuestPhysical, HostPhysical};

use super::features::{SvmError, SvmErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NptPageSize {
    Size4K,
    Size2M,
}

impl NptPageSize {
    pub const fn bytes(self) -> u64 {
        match self {
            Self::Size4K => 4096,
            Self::Size2M => 2 * 1024 * 1024,
        }
    }

    pub const fn align_down(self, value: u64) -> u64 {
        value - (value % self.bytes())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NptPermissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl NptPermissions {
    pub const NONE: Self = Self {
        read: false,
        write: false,
        execute: false,
    };
    pub const READ: Self = Self {
        read: true,
        write: false,
        execute: false,
    };
    pub const READ_WRITE: Self = Self {
        read: true,
        write: true,
        execute: false,
    };
    pub const READ_EXECUTE: Self = Self {
        read: true,
        write: false,
        execute: true,
    };
    pub const READ_WRITE_EXECUTE: Self = Self {
        read: true,
        write: true,
        execute: true,
    };

    pub const fn without_execute(self) -> Self {
        Self {
            execute: false,
            ..self
        }
    }

    pub const fn without_write(self) -> Self {
        Self {
            write: false,
            ..self
        }
    }

    pub const fn with_write(self) -> Self {
        Self {
            write: true,
            ..self
        }
    }

    pub const fn allows(self, access: NptAccess) -> bool {
        match access {
            NptAccess::Read => self.read,
            NptAccess::Write => self.write,
            NptAccess::Execute => self.execute,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NptAccess {
    Read,
    Write,
    Execute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NptMapping {
    pub guest_physical: GuestPhysical,
    pub host_physical: HostPhysical,
    pub page_size: NptPageSize,
    pub permissions: NptPermissions,
}

impl NptMapping {
    pub fn new(
        guest_physical: GuestPhysical,
        host_physical: HostPhysical,
        page_size: NptPageSize,
        permissions: NptPermissions,
    ) -> Result<Self, SvmError> {
        let bytes = page_size.bytes();
        if guest_physical.get() % bytes != 0 || host_physical.get() % bytes != 0 {
            return Err(SvmError::new(
                SvmErrorKind::InvalidNptMapping,
                "NPT mapping addresses must be aligned to the selected page size",
            ));
        }
        Ok(Self {
            guest_physical,
            host_physical,
            page_size,
            permissions,
        })
    }

    pub const fn end(self) -> u64 {
        self.guest_physical.get() + self.page_size.bytes()
    }

    pub const fn host_end(self) -> u64 {
        self.host_physical.get() + self.page_size.bytes()
    }

    pub const fn contains(self, gpa: GuestPhysical) -> bool {
        self.guest_physical.get() <= gpa.get() && gpa.get() < self.end()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtectedRange {
    pub start: HostPhysical,
    pub length: u64,
}

impl ProtectedRange {
    pub fn new(start: HostPhysical, length: u64) -> Result<Self, SvmError> {
        if length == 0 {
            return Err(SvmError::new(
                SvmErrorKind::InvalidNptMapping,
                "protected hypervisor range must not be empty",
            ));
        }
        Ok(Self { start, length })
    }

    const fn end(self) -> u64 {
        self.start.get() + self.length
    }

    const fn overlaps_mapping(self, mapping: NptMapping) -> bool {
        mapping.host_physical.get() < self.end() && self.start.get() < mapping.host_end()
    }
}

pub struct NptMapPlan<const N: usize, const P: usize> {
    mappings: [Option<NptMapping>; N],
    protected: [Option<ProtectedRange>; P],
    len: usize,
    protected_len: usize,
}

impl<const N: usize, const P: usize> NptMapPlan<N, P> {
    pub const fn new() -> Self {
        Self {
            mappings: [None; N],
            protected: [None; P],
            len: 0,
            protected_len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn mappings(&self) -> impl Iterator<Item = NptMapping> + '_ {
        self.mappings[..self.len]
            .iter()
            .filter_map(|mapping| *mapping)
    }

    pub fn protect_hypervisor_range(&mut self, range: ProtectedRange) -> Result<(), SvmError> {
        if self.protected_len >= P {
            return Err(SvmError::new(
                SvmErrorKind::InvalidNptMapping,
                "NPT protected range plan is full",
            ));
        }
        self.protected[self.protected_len] = Some(range);
        self.protected_len += 1;
        Ok(())
    }

    pub fn map(&mut self, mapping: NptMapping) -> Result<(), SvmError> {
        if self.len >= N {
            return Err(SvmError::new(
                SvmErrorKind::InvalidNptMapping,
                "NPT map plan is full",
            ));
        }
        self.reject_overlap(mapping)?;
        self.reject_hypervisor_overlap(mapping)?;
        self.mappings[self.len] = Some(mapping);
        self.len += 1;
        Ok(())
    }

    pub fn lookup(&self, gpa: GuestPhysical) -> Option<NptMapping> {
        self.mappings().find(|mapping| mapping.contains(gpa))
    }

    pub fn set_permissions(
        &mut self,
        gpa: GuestPhysical,
        permissions: NptPermissions,
    ) -> Result<NptPermissions, SvmError> {
        for mapping in &mut self.mappings[..self.len] {
            let Some(mapping) = mapping.as_mut() else {
                continue;
            };
            if mapping.contains(gpa) {
                let previous = mapping.permissions;
                mapping.permissions = permissions;
                return Ok(previous);
            }
        }
        Err(SvmError::new(
            SvmErrorKind::InvalidNptMapping,
            "NPT permission update did not find a covering mapping",
        ))
    }

    fn reject_overlap(&self, candidate: NptMapping) -> Result<(), SvmError> {
        for existing in self.mappings() {
            if candidate.guest_physical.get() < existing.end()
                && existing.guest_physical.get() < candidate.end()
            {
                return Err(SvmError::new(
                    SvmErrorKind::InvalidNptMapping,
                    "NPT mapping overlaps an existing guest physical range",
                ));
            }
        }
        Ok(())
    }

    fn reject_hypervisor_overlap(&self, candidate: NptMapping) -> Result<(), SvmError> {
        for protected in &self.protected[..self.protected_len] {
            if let Some(range) = protected {
                if range.overlaps_mapping(candidate) {
                    return Err(SvmError::new(
                        SvmErrorKind::InvalidNptMapping,
                        "NPT mapping would expose protected hypervisor memory",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl<const N: usize, const P: usize> Default for NptMapPlan<N, P> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NestedPageFault {
    pub access: NptAccess,
    pub guest_physical: GuestPhysical,
    pub present: bool,
    pub reserved_bits: bool,
}

impl NestedPageFault {
    pub const fn decode(error_code: u64, guest_physical: GuestPhysical) -> Result<Self, SvmError> {
        let access = if error_code & (1 << 4) != 0 {
            NptAccess::Execute
        } else if error_code & (1 << 1) != 0 {
            NptAccess::Write
        } else {
            NptAccess::Read
        };
        Ok(Self {
            access,
            guest_physical,
            present: error_code & 1 != 0,
            reserved_bits: error_code & (1 << 3) != 0,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NestedPageFaultAction {
    MissingMapping,
    PermissionTrap,
    UnexpectedAccess,
    ReservedBitFault,
}

pub fn handle_nested_page_fault<const N: usize, const P: usize>(
    plan: &NptMapPlan<N, P>,
    fault: NestedPageFault,
) -> Result<NestedPageFaultAction, SvmError> {
    if fault.reserved_bits {
        return Ok(NestedPageFaultAction::ReservedBitFault);
    }
    let Some(mapping) = plan.lookup(fault.guest_physical) else {
        return Ok(NestedPageFaultAction::MissingMapping);
    };
    if mapping.permissions.allows(fault.access) {
        Ok(NestedPageFaultAction::UnexpectedAccess)
    } else {
        Ok(NestedPageFaultAction::PermissionTrap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(gpa: u64, hpa: u64, permissions: NptPermissions) -> NptMapping {
        NptMapping::new(
            GuestPhysical::new(gpa).unwrap(),
            HostPhysical::new(hpa).unwrap(),
            NptPageSize::Size4K,
            permissions,
        )
        .unwrap()
    }

    #[test]
    fn npt_plan_rejects_mapping_over_protected_hypervisor_memory() {
        let mut plan = NptMapPlan::<2, 1>::new();
        plan.protect_hypervisor_range(
            ProtectedRange::new(HostPhysical::new(0x100000).unwrap(), 0x2000).unwrap(),
        )
        .unwrap();

        let err = plan
            .map(mapping(0x4000, 0x100000, NptPermissions::READ))
            .unwrap_err();

        assert_eq!(err.kind, SvmErrorKind::InvalidNptMapping);
    }

    #[test]
    fn npt_plan_updates_permissions_for_covering_mapping() {
        let mut plan = NptMapPlan::<2, 0>::new();
        plan.map(mapping(
            0x8000,
            0x200000,
            NptPermissions::READ_WRITE_EXECUTE,
        ))
        .unwrap();

        let previous = plan
            .set_permissions(GuestPhysical::new(0x8000).unwrap(), NptPermissions::READ)
            .unwrap();

        assert_eq!(previous, NptPermissions::READ_WRITE_EXECUTE);
        assert_eq!(
            plan.lookup(GuestPhysical::new(0x8000).unwrap())
                .unwrap()
                .permissions,
            NptPermissions::READ
        );
    }

    #[test]
    fn nested_page_fault_reports_permission_trap() {
        let mut plan = NptMapPlan::<2, 0>::new();
        plan.map(mapping(0x9000, 0x300000, NptPermissions::READ))
            .unwrap();
        let fault = NestedPageFault::decode(1 << 4, GuestPhysical::new(0x9000).unwrap()).unwrap();

        assert_eq!(
            handle_nested_page_fault(&plan, fault).unwrap(),
            NestedPageFaultAction::PermissionTrap
        );
    }

    #[test]
    fn nested_page_fault_reports_reserved_bit_fault_separately() {
        let plan = NptMapPlan::<1, 0>::new();
        let fault = NestedPageFault::decode(1 << 3, GuestPhysical::new(0).unwrap()).unwrap();

        assert_eq!(
            handle_nested_page_fault(&plan, fault).unwrap(),
            NestedPageFaultAction::ReservedBitFault
        );
    }
}
