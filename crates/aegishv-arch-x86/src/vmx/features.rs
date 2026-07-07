use core::fmt;

pub const CPUID_LEAF1_ECX_VMX: u32 = 1 << 5;
pub const IA32_FEATURE_CONTROL_LOCK: u64 = 1 << 0;
pub const IA32_FEATURE_CONTROL_VMX_INSIDE_SMX: u64 = 1 << 1;
pub const IA32_FEATURE_CONTROL_VMX_OUTSIDE_SMX: u64 = 1 << 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmxErrorKind {
    MissingCpuidBit,
    FeatureControlUnlocked,
    VmxDisabledOutsideSmx,
    InvalidRevisionId,
    MisalignedRegion,
    RegionTooSmall,
    InstructionFailed,
    InvalidVmcsState,
    InvalidVmcsField,
    InvalidControlBits,
    InvalidGuestState,
    UnsupportedExit,
    UnsupportedMsr,
    UnsupportedControlRegister,
    InvalidEptMapping,
    InvalidEptViolation,
    UnsupportedCapability,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmxError {
    pub kind: VmxErrorKind,
    pub message: &'static str,
}

impl VmxError {
    pub const fn new(kind: VmxErrorKind, message: &'static str) -> Self {
        Self { kind, message }
    }
}

impl fmt::Display for VmxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for VmxError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuidLeaf1 {
    pub ecx: u32,
}

impl CpuidLeaf1 {
    pub const fn vmx_present(self) -> bool {
        self.ecx & CPUID_LEAF1_ECX_VMX != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureControlMsr {
    raw: u64,
}

impl FeatureControlMsr {
    pub const fn new(raw: u64) -> Self {
        Self { raw }
    }

    pub const fn raw(self) -> u64 {
        self.raw
    }

    pub const fn locked(self) -> bool {
        self.raw & IA32_FEATURE_CONTROL_LOCK != 0
    }

    pub const fn vmx_inside_smx(self) -> bool {
        self.raw & IA32_FEATURE_CONTROL_VMX_INSIDE_SMX != 0
    }

    pub const fn vmx_outside_smx(self) -> bool {
        self.raw & IA32_FEATURE_CONTROL_VMX_OUTSIDE_SMX != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VmxFeatureSet {
    pub cpuid_vmx: bool,
    pub feature_control_locked: bool,
    pub vmx_inside_smx: bool,
    pub vmx_outside_smx: bool,
}

impl VmxFeatureSet {
    pub const fn from_registers(
        cpuid_leaf1: CpuidLeaf1,
        feature_control: FeatureControlMsr,
    ) -> Self {
        Self {
            cpuid_vmx: cpuid_leaf1.vmx_present(),
            feature_control_locked: feature_control.locked(),
            vmx_inside_smx: feature_control.vmx_inside_smx(),
            vmx_outside_smx: feature_control.vmx_outside_smx(),
        }
    }

    pub const fn validate_non_smx(self) -> Result<Self, VmxError> {
        if !self.cpuid_vmx {
            return Err(VmxError::new(
                VmxErrorKind::MissingCpuidBit,
                "CPUID.1:ECX does not expose VMX support",
            ));
        }
        if !self.feature_control_locked {
            return Err(VmxError::new(
                VmxErrorKind::FeatureControlUnlocked,
                "IA32_FEATURE_CONTROL must be locked before VMX is used",
            ));
        }
        if !self.vmx_outside_smx {
            return Err(VmxError::new(
                VmxErrorKind::VmxDisabledOutsideSmx,
                "IA32_FEATURE_CONTROL disables VMX outside SMX",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrFixedBits {
    pub fixed0: u64,
    pub fixed1: u64,
}

impl CrFixedBits {
    pub const fn new(fixed0: u64, fixed1: u64) -> Self {
        Self { fixed0, fixed1 }
    }

    pub const fn validate(self, value: u64) -> bool {
        (value & self.fixed0) == self.fixed0 && (value & !self.fixed1) == 0
    }
}

pub const fn validate_control_register(
    value: u64,
    fixed: CrFixedBits,
    message: &'static str,
) -> Result<u64, VmxError> {
    if fixed.validate(value) {
        Ok(value)
    } else {
        Err(VmxError::new(VmxErrorKind::InvalidGuestState, message))
    }
}

pub const fn is_canonical_u64(value: u64) -> bool {
    let high = value >> 48;
    let sign = (value >> 47) & 1;
    (sign == 0 && high == 0) || (sign == 1 && high == 0xffff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vmx_feature_validation_accepts_locked_non_smx_vmx() {
        let features = VmxFeatureSet::from_registers(
            CpuidLeaf1 {
                ecx: CPUID_LEAF1_ECX_VMX,
            },
            FeatureControlMsr::new(
                IA32_FEATURE_CONTROL_LOCK | IA32_FEATURE_CONTROL_VMX_OUTSIDE_SMX,
            ),
        );

        assert_eq!(features.validate_non_smx().unwrap(), features);
    }

    #[test]
    fn vmx_feature_validation_rejects_missing_cpuid_bit() {
        let err = VmxFeatureSet::from_registers(
            CpuidLeaf1 { ecx: 0 },
            FeatureControlMsr::new(
                IA32_FEATURE_CONTROL_LOCK | IA32_FEATURE_CONTROL_VMX_OUTSIDE_SMX,
            ),
        )
        .validate_non_smx()
        .unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::MissingCpuidBit);
    }

    #[test]
    fn vmx_feature_validation_rejects_unlocked_feature_control_msr() {
        let err = VmxFeatureSet::from_registers(
            CpuidLeaf1 {
                ecx: CPUID_LEAF1_ECX_VMX,
            },
            FeatureControlMsr::new(IA32_FEATURE_CONTROL_VMX_OUTSIDE_SMX),
        )
        .validate_non_smx()
        .unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::FeatureControlUnlocked);
    }

    #[test]
    fn fixed_bits_reject_forced_zero_and_forced_one_violations() {
        let fixed = CrFixedBits::new(0b0101, 0b0111);

        assert!(fixed.validate(0b0101));
        assert!(!fixed.validate(0b0001));
        assert!(!fixed.validate(0b1101));
    }
}
