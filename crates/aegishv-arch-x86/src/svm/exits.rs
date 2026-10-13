use super::features::{SvmError, SvmErrorKind};

pub const SVM_EXIT_CPUID: u64 = 0x72;
pub const SVM_EXIT_HLT: u64 = 0x78;
pub const SVM_EXIT_PAUSE: u64 = 0x77;
pub const SVM_EXIT_IOIO: u64 = 0x7b;
pub const SVM_EXIT_MSR: u64 = 0x7c;
pub const SVM_EXIT_NPF: u64 = 0x400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvmExitCode {
    Cpuid,
    Hlt,
    Pause,
    Ioio,
    Msr,
    CrRead(u8),
    CrWrite(u8),
    NestedPageFault,
    Unknown(u64),
}

impl SvmExitCode {
    pub const fn from_raw(raw: u64) -> Self {
        match raw {
            SVM_EXIT_CPUID => Self::Cpuid,
            SVM_EXIT_HLT => Self::Hlt,
            SVM_EXIT_PAUSE => Self::Pause,
            SVM_EXIT_IOIO => Self::Ioio,
            SVM_EXIT_MSR => Self::Msr,
            SVM_EXIT_NPF => Self::NestedPageFault,
            0x000..=0x00f => Self::CrRead(raw as u8),
            0x010..=0x01f => Self::CrWrite((raw - 0x10) as u8),
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
    pub fn read_gp(self, index: u8) -> Result<u64, SvmError> {
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
                return Err(SvmError::new(
                    SvmErrorKind::InvalidControlRegister,
                    "SVM intercept names a general register outside the x86-64 set",
                ))
            }
        })
    }

    pub fn write_gp(&mut self, index: u8, value: u64) -> Result<(), SvmError> {
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
                return Err(SvmError::new(
                    SvmErrorKind::InvalidControlRegister,
                    "SVM intercept names a general register outside the x86-64 set",
                ))
            }
        }
        Ok(())
    }

    pub fn advance_rip(&mut self, instruction_len: u8) -> Result<(), SvmError> {
        if instruction_len == 0 {
            return Err(SvmError::new(
                SvmErrorKind::UnsupportedExit,
                "SVM intercept did not provide a next-RIP length",
            ));
        }
        self.rip = self
            .rip
            .checked_add(instruction_len as u64)
            .ok_or(SvmError::new(
                SvmErrorKind::UnsupportedExit,
                "advancing RIP after SVM intercept overflowed",
            ))?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlRegisters {
    pub cr0: u64,
    pub cr3: u64,
    pub cr4: u64,
}

impl ControlRegisters {
    fn read(self, cr_number: u8) -> Result<u64, SvmError> {
        Ok(match cr_number {
            0 => self.cr0,
            3 => self.cr3,
            4 => self.cr4,
            _ => {
                return Err(SvmError::new(
                    SvmErrorKind::InvalidControlRegister,
                    "SVM CR intercept names a control register that is not modeled",
                ))
            }
        })
    }

    fn write(&mut self, cr_number: u8, value: u64) -> Result<(), SvmError> {
        match cr_number {
            0 => self.cr0 = value,
            3 => self.cr3 = value,
            4 => self.cr4 = value,
            _ => {
                return Err(SvmError::new(
                    SvmErrorKind::InvalidControlRegister,
                    "SVM CR intercept names a control register that is not modeled",
                ))
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvmExitAction {
    Resume,
    HaltGuest,
    PauseGuest,
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

    pub fn lookup(&self, leaf: u32, subleaf: u32) -> Result<CpuidResult, SvmError> {
        for entry in &self.leaves {
            if entry.leaf == leaf && entry.subleaf == subleaf {
                return Ok(entry.result);
            }
        }
        Err(SvmError::new(
            SvmErrorKind::UnsupportedExit,
            "SVM CPUID intercept reached a leaf outside the explicit policy",
        ))
    }
}

pub fn handle_cpuid<const N: usize>(
    regs: &mut GeneralRegisters,
    policy: &StaticCpuidPolicy<N>,
    instruction_len: u8,
) -> Result<SvmExitAction, SvmError> {
    let result = policy.lookup(regs.rax as u32, regs.rcx as u32)?;
    regs.rax = result.eax as u64;
    regs.rbx = result.ebx as u64;
    regs.rcx = result.ecx as u64;
    regs.rdx = result.edx as u64;
    regs.advance_rip(instruction_len)?;
    Ok(SvmExitAction::Resume)
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

    pub fn read(&self, index: u32) -> Result<u64, SvmError> {
        for entry in &self.entries {
            if entry.index == index {
                return Ok(entry.value);
            }
        }
        Err(SvmError::new(
            SvmErrorKind::UnsupportedMsr,
            "SVM MSR intercept reached an MSR outside the explicit policy",
        ))
    }

    pub fn writable(&self, index: u32) -> Result<bool, SvmError> {
        for entry in &self.entries {
            if entry.index == index {
                return Ok(entry.writable);
            }
        }
        Err(SvmError::new(
            SvmErrorKind::UnsupportedMsr,
            "SVM MSR intercept reached an MSR outside the explicit policy",
        ))
    }
}

pub fn handle_msr_read<const N: usize>(
    regs: &mut GeneralRegisters,
    policy: &StaticMsrPolicy<N>,
    instruction_len: u8,
) -> Result<SvmExitAction, SvmError> {
    let value = policy.read(regs.rcx as u32)?;
    regs.rax = value & 0xffff_ffff;
    regs.rdx = value >> 32;
    regs.advance_rip(instruction_len)?;
    Ok(SvmExitAction::Resume)
}

pub fn handle_msr_write<const N: usize>(
    regs: &mut GeneralRegisters,
    policy: &StaticMsrPolicy<N>,
    instruction_len: u8,
) -> Result<SvmExitAction, SvmError> {
    if !policy.writable(regs.rcx as u32)? {
        return Err(SvmError::new(
            SvmErrorKind::UnsupportedMsr,
            "SVM MSR intercept tried to write a read-only policy MSR",
        ));
    }
    regs.advance_rip(instruction_len)?;
    Ok(SvmExitAction::Resume)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrIntercept {
    pub cr_number: u8,
    pub write: bool,
    pub gp_register: u8,
}

pub fn handle_cr_intercept(
    regs: &mut GeneralRegisters,
    controls: &mut ControlRegisters,
    intercept: CrIntercept,
    instruction_len: u8,
) -> Result<SvmExitAction, SvmError> {
    if intercept.write {
        let value = regs.read_gp(intercept.gp_register)?;
        controls.write(intercept.cr_number, value)?;
    } else {
        let value = controls.read(intercept.cr_number)?;
        regs.write_gp(intercept.gp_register, value)?;
    }
    regs.advance_rip(instruction_len)?;
    Ok(SvmExitAction::Resume)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoDirection {
    In,
    Out,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IoIntercept {
    pub port: u16,
    pub size: u8,
    pub direction: IoDirection,
}

impl IoIntercept {
    pub const fn decode(exit_info1: u64) -> Result<Self, SvmError> {
        let size = (exit_info1 & 0x7) as u8;
        if size == 0 || size > 4 {
            return Err(SvmError::new(
                SvmErrorKind::UnsupportedIo,
                "SVM IO intercept has an unsupported access size",
            ));
        }
        let direction = if exit_info1 & (1 << 3) != 0 {
            IoDirection::In
        } else {
            IoDirection::Out
        };
        Ok(Self {
            port: ((exit_info1 >> 16) & 0xffff) as u16,
            size,
            direction,
        })
    }
}

pub fn handle_io_intercept(
    regs: &mut GeneralRegisters,
    intercept: IoIntercept,
    allowed_port: u16,
    instruction_len: u8,
) -> Result<SvmExitAction, SvmError> {
    if intercept.port != allowed_port {
        return Err(SvmError::new(
            SvmErrorKind::UnsupportedIo,
            "SVM IO intercept reached a port outside the lab policy",
        ));
    }
    if intercept.direction == IoDirection::In {
        regs.rax = 0;
    }
    regs.advance_rip(instruction_len)?;
    Ok(SvmExitAction::Resume)
}

pub fn handle_hlt(
    regs: &mut GeneralRegisters,
    instruction_len: u8,
) -> Result<SvmExitAction, SvmError> {
    regs.advance_rip(instruction_len)?;
    Ok(SvmExitAction::HaltGuest)
}

pub fn handle_pause(
    regs: &mut GeneralRegisters,
    instruction_len: u8,
) -> Result<SvmExitAction, SvmError> {
    regs.advance_rip(instruction_len)?;
    Ok(SvmExitAction::PauseGuest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_decoder_keeps_svm_specific_codes() {
        assert_eq!(SvmExitCode::from_raw(SVM_EXIT_HLT), SvmExitCode::Hlt);
        assert_eq!(SvmExitCode::from_raw(0x13), SvmExitCode::CrWrite(3));
        assert_eq!(
            SvmExitCode::from_raw(SVM_EXIT_NPF),
            SvmExitCode::NestedPageFault
        );
    }

    #[test]
    fn cpuid_intercept_uses_policy_and_advances_rip() {
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
            handle_cpuid(&mut regs, &policy, 2).unwrap(),
            SvmExitAction::Resume
        );
        assert_eq!(
            (regs.rax, regs.rbx, regs.rcx, regs.rdx, regs.rip),
            (1, 2, 3, 4, 0x1002)
        );
    }

    #[test]
    fn msr_intercept_rejects_write_to_read_only_entry() {
        let policy = StaticMsrPolicy::new([MsrEntry {
            index: 0xc000_0080,
            value: 0x500,
            writable: false,
        }]);
        let mut regs = GeneralRegisters {
            rcx: 0xc000_0080,
            ..Default::default()
        };

        assert_eq!(
            handle_msr_write(&mut regs, &policy, 2).unwrap_err().kind,
            SvmErrorKind::UnsupportedMsr
        );
    }

    #[test]
    fn cr_intercept_moves_values_between_register_sets() {
        let mut regs = GeneralRegisters {
            rax: 0x9000,
            rip: 0x40,
            ..Default::default()
        };
        let mut controls = ControlRegisters::default();

        handle_cr_intercept(
            &mut regs,
            &mut controls,
            CrIntercept {
                cr_number: 3,
                write: true,
                gp_register: 0,
            },
            3,
        )
        .unwrap();

        assert_eq!(controls.cr3, 0x9000);
        assert_eq!(regs.rip, 0x43);
    }

    #[test]
    fn io_intercept_rejects_unlisted_port() {
        let mut regs = GeneralRegisters::default();
        let err = handle_io_intercept(
            &mut regs,
            IoIntercept {
                port: 0xcf9,
                size: 1,
                direction: IoDirection::Out,
            },
            0x3f8,
            1,
        )
        .unwrap_err();

        assert_eq!(err.kind, SvmErrorKind::UnsupportedIo);
    }
}
