use aegishv_hypervisor_core::ids::HostPhysical;

use super::features::{VmxError, VmxErrorKind};

pub const VMX_REGION_SIZE: usize = 4096;
pub const VMX_REGION_ALIGNMENT: u64 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmxRevisionId(u32);

impl VmxRevisionId {
    pub const fn new(raw: u32) -> Result<Self, VmxError> {
        if raw == 0 || raw & (1 << 31) != 0 {
            Err(VmxError::new(
                VmxErrorKind::InvalidRevisionId,
                "VMCS revision id must be non-zero and must not set bit 31",
            ))
        } else {
            Ok(Self(raw))
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmxRegion {
    physical_address: HostPhysical,
    revision_id: VmxRevisionId,
    bytes: [u8; VMX_REGION_SIZE],
}

impl VmxRegion {
    pub fn new(
        physical_address: HostPhysical,
        revision_id: VmxRevisionId,
    ) -> Result<Self, VmxError> {
        if physical_address.get() % VMX_REGION_ALIGNMENT != 0 {
            return Err(VmxError::new(
                VmxErrorKind::MisalignedRegion,
                "VMX region physical address must be 4K-aligned",
            ));
        }

        let mut bytes = [0u8; VMX_REGION_SIZE];
        bytes[..4].copy_from_slice(&revision_id.get().to_le_bytes());
        Ok(Self {
            physical_address,
            revision_id,
            bytes,
        })
    }

    pub const fn physical_address(&self) -> HostPhysical {
        self.physical_address
    }

    pub const fn revision_id(&self) -> VmxRevisionId {
        self.revision_id
    }

    pub fn bytes(&self) -> &[u8; VMX_REGION_SIZE] {
        &self.bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmxonRegion {
    region: VmxRegion,
}

impl VmxonRegion {
    pub fn new(
        physical_address: HostPhysical,
        revision_id: VmxRevisionId,
    ) -> Result<Self, VmxError> {
        Ok(Self {
            region: VmxRegion::new(physical_address, revision_id)?,
        })
    }

    pub const fn physical_address(&self) -> HostPhysical {
        self.region.physical_address()
    }

    pub const fn revision_id(&self) -> VmxRevisionId {
        self.region.revision_id()
    }

    pub fn bytes(&self) -> &[u8; VMX_REGION_SIZE] {
        self.region.bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vmxon_region_writes_revision_id_and_keeps_rest_zeroed() {
        let region = VmxonRegion::new(
            HostPhysical::new(0x4000).unwrap(),
            VmxRevisionId::new(0x19).unwrap(),
        )
        .unwrap();

        assert_eq!(&region.bytes()[..4], &0x19u32.to_le_bytes());
        assert!(region.bytes()[4..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn vmxon_region_rejects_misaligned_physical_address() {
        let err = VmxonRegion::new(
            HostPhysical::new(0x4100).unwrap(),
            VmxRevisionId::new(0x19).unwrap(),
        )
        .unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::MisalignedRegion);
    }

    #[test]
    fn vmcs_revision_rejects_shadow_vmcs_bit() {
        let err = VmxRevisionId::new(1 << 31).unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::InvalidRevisionId);
    }
}
