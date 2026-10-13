use aegishv_hypervisor_core::ids::HostPhysical;

use crate::features::{Arm64Error, Arm64ErrorKind, Granule, VmidBits};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhysicalAddressSize {
    Bits32,
    Bits36,
    Bits40,
    Bits42,
    Bits44,
    Bits48,
    Bits52,
}

impl PhysicalAddressSize {
    pub const fn from_bits(bits: u8) -> Result<Self, Arm64Error> {
        match bits {
            32 => Ok(Self::Bits32),
            36 => Ok(Self::Bits36),
            40 => Ok(Self::Bits40),
            42 => Ok(Self::Bits42),
            44 => Ok(Self::Bits44),
            48 => Ok(Self::Bits48),
            52 => Ok(Self::Bits52),
            _ => Err(Arm64Error::new(
                Arm64ErrorKind::UnsupportedIpaSize,
                "ARM64 VTCR IPA size is not supported by the model",
            )),
        }
    }

    pub const fn ps_field(self) -> u64 {
        match self {
            Self::Bits32 => 0,
            Self::Bits36 => 1,
            Self::Bits40 => 2,
            Self::Bits42 => 3,
            Self::Bits44 => 4,
            Self::Bits48 => 5,
            Self::Bits52 => 6,
        }
    }

    pub const fn bits(self) -> u8 {
        match self {
            Self::Bits32 => 32,
            Self::Bits36 => 36,
            Self::Bits40 => 40,
            Self::Bits42 => 42,
            Self::Bits44 => 44,
            Self::Bits48 => 48,
            Self::Bits52 => 52,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VtcrConfig {
    pub ipa_size: PhysicalAddressSize,
    pub granule: Granule,
    pub vmid_bits: VmidBits,
    pub inner_shareable: bool,
}

impl VtcrConfig {
    pub fn new(ipa_bits: u8, granule: Granule, vmid_bits: VmidBits) -> Result<Self, Arm64Error> {
        match granule {
            Granule::Size4K => {}
            Granule::Size16K | Granule::Size64K => {
                return Err(Arm64Error::new(
                    Arm64ErrorKind::UnsupportedGranule,
                    "ARM64 VTCR model currently supports 4K Stage-2 granule",
                ))
            }
        }
        Ok(Self {
            ipa_size: PhysicalAddressSize::from_bits(ipa_bits)?,
            granule,
            vmid_bits,
            inner_shareable: true,
        })
    }

    pub const fn encode(self) -> u64 {
        let t0sz = 64 - self.ipa_size.bits() as u64;
        let sl0 = 1u64 << 6;
        let sh0_inner = if self.inner_shareable { 3u64 << 12 } else { 0 };
        let orgn0_wb = 1u64 << 10;
        let irgn0_wb = 1u64 << 8;
        let vs = match self.vmid_bits {
            VmidBits::Bits8 => 0,
            VmidBits::Bits16 => 1u64 << 19,
        };
        t0sz | sl0 | irgn0_wb | orgn0_wb | sh0_inner | (self.ipa_size.ps_field() << 16) | vs
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vttbr {
    pub vmid: u16,
    pub root: HostPhysical,
    pub vmid_bits: VmidBits,
}

impl Vttbr {
    pub fn new(vmid: u16, root: HostPhysical, vmid_bits: VmidBits) -> Result<Self, Arm64Error> {
        if vmid == 0 || vmid > vmid_bits.max_vmid() {
            return Err(Arm64Error::new(
                Arm64ErrorKind::InvalidVttbr,
                "ARM64 VTTBR VMID is outside the supported VMID width",
            ));
        }
        if root.get() % 4096 != 0 {
            return Err(Arm64Error::new(
                Arm64ErrorKind::InvalidVttbr,
                "ARM64 VTTBR root must be 4K-aligned",
            ));
        }
        Ok(Self {
            vmid,
            root,
            vmid_bits,
        })
    }

    pub const fn encode(self) -> u64 {
        ((self.vmid as u64) << 48) | (self.root.get() & 0x0000_ffff_ffff_f000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vtcr_encode_sets_4k_wb_inner_shareable_and_vmid_width() {
        let vtcr = VtcrConfig::new(48, Granule::Size4K, VmidBits::Bits16)
            .unwrap()
            .encode();

        assert_eq!(vtcr & 0x3f, 16);
        assert_ne!(vtcr & (1 << 19), 0);
        assert_eq!((vtcr >> 16) & 0x7, 5);
    }

    #[test]
    fn vtcr_rejects_unsupported_granule_for_first_model() {
        assert_eq!(
            VtcrConfig::new(48, Granule::Size16K, VmidBits::Bits8)
                .unwrap_err()
                .kind,
            Arm64ErrorKind::UnsupportedGranule
        );
    }

    #[test]
    fn vttbr_rejects_reserved_vmid_and_misaligned_root() {
        assert_eq!(
            Vttbr::new(0, HostPhysical::new(0x4000).unwrap(), VmidBits::Bits8)
                .unwrap_err()
                .kind,
            Arm64ErrorKind::InvalidVttbr
        );
        assert_eq!(
            Vttbr::new(1, HostPhysical::new(0x4100).unwrap(), VmidBits::Bits8)
                .unwrap_err()
                .kind,
            Arm64ErrorKind::InvalidVttbr
        );
    }
}
