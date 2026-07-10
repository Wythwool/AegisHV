use super::features::{VmxError, VmxErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmxExitReason {
    Cpuid,
    Hlt,
    Rdmsr,
    Wrmsr,
    CrAccess,
    EptViolation,
    MonitorTrapFlag,
    VmEntryFailure(u32),
    Unknown(u32),
}

impl VmxExitReason {
    pub const fn from_basic_reason(raw: u32) -> Self {
        if raw & (1 << 31) != 0 {
            return Self::VmEntryFailure(raw & 0xffff);
        }
        match raw & 0xffff {
            10 => Self::Cpuid,
            12 => Self::Hlt,
            28 => Self::CrAccess,
            31 => Self::Rdmsr,
            32 => Self::Wrmsr,
            48 => Self::EptViolation,
            37 => Self::MonitorTrapFlag,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeneralRegisters {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
}

impl GeneralRegisters {
    pub fn read_gp(self, index: u8) -> Result<u64, VmxError> {
        Ok(match index {
            0 => self.rax,
            1 => self.rcx,
            2 => self.rdx,
            3 => self.rbx,
            4 => self.rsp,
            5 => self.rbp,
            6 => self.rsi,
            7 => self.rdi,
            8 => self.r8,
            9 => self.r9,
            10 => self.r10,
            11 => self.r11,
            12 => self.r12,
            13 => self.r13,
            14 => self.r14,
            15 => self.r15,
            _ => {
                return Err(VmxError::new(
                    VmxErrorKind::UnsupportedControlRegister,
                    "CR access names a general register outside the x86-64 set",
                ))
            }
        })
    }

    pub fn write_gp(&mut self, index: u8, value: u64) -> Result<(), VmxError> {
        match index {
            0 => self.rax = value,
            1 => self.rcx = value,
            2 => self.rdx = value,
            3 => self.rbx = value,
            4 => self.rsp = value,
            5 => self.rbp = value,
            6 => self.rsi = value,
            7 => self.rdi = value,
            8 => self.r8 = value,
            9 => self.r9 = value,
            10 => self.r10 = value,
            11 => self.r11 = value,
            12 => self.r12 = value,
            13 => self.r13 = value,
            14 => self.r14 = value,
            15 => self.r15 = value,
            _ => {
                return Err(VmxError::new(
                    VmxErrorKind::UnsupportedControlRegister,
                    "CR access names a general register outside the x86-64 set",
                ))
            }
        }
        Ok(())
    }

    pub fn advance_rip(&mut self, instruction_len: u32) -> Result<(), VmxError> {
        if instruction_len == 0 {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedExit,
                "VM-exit instruction length is zero",
            ));
        }
        self.rip = self
            .rip
            .checked_add(instruction_len as u64)
            .ok_or(VmxError::new(
                VmxErrorKind::UnsupportedExit,
                "advancing RIP after VM-exit overflowed",
            ))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitAction {
    Resume,
    HaltGuest,
    InjectUnsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuidResult {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuidLeaf {
    pub leaf: u32,
    pub subleaf: u32,
    pub result: CpuidResult,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticCpuidPolicy<const N: usize> {
    leaves: [CpuidLeaf; N],
}

impl<const N: usize> StaticCpuidPolicy<N> {
    pub const fn new(leaves: [CpuidLeaf; N]) -> Self {
        Self { leaves }
    }

    pub fn lookup(&self, leaf: u32, subleaf: u32) -> Result<CpuidResult, VmxError> {
        for entry in &self.leaves {
            if entry.leaf == leaf && entry.subleaf == subleaf {
                return Ok(entry.result);
            }
        }
        Err(VmxError::new(
            VmxErrorKind::UnsupportedExit,
            "CPUID exit reached a leaf that is not in the explicit policy",
        ))
    }
}

pub fn handle_cpuid_exit<const N: usize>(
    regs: &mut GeneralRegisters,
    policy: &StaticCpuidPolicy<N>,
    instruction_len: u32,
) -> Result<ExitAction, VmxError> {
    let result = policy.lookup(regs.rax as u32, regs.rcx as u32)?;
    regs.rax = result.eax as u64;
    regs.rbx = result.ebx as u64;
    regs.rcx = result.ecx as u64;
    regs.rdx = result.edx as u64;
    regs.advance_rip(instruction_len)?;
    Ok(ExitAction::Resume)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MsrEntry {
    pub index: u32,
    pub value: u64,
    pub writable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticMsrPolicy<const N: usize> {
    entries: [MsrEntry; N],
}

impl<const N: usize> StaticMsrPolicy<N> {
    pub const fn new(entries: [MsrEntry; N]) -> Self {
        Self { entries }
    }

    pub fn read(&self, index: u32) -> Result<u64, VmxError> {
        for entry in &self.entries {
            if entry.index == index {
                return Ok(entry.value);
            }
        }
        Err(VmxError::new(
            VmxErrorKind::UnsupportedMsr,
            "RDMSR exit reached an MSR that is not in the explicit policy",
        ))
    }

    pub fn writable(&self, index: u32) -> Result<bool, VmxError> {
        for entry in &self.entries {
            if entry.index == index {
                return Ok(entry.writable);
            }
        }
        Err(VmxError::new(
            VmxErrorKind::UnsupportedMsr,
            "WRMSR exit reached an MSR that is not in the explicit policy",
        ))
    }
}

pub fn handle_rdmsr_exit<const N: usize>(
    regs: &mut GeneralRegisters,
    policy: &StaticMsrPolicy<N>,
    instruction_len: u32,
) -> Result<ExitAction, VmxError> {
    let value = policy.read(regs.rcx as u32)?;
    regs.rax = value & 0xffff_ffff;
    regs.rdx = value >> 32;
    regs.advance_rip(instruction_len)?;
    Ok(ExitAction::Resume)
}

pub fn handle_wrmsr_exit<const N: usize>(
    regs: &mut GeneralRegisters,
    policy: &StaticMsrPolicy<N>,
    instruction_len: u32,
) -> Result<ExitAction, VmxError> {
    if !policy.writable(regs.rcx as u32)? {
        return Err(VmxError::new(
            VmxErrorKind::UnsupportedMsr,
            "WRMSR exit tried to write a read-only policy MSR",
        ));
    }
    regs.advance_rip(instruction_len)?;
    Ok(ExitAction::Resume)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrAccessType {
    MoveToCr,
    MoveFromCr,
    Clts,
    Lmsw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrAccessQualification {
    pub cr_number: u8,
    pub access_type: CrAccessType,
    pub gp_register: u8,
}

impl CrAccessQualification {
    pub const fn decode(raw: u64) -> Result<Self, VmxError> {
        let access_type = match (raw >> 4) & 0x3 {
            0 => CrAccessType::MoveToCr,
            1 => CrAccessType::MoveFromCr,
            2 => CrAccessType::Clts,
            3 => CrAccessType::Lmsw,
            _ => CrAccessType::MoveToCr,
        };
        Ok(Self {
            cr_number: (raw & 0xf) as u8,
            access_type,
            gp_register: ((raw >> 8) & 0xf) as u8,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlRegisters {
    pub cr0: u64,
    pub cr3: u64,
    pub cr4: u64,
}

impl ControlRegisters {
    fn read(self, cr_number: u8) -> Result<u64, VmxError> {
        Ok(match cr_number {
            0 => self.cr0,
            3 => self.cr3,
            4 => self.cr4,
            _ => {
                return Err(VmxError::new(
                    VmxErrorKind::UnsupportedControlRegister,
                    "CR access names a control register that is not modeled",
                ))
            }
        })
    }

    fn write(&mut self, cr_number: u8, value: u64) -> Result<(), VmxError> {
        match cr_number {
            0 => self.cr0 = value,
            3 => self.cr3 = value,
            4 => self.cr4 = value,
            _ => {
                return Err(VmxError::new(
                    VmxErrorKind::UnsupportedControlRegister,
                    "CR access names a control register that is not modeled",
                ))
            }
        }
        Ok(())
    }
}

pub fn handle_cr_access_exit(
    regs: &mut GeneralRegisters,
    controls: &mut ControlRegisters,
    qualification: CrAccessQualification,
    instruction_len: u32,
) -> Result<ExitAction, VmxError> {
    match qualification.access_type {
        CrAccessType::MoveToCr => {
            let value = regs.read_gp(qualification.gp_register)?;
            controls.write(qualification.cr_number, value)?;
        }
        CrAccessType::MoveFromCr => {
            let value = controls.read(qualification.cr_number)?;
            regs.write_gp(qualification.gp_register, value)?;
        }
        CrAccessType::Clts | CrAccessType::Lmsw => {
            return Err(VmxError::new(
                VmxErrorKind::UnsupportedControlRegister,
                "CLTS and LMSW exits are not handled by the lab model",
            ))
        }
    }
    regs.advance_rip(instruction_len)?;
    Ok(ExitAction::Resume)
}

pub fn handle_hlt_exit(
    regs: &mut GeneralRegisters,
    instruction_len: u32,
) -> Result<ExitAction, VmxError> {
    regs.advance_rip(instruction_len)?;
    Ok(ExitAction::HaltGuest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpuid_exit_uses_explicit_policy_and_advances_rip() {
        let policy = StaticCpuidPolicy::new([CpuidLeaf {
            leaf: 0,
            subleaf: 0,
            result: CpuidResult {
                eax: 1,
                ebx: 2,
                ecx: 3,
                edx: 4,
            },
        }]);
        let mut regs = GeneralRegisters {
            rip: 0x1000,
            ..Default::default()
        };

        assert_eq!(
            handle_cpuid_exit(&mut regs, &policy, 2).unwrap(),
            ExitAction::Resume
        );
        assert_eq!(
            (regs.rax, regs.rbx, regs.rcx, regs.rdx, regs.rip),
            (1, 2, 3, 4, 0x1002)
        );
    }

    #[test]
    fn exit_reason_preserves_vm_entry_failure() {
        assert_eq!(
            VmxExitReason::from_basic_reason((1 << 31) | 33),
            VmxExitReason::VmEntryFailure(33)
        );
    }

    #[test]
    fn cpuid_exit_rejects_unlisted_leaf() {
        let policy = StaticCpuidPolicy::<0>::new([]);
        let mut regs = GeneralRegisters {
            rax: 7,
            ..Default::default()
        };

        assert_eq!(
            handle_cpuid_exit(&mut regs, &policy, 2).unwrap_err().kind,
            VmxErrorKind::UnsupportedExit
        );
    }

    #[test]
    fn msr_handlers_split_read_value_and_reject_read_only_write() {
        let policy = StaticMsrPolicy::new([MsrEntry {
            index: 0x174,
            value: 0x1122_3344_5566_7788,
            writable: false,
        }]);
        let mut regs = GeneralRegisters {
            rcx: 0x174,
            rip: 0x80,
            ..Default::default()
        };

        handle_rdmsr_exit(&mut regs, &policy, 2).unwrap();
        assert_eq!(regs.rax, 0x5566_7788);
        assert_eq!(regs.rdx, 0x1122_3344);

        assert_eq!(
            handle_wrmsr_exit(&mut regs, &policy, 2).unwrap_err().kind,
            VmxErrorKind::UnsupportedMsr
        );
    }

    #[test]
    fn cr_access_moves_values_between_gp_and_control_registers() {
        let raw = 3 | (1 << 8);
        let qualification = CrAccessQualification::decode(raw).unwrap();
        let mut regs = GeneralRegisters {
            rcx: 0x9000,
            rip: 0x40,
            ..Default::default()
        };
        let mut controls = ControlRegisters::default();

        handle_cr_access_exit(&mut regs, &mut controls, qualification, 3).unwrap();

        assert_eq!(controls.cr3, 0x9000);
        assert_eq!(regs.rip, 0x43);
    }
}
