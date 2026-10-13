use aegishv_hypervisor_core::ids::{GuestPhysical, HostPhysical};

use super::features::{VmxError, VmxErrorKind};

pub const EPT_VPID_CAP_EXECUTE_ONLY: u64 = 1 << 0;
pub const EPT_VPID_CAP_PAGE_WALK_LENGTH_4: u64 = 1 << 6;
pub const EPT_VPID_CAP_MEMORY_TYPE_UC: u64 = 1 << 8;
pub const EPT_VPID_CAP_MEMORY_TYPE_WB: u64 = 1 << 14;

const EPT_PAGE_ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const EPT_READ: u64 = 1 << 0;
const EPT_WRITE: u64 = 1 << 1;
const EPT_EXECUTE: u64 = 1 << 2;
const EPT_MEMORY_TYPE_SHIFT: u32 = 3;
const EPTP_PAGE_WALK_LENGTH_4: u64 = 3 << 3;
const EPT_PAGE_SIZE_4K: u64 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EptCapabilities {
    raw: u64,
}

impl EptCapabilities {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    pub const fn raw(self) -> u64 {
        self.raw
    }

    pub const fn supports_4_level_walk(self) -> bool {
        self.raw & EPT_VPID_CAP_PAGE_WALK_LENGTH_4 != 0
    }

    pub const fn supports_execute_only(self) -> bool {
        self.raw & EPT_VPID_CAP_EXECUTE_ONLY != 0
    }

    pub const fn supports_memory_type(self, memory_type: EptMemoryType) -> bool {
        let capability = match memory_type {
            EptMemoryType::Uncacheable => EPT_VPID_CAP_MEMORY_TYPE_UC,
            EptMemoryType::WriteBack => EPT_VPID_CAP_MEMORY_TYPE_WB,
        };
        self.raw & capability != 0
    }

