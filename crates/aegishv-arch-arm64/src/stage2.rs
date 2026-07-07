use aegishv_hypervisor_core::ids::{GuestPhysical, HostPhysical};

use crate::features::{Arm64Error, Arm64ErrorKind, Granule};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage2MemoryAttr {
    Device,
    NormalWriteBack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage2Shareability {
    NonShareable,
    InnerShareable,
    OuterShareable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage2Permissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl Stage2Permissions {
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

    pub const fn allows(self, access: Stage2Access) -> bool {
        match access {
            Stage2Access::Read => self.read,
            Stage2Access::Write => self.write,
            Stage2Access::Execute => self.execute,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage2Access {
    Read,
    Write,
    Execute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage2Mapping {
    pub ipa: GuestPhysical,
    pub pa: HostPhysical,
    pub length: u64,
    pub granule: Granule,
    pub permissions: Stage2Permissions,
    pub attr: Stage2MemoryAttr,
    pub shareability: Stage2Shareability,
}

impl Stage2Mapping {
    pub fn new(
        ipa: GuestPhysical,
        pa: HostPhysical,
        length: u64,
        granule: Granule,
        permissions: Stage2Permissions,
        attr: Stage2MemoryAttr,
        shareability: Stage2Shareability,
    ) -> Result<Self, Arm64Error> {
        let bytes = granule.bytes();
        if length == 0 || length % bytes != 0 || ipa.get() % bytes != 0 || pa.get() % bytes != 0 {
            return Err(Arm64Error::new(
                Arm64ErrorKind::InvalidStage2Mapping,
                "ARM64 Stage-2 mapping must use non-empty aligned ranges",
            ));
        }
        Ok(Self {
            ipa,
            pa,
            length,
            granule,
            permissions,
            attr,
            shareability,
        })
    }

    pub const fn end(self) -> u64 {
        self.ipa.get() + self.length
    }

    pub const fn contains(self, ipa: GuestPhysical) -> bool {
        self.ipa.get() <= ipa.get() && ipa.get() < self.end()
    }
}

pub struct Stage2MapPlan<const N: usize> {
    mappings: [Option<Stage2Mapping>; N],
    len: usize,
}

impl<const N: usize> Stage2MapPlan<N> {
    pub const fn new() -> Self {
        Self {
            mappings: [None; N],
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub fn mappings(&self) -> impl Iterator<Item = Stage2Mapping> + '_ {
        self.mappings[..self.len]
            .iter()
            .filter_map(|mapping| *mapping)
    }

    pub fn map(&mut self, mapping: Stage2Mapping) -> Result<(), Arm64Error> {
        if self.len >= N {
            return Err(Arm64Error::new(
                Arm64ErrorKind::InvalidStage2Mapping,
                "ARM64 Stage-2 map plan is full",
            ));
        }
        for existing in self.mappings() {
            if mapping.ipa.get() < existing.end() && existing.ipa.get() < mapping.end() {
                return Err(Arm64Error::new(
                    Arm64ErrorKind::InvalidStage2Mapping,
                    "ARM64 Stage-2 mapping overlaps an existing IPA range",
                ));
            }
        }
        self.mappings[self.len] = Some(mapping);
        self.len += 1;
        Ok(())
    }

    pub fn lookup(&self, ipa: GuestPhysical) -> Option<Stage2Mapping> {
        self.mappings().find(|mapping| mapping.contains(ipa))
    }

    pub fn set_permissions(
        &mut self,
        ipa: GuestPhysical,
        permissions: Stage2Permissions,
    ) -> Result<Stage2Permissions, Arm64Error> {
        for mapping in &mut self.mappings[..self.len] {
            let Some(mapping) = mapping.as_mut() else {
                continue;
            };
            if mapping.contains(ipa) {
                let previous = mapping.permissions;
                mapping.permissions = permissions;
                return Ok(previous);
            }
        }
        Err(Arm64Error::new(
            Arm64ErrorKind::InvalidStage2Mapping,
            "ARM64 Stage-2 permission update did not find a covering mapping",
        ))
    }
}

impl<const N: usize> Default for Stage2MapPlan<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(ipa: u64, permissions: Stage2Permissions) -> Stage2Mapping {
        Stage2Mapping::new(
            GuestPhysical::new(ipa).unwrap(),
            HostPhysical::new(ipa + 0x100000).unwrap(),
            4096,
            Granule::Size4K,
            permissions,
            Stage2MemoryAttr::NormalWriteBack,
            Stage2Shareability::InnerShareable,
        )
        .unwrap()
    }

    #[test]
    fn stage2_plan_rejects_misaligned_mapping() {
        let err = Stage2Mapping::new(
            GuestPhysical::new(0x1001).unwrap(),
            HostPhysical::new(0x2000).unwrap(),
            4096,
            Granule::Size4K,
            Stage2Permissions::READ,
            Stage2MemoryAttr::NormalWriteBack,
            Stage2Shareability::InnerShareable,
        )
        .unwrap_err();

        assert_eq!(err.kind, Arm64ErrorKind::InvalidStage2Mapping);
    }

    #[test]
    fn stage2_plan_rejects_overlapping_ipa_ranges() {
        let mut plan = Stage2MapPlan::<2>::new();
        plan.map(mapping(0x4000, Stage2Permissions::READ)).unwrap();

        assert_eq!(
            plan.map(mapping(0x4000, Stage2Permissions::READ))
                .unwrap_err()
                .kind,
            Arm64ErrorKind::InvalidStage2Mapping
        );
    }

    #[test]
    fn stage2_plan_updates_permissions_for_covering_mapping() {
        let mut plan = Stage2MapPlan::<2>::new();
        plan.map(mapping(0x8000, Stage2Permissions::READ_WRITE_EXECUTE))
            .unwrap();

        let previous = plan
            .set_permissions(GuestPhysical::new(0x8000).unwrap(), Stage2Permissions::READ)
            .unwrap();

        assert_eq!(previous, Stage2Permissions::READ_WRITE_EXECUTE);
        assert_eq!(
            plan.lookup(GuestPhysical::new(0x8000).unwrap())
                .unwrap()
                .permissions,
            Stage2Permissions::READ
        );
    }
}
