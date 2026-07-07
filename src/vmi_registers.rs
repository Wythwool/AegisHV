use crate::vmi::RegisterReadError;

const ARCH_X86_64: &str = "x86_64";
const ARCH_ARM64: &str = "arm64";
const X86_CR4_LA57: u64 = 1 << 12;
const X86_EFER_LME: u64 = 1 << 8;
const X86_EFER_LMA: u64 = 1 << 10;
const X86_EFER_NXE: u64 = 1 << 11;
const ARM64_SCTLR_M: u64 = 1 << 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DescriptorTableRegister {
    pub base: u64,
    pub limit: u16,
}

impl DescriptorTableRegister {
    pub fn new(base: u64, limit: u16) -> Self {
        Self { base, limit }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X86_64RegisterSnapshot {
    pub cr0: Option<u64>,
    pub cr2: Option<u64>,
    pub cr3: Option<u64>,
    pub cr4: Option<u64>,
    pub efer: Option<u64>,
    pub lstar: Option<u64>,
    pub idtr: Option<DescriptorTableRegister>,
    pub gdtr: Option<DescriptorTableRegister>,
}

impl X86_64RegisterSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cr0: u64,
        cr2: u64,
        cr3: u64,
        cr4: u64,
        efer: u64,
        idtr: DescriptorTableRegister,
        gdtr: DescriptorTableRegister,
    ) -> Self {
        Self {
            cr0: Some(cr0),
            cr2: Some(cr2),
            cr3: Some(cr3),
            cr4: Some(cr4),
            efer: Some(efer),
            lstar: None,
            idtr: Some(idtr),
            gdtr: Some(gdtr),
        }
    }

    pub fn with_lstar(mut self, lstar: u64) -> Self {
        self.lstar = Some(lstar);
        self
    }

    pub fn partial() -> Self {
        Self {
            cr0: None,
            cr2: None,
            cr3: None,
            cr4: None,
            efer: None,
            lstar: None,
            idtr: None,
            gdtr: None,
        }
    }

    pub fn cr0(&self) -> Result<u64, RegisterReadError> {
        required_u64(ARCH_X86_64, "cr0", self.cr0)
    }

    pub fn cr2(&self) -> Result<u64, RegisterReadError> {
        required_u64(ARCH_X86_64, "cr2", self.cr2)
    }

    pub fn cr3(&self) -> Result<u64, RegisterReadError> {
        required_u64(ARCH_X86_64, "cr3", self.cr3)
    }

    pub fn cr4(&self) -> Result<u64, RegisterReadError> {
        required_u64(ARCH_X86_64, "cr4", self.cr4)
    }

    pub fn efer(&self) -> Result<u64, RegisterReadError> {
        required_u64(ARCH_X86_64, "efer", self.efer)
    }

    pub fn lstar(&self) -> Result<u64, RegisterReadError> {
        required_u64(ARCH_X86_64, "lstar", self.lstar)
    }

    pub fn idtr(&self) -> Result<DescriptorTableRegister, RegisterReadError> {
        required_descriptor(ARCH_X86_64, "idtr", self.idtr)
    }

    pub fn gdtr(&self) -> Result<DescriptorTableRegister, RegisterReadError> {
        required_descriptor(ARCH_X86_64, "gdtr", self.gdtr)
    }

    pub fn la57_enabled(&self) -> Result<bool, RegisterReadError> {
        Ok(self.cr4()? & X86_CR4_LA57 != 0)
    }

    pub fn long_mode_enabled(&self) -> Result<bool, RegisterReadError> {
        Ok(self.efer()? & X86_EFER_LME != 0)
    }

    pub fn long_mode_active(&self) -> Result<bool, RegisterReadError> {
        Ok(self.efer()? & X86_EFER_LMA != 0)
    }