    pub const fn validate_4_level_write_back(self) -> Result<(), VmxError> {
        if !self.supports_4_level_walk() {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "CPU does not support a four-level EPT page walk",
            ));
        }
        if !self.supports_memory_type(EptMemoryType::WriteBack) {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "CPU does not support the write-back EPT memory type",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EptPointer {
    root: HostPhysical,
    raw: u64,
}

impl EptPointer {
    pub fn new(root: HostPhysical, capabilities: EptCapabilities) -> Result<Self, VmxError> {
        capabilities.validate_4_level_write_back()?;
        let root_raw = validate_ept_page_address(root)?;
        let raw = root_raw | EptMemoryType::WriteBack.eptp_encoding() | EPTP_PAGE_WALK_LENGTH_4;
        Ok(Self { root, raw })
    }

    pub const fn root(self) -> HostPhysical {
        self.root
    }

    pub const fn raw(self) -> u64 {
        self.raw
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EptTableEntry {
    raw: u64,
}

impl EptTableEntry {
    pub fn new(next_level: HostPhysical) -> Result<Self, VmxError> {
        let address = validate_ept_page_address(next_level)?;
        Ok(Self {
            raw: address | EPT_READ | EPT_WRITE | EPT_EXECUTE,
        })
    }

    pub const fn raw(self) -> u64 {
        self.raw
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EptLeafEntry4K {
    raw: u64,
}

impl EptLeafEntry4K {
    pub fn new(
        host_physical: HostPhysical,
        permissions: EptPermissions,
        memory_type: EptMemoryType,
        capabilities: EptCapabilities,
    ) -> Result<Self, VmxError> {
        let address = validate_ept_page_address(host_physical)?;
        let permission_bits = encode_ept_permissions(permissions, capabilities)?;
        if !capabilities.supports_memory_type(memory_type) {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedCapability,
                "CPU does not support the requested EPT leaf memory type",
            ));
        }
        Ok(Self {
            raw: address | permission_bits | memory_type.leaf_encoding(),
        })
    }

    pub const fn raw(self) -> u64 {
        self.raw
    }
}

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

impl EptMemoryType {
    const fn raw(self) -> u64 {
        match self {
            Self::Uncacheable => 0,
            Self::WriteBack => 6,
        }
    }

    const fn eptp_encoding(self) -> u64 {
        self.raw()
    }

    const fn leaf_encoding(self) -> u64 {
        self.raw() << EPT_MEMORY_TYPE_SHIFT
    }
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

const fn validate_ept_page_address(address: HostPhysical) -> Result<u64, VmxError> {
    let raw = address.get();
    if raw % EPT_PAGE_SIZE_4K != 0 {
        return Err(VmxError::new(
            VmxErrorKind::InvalidEptMapping,
            "EPT page address must be 4K aligned",
        ));
    }
    if raw & !EPT_PAGE_ADDRESS_MASK != 0 {
        return Err(VmxError::new(
            VmxErrorKind::InvalidEptMapping,
            "EPT page address sets bits outside the architectural address field",
        ));
    }
    Ok(raw)
}

const fn encode_ept_permissions(
    permissions: EptPermissions,
    capabilities: EptCapabilities,
) -> Result<u64, VmxError> {
    if !permissions.read && !permissions.write && !permissions.execute {
        return Err(VmxError::new(
            VmxErrorKind::InvalidEptMapping,
            "EPT 4K leaf must grant at least one permission",
        ));
    }
    if permissions.write && !permissions.read {
        return Err(VmxError::new(
            VmxErrorKind::InvalidEptMapping,
            "EPT write permission requires read permission",
        ));
    }
    if permissions.execute && !permissions.read && !capabilities.supports_execute_only() {
        return Err(VmxError::new(
            VmxErrorKind::UnsupportedCapability,
            "CPU does not support execute-only EPT translations",
        ));
    }
    let mut raw = 0;
    if permissions.read {
        raw |= EPT_READ;
    }
    if permissions.write {
        raw |= EPT_WRITE;
    }
    if permissions.execute {
        raw |= EPT_EXECUTE;
    }
    Ok(raw)
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

    fn ept_capabilities() -> EptCapabilities {
        EptCapabilities::new(
            EPT_VPID_CAP_PAGE_WALK_LENGTH_4
                | EPT_VPID_CAP_MEMORY_TYPE_UC
                | EPT_VPID_CAP_MEMORY_TYPE_WB,
        )
    }

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
    fn ept_capabilities_require_four_levels_and_write_back() {
        ept_capabilities().validate_4_level_write_back().unwrap();

        let missing_walk = EptCapabilities::new(EPT_VPID_CAP_MEMORY_TYPE_WB)
            .validate_4_level_write_back()
            .unwrap_err();
        assert_eq!(missing_walk.kind, VmxErrorKind::UnsupportedCapability);

        let missing_write_back = EptCapabilities::new(EPT_VPID_CAP_PAGE_WALK_LENGTH_4)
            .validate_4_level_write_back()
            .unwrap_err();
        assert_eq!(missing_write_back.kind, VmxErrorKind::UnsupportedCapability);
    }

    #[test]
    fn ept_pointer_encodes_write_back_four_level_walk() {
        let root = HostPhysical::new(0x1234_5000).unwrap();
        let pointer = EptPointer::new(root, ept_capabilities()).unwrap();

        assert_eq!(pointer.root(), root);
        assert_eq!(pointer.raw(), 0x1234_501e);
    }

    #[test]
    fn ept_pointer_rejects_misaligned_and_out_of_field_roots() {
        assert_eq!(
            EptPointer::new(HostPhysical::new(0x1234_5001).unwrap(), ept_capabilities(),)
                .unwrap_err()
                .kind,
            VmxErrorKind::InvalidEptMapping
        );
        assert_eq!(
            EptPointer::new(HostPhysical::new(1 << 52).unwrap(), ept_capabilities(),)
                .unwrap_err()
                .kind,
            VmxErrorKind::InvalidEptMapping
        );
    }

    #[test]
    fn ept_table_entry_encodes_rwx_next_level_pointer() {
        let entry = EptTableEntry::new(HostPhysical::new(0x2345_6000).unwrap()).unwrap();

        assert_eq!(entry.raw(), 0x2345_6007);
    }

    #[test]
    fn ept_4k_leaf_encodes_permissions_and_memory_type() {
        let host = HostPhysical::new(0x3456_7000).unwrap();

        assert_eq!(
            EptLeafEntry4K::new(
                host,
                EptPermissions::READ_WRITE,
                EptMemoryType::WriteBack,
                ept_capabilities(),
            )
            .unwrap()
            .raw(),
            0x3456_7033
        );
        assert_eq!(
            EptLeafEntry4K::new(
                host,
                EptPermissions::READ_EXECUTE,
                EptMemoryType::WriteBack,
                ept_capabilities(),
            )
            .unwrap()
            .raw(),
            0x3456_7035
        );
        assert_eq!(
            EptLeafEntry4K::new(
                host,
                EptPermissions::READ_WRITE_EXECUTE,
                EptMemoryType::WriteBack,
                ept_capabilities(),
            )
            .unwrap()
            .raw(),
            0x3456_7037
        );
        assert_eq!(
            EptLeafEntry4K::new(
                host,
                EptPermissions::READ,
                EptMemoryType::Uncacheable,
                ept_capabilities(),
            )
            .unwrap()
            .raw(),
            0x3456_7001
        );
    }

    #[test]
    fn ept_4k_leaf_rejects_nonpresent_and_write_only_permissions() {
        let host = HostPhysical::new(0x3456_7000).unwrap();
        let write_only = EptPermissions {
            read: false,
            write: true,
            execute: false,
        };

        for permissions in [EptPermissions::NONE, write_only] {
            assert_eq!(
                EptLeafEntry4K::new(
                    host,
                    permissions,
                    EptMemoryType::WriteBack,
                    ept_capabilities(),
                )
                .unwrap_err()
                .kind,
                VmxErrorKind::InvalidEptMapping
            );
        }
    }

    #[test]
    fn ept_4k_leaf_gates_execute_only_permission_on_capability() {
        let host = HostPhysical::new(0x3456_7000).unwrap();
        let execute_only = EptPermissions {
            read: false,
            write: false,
            execute: true,
        };

        assert_eq!(
            EptLeafEntry4K::new(
                host,
                execute_only,
                EptMemoryType::WriteBack,
                ept_capabilities(),
            )
            .unwrap_err()
            .kind,
            VmxErrorKind::UnsupportedCapability
        );

        let with_execute_only =
            EptCapabilities::new(ept_capabilities().raw() | EPT_VPID_CAP_EXECUTE_ONLY);
        assert_eq!(
            EptLeafEntry4K::new(
                host,
                execute_only,
                EptMemoryType::WriteBack,
                with_execute_only,
            )
            .unwrap()
            .raw(),
            0x3456_7034
        );
    }

    #[test]
    fn ept_4k_leaf_rejects_unsupported_memory_type_and_misalignment() {
        let write_back_only =
            EptCapabilities::new(EPT_VPID_CAP_PAGE_WALK_LENGTH_4 | EPT_VPID_CAP_MEMORY_TYPE_WB);
        assert_eq!(
            EptLeafEntry4K::new(
                HostPhysical::new(0x4000).unwrap(),
                EptPermissions::READ,
                EptMemoryType::Uncacheable,
                write_back_only,
            )
            .unwrap_err()
            .kind,
            VmxErrorKind::UnsupportedCapability
        );
        assert_eq!(
            EptLeafEntry4K::new(
                HostPhysical::new(0x4001).unwrap(),
                EptPermissions::READ,
                EptMemoryType::WriteBack,
                ept_capabilities(),
            )
            .unwrap_err()
            .kind,
            VmxErrorKind::InvalidEptMapping
        );
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
