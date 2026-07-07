use crate::error::{CoreError, CoreErrorKind};
use crate::ids::{DeviceId, HostPhysical, VmId};

pub const DMA_PAGE_SIZE: u64 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DmaDomainId(u32);

impl DmaDomainId {
    pub const fn new(raw: u32) -> Result<Self, CoreError> {
        if raw == 0 {
            Err(CoreError::new(
                CoreErrorKind::InvalidId,
                "DMA domain id 0 is reserved",
            ))
        } else {
            Ok(Self(raw))
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IommuIsolationProof {
    pub translation_enabled: bool,
    pub interrupt_remapping_enabled: bool,
    pub fault_reporting_enabled: bool,
}

impl IommuIsolationProof {
    pub const fn proven(self) -> bool {
        self.translation_enabled && self.interrupt_remapping_enabled && self.fault_reporting_enabled
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DmaPermissions {
    pub read: bool,
    pub write: bool,
}

impl DmaPermissions {
    pub const READ_ONLY: Self = Self {
        read: true,
        write: false,
    };
    pub const READ_WRITE: Self = Self {
        read: true,
        write: true,
    };

    const fn empty(self) -> bool {
        !self.read && !self.write
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DmaMapping {
    pub device: DeviceId,
    pub iova: u64,
    pub host_physical: HostPhysical,
    pub length: u64,
    pub permissions: DmaPermissions,
}

impl DmaMapping {
    pub fn new(
        device: DeviceId,
        iova: u64,
        host_physical: HostPhysical,
        length: u64,
        permissions: DmaPermissions,
    ) -> Result<Self, CoreError> {
        if length == 0
            || iova % DMA_PAGE_SIZE != 0
            || host_physical.get() % DMA_PAGE_SIZE != 0
            || length % DMA_PAGE_SIZE != 0
        {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "DMA mappings must be non-empty and 4K aligned",
            ));
        }
        if permissions.empty() {
            return Err(CoreError::new(
                CoreErrorKind::PermissionViolation,
                "DMA mapping must allow at least one direction",
            ));
        }
        host_physical.checked_add(length)?;
        iova.checked_add(length).ok_or(CoreError::new(
            CoreErrorKind::InvalidAddress,
            "DMA IOVA range overflowed",
        ))?;
        Ok(Self {
            device,
            iova,
            host_physical,
            length,
            permissions,
        })
    }

    fn end_iova(self) -> u64 {
        self.iova + self.length
    }

    fn overlaps_iova(self, other: Self) -> bool {
        self.iova < other.end_iova() && other.iova < self.end_iova()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IommuFaultKind {
    Translation,
    Permission,
    UnknownDevice,
    IsolationMissing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IommuFaultEvent {
    pub domain: DmaDomainId,
    pub device: DeviceId,
    pub iova: u64,
    pub kind: IommuFaultKind,
}

pub struct DmaDomain<const DEVICES: usize, const MAPS: usize> {
    id: DmaDomainId,
    vm: VmId,
    devices: [Option<DeviceId>; DEVICES],
    device_len: usize,
    mappings: [Option<DmaMapping>; MAPS],
    mapping_len: usize,
}

impl<const DEVICES: usize, const MAPS: usize> DmaDomain<DEVICES, MAPS> {
    pub const fn new(id: DmaDomainId, vm: VmId) -> Self {
        Self {
            id,
            vm,
            devices: [None; DEVICES],
            device_len: 0,
            mappings: [None; MAPS],
            mapping_len: 0,
        }
    }

    pub const fn id(&self) -> DmaDomainId {
        self.id
    }

    pub const fn vm(&self) -> VmId {
        self.vm
    }

    pub fn devices(&self) -> impl Iterator<Item = DeviceId> + '_ {
        self.devices[..self.device_len]
            .iter()
            .filter_map(|device| *device)
    }

    pub fn mappings(&self) -> impl Iterator<Item = DmaMapping> + '_ {
        self.mappings[..self.mapping_len]
            .iter()
            .filter_map(|mapping| *mapping)
    }

    pub fn assign_device(
        &mut self,
        device: DeviceId,
        proof: IommuIsolationProof,
    ) -> Result<(), CoreError> {
        if !proof.proven() {
            return Err(CoreError::new(
                CoreErrorKind::Unsupported,
                "cannot assign DMA device without proven IOMMU isolation",
            ));
        }
        if self.devices().any(|existing| existing == device) {
            return Ok(());
        }
        if self.device_len >= DEVICES {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "DMA domain device table is full",
            ));
        }
        self.devices[self.device_len] = Some(device);
        self.device_len += 1;
        Ok(())
    }

    pub fn map_dma(
        &mut self,
        mapping: DmaMapping,
        proof: IommuIsolationProof,
    ) -> Result<(), CoreError> {
        if !proof.proven() {
            return Err(CoreError::new(
                CoreErrorKind::Unsupported,
                "cannot map DMA without proven IOMMU isolation",
            ));
        }
        if !self.devices().any(|device| device == mapping.device) {
            return Err(CoreError::new(
                CoreErrorKind::PermissionViolation,
                "DMA mapping references a device outside the domain",
            ));
        }
        if self.mapping_len >= MAPS {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "DMA mapping table is full",
            ));
        }
        for existing in self.mappings() {
            if existing.overlaps_iova(mapping) {
                return Err(CoreError::new(
                    CoreErrorKind::Overlap,
                    "DMA mappings in a domain must not overlap IOVA ranges",
                ));
            }
        }
        self.mappings[self.mapping_len] = Some(mapping);
        self.mapping_len += 1;
        Ok(())
    }

    pub const fn fault(
        &self,
        device: DeviceId,
        iova: u64,
        kind: IommuFaultKind,
    ) -> IommuFaultEvent {
        IommuFaultEvent {
            domain: self.id,
            device,
            iova,
            kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proof() -> IommuIsolationProof {
        IommuIsolationProof {
            translation_enabled: true,
            interrupt_remapping_enabled: true,
            fault_reporting_enabled: true,
        }
    }

    #[test]
    fn dma_domain_requires_proven_iommu_before_device_assignment() {
        let mut domain =
            DmaDomain::<2, 2>::new(DmaDomainId::new(1).unwrap(), VmId::new(1).unwrap());
        let device = DeviceId::new(9).unwrap();

        let err = domain
            .assign_device(
                device,
                IommuIsolationProof {
                    translation_enabled: true,
                    interrupt_remapping_enabled: false,
                    fault_reporting_enabled: true,
                },
            )
            .unwrap_err();

        assert_eq!(err.kind, CoreErrorKind::Unsupported);
    }

    #[test]
    fn dma_mapping_rejects_unassigned_device_and_overlaps() {
        let mut domain =
            DmaDomain::<2, 2>::new(DmaDomainId::new(1).unwrap(), VmId::new(1).unwrap());
        let device = DeviceId::new(9).unwrap();
        let other = DeviceId::new(10).unwrap();
        domain.assign_device(device, proof()).unwrap();
        domain
            .map_dma(
                DmaMapping::new(
                    device,
                    0x1000,
                    HostPhysical::new(0x8000).unwrap(),
                    0x1000,
                    DmaPermissions::READ_WRITE,
                )
                .unwrap(),
                proof(),
            )
            .unwrap();

        assert_eq!(
            domain
                .map_dma(
                    DmaMapping::new(
                        other,
                        0x3000,
                        HostPhysical::new(0xa000).unwrap(),
                        0x1000,
                        DmaPermissions::READ_ONLY,
                    )
                    .unwrap(),
                    proof(),
                )
                .unwrap_err()
                .kind,
            CoreErrorKind::PermissionViolation
        );
        assert_eq!(
            domain
                .map_dma(
                    DmaMapping::new(
                        device,
                        0x1000,
                        HostPhysical::new(0xb000).unwrap(),
                        0x1000,
                        DmaPermissions::READ_ONLY,
                    )
                    .unwrap(),
                    proof(),
                )
                .unwrap_err()
                .kind,
            CoreErrorKind::Overlap
        );
    }
}