    pub fn nx_enabled(&self) -> Result<bool, RegisterReadError> {
        Ok(self.efer()? & X86_EFER_NXE != 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arm64RegisterSnapshot {
    pub ttbr0_el1: Option<u64>,
    pub ttbr1_el1: Option<u64>,
    pub tcr_el1: Option<u64>,
    pub sctlr_el1: Option<u64>,
    pub vbar_el1: Option<u64>,
}

impl Arm64RegisterSnapshot {
    pub fn new(
        ttbr0_el1: u64,
        ttbr1_el1: u64,
        tcr_el1: u64,
        sctlr_el1: u64,
        vbar_el1: u64,
    ) -> Self {
        Self {
            ttbr0_el1: Some(ttbr0_el1),
            ttbr1_el1: Some(ttbr1_el1),
            tcr_el1: Some(tcr_el1),
            sctlr_el1: Some(sctlr_el1),
            vbar_el1: Some(vbar_el1),
        }
    }

    pub fn partial() -> Self {
        Self {
            ttbr0_el1: None,
            ttbr1_el1: None,
            tcr_el1: None,
            sctlr_el1: None,
            vbar_el1: None,
        }
    }

    pub fn ttbr0_el1(&self) -> Result<u64, RegisterReadError> {
        required_u64(ARCH_ARM64, "ttbr0_el1", self.ttbr0_el1)
    }

    pub fn ttbr1_el1(&self) -> Result<u64, RegisterReadError> {
        required_u64(ARCH_ARM64, "ttbr1_el1", self.ttbr1_el1)
    }

    pub fn tcr_el1(&self) -> Result<u64, RegisterReadError> {
        required_u64(ARCH_ARM64, "tcr_el1", self.tcr_el1)
    }

    pub fn sctlr_el1(&self) -> Result<u64, RegisterReadError> {
        required_u64(ARCH_ARM64, "sctlr_el1", self.sctlr_el1)
    }

    pub fn vbar_el1(&self) -> Result<u64, RegisterReadError> {
        required_u64(ARCH_ARM64, "vbar_el1", self.vbar_el1)
    }

    pub fn mmu_enabled(&self) -> Result<bool, RegisterReadError> {
        Ok(self.sctlr_el1()? & ARM64_SCTLR_M != 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterSnapshot {
    X86_64(X86_64RegisterSnapshot),
    Arm64(Arm64RegisterSnapshot),
    UnsupportedArchitecture { arch: String },
}

impl RegisterSnapshot {
    pub fn x86_64(snapshot: X86_64RegisterSnapshot) -> Self {
        Self::X86_64(snapshot)
    }

    pub fn arm64(snapshot: Arm64RegisterSnapshot) -> Self {
        Self::Arm64(snapshot)
    }

    pub fn unsupported_architecture(arch: impl Into<String>) -> Self {
        Self::UnsupportedArchitecture { arch: arch.into() }
    }

    pub fn architecture(&self) -> &str {
        match self {
            Self::X86_64(_) => ARCH_X86_64,
            Self::Arm64(_) => ARCH_ARM64,
            Self::UnsupportedArchitecture { arch } => arch.as_str(),
        }
    }

    pub fn as_x86_64(&self) -> Result<&X86_64RegisterSnapshot, RegisterReadError> {
        match self {
            Self::X86_64(snapshot) => Ok(snapshot),
            Self::Arm64(_) => Err(wrong_architecture(ARCH_X86_64, ARCH_ARM64)),
            Self::UnsupportedArchitecture { arch } => {
                Err(RegisterReadError::UnsupportedArchitecture { arch: arch.clone() })
            }
        }
    }

    pub fn as_arm64(&self) -> Result<&Arm64RegisterSnapshot, RegisterReadError> {
        match self {
            Self::Arm64(snapshot) => Ok(snapshot),
            Self::X86_64(_) => Err(wrong_architecture(ARCH_ARM64, ARCH_X86_64)),
            Self::UnsupportedArchitecture { arch } => {
                Err(RegisterReadError::UnsupportedArchitecture { arch: arch.clone() })
            }
        }
    }

    pub fn x86_cr3(&self) -> Result<u64, RegisterReadError> {
        self.as_x86_64()?.cr3()
    }

    pub fn x86_la57_enabled(&self) -> Result<bool, RegisterReadError> {
        self.as_x86_64()?.la57_enabled()
    }

    pub fn x86_long_mode_enabled(&self) -> Result<bool, RegisterReadError> {
        self.as_x86_64()?.long_mode_enabled()
    }

    pub fn x86_long_mode_active(&self) -> Result<bool, RegisterReadError> {
        self.as_x86_64()?.long_mode_active()
    }

    pub fn x86_nx_enabled(&self) -> Result<bool, RegisterReadError> {
        self.as_x86_64()?.nx_enabled()
    }

    pub fn x86_lstar(&self) -> Result<u64, RegisterReadError> {
        self.as_x86_64()?.lstar()
    }

    pub fn x86_idtr(&self) -> Result<DescriptorTableRegister, RegisterReadError> {
        self.as_x86_64()?.idtr()
    }

    pub fn x86_gdtr(&self) -> Result<DescriptorTableRegister, RegisterReadError> {
        self.as_x86_64()?.gdtr()
    }

    pub fn arm64_ttbr0_el1(&self) -> Result<u64, RegisterReadError> {
        self.as_arm64()?.ttbr0_el1()
    }

    pub fn arm64_ttbr1_el1(&self) -> Result<u64, RegisterReadError> {
        self.as_arm64()?.ttbr1_el1()
    }

    pub fn arm64_tcr_el1(&self) -> Result<u64, RegisterReadError> {
        self.as_arm64()?.tcr_el1()
    }

    pub fn arm64_sctlr_el1(&self) -> Result<u64, RegisterReadError> {
        self.as_arm64()?.sctlr_el1()
    }

    pub fn arm64_vbar_el1(&self) -> Result<u64, RegisterReadError> {
        self.as_arm64()?.vbar_el1()
    }
}

fn required_u64(
    arch: &'static str,
    register: &'static str,
    value: Option<u64>,
) -> Result<u64, RegisterReadError> {
    value.ok_or(RegisterReadError::MissingRegister { arch, register })
}

fn required_descriptor(
    arch: &'static str,
    register: &'static str,
    value: Option<DescriptorTableRegister>,
) -> Result<DescriptorTableRegister, RegisterReadError> {
    value.ok_or(RegisterReadError::MissingRegister { arch, register })
}

fn wrong_architecture(expected: &'static str, actual: &'static str) -> RegisterReadError {
    RegisterReadError::WrongArchitecture { expected, actual }
}
