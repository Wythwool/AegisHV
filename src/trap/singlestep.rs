use super::stage2::Stage2BackendKind;
use super::{TrapError, TrapErrorKind};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SingleStepCapabilities {
    pub intel_monitor_trap_flag: bool,
    pub x86_trap_flag: bool,
    pub amd_vmcb_single_step: bool,
    pub arm_software_step: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleStepStrategy {
    IntelMonitorTrapFlag,
    X86TrapFlag,
    AmdVmcbSingleStep,
    ArmSoftwareStep,
    SyntheticStep,
}

impl SingleStepStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IntelMonitorTrapFlag => "intel_mtf",
            Self::X86TrapFlag => "x86_tf",
            Self::AmdVmcbSingleStep => "amd_vmcb_single_step",
            Self::ArmSoftwareStep => "arm_software_step",
            Self::SyntheticStep => "synthetic_step",
        }
    }
}

pub fn select_single_step(
    backend: Stage2BackendKind,
    caps: SingleStepCapabilities,
) -> Result<SingleStepStrategy, TrapError> {
    match backend {
        Stage2BackendKind::Synthetic => Ok(SingleStepStrategy::SyntheticStep),
        Stage2BackendKind::IntelEpt => {
            if caps.intel_monitor_trap_flag {
                Ok(SingleStepStrategy::IntelMonitorTrapFlag)
            } else if caps.x86_trap_flag {
                Ok(SingleStepStrategy::X86TrapFlag)
            } else {
                unsupported("Intel EPT trap lifecycle needs MTF or x86 TF fallback")
            }
        }
        Stage2BackendKind::AmdNpt => {
            if caps.amd_vmcb_single_step {
                Ok(SingleStepStrategy::AmdVmcbSingleStep)
            } else if caps.x86_trap_flag {
                Ok(SingleStepStrategy::X86TrapFlag)
            } else {
                unsupported("AMD NPT trap lifecycle needs VMCB single-step or x86 TF fallback")
            }
        }
        Stage2BackendKind::ArmStage2 => {
            if caps.arm_software_step {
                Ok(SingleStepStrategy::ArmSoftwareStep)
            } else {
                unsupported("ARM Stage-2 trap lifecycle needs an explicit single-step strategy")
            }
        }
    }
}

fn unsupported(detail: &'static str) -> Result<SingleStepStrategy, TrapError> {
    Err(TrapError::new(TrapErrorKind::UnsupportedCapability, detail))
}
