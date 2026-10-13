use core::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arm64ErrorKind {
    MissingEl2,
    UnsupportedGranule,
    UnsupportedIpaSize,
    UnsupportedVmidWidth,
    MissingGicVirtualization,
    MissingSmmu,
    InvalidAddress,
    InvalidStage2Mapping,
    InvalidVtcr,
    InvalidVttbr,
    InvalidEsr,
    UnsupportedTrap,
    UnsupportedCapability,
    InvalidTimerState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arm64Error {
    pub kind: Arm64ErrorKind,
    pub message: &'static str,
}

impl Arm64Error {
    pub const fn new(kind: Arm64ErrorKind, message: &'static str) -> Self {
        Self { kind, message }
    }
}

impl fmt::Display for Arm64Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Arm64Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum El2Mode {
    Vhe,
    Nvhe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Granule {
    Size4K,
    Size16K,
    Size64K,
}

impl Granule {
    pub const fn bytes(self) -> u64 {
        match self {
            Self::Size4K => 4096,
            Self::Size16K => 16 * 1024,
            Self::Size64K => 64 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmidBits {
    Bits8,
    Bits16,
}

impl VmidBits {
    pub const fn width(self) -> u8 {
        match self {
            Self::Bits8 => 8,
            Self::Bits16 => 16,
        }
    }

    pub const fn max_vmid(self) -> u16 {
        match self {
            Self::Bits8 => 0xff,
            Self::Bits16 => 0xffff,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GicVirtualization {
    None,
    Gicv2,
    Gicv3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmmuCapability {
    None,
    Smmuv2,
    Smmuv3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arm64FeatureSet {
    pub has_el2: bool,
    pub mode: El2Mode,
    pub vmid_bits: VmidBits,
    pub ipa_bits: u8,
    pub granule_4k: bool,
    pub granule_16k: bool,
    pub granule_64k: bool,
    pub gic: GicVirtualization,
    pub smmu: SmmuCapability,
    pub protected_guest: bool,
}

impl Arm64FeatureSet {
    pub const fn validate_stage2_4k(self) -> Result<Self, Arm64Error> {
        if !self.has_el2 {
            return Err(Arm64Error::new(
                Arm64ErrorKind::MissingEl2,
                "ARM64 EL2 is not available",
            ));
        }
        if !self.granule_4k {
            return Err(Arm64Error::new(
                Arm64ErrorKind::UnsupportedGranule,
                "ARM64 Stage-2 lab requires 4K granule support",
            ));
        }
        if self.ipa_bits < 32 || self.ipa_bits > 52 {
            return Err(Arm64Error::new(
                Arm64ErrorKind::UnsupportedIpaSize,
                "ARM64 IPA size must be between 32 and 52 bits for this model",
            ));
        }
        Ok(self)
    }

    pub const fn validate_gic(self) -> Result<Self, Arm64Error> {
        match self.gic {
            GicVirtualization::Gicv2 | GicVirtualization::Gicv3 => Ok(self),
            GicVirtualization::None => Err(Arm64Error::new(
                Arm64ErrorKind::MissingGicVirtualization,
                "ARM64 interrupt virtualization needs a GIC virtualization model",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdAa64Mmfr0El1 {
    raw: u64,
}

impl IdAa64Mmfr0El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    pub const fn pa_range_bits(self) -> u8 {
        match self.raw & 0xf {
            0 => 32,
            1 => 36,
            2 => 40,
            3 => 42,
            4 => 44,
            5 => 48,
            6 => 52,
            _ => 0,
        }
    }

    pub const fn granule_4k_supported(self) -> bool {
        ((self.raw >> 28) & 0xf) == 0
    }

    pub const fn granule_16k_supported(self) -> bool {
        ((self.raw >> 20) & 0xf) <= 1
    }

    pub const fn granule_64k_supported(self) -> bool {
        ((self.raw >> 24) & 0xf) == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdAa64Mmfr1El1 {
    raw: u64,
}

impl IdAa64Mmfr1El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    pub const fn vmid_bits(self) -> VmidBits {
        if ((self.raw >> 4) & 0xf) != 0 {
            VmidBits::Bits16
        } else {
            VmidBits::Bits8
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdAa64Pfr0El1 {
    raw: u64,
}

impl IdAa64Pfr0El1 {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    pub const fn has_el2(self) -> bool {
        ((self.raw >> 8) & 0xf) != 0
    }

    pub const fn has_vhe(self) -> bool {
        ((self.raw >> 8) & 0xf) >= 2
    }
}

pub const fn features_from_id_registers(
    pfr0: IdAa64Pfr0El1,
    mmfr0: IdAa64Mmfr0El1,
    mmfr1: IdAa64Mmfr1El1,
    gic: GicVirtualization,
    smmu: SmmuCapability,
    protected_guest: bool,
) -> Arm64FeatureSet {
    Arm64FeatureSet {
        has_el2: pfr0.has_el2(),
        mode: if pfr0.has_vhe() {
            El2Mode::Vhe
        } else {
            El2Mode::Nvhe
        },
        vmid_bits: mmfr1.vmid_bits(),
        ipa_bits: mmfr0.pa_range_bits(),
        granule_4k: mmfr0.granule_4k_supported(),
        granule_16k: mmfr0.granule_16k_supported(),
        granule_64k: mmfr0.granule_64k_supported(),
        gic,
        smmu,
        protected_guest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_register_decode_reports_el2_vhe_ipa_and_granules() {
        let features = features_from_id_registers(
            IdAa64Pfr0El1::new(2 << 8),
            IdAa64Mmfr0El1::new(5),
            IdAa64Mmfr1El1::new(1 << 4),
            GicVirtualization::Gicv3,
            SmmuCapability::Smmuv3,
            false,
        );

        assert_eq!(features.mode, El2Mode::Vhe);
        assert_eq!(features.ipa_bits, 48);
        assert_eq!(features.vmid_bits, VmidBits::Bits16);
        assert!(features.granule_4k);
        assert!(features.validate_stage2_4k().is_ok());
    }

    #[test]
    fn feature_validation_rejects_missing_el2() {
        let features = features_from_id_registers(
            IdAa64Pfr0El1::new(0),
            IdAa64Mmfr0El1::new(5),
            IdAa64Mmfr1El1::new(0),
            GicVirtualization::Gicv3,
            SmmuCapability::Smmuv3,
            false,
        );

        assert_eq!(
            features.validate_stage2_4k().unwrap_err().kind,
            Arm64ErrorKind::MissingEl2
        );
    }

    #[test]
    fn gic_validation_rejects_missing_virtualization() {
        let features = features_from_id_registers(
            IdAa64Pfr0El1::new(1 << 8),
            IdAa64Mmfr0El1::new(5),
            IdAa64Mmfr1El1::new(0),
            GicVirtualization::None,
            SmmuCapability::None,
            false,
        );

        assert_eq!(
            features.validate_gic().unwrap_err().kind,
            Arm64ErrorKind::MissingGicVirtualization
        );
    }
}
