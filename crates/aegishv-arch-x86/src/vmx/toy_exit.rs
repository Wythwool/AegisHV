use super::exits::{
    handle_cpuid_exit, CpuidLeaf, CpuidResult, ExitAction, GeneralRegisters, StaticCpuidPolicy,
    VmxExitReason,
};
use super::features::{VmxError, VmxErrorKind};
use super::instructions::VmxInstructionExecutor;
use super::vmcs::VmcsField;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToyVmxExitSequence {
    AwaitingCpuid,
    AwaitingHlt,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToyVmxExitContract {
    pub cpuid_rip: u64,
    pub hlt_rip: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToyVmxExitAction {
    Resume,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToyVmxExitError {
    VmcsRead {
        field: VmcsField,
        kind: VmxErrorKind,
    },
    VmcsWrite {
        field: VmcsField,
        kind: VmxErrorKind,
    },
    VmEntryFailure(u32),
    UnexpectedReason(VmxExitReason),
    InvalidSequence,
    InvalidInstructionLength,
    InvalidGuestRip,
    Cpuid(VmxErrorKind),
}

pub trait ToyVmcsAccess {
    fn read(&mut self, field: VmcsField) -> Result<u64, VmxError>;
    fn write(&mut self, field: VmcsField, value: u64) -> Result<(), VmxError>;
}

pub struct InstructionVmcsAccess<'a, E: VmxInstructionExecutor> {
    executor: &'a mut E,
}

impl<'a, E: VmxInstructionExecutor> InstructionVmcsAccess<'a, E> {
    /// # Safety
    ///
    /// A current VMCS must be loaded and owned exclusively by the current CPU
    /// for the full lifetime of the returned adapter.
    pub unsafe fn new(executor: &'a mut E) -> Self {
        Self { executor }
    }
}

impl<E: VmxInstructionExecutor> ToyVmcsAccess for InstructionVmcsAccess<'_, E> {
    fn read(&mut self, field: VmcsField) -> Result<u64, VmxError> {
        // SAFETY: the adapter's unsafe constructor established that this CPU
        // owns a loaded current VMCS for the adapter lifetime.
        unsafe { self.executor.vmread(field.raw()) }
    }

    fn write(&mut self, field: VmcsField, value: u64) -> Result<(), VmxError> {
        // SAFETY: the adapter's unsafe constructor established that this CPU
        // owns a loaded current VMCS for the adapter lifetime.
        unsafe { self.executor.vmwrite(field.raw(), value) }
    }
}

pub fn dispatch_toy_vmx_exit(
    access: &mut impl ToyVmcsAccess,
    registers: &mut GeneralRegisters,
    sequence: &mut ToyVmxExitSequence,
    contract: ToyVmxExitContract,
) -> Result<ToyVmxExitAction, ToyVmxExitError> {
    let raw_reason = read_vmcs(access, VmcsField::VM_EXIT_REASON)? as u32;
    let reason = VmxExitReason::from_basic_reason(raw_reason);
    if let VmxExitReason::VmEntryFailure(code) = reason {
        return Err(ToyVmxExitError::VmEntryFailure(code));
    }
    let rip = read_vmcs(access, VmcsField::GUEST_RIP)?;
    let rsp = read_vmcs(access, VmcsField::GUEST_RSP)?;
    let instruction_length = read_vmcs(access, VmcsField::VM_EXIT_INSTRUCTION_LENGTH)? as u32;
    registers.rip = rip;
    registers.rsp = rsp;

    match reason {
        VmxExitReason::Cpuid => {
            if *sequence != ToyVmxExitSequence::AwaitingCpuid {
                return Err(ToyVmxExitError::InvalidSequence);
            }
            if rip != contract.cpuid_rip {
                return Err(ToyVmxExitError::InvalidGuestRip);
            }
            if instruction_length != 2 {
                return Err(ToyVmxExitError::InvalidInstructionLength);
            }
            let policy = StaticCpuidPolicy::new([CpuidLeaf {
                leaf: 0,
                subleaf: 0,
                result: CpuidResult {
                    eax: 0,
                    ebx: u32::from_le_bytes(*b"Aegi"),
                    ecx: u32::from_le_bytes(*b"Toy!"),
                    edx: u32::from_le_bytes(*b"sHV "),
                },
            }]);
            match handle_cpuid_exit(registers, &policy, instruction_length)
                .map_err(|error| ToyVmxExitError::Cpuid(error.kind))?
            {
                ExitAction::Resume => {}
                _ => return Err(ToyVmxExitError::InvalidSequence),
            }
            write_vmcs(access, VmcsField::GUEST_RIP, registers.rip)?;
            *sequence = ToyVmxExitSequence::AwaitingHlt;
            Ok(ToyVmxExitAction::Resume)
        }
        VmxExitReason::Hlt => {
            if *sequence != ToyVmxExitSequence::AwaitingHlt {
                return Err(ToyVmxExitError::InvalidSequence);
            }
            if rip != contract.hlt_rip {
                return Err(ToyVmxExitError::InvalidGuestRip);
            }
            if instruction_length != 1 {
                return Err(ToyVmxExitError::InvalidInstructionLength);
            }
            *sequence = ToyVmxExitSequence::Complete;
            Ok(ToyVmxExitAction::Stop)
        }
        other => Err(ToyVmxExitError::UnexpectedReason(other)),
    }
}

fn read_vmcs(access: &mut impl ToyVmcsAccess, field: VmcsField) -> Result<u64, ToyVmxExitError> {
    access
        .read(field)
        .map_err(|error| ToyVmxExitError::VmcsRead {
            field,
            kind: error.kind,
        })
}

fn write_vmcs(
    access: &mut impl ToyVmcsAccess,
    field: VmcsField,
    value: u64,
) -> Result<(), ToyVmxExitError> {
    access
        .write(field, value)
        .map_err(|error| ToyVmxExitError::VmcsWrite {
            field,
            kind: error.kind,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAccess {
        reason: u64,
        rip: u64,
        rsp: u64,
        length: u64,
        rip_write: Option<u64>,
        fail_read: Option<VmcsField>,
        fail_write: bool,
    }

    impl MockAccess {
        fn cpuid() -> Self {
            Self {
                reason: 10,
                rip: 0x1005,
                rsp: 0x2ff0,
                length: 2,
                rip_write: None,
                fail_read: None,
                fail_write: false,
            }
        }
    }

    impl ToyVmcsAccess for MockAccess {
        fn read(&mut self, field: VmcsField) -> Result<u64, VmxError> {
            if self.fail_read == Some(field) {
                return Err(VmxError::new(
                    VmxErrorKind::InstructionFailed,
                    "mock VMREAD failed",
                ));
            }
            Ok(if field == VmcsField::VM_EXIT_REASON {
                self.reason
            } else if field == VmcsField::GUEST_RIP {
                self.rip
            } else if field == VmcsField::GUEST_RSP {
                self.rsp
            } else if field == VmcsField::VM_EXIT_INSTRUCTION_LENGTH {
                self.length
            } else {
                0
            })
        }

        fn write(&mut self, field: VmcsField, value: u64) -> Result<(), VmxError> {
            if self.fail_write {
                return Err(VmxError::new(
                    VmxErrorKind::InstructionFailed,
                    "mock VMWRITE failed",
                ));
            }
            if field == VmcsField::GUEST_RIP {
                self.rip_write = Some(value);
            }
            Ok(())
        }
    }

    fn contract() -> ToyVmxExitContract {
        ToyVmxExitContract {
            cpuid_rip: 0x1005,
            hlt_rip: 0x1007,
        }
    }

    #[test]
    fn cpuid_then_hlt_proves_launch_and_resume_sequence() {
        let mut access = MockAccess::cpuid();
        let mut registers = GeneralRegisters {
            rax: 0,
            rcx: 0,
            ..GeneralRegisters::default()
        };
        let mut sequence = ToyVmxExitSequence::AwaitingCpuid;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract()).unwrap(),
            ToyVmxExitAction::Resume
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingHlt);
        assert_eq!(access.rip_write, Some(0x1007));
        assert_eq!(registers.rbx as u32, u32::from_le_bytes(*b"Aegi"));

        access.reason = 12;
        access.rip = 0x1007;
        access.length = 1;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract()).unwrap(),
            ToyVmxExitAction::Stop
        );
        assert_eq!(sequence, ToyVmxExitSequence::Complete);
    }

    #[test]
    fn hlt_before_cpuid_is_rejected() {
        let mut access = MockAccess::cpuid();
        access.reason = 12;
        access.rip = 0x1007;
        access.length = 1;
        let mut registers = GeneralRegisters::default();
        let mut sequence = ToyVmxExitSequence::AwaitingCpuid;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidSequence
        );
    }

    #[test]
    fn bad_instruction_length_and_vm_entry_failure_are_typed() {
        let mut access = MockAccess::cpuid();
        access.length = 3;
        let mut registers = GeneralRegisters::default();
        let mut sequence = ToyVmxExitSequence::AwaitingCpuid;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidInstructionLength
        );

        access.reason = (1 << 31) | 7;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::VmEntryFailure(7)
        );
    }

    #[test]
    fn vmread_and_vmwrite_failures_preserve_the_field() {
        let mut access = MockAccess::cpuid();
        access.fail_read = Some(VmcsField::GUEST_RIP);
        let mut registers = GeneralRegisters::default();
        let mut sequence = ToyVmxExitSequence::AwaitingCpuid;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::VmcsRead {
                field: VmcsField::GUEST_RIP,
                kind: VmxErrorKind::InstructionFailed,
            }
        );

        access.fail_read = None;
        access.fail_write = true;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::VmcsWrite {
                field: VmcsField::GUEST_RIP,
                kind: VmxErrorKind::InstructionFailed,
            }
        );
    }
}
