use aegishv_hypervisor_core::ids::{GuestPhysical, HostPhysical};

use super::features::{VmxError, VmxErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EptPageSize {
    Size4K,
    Size2M,
}

impl EptPageSize {
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
pub enum EptMemoryType {
    Uncacheable,
    WriteBack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EptPermissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl EptPermissions {
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

    pub const fn allows(self, access: EptAccess) -> bool {
        match access {
            EptAccess::Read => self.read,
            EptAccess::Write => self.write,
            EptAccess::Execute => self.execute,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EptAccess {
    Read,
    Write,
    Execute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EptMapping {
    pub guest_physical: GuestPhysical,
    pub host_physical: HostPhysical,
    pub page_size: EptPageSize,
    pub permissions: EptPermissions,
    pub memory_type: EptMemoryType,
}

impl EptMapping {
    pub fn new(
        guest_physical: GuestPhysical,
        host_physical: HostPhysical,
        page_size: EptPageSize,
        permissions: EptPermissions,
        memory_type: EptMemoryType,
    ) -> Result<Self, VmxError> {
        let bytes = page_size.bytes();
        if guest_physical.get() % bytes != 0 || host_physical.get() % bytes != 0 {
            return Err(VmxError::new(
                VmxErrorKind::InvalidEptMapping,
                "EPT mapping addresses must be aligned to the selected page size",
            ));
        }
        Ok(Self {
            guest_physical,
            host_physical,
            page_size,
            permissions,
            memory_type,
        })
    }

    pub const fn end(self) -> u64 {
        self.guest_physical.get() + self.page_size.bytes()
    }

    pub const fn contains(self, gpa: GuestPhysical) -> bool {
        self.guest_physical.get() <= gpa.get() && gpa.get() < self.end()
    }
}

pub struct EptMapPlan<const N: usize> {
    mappings: [Option<EptMapping>; N],
    len: usize,
}

impl<const N: usize> EptMapPlan<N> {
    pub const fn new() -> Self {
        Self {
            mappings: [None; N],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn mappings(&self) -> impl Iterator<Item = EptMapping> + '_ {
        self.mappings[..self.len]
            .iter()
            .filter_map(|mapping| *mapping)
    }

    pub fn map(&mut self, mapping: EptMapping) -> Result<(), VmxError> {
        if self.len >= N {
            return Err(VmxError::new(
                VmxErrorKind::InvalidEptMapping,
                "EPT map plan is full",
            ));
        }
        self.reject_overlap(mapping)?;
        self.mappings[self.len] = Some(mapping);
        self.len += 1;
        Ok(())
    }

    pub fn lookup(&self, gpa: GuestPhysical) -> Option<EptMapping> {
        self.mappings().find(|mapping| mapping.contains(gpa))
    }

    pub fn set_permissions(
        &mut self,
        gpa: GuestPhysical,
        permissions: EptPermissions,
    ) -> Result<EptPermissions, VmxError> {
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
        Err(VmxError::new(
            VmxErrorKind::InvalidEptMapping,
            "EPT permission update did not find a covering mapping",
        ))
    }

    fn reject_overlap(&self, candidate: EptMapping) -> Result<(), VmxError> {
        for existing in self.mappings() {
            if candidate.guest_physical.get() < existing.end()
                && existing.guest_physical.get() < candidate.end()
            {
                return Err(VmxError::new(
                    VmxErrorKind::InvalidEptMapping,
                    "EPT mapping overlaps an existing guest physical range",
                ));
            }
        }
        Ok(())
    }
}

impl<const N: usize> Default for EptMapPlan<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EptViolationQualification {
    raw: u64,
}

impl EptViolationQualification {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    pub const fn for_access(access: EptAccess) -> Self {
        let bit = match access {
            EptAccess::Read => 1,
            EptAccess::Write => 1 << 1,
            EptAccess::Execute => 1 << 2,
        };
        Self { raw: bit }
    }

    pub const fn access(self) -> Result<EptAccess, VmxError> {
        let access_bits = self.raw & 0x7;
        match access_bits {
            1 => Ok(EptAccess::Read),
            2 => Ok(EptAccess::Write),
            4 => Ok(EptAccess::Execute),
            _ => Err(VmxError::new(
                VmxErrorKind::InvalidEptViolation,
                "EPT violation qualification must describe one access type",
            )),
        }
    }

    pub const fn guest_linear_address_valid(self) -> bool {
        self.raw & (1 << 7) != 0
    }

    pub const fn caused_by_guest_page_walk(self) -> bool {
        self.raw & (1 << 8) != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EptViolationAction {
    UnexpectedAccess,
    MissingMapping,
    ResumeAfterPermissionChange,
}

pub fn handle_ept_violation<const N: usize>(
    plan: &EptMapPlan<N>,
    gpa: GuestPhysical,
    qualification: EptViolationQualification,
) -> Result<EptViolationAction, VmxError> {
    let access = qualification.access()?;
    let mapping = match plan.lookup(gpa) {
        Some(mapping) => mapping,
        None => return Ok(EptViolationAction::MissingMapping),
    };
    if mapping.permissions.allows(access) {
        Ok(EptViolationAction::UnexpectedAccess)
    } else {
        Ok(EptViolationAction::ResumeAfterPermissionChange)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vpid(u16);

impl Vpid {
    pub const fn new(raw: u16) -> Result<Self, VmxError> {
        if raw == 0 {
            Err(VmxError::new(
                VmxErrorKind::InvalidControlBits,
                "VPID 0 is reserved by VMX",
            ))
        } else {
            Ok(Self(raw))
        }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InveptKind {
    SingleContext,
    AllContexts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InveptDescriptor {
    pub ept_pointer: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VpidFlushKind {
    IndividualAddress,
    SingleContext,
    AllContexts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmxInvalidationPlan {
    pub invept: Option<(InveptKind, InveptDescriptor)>,
    pub vpid: Option<(VpidFlushKind, Vpid, GuestPhysical)>,
}

impl VmxInvalidationPlan {
    pub const fn ept_single_context(ept_pointer: u64) -> Self {
        Self {
            invept: Some((InveptKind::SingleContext, InveptDescriptor { ept_pointer })),
            vpid: None,
        }
    }

    pub const fn vpid_individual(vpid: Vpid, guest_physical: GuestPhysical) -> Self {
        Self {
            invept: None,
            vpid: Some((VpidFlushKind::IndividualAddress, vpid, guest_physical)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(gpa: u64, perms: EptPermissions) -> EptMapping {
        EptMapping::new(
            GuestPhysical::new(gpa).unwrap(),
            HostPhysical::new(gpa + 0x100000).unwrap(),
            EptPageSize::Size4K,
            perms,
            EptMemoryType::WriteBack,
        )
        .unwrap()
    }

    #[test]
    fn ept_map_plan_rejects_overlaps() {
        let mut plan = EptMapPlan::<2>::new();
        plan.map(mapping(0x2000, EptPermissions::READ)).unwrap();

        assert_eq!(
            plan.map(mapping(0x2000, EptPermissions::READ))
                .unwrap_err()
                .kind,
            VmxErrorKind::InvalidEptMapping
        );
    }

    #[test]
    fn ept_violation_reports_permission_trap_when_access_is_blocked() {
        let mut plan = EptMapPlan::<2>::new();
        plan.map(mapping(0x4000, EptPermissions::READ)).unwrap();

        let action = handle_ept_violation(
            &plan,
            GuestPhysical::new(0x4000).unwrap(),
            EptViolationQualification::for_access(EptAccess::Execute),
        )
        .unwrap();

        assert_eq!(action, EptViolationAction::ResumeAfterPermissionChange);
    }

    #[test]
    fn ept_violation_rejects_ambiguous_access_bits() {
        let plan = EptMapPlan::<1>::new();
        let err = handle_ept_violation(
            &plan,
            GuestPhysical::new(0).unwrap(),
            EptViolationQualification::new(0b11 | (1 << 7)),
        )
        .unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::InvalidEptViolation);
    }

    #[test]
    fn ept_violation_does_not_require_a_guest_linear_address() {
        let mut plan = EptMapPlan::<1>::new();
        plan.map(mapping(0x4000, EptPermissions::READ)).unwrap();

        let qualification = EptViolationQualification::for_access(EptAccess::Execute);
        assert!(!qualification.guest_linear_address_valid());
        assert_eq!(
            handle_ept_violation(&plan, GuestPhysical::new(0x4000).unwrap(), qualification,)
                .unwrap(),
            EptViolationAction::ResumeAfterPermissionChange
        );
    }

    #[test]
    fn vpid_zero_is_rejected() {
        assert_eq!(
            Vpid::new(0).unwrap_err().kind,
            VmxErrorKind::InvalidControlBits
        );
    }
}
