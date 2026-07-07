use core::fmt;

pub const CPUID_EXT1_ECX_SVM: u32 = 1 << 2;
pub const CPUID_SVM_EDX_NPT: u32 = 1 << 0;
pub const CPUID_SVM_EDX_FLUSH_BY_ASID: u32 = 1 << 6;
pub const CPUID_SVM_EDX_DECODE_ASSISTS: u32 = 1 << 7;
pub const CPUID_SVM_EDX_AVIC: u32 = 1 << 13;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvmErrorKind {
    MissingCpuidBit,
    MissingNpt,
    MissingAsidCapacity,
    InvalidAsid,
    InvalidVmcbAddress,
    InvalidVmcbState,
    InvalidIntercept,
    InvalidControlRegister,
    UnsupportedCapability,
    UnsupportedExit,
    UnsupportedMsr,
    UnsupportedIo,
    InvalidNptMapping,
    InvalidNestedPageFault,
    InstructionFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvmError {
    pub kind: SvmErrorKind,
    pub message: &'static str,
}

impl SvmError {
    pub const fn new(kind: SvmErrorKind, message: &'static str) -> Self {
        Self { kind, message }
    }
}

impl fmt::Display for SvmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SvmError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvmCpuidExt1 {
    pub ecx: u32,
}

impl SvmCpuidExt1 {
    pub const fn svm_present(self) -> bool {
        self.ecx & CPUID_EXT1_ECX_SVM != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvmCpuidLeaf {
    pub ebx: u32,
    pub edx: u32,
}

impl SvmCpuidLeaf {
    pub const fn asid_capacity(self) -> u32 {
        self.ebx
    }

    pub const fn npt_present(self) -> bool {
        self.edx & CPUID_SVM_EDX_NPT != 0
    }

    pub const fn flush_by_asid(self) -> bool {
        self.edx & CPUID_SVM_EDX_FLUSH_BY_ASID != 0
    }

    pub const fn decode_assists(self) -> bool {
        self.edx & CPUID_SVM_EDX_DECODE_ASSISTS != 0
    }

    pub const fn avic_present(self) -> bool {
        self.edx & CPUID_SVM_EDX_AVIC != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvmFeatureSet {
    pub svm: bool,
    pub npt: bool,
    pub asid_capacity: u32,
    pub flush_by_asid: bool,
    pub decode_assists: bool,
    pub avic: bool,
}

impl SvmFeatureSet {
    pub const fn from_cpuid(ext1: SvmCpuidExt1, svm_leaf: SvmCpuidLeaf) -> Self {
        Self {
            svm: ext1.svm_present(),
            npt: svm_leaf.npt_present(),
            asid_capacity: svm_leaf.asid_capacity(),
            flush_by_asid: svm_leaf.flush_by_asid(),
            decode_assists: svm_leaf.decode_assists(),
            avic: svm_leaf.avic_present(),
        }
    }

    pub const fn validate_for_npt_lab(self) -> Result<Self, SvmError> {
        if !self.svm {
            return Err(SvmError::new(
                SvmErrorKind::MissingCpuidBit,
                "CPUID.80000001H:ECX does not expose AMD SVM support",
            ));
        }
        if !self.npt {
            return Err(SvmError::new(
                SvmErrorKind::MissingNpt,
                "CPUID.8000000AH does not expose nested paging support",
            ));
        }
        if self.asid_capacity == 0 {
            return Err(SvmError::new(
                SvmErrorKind::MissingAsidCapacity,
                "CPUID.8000000AH reports zero guest ASIDs",
            ));
        }
        Ok(self)
    }
}

pub const EFER_SVME: u64 = 1 << 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EferValue(u64);

impl EferValue {
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }

    pub const fn svme_enabled(self) -> bool {
        self.0 & EFER_SVME != 0
    }

    pub const fn with_svme(self) -> Self {
        Self(self.0 | EFER_SVME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn npt_leaf() -> SvmCpuidLeaf {
        SvmCpuidLeaf {
            ebx: 8,
            edx: CPUID_SVM_EDX_NPT | CPUID_SVM_EDX_FLUSH_BY_ASID,
        }
    }

    #[test]
    fn svm_feature_validation_accepts_svm_npt_and_asids() {
        let features = SvmFeatureSet::from_cpuid(
            SvmCpuidExt1 {
                ecx: CPUID_EXT1_ECX_SVM,
            },
            npt_leaf(),
        );

        assert_eq!(features.validate_for_npt_lab().unwrap(), features);
        assert!(features.flush_by_asid);
    }

    #[test]
    fn svm_feature_validation_rejects_missing_svm_bit() {
        let err = SvmFeatureSet::from_cpuid(SvmCpuidExt1 { ecx: 0 }, npt_leaf())
            .validate_for_npt_lab()
            .unwrap_err();

        assert_eq!(err.kind, SvmErrorKind::MissingCpuidBit);
    }

    #[test]
    fn svm_feature_validation_rejects_missing_npt() {
        let err = SvmFeatureSet::from_cpuid(
            SvmCpuidExt1 {
                ecx: CPUID_EXT1_ECX_SVM,
            },
            SvmCpuidLeaf { ebx: 4, edx: 0 },
        )
        .validate_for_npt_lab()
        .unwrap_err();

        assert_eq!(err.kind, SvmErrorKind::MissingNpt);
    }

    #[test]
    fn efer_value_enables_svme_without_changing_other_bits() {
        let efer = EferValue::new(0x500).with_svme();

        assert!(efer.svme_enabled());
        assert_eq!(efer.raw() & 0x500, 0x500);
    }
}
