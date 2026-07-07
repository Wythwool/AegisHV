use crate::error::{CoreError, CoreErrorKind};
use crate::ids::{GuestPhysical, HostPhysical};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TranslationPageSize {
    Size4K,
    Size2M,
    Size1G,
}

impl TranslationPageSize {
    pub const fn bytes(self) -> u64 {
        match self {
            Self::Size4K => 4096,
            Self::Size2M => 2 * 1024 * 1024,
            Self::Size1G => 1024 * 1024 * 1024,
        }
    }

    const fn parent(self) -> Option<Self> {
        match self {
            Self::Size4K => Some(Self::Size2M),
            Self::Size2M => Some(Self::Size1G),
            Self::Size1G => None,
        }
    }

    const fn child(self) -> Option<Self> {
        match self {
            Self::Size1G => Some(Self::Size2M),
            Self::Size2M => Some(Self::Size4K),
            Self::Size4K => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranslationPermissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl TranslationPermissions {
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

    pub const fn is_wx(self) -> bool {
        self.write && self.execute
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TranslationMapping {
    pub guest_physical: GuestPhysical,
    pub host_physical: HostPhysical,
    pub page_size: TranslationPageSize,
    pub permissions: TranslationPermissions,
}

impl TranslationMapping {
    pub fn new(
        guest_physical: GuestPhysical,
        host_physical: HostPhysical,
        page_size: TranslationPageSize,
        permissions: TranslationPermissions,
    ) -> Result<Self, CoreError> {
        let bytes = page_size.bytes();
        if guest_physical.get() % bytes != 0 || host_physical.get() % bytes != 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "translation mapping must be aligned to the selected page size",
            ));
        }
        if permissions.is_wx() {
            return Err(CoreError::new(
                CoreErrorKind::PermissionViolation,
                "translation mapping must not be writable and executable",
            ));
        }
        Ok(Self {
            guest_physical,
            host_physical,
            page_size,
            permissions,
        })
    }

    fn end_gpa(self) -> Result<u64, CoreError> {
        self.guest_physical
            .get()
            .checked_add(self.page_size.bytes())
            .ok_or(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "translation mapping guest end address overflowed",
            ))
    }

    fn end_hpa(self) -> Result<u64, CoreError> {
        self.host_physical
            .get()
            .checked_add(self.page_size.bytes())
            .ok_or(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "translation mapping host end address overflowed",
            ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidationScope {
    Page,
    Range,
    AddressSpace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitPlan {
    pub source: TranslationMapping,
    pub child_size: TranslationPageSize,
    pub child_count: u64,
    pub invalidation: InvalidationScope,
}

pub fn plan_split(mapping: TranslationMapping) -> Result<SplitPlan, CoreError> {
    let child = mapping.page_size.child().ok_or(CoreError::new(
        CoreErrorKind::Unsupported,
        "4K mapping cannot be split further",
    ))?;
    Ok(SplitPlan {
        source: mapping,
        child_size: child,
        child_count: mapping.page_size.bytes() / child.bytes(),
        invalidation: InvalidationScope::Range,
    })
}

pub fn plan_merge(mappings: &[TranslationMapping]) -> Result<TranslationMapping, CoreError> {
    let Some(first) = mappings.first().copied() else {
        return Err(CoreError::new(
            CoreErrorKind::InvalidArgument,
            "huge page merge requires at least one child mapping",
        ));
    };
    let parent_size = first.page_size.parent().ok_or(CoreError::new(
        CoreErrorKind::Unsupported,
        "1G mapping cannot be merged into a larger mapping",
    ))?;
    let expected_count = parent_size.bytes() / first.page_size.bytes();
    if mappings.len() as u64 != expected_count {
        return Err(CoreError::new(
            CoreErrorKind::InvalidArgument,
            "huge page merge requires a complete aligned child run",
        ));
    }
    if first.guest_physical.get() % parent_size.bytes() != 0
        || first.host_physical.get() % parent_size.bytes() != 0
    {
        return Err(CoreError::new(
            CoreErrorKind::InvalidAddress,
            "huge page merge base is not aligned to the parent size",
        ));
    }

    let mut expected_gpa = first.guest_physical.get();
    let mut expected_hpa = first.host_physical.get();
    for mapping in mappings {
        if mapping.page_size != first.page_size || mapping.permissions != first.permissions {
            return Err(CoreError::new(
                CoreErrorKind::PermissionViolation,
                "huge page merge requires identical child permissions and sizes",
            ));
        }
        if mapping.guest_physical.get() != expected_gpa
            || mapping.host_physical.get() != expected_hpa
        {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "huge page merge children must be physically contiguous",
            ));
        }
        expected_gpa = mapping.end_gpa()?;
        expected_hpa = mapping.end_hpa()?;
    }

    TranslationMapping::new(
        first.guest_physical,
        first.host_physical,
        parent_size,
        first.permissions,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_plan_models_one_gigabyte_to_two_megabyte_children() {
        let mapping = TranslationMapping::new(
            GuestPhysical::new(0).unwrap(),
            HostPhysical::new(0x4000_0000).unwrap(),
            TranslationPageSize::Size1G,
            TranslationPermissions::READ_EXECUTE,
        )
        .unwrap();

        let plan = plan_split(mapping).unwrap();

        assert_eq!(plan.child_size, TranslationPageSize::Size2M);
        assert_eq!(plan.child_count, 512);
        assert_eq!(plan.invalidation, InvalidationScope::Range);
    }

    #[test]
    fn merge_plan_requires_contiguous_same_permission_children() {
        let children = [
            TranslationMapping::new(
                GuestPhysical::new(0).unwrap(),
                HostPhysical::new(0x20_0000).unwrap(),
                TranslationPageSize::Size4K,
                TranslationPermissions::READ,
            )
            .unwrap(),
            TranslationMapping::new(
                GuestPhysical::new(0x1000).unwrap(),
                HostPhysical::new(0x21_0000).unwrap(),
                TranslationPageSize::Size4K,
                TranslationPermissions::READ_WRITE,
            )
            .unwrap(),
        ];

        assert_eq!(
            plan_merge(&children).unwrap_err().kind,
            CoreErrorKind::InvalidArgument
        );
    }

    #[test]
    fn mapping_rejects_writable_executable_permissions() {
        let wx = TranslationPermissions {
            read: true,
            write: true,
            execute: true,
        };

        assert_eq!(
            TranslationMapping::new(
                GuestPhysical::new(0).unwrap(),
                HostPhysical::new(0).unwrap(),
                TranslationPageSize::Size4K,
                wx,
            )
            .unwrap_err()
            .kind,
            CoreErrorKind::PermissionViolation
        );
    }
}
