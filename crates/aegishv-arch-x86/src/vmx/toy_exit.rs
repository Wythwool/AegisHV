use super::exits::{
    handle_cpuid_exit, handle_rdmsr_exit, CpuidLeaf, CpuidResult, ExitAction, GeneralRegisters,
    IoAccessSize, IoDirection, IoInstructionQualification, MsrEntry, StaticCpuidPolicy,
    StaticMsrPolicy, VmxExitReason,
};
use super::features::{VmxError, VmxErrorKind};
use super::instructions::VmxInstructionExecutor;
use super::vmcs::VmcsField;
use super::vmcs_config::{
    VmcsDescriptorTableState, VmxPat, VMX_CR0_EMULATION, VMX_CR0_MONITOR_COPROCESSOR,
    VMX_CR0_TASK_SWITCHED, VMX_CR4_OSFXSR,
};

pub const TOY_RDMSR_IA32_EFER: u32 = 0xc000_0080;
pub const TOY_RDMSR_IA32_PAT: u32 = 0x277;
pub const TOY_NM_INTERRUPTION_INFO: u64 = 0x8000_0307;
pub const TOY_UD_ENTRY_INTERRUPTION_INFO: u64 = 0x8000_0306;

const INTERRUPTION_INFO_VALID: u64 = 1 << 31;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToyVmxExitSequence {
    AwaitingPreemption,
    AwaitingDeadlineProbe,
    AwaitingIo,
    AwaitingIoBitmapB,
    AwaitingCpuid,
    AwaitingRdmsr,
    AwaitingX87Guard,
    AwaitingSimdGuard,
    AwaitingUdDelivery,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToyVmxExitContract {
    pub initial_rip: u64,
    pub deadline_probe_rips: [u64; 9],
    pub deadline_fallback_rip: u64,
    pub continuation_rip: u64,
    pub io_rip: u64,
    pub io_bitmap_b_rip: u64,
    pub cpuid_rip: u64,
    pub rdmsr_rip: u64,
    pub pat_rdmsr_rip: u64,
    pub x87_guard_rip: u64,
    pub simd_guard_rip: u64,
    pub ud2_rip: u64,
    pub hlt_rip: u64,
    pub pat_mismatch_hlt_rip: u64,
    pub hlt_rsp: u64,
    pub hlt_cs: u16,
    pub hlt_ss: u16,
    pub hlt_rflags: u64,
    pub guest_gdtr: VmcsDescriptorTableState,
    pub guest_idtr: VmcsDescriptorTableState,
    pub ud_handler_cookie: u64,
    pub io_port: u16,
    pub io_bitmap_b_port: u16,
    pub io_value: u8,
    pub preemption_timer_reload: u32,
    pub guest_pat: VmxPat,
    pub guest_cr0: u64,
    pub guest_cr4: u64,
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
    InvalidIoQualification,
    InvalidIoValue,
    InvalidInterruptionInfo,
    InvalidExceptionInjection,
    ExceptionDeliveryFailed(VmxExitReason),
    InvalidIdtVectoringState,
    InvalidGuestPat,
    InvalidGuestStack,
    InvalidGuestCookie,
    InvalidGuestReturnState,
    InvalidDescriptorTableState,
    InvalidFpuGuardState,
    GuestPatMismatch,
    InvalidPreemptionReload,
    ExecutionDeadlineExpired,
    RipAdvance(VmxErrorKind),
    Cpuid(VmxErrorKind),
    Rdmsr(VmxErrorKind),
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
    registers.rip = rip;
    registers.rsp = rsp;
    if read_vmcs(access, VmcsField::GUEST_IA32_PAT)? != contract.guest_pat.raw() {
        return Err(ToyVmxExitError::InvalidGuestPat);
    }

    if *sequence == ToyVmxExitSequence::AwaitingUdDelivery {
        let action = dispatch_ud_delivery_exit(access, registers, reason, contract)?;
        *sequence = ToyVmxExitSequence::Complete;
        return Ok(action);
    }

    if reason == VmxExitReason::PreemptionTimer {
        return match *sequence {
            ToyVmxExitSequence::AwaitingPreemption => {
                if rip == contract.deadline_fallback_rip {
                    return Err(ToyVmxExitError::ExecutionDeadlineExpired);
                }
                if rip != contract.initial_rip {
                    return Err(ToyVmxExitError::InvalidGuestRip);
                }
                rearm_preemption_timer(access, contract.preemption_timer_reload)?;
                *sequence = ToyVmxExitSequence::AwaitingDeadlineProbe;
                Ok(ToyVmxExitAction::Resume)
            }
            ToyVmxExitSequence::AwaitingDeadlineProbe => {
                if rip == contract.deadline_fallback_rip {
                    return Err(ToyVmxExitError::ExecutionDeadlineExpired);
                }
                if !contract.deadline_probe_rips.contains(&rip) {
                    return Err(ToyVmxExitError::InvalidGuestRip);
                }
                registers.rip = contract.continuation_rip;
                write_vmcs(access, VmcsField::GUEST_RIP, registers.rip)?;
                rearm_preemption_timer(access, contract.preemption_timer_reload)?;
                *sequence = ToyVmxExitSequence::AwaitingIo;
                Ok(ToyVmxExitAction::Resume)
            }
            _ => Err(ToyVmxExitError::ExecutionDeadlineExpired),
        };
    }

    if reason == VmxExitReason::ExceptionOrNmi {
        return dispatch_fpu_guard_exit(access, registers, sequence, contract);
    }
    let instruction_length = read_vmcs(access, VmcsField::VM_EXIT_INSTRUCTION_LENGTH)? as u32;

    match reason {
        VmxExitReason::ExceptionOrNmi => Err(ToyVmxExitError::InvalidSequence),
        VmxExitReason::IoInstruction => {
            let (expected_rip, expected_port, immediate, expected_length, next_sequence) =
                match *sequence {
                    ToyVmxExitSequence::AwaitingIo => (
                        contract.io_rip,
                        contract.io_port,
                        true,
                        2,
                        ToyVmxExitSequence::AwaitingIoBitmapB,
                    ),
                    ToyVmxExitSequence::AwaitingIoBitmapB => (
                        contract.io_bitmap_b_rip,
                        contract.io_bitmap_b_port,
                        false,
                        1,
                        ToyVmxExitSequence::AwaitingCpuid,
                    ),
                    _ => return Err(ToyVmxExitError::InvalidSequence),
                };
            if rip != expected_rip {
                return Err(ToyVmxExitError::InvalidGuestRip);
            }
            if instruction_length != expected_length {
                return Err(ToyVmxExitError::InvalidInstructionLength);
            }
            let qualification = IoInstructionQualification::decode(read_vmcs(
                access,
                VmcsField::EXIT_QUALIFICATION,
            )?)
            .map_err(|_| ToyVmxExitError::InvalidIoQualification)?;
            if qualification.size != IoAccessSize::Byte
                || qualification.direction != IoDirection::Out
                || qualification.string
                || qualification.rep
                || qualification.immediate != immediate
                || qualification.port != expected_port
            {
                return Err(ToyVmxExitError::InvalidIoQualification);
            }
            if registers.rax as u8 != contract.io_value {
                return Err(ToyVmxExitError::InvalidIoValue);
            }
            registers
                .advance_rip(instruction_length)
                .map_err(|error| ToyVmxExitError::RipAdvance(error.kind))?;
            write_vmcs(access, VmcsField::GUEST_RIP, registers.rip)?;
            rearm_preemption_timer(access, contract.preemption_timer_reload)?;
            *sequence = next_sequence;
            Ok(ToyVmxExitAction::Resume)
        }
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
            rearm_preemption_timer(access, contract.preemption_timer_reload)?;
            *sequence = ToyVmxExitSequence::AwaitingRdmsr;
            Ok(ToyVmxExitAction::Resume)
        }
        VmxExitReason::Rdmsr => {
            if *sequence == ToyVmxExitSequence::AwaitingX87Guard
                && rip == contract.pat_rdmsr_rip
                && instruction_length == 2
                && registers.rcx as u32 == TOY_RDMSR_IA32_PAT
            {
                return Err(ToyVmxExitError::InvalidGuestPat);
            }
            if *sequence != ToyVmxExitSequence::AwaitingRdmsr {
                return Err(ToyVmxExitError::InvalidSequence);
            }
            if rip != contract.rdmsr_rip {
                return Err(ToyVmxExitError::InvalidGuestRip);
            }
            if instruction_length != 2 {
                return Err(ToyVmxExitError::InvalidInstructionLength);
            }
            let policy = StaticMsrPolicy::new([MsrEntry {
                index: TOY_RDMSR_IA32_EFER,
                value: 0,
                writable: false,
            }]);
            match handle_rdmsr_exit(registers, &policy, instruction_length)
                .map_err(|error| ToyVmxExitError::Rdmsr(error.kind))?
            {
                ExitAction::Resume => {}
                _ => return Err(ToyVmxExitError::InvalidSequence),
            }
            write_vmcs(access, VmcsField::GUEST_RIP, registers.rip)?;
            rearm_preemption_timer(access, contract.preemption_timer_reload)?;
            *sequence = ToyVmxExitSequence::AwaitingX87Guard;
            Ok(ToyVmxExitAction::Resume)
        }
        VmxExitReason::Hlt => {
            if matches!(
                *sequence,
                ToyVmxExitSequence::AwaitingPreemption | ToyVmxExitSequence::AwaitingDeadlineProbe
            ) {
                return if rip == contract.deadline_fallback_rip && instruction_length == 1 {
                    Err(ToyVmxExitError::ExecutionDeadlineExpired)
                } else {
                    Err(ToyVmxExitError::InvalidSequence)
                };
            }
            if *sequence == ToyVmxExitSequence::AwaitingX87Guard
                && rip == contract.pat_mismatch_hlt_rip
            {
                return if instruction_length == 1 {
                    Err(ToyVmxExitError::GuestPatMismatch)
                } else {
                    Err(ToyVmxExitError::InvalidInstructionLength)
                };
            }
            Err(ToyVmxExitError::InvalidSequence)
        }
        other => Err(ToyVmxExitError::UnexpectedReason(other)),
    }
}

fn dispatch_fpu_guard_exit(
    access: &mut impl ToyVmcsAccess,
    registers: &mut GeneralRegisters,
    sequence: &mut ToyVmxExitSequence,
    contract: ToyVmxExitContract,
) -> Result<ToyVmxExitAction, ToyVmxExitError> {
    let (expected_rip, fixed_length, next_sequence) = match *sequence {
        ToyVmxExitSequence::AwaitingX87Guard => (
            contract.x87_guard_rip,
            2,
            ToyVmxExitSequence::AwaitingSimdGuard,
        ),
        ToyVmxExitSequence::AwaitingSimdGuard => (
            contract.simd_guard_rip,
            4,
            ToyVmxExitSequence::AwaitingUdDelivery,
        ),
        _ => return Err(ToyVmxExitError::InvalidSequence),
    };
    if registers.rip != expected_rip {
        return Err(ToyVmxExitError::InvalidGuestRip);
    }
    if read_vmcs(access, VmcsField::VM_EXIT_INTERRUPTION_INFO)? != TOY_NM_INTERRUPTION_INFO {
        return Err(ToyVmxExitError::InvalidInterruptionInfo);
    }

    validate_guest_pat_registers(*registers, contract.guest_pat)?;
    if contract.guest_cr0 & (VMX_CR0_MONITOR_COPROCESSOR | VMX_CR0_TASK_SWITCHED)
        != (VMX_CR0_MONITOR_COPROCESSOR | VMX_CR0_TASK_SWITCHED)
        || contract.guest_cr0 & VMX_CR0_EMULATION != 0
        || contract.guest_cr4 & VMX_CR4_OSFXSR == 0
        || read_vmcs(access, VmcsField::GUEST_CR0)? != contract.guest_cr0
        || read_vmcs(access, VmcsField::GUEST_CR4)? != contract.guest_cr4
    {
        return Err(ToyVmxExitError::InvalidFpuGuardState);
    }

    registers
        .advance_rip(fixed_length)
        .map_err(|error| ToyVmxExitError::RipAdvance(error.kind))?;
    if next_sequence == ToyVmxExitSequence::AwaitingUdDelivery && registers.rip != contract.ud2_rip
    {
        return Err(ToyVmxExitError::InvalidGuestRip);
    }
    write_vmcs(access, VmcsField::GUEST_RIP, registers.rip)?;
    rearm_preemption_timer(access, contract.preemption_timer_reload)?;
    if next_sequence == ToyVmxExitSequence::AwaitingUdDelivery {
        arm_ud_delivery(access)?;
    }
    *sequence = next_sequence;
    Ok(ToyVmxExitAction::Resume)
}

fn arm_ud_delivery(access: &mut impl ToyVmcsAccess) -> Result<(), ToyVmxExitError> {
    if read_vmcs(access, VmcsField::VM_ENTRY_INTERRUPTION_INFO)? & INTERRUPTION_INFO_VALID != 0 {
        return Err(ToyVmxExitError::InvalidExceptionInjection);
    }

    write_vmcs(access, VmcsField::VM_ENTRY_EXCEPTION_ERROR_CODE, 0)?;
    write_vmcs(access, VmcsField::VM_ENTRY_INSTRUCTION_LENGTH, 0)?;
    // Keep the valid bit as the final write so a partial setup cannot describe
    // a deliverable event.
    write_vmcs(
        access,
        VmcsField::VM_ENTRY_INTERRUPTION_INFO,
        TOY_UD_ENTRY_INTERRUPTION_INFO,
    )?;

    if read_vmcs(access, VmcsField::VM_ENTRY_EXCEPTION_ERROR_CODE)? != 0
        || read_vmcs(access, VmcsField::VM_ENTRY_INSTRUCTION_LENGTH)? != 0
        || read_vmcs(access, VmcsField::VM_ENTRY_INTERRUPTION_INFO)?
            != TOY_UD_ENTRY_INTERRUPTION_INFO
    {
        return Err(ToyVmxExitError::InvalidExceptionInjection);
    }
    Ok(())
}

fn dispatch_ud_delivery_exit(
    access: &mut impl ToyVmcsAccess,
    registers: &GeneralRegisters,
    reason: VmxExitReason,
    contract: ToyVmxExitContract,
) -> Result<ToyVmxExitAction, ToyVmxExitError> {
    if reason != VmxExitReason::Hlt {
        return Err(ToyVmxExitError::ExceptionDeliveryFailed(reason));
    }
    if registers.rip != contract.hlt_rip {
        return Err(ToyVmxExitError::InvalidGuestRip);
    }
    if read_vmcs(access, VmcsField::VM_EXIT_INSTRUCTION_LENGTH)? != 1 {
        return Err(ToyVmxExitError::InvalidInstructionLength);
    }
    if registers.rsp != contract.hlt_rsp {
        return Err(ToyVmxExitError::InvalidGuestStack);
    }
    if registers.r15 != contract.ud_handler_cookie {
        return Err(ToyVmxExitError::InvalidGuestCookie);
    }
    if read_vmcs(access, VmcsField::GUEST_CS_SELECTOR)? != u64::from(contract.hlt_cs)
        || read_vmcs(access, VmcsField::GUEST_SS_SELECTOR)? != u64::from(contract.hlt_ss)
        || read_vmcs(access, VmcsField::GUEST_RFLAGS)? != contract.hlt_rflags
    {
        return Err(ToyVmxExitError::InvalidGuestReturnState);
    }
    if read_vmcs(access, VmcsField::GUEST_GDTR_BASE)? != contract.guest_gdtr.base
        || read_vmcs(access, VmcsField::GUEST_GDTR_LIMIT)? != u64::from(contract.guest_gdtr.limit)
        || read_vmcs(access, VmcsField::GUEST_IDTR_BASE)? != contract.guest_idtr.base
        || read_vmcs(access, VmcsField::GUEST_IDTR_LIMIT)? != u64::from(contract.guest_idtr.limit)
    {
        return Err(ToyVmxExitError::InvalidDescriptorTableState);
    }
    validate_guest_pat_registers(*registers, contract.guest_pat)?;
    if read_vmcs(access, VmcsField::VM_ENTRY_INTERRUPTION_INFO)? & INTERRUPTION_INFO_VALID != 0 {
        return Err(ToyVmxExitError::InvalidExceptionInjection);
    }
    if read_vmcs(access, VmcsField::IDT_VECTORING_INFO)? & INTERRUPTION_INFO_VALID != 0 {
        return Err(ToyVmxExitError::InvalidIdtVectoringState);
    }
    Ok(ToyVmxExitAction::Stop)
}

fn validate_guest_pat_registers(
    registers: GeneralRegisters,
    guest_pat: VmxPat,
) -> Result<(), ToyVmxExitError> {
    let pat = guest_pat.raw();
    if registers.rax != u64::from(pat as u32) || registers.rdx != u64::from((pat >> 32) as u32) {
        return Err(ToyVmxExitError::InvalidGuestPat);
    }
    Ok(())
}

fn rearm_preemption_timer(
    access: &mut impl ToyVmcsAccess,
    reload: u32,
) -> Result<(), ToyVmxExitError> {
    if reload < 2 {
        return Err(ToyVmxExitError::InvalidPreemptionReload);
    }
    write_vmcs(
        access,
        VmcsField::VMX_PREEMPTION_TIMER_VALUE,
        u64::from(reload),
    )
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
    use crate::vmx::vmcs_config::VMX_TOY_GUEST_PAT_RAW;

    struct MockAccess {
        reason: u64,
        rip: u64,
        rsp: u64,
        length: u64,
        qualification: u64,
        interruption_info: u64,
        entry_interruption_info: u64,
        entry_exception_error_code: u64,
        entry_instruction_length: u64,
        idt_vectoring_info: u64,
        guest_pat: u64,
        guest_cr0: u64,
        guest_cr4: u64,
        guest_cs: u64,
        guest_ss: u64,
        guest_rflags: u64,
        guest_gdtr_base: u64,
        guest_gdtr_limit: u64,
        guest_idtr_base: u64,
        guest_idtr_limit: u64,
        rip_write: Option<u64>,
        timer_write: Option<u64>,
        entry_interruption_write: Option<u64>,
        entry_error_write: Option<u64>,
        entry_length_write: Option<u64>,
        corrupt_entry_write: Option<VmcsField>,
        fail_read: Option<(VmcsField, usize)>,
        fail_write: Option<(VmcsField, usize)>,
        matching_reads: usize,
        matching_writes: usize,
    }

    impl MockAccess {
        fn preemption() -> Self {
            Self {
                reason: 52,
                rip: 0x1000,
                rsp: 0x2ff0,
                length: 0,
                qualification: 0,
                interruption_info: TOY_NM_INTERRUPTION_INFO,
                entry_interruption_info: 0,
                entry_exception_error_code: 0,
                entry_instruction_length: 0,
                idt_vectoring_info: 0x306,
                guest_pat: VMX_TOY_GUEST_PAT_RAW,
                guest_cr0: 0x8000_002b,
                guest_cr4: 0x2220,
                guest_cs: 0x08,
                guest_ss: 0x10,
                guest_rflags: 0x46,
                guest_gdtr_base: 0x1200,
                guest_gdtr_limit: 0x17,
                guest_idtr_base: 0x1300,
                guest_idtr_limit: 0x6f,
                rip_write: None,
                timer_write: None,
                entry_interruption_write: None,
                entry_error_write: None,
                entry_length_write: None,
                corrupt_entry_write: None,
                fail_read: None,
                fail_write: None,
                matching_reads: 0,
                matching_writes: 0,
            }
        }
    }

    impl ToyVmcsAccess for MockAccess {
        fn read(&mut self, field: VmcsField) -> Result<u64, VmxError> {
            if let Some((target, occurrence)) = self.fail_read {
                if field == target {
                    self.matching_reads += 1;
                    if self.matching_reads == occurrence {
                        return Err(VmxError::new(
                            VmxErrorKind::InstructionFailed,
                            "mock VMREAD failed",
                        ));
                    }
                }
            }
            Ok(if field == VmcsField::VM_EXIT_REASON {
                self.reason
            } else if field == VmcsField::GUEST_RIP {
                self.rip
            } else if field == VmcsField::GUEST_RSP {
                self.rsp
            } else if field == VmcsField::VM_EXIT_INSTRUCTION_LENGTH {
                self.length
            } else if field == VmcsField::EXIT_QUALIFICATION {
                self.qualification
            } else if field == VmcsField::VM_EXIT_INTERRUPTION_INFO {
                self.interruption_info
            } else if field == VmcsField::VM_ENTRY_INTERRUPTION_INFO {
                self.entry_interruption_info
            } else if field == VmcsField::VM_ENTRY_EXCEPTION_ERROR_CODE {
                self.entry_exception_error_code
            } else if field == VmcsField::VM_ENTRY_INSTRUCTION_LENGTH {
                self.entry_instruction_length
            } else if field == VmcsField::IDT_VECTORING_INFO {
                self.idt_vectoring_info
            } else if field == VmcsField::GUEST_IA32_PAT {
                self.guest_pat
            } else if field == VmcsField::GUEST_CR0 {
                self.guest_cr0
            } else if field == VmcsField::GUEST_CR4 {
                self.guest_cr4
            } else if field == VmcsField::GUEST_CS_SELECTOR {
                self.guest_cs
            } else if field == VmcsField::GUEST_SS_SELECTOR {
                self.guest_ss
            } else if field == VmcsField::GUEST_RFLAGS {
                self.guest_rflags
            } else if field == VmcsField::GUEST_GDTR_BASE {
                self.guest_gdtr_base
            } else if field == VmcsField::GUEST_GDTR_LIMIT {
                self.guest_gdtr_limit
            } else if field == VmcsField::GUEST_IDTR_BASE {
                self.guest_idtr_base
            } else if field == VmcsField::GUEST_IDTR_LIMIT {
                self.guest_idtr_limit
            } else {
                0
            })
        }

        fn write(&mut self, field: VmcsField, value: u64) -> Result<(), VmxError> {
            if let Some((target, occurrence)) = self.fail_write {
                if field == target {
                    self.matching_writes += 1;
                    if self.matching_writes == occurrence {
                        return Err(VmxError::new(
                            VmxErrorKind::InstructionFailed,
                            "mock VMWRITE failed",
                        ));
                    }
                }
            }
            if field == VmcsField::GUEST_RIP {
                self.rip_write = Some(value);
            } else if field == VmcsField::VMX_PREEMPTION_TIMER_VALUE {
                self.timer_write = Some(value);
            } else if field == VmcsField::VM_ENTRY_INTERRUPTION_INFO {
                self.entry_interruption_info = if self.corrupt_entry_write == Some(field) {
                    value ^ 1
                } else {
                    value
                };
                self.entry_interruption_write = Some(value);
            } else if field == VmcsField::VM_ENTRY_EXCEPTION_ERROR_CODE {
                self.entry_exception_error_code = if self.corrupt_entry_write == Some(field) {
                    value ^ 1
                } else {
                    value
                };
                self.entry_error_write = Some(value);
            } else if field == VmcsField::VM_ENTRY_INSTRUCTION_LENGTH {
                self.entry_instruction_length = if self.corrupt_entry_write == Some(field) {
                    value ^ 1
                } else {
                    value
                };
                self.entry_length_write = Some(value);
            }
            Ok(())
        }
    }

    fn contract() -> ToyVmxExitContract {
        ToyVmxExitContract {
            initial_rip: 0x1000,
            deadline_probe_rips: [
                0x1000, 0x1002, 0x1004, 0x100a, 0x100f, 0x1011, 0x1013, 0x1015, 0x1017,
            ],
            deadline_fallback_rip: 0x1019,
            continuation_rip: 0x101a,
            io_rip: 0x101c,
            io_bitmap_b_rip: 0x1022,
            cpuid_rip: 0x1027,
            rdmsr_rip: 0x102e,
            pat_rdmsr_rip: 0x1035,
            x87_guard_rip: 0x1046,
            simd_guard_rip: 0x1048,
            ud2_rip: 0x104c,
            hlt_rip: 0x104e,
            pat_mismatch_hlt_rip: 0x104f,
            hlt_rsp: 0x2ff0,
            hlt_cs: 0x08,
            hlt_ss: 0x10,
            hlt_rflags: 0x46,
            guest_gdtr: VmcsDescriptorTableState::new(0x1200, 0x17),
            guest_idtr: VmcsDescriptorTableState::new(0x1300, 0x6f),
            ud_handler_cookie: 0x5544_2d4f_4b21_0006,
            io_port: 0xe9,
            io_bitmap_b_port: 0x8000,
            io_value: b'A',
            preemption_timer_reload: 0x4000,
            guest_pat: VmxPat::toy_guest(),
            guest_cr0: 0x8000_002b,
            guest_cr4: 0x2220,
        }
    }

    #[test]
    fn preemption_io_pat_fpu_and_injected_ud_prove_the_bounded_sequence() {
        let mut access = MockAccess::preemption();
        let mut registers = GeneralRegisters::default();
        let mut sequence = ToyVmxExitSequence::AwaitingPreemption;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract()).unwrap(),
            ToyVmxExitAction::Resume
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingDeadlineProbe);
        assert_eq!(access.timer_write, Some(0x4000));

        access.rip = 0x100a;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract()).unwrap(),
            ToyVmxExitAction::Resume
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingIo);
        assert_eq!(access.timer_write, Some(0x4000));
        assert_eq!(access.rip_write, Some(0x101a));

        access.reason = 30;
        access.rip = 0x101c;
        access.length = 2;
        access.qualification = (0xe9_u64 << 16) | (1 << 6);
        registers.rax = u64::from(b'A');
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract()).unwrap(),
            ToyVmxExitAction::Resume
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingIoBitmapB);
        assert_eq!(access.rip_write, Some(0x101e));

        access.reason = 30;
        access.rip = 0x1022;
        access.length = 1;
        access.qualification = 0x8000_u64 << 16;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract()).unwrap(),
            ToyVmxExitAction::Resume
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingCpuid);
        assert_eq!(access.rip_write, Some(0x1023));

        access.reason = 10;
        access.rip = 0x1027;
        access.length = 2;
        registers.rax = 0;
        registers.rcx = 0;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract()).unwrap(),
            ToyVmxExitAction::Resume
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingRdmsr);
        assert_eq!(access.rip_write, Some(0x1029));
        assert_eq!(registers.rbx as u32, u32::from_le_bytes(*b"Aegi"));

        access.reason = 31;
        access.rip = 0x102e;
        access.length = 2;
        registers.rcx = u64::from(TOY_RDMSR_IA32_EFER);
        registers.rax = u64::MAX;
        registers.rdx = u64::MAX;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract()).unwrap(),
            ToyVmxExitAction::Resume
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingX87Guard);
        assert_eq!(access.rip_write, Some(0x1030));
        assert_eq!(registers.rax, 0);
        assert_eq!(registers.rdx, 0);

        let pat = VmxPat::toy_guest().raw();
        registers.rax = u64::from(pat as u32);
        registers.rdx = u64::from((pat >> 32) as u32);
        access.reason = 0;
        access.rip = 0x1046;
        access.fail_read = Some((VmcsField::VM_EXIT_INSTRUCTION_LENGTH, 1));
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract()).unwrap(),
            ToyVmxExitAction::Resume
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingSimdGuard);
        assert_eq!(access.rip_write, Some(0x1048));

        access.rip = 0x1048;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract()).unwrap(),
            ToyVmxExitAction::Resume
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingUdDelivery);
        assert_eq!(access.rip_write, Some(0x104c));
        assert_eq!(access.entry_error_write, Some(0));
        assert_eq!(access.entry_length_write, Some(0));
        assert_eq!(
            access.entry_interruption_write,
            Some(TOY_UD_ENTRY_INTERRUPTION_INFO)
        );

        access.fail_read = None;
        access.entry_interruption_info &= !INTERRUPTION_INFO_VALID;
        access.reason = 12;
        access.rip = 0x104e;
        access.length = 1;
        registers.r15 = contract().ud_handler_cookie;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract()).unwrap(),
            ToyVmxExitAction::Stop
        );
        assert_eq!(sequence, ToyVmxExitSequence::Complete);
    }

    #[test]
    fn hlt_before_cpuid_is_rejected() {
        let mut access = MockAccess::preemption();
        access.reason = 12;
        access.rip = 0x104e;
        access.length = 1;
        let mut registers = GeneralRegisters::default();
        let mut sequence = ToyVmxExitSequence::AwaitingPreemption;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidSequence
        );
    }

    #[test]
    fn bad_instruction_length_and_vm_entry_failure_are_typed() {
        let mut access = MockAccess::preemption();
        access.reason = 10;
        access.rip = 0x1027;
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
        let mut access = MockAccess::preemption();
        access.fail_read = Some((VmcsField::GUEST_RIP, 1));
        let mut registers = GeneralRegisters::default();
        let mut sequence = ToyVmxExitSequence::AwaitingPreemption;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::VmcsRead {
                field: VmcsField::GUEST_RIP,
                kind: VmxErrorKind::InstructionFailed,
            }
        );

        access.fail_read = None;
        access.fail_write = Some((VmcsField::VMX_PREEMPTION_TIMER_VALUE, 1));
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::VmcsWrite {
                field: VmcsField::VMX_PREEMPTION_TIMER_VALUE,
                kind: VmxErrorKind::InstructionFailed,
            }
        );
    }

    #[test]
    fn every_true_exit_rejects_a_saved_guest_pat_mismatch() {
        let mut access = MockAccess::preemption();
        access.guest_pat ^= 1;
        let mut registers = GeneralRegisters::default();
        let mut sequence = ToyVmxExitSequence::AwaitingPreemption;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidGuestPat
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingPreemption);
    }

    #[test]
    fn timer_expiry_after_the_probe_is_a_deadline_failure() {
        let mut access = MockAccess::preemption();
        let mut registers = GeneralRegisters::default();
        let mut sequence = ToyVmxExitSequence::AwaitingIo;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::ExecutionDeadlineExpired
        );
    }

    #[test]
    fn deadline_probe_fallback_exit_reports_timer_failure() {
        let mut access = MockAccess::preemption();
        access.reason = 12;
        access.rip = 0x1019;
        access.length = 1;
        let mut registers = GeneralRegisters::default();
        let mut sequence = ToyVmxExitSequence::AwaitingDeadlineProbe;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::ExecutionDeadlineExpired
        );
    }

    #[test]
    fn timer_at_the_fallback_boundary_missed_its_deadline() {
        let mut access = MockAccess::preemption();
        access.rip = 0x1019;
        let mut registers = GeneralRegisters::default();
        let mut sequence = ToyVmxExitSequence::AwaitingDeadlineProbe;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::ExecutionDeadlineExpired
        );
    }

    #[test]
    fn fallback_also_catches_a_broken_zero_sentinel() {
        let mut access = MockAccess::preemption();
        access.reason = 12;
        access.rip = 0x1019;
        access.length = 1;
        let mut registers = GeneralRegisters::default();
        let mut sequence = ToyVmxExitSequence::AwaitingPreemption;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::ExecutionDeadlineExpired
        );
    }

    #[test]
    fn preemption_probe_rejects_reload_zero_and_one() {
        for reload in [0, 1] {
            let mut access = MockAccess::preemption();
            let mut registers = GeneralRegisters::default();
            let mut sequence = ToyVmxExitSequence::AwaitingPreemption;
            let mut contract = contract();
            contract.preemption_timer_reload = reload;

            assert_eq!(
                dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract)
                    .unwrap_err(),
                ToyVmxExitError::InvalidPreemptionReload
            );
        }
    }

    #[test]
    fn io_contract_rejects_direction_port_value_and_reserved_bits() {
        let mut access = MockAccess::preemption();
        access.reason = 30;
        access.rip = 0x101c;
        access.length = 2;
        let mut registers = GeneralRegisters {
            rax: u64::from(b'A'),
            ..GeneralRegisters::default()
        };
        let mut sequence = ToyVmxExitSequence::AwaitingIo;

        for qualification in [
            (0xe9_u64 << 16) | (1 << 3) | (1 << 6),
            (0xe9_u64 << 16) | (1 << 4) | (1 << 6),
            (0xe9_u64 << 16) | (1 << 5) | (1 << 6),
            0xe9_u64 << 16,
            (0xe9_u64 << 16) | 1 | (1 << 6),
            (0x80_u64 << 16) | (1 << 6),
            (0xe9_u64 << 16) | (1 << 6) | (1 << 40),
        ] {
            access.qualification = qualification;
            assert_eq!(
                dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                    .unwrap_err(),
                ToyVmxExitError::InvalidIoQualification
            );
        }

        access.qualification = (0xe9_u64 << 16) | (1 << 6);
        registers.rax = 0;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidIoValue
        );
    }

    #[test]
    fn bitmap_b_io_contract_requires_dx_form_and_high_port() {
        let mut access = MockAccess::preemption();
        access.reason = 30;
        access.rip = 0x1022;
        access.length = 1;
        let mut registers = GeneralRegisters {
            rax: u64::from(b'A'),
            ..GeneralRegisters::default()
        };
        let mut sequence = ToyVmxExitSequence::AwaitingIoBitmapB;

        for qualification in [
            (0x8000_u64 << 16) | (1 << 6),
            0xe9_u64 << 16,
            (0x8000_u64 << 16) | (1 << 3),
            (0x8000_u64 << 16) | 1,
        ] {
            access.qualification = qualification;
            assert_eq!(
                dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                    .unwrap_err(),
                ToyVmxExitError::InvalidIoQualification
            );
        }

        access.qualification = 0x8000_u64 << 16;
        access.length = 2;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidInstructionLength
        );
    }

    fn pat_registers() -> GeneralRegisters {
        let pat = VmxPat::toy_guest().raw();
        GeneralRegisters {
            rax: u64::from(pat as u32),
            rdx: u64::from((pat >> 32) as u32),
            ..GeneralRegisters::default()
        }
    }

    fn ready_ud_delivery() -> (MockAccess, GeneralRegisters, ToyVmxExitSequence) {
        let mut access = MockAccess::preemption();
        access.reason = 12;
        access.rip = contract().hlt_rip;
        access.length = 1;
        access.entry_interruption_info = TOY_UD_ENTRY_INTERRUPTION_INFO & !INTERRUPTION_INFO_VALID;
        let mut registers = pat_registers();
        registers.r15 = contract().ud_handler_cookie;
        (access, registers, ToyVmxExitSequence::AwaitingUdDelivery)
    }

    #[test]
    fn fpu_guards_require_exact_nm_pat_and_control_state() {
        let mut access = MockAccess::preemption();
        access.reason = 0;
        access.rip = contract().x87_guard_rip;
        access.interruption_info ^= 1;
        let mut registers = pat_registers();
        let mut sequence = ToyVmxExitSequence::AwaitingX87Guard;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidInterruptionInfo
        );

        let mut access = MockAccess::preemption();
        access.reason = 0;
        access.rip = contract().x87_guard_rip;
        let mut registers = pat_registers();
        registers.rdx ^= 1;
        let mut sequence = ToyVmxExitSequence::AwaitingX87Guard;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidGuestPat
        );

        for corrupt_cr0 in [true, false] {
            let mut access = MockAccess::preemption();
            access.reason = 0;
            access.rip = contract().x87_guard_rip;
            if corrupt_cr0 {
                access.guest_cr0 ^= VMX_CR0_TASK_SWITCHED;
            } else {
                access.guest_cr4 ^= VMX_CR4_OSFXSR;
            }
            let mut registers = pat_registers();
            let mut sequence = ToyVmxExitSequence::AwaitingX87Guard;
            assert_eq!(
                dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                    .unwrap_err(),
                ToyVmxExitError::InvalidFpuGuardState
            );
        }
    }

    #[test]
    fn simd_guard_arms_only_the_exact_hardware_ud_event() {
        let mut access = MockAccess::preemption();
        access.reason = 0;
        access.rip = contract().simd_guard_rip;
        access.entry_interruption_info = 0x306;
        let mut registers = pat_registers();
        let mut sequence = ToyVmxExitSequence::AwaitingSimdGuard;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract()).unwrap(),
            ToyVmxExitAction::Resume
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingUdDelivery);
        assert_eq!(access.rip_write, Some(contract().ud2_rip));
        assert_eq!(
            access.entry_interruption_info,
            TOY_UD_ENTRY_INTERRUPTION_INFO
        );

        let mut access = MockAccess::preemption();
        access.reason = 0;
        access.rip = contract().simd_guard_rip;
        let mut registers = pat_registers();
        let mut sequence = ToyVmxExitSequence::AwaitingSimdGuard;
        let mut bad_contract = contract();
        bad_contract.ud2_rip ^= 1;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, bad_contract,)
                .unwrap_err(),
            ToyVmxExitError::InvalidGuestRip
        );
        assert_eq!(access.entry_interruption_write, None);

        let mut access = MockAccess::preemption();
        access.reason = 0;
        access.rip = contract().simd_guard_rip;
        access.entry_interruption_info = INTERRUPTION_INFO_VALID;
        let mut registers = pat_registers();
        let mut sequence = ToyVmxExitSequence::AwaitingSimdGuard;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidExceptionInjection
        );

        for field in [
            VmcsField::VM_ENTRY_EXCEPTION_ERROR_CODE,
            VmcsField::VM_ENTRY_INSTRUCTION_LENGTH,
            VmcsField::VM_ENTRY_INTERRUPTION_INFO,
        ] {
            let mut access = MockAccess::preemption();
            access.reason = 0;
            access.rip = contract().simd_guard_rip;
            access.corrupt_entry_write = Some(field);
            let mut registers = pat_registers();
            let mut sequence = ToyVmxExitSequence::AwaitingSimdGuard;
            assert_eq!(
                dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                    .unwrap_err(),
                ToyVmxExitError::InvalidExceptionInjection
            );
        }
    }

    #[test]
    fn every_ud_arm_vmcs_operation_fails_without_advancing_the_sequence() {
        for (field, occurrence) in [
            (VmcsField::VM_ENTRY_INTERRUPTION_INFO, 1),
            (VmcsField::VM_ENTRY_EXCEPTION_ERROR_CODE, 1),
            (VmcsField::VM_ENTRY_INSTRUCTION_LENGTH, 1),
            (VmcsField::VM_ENTRY_INTERRUPTION_INFO, 2),
        ] {
            let mut access = MockAccess::preemption();
            access.reason = 0;
            access.rip = contract().simd_guard_rip;
            access.fail_read = Some((field, occurrence));
            let mut registers = pat_registers();
            let mut sequence = ToyVmxExitSequence::AwaitingSimdGuard;

            assert_eq!(
                dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                    .unwrap_err(),
                ToyVmxExitError::VmcsRead {
                    field,
                    kind: VmxErrorKind::InstructionFailed,
                }
            );
            assert_eq!(sequence, ToyVmxExitSequence::AwaitingSimdGuard);
            if field == VmcsField::VM_ENTRY_INTERRUPTION_INFO && occurrence == 2 {
                assert_eq!(
                    access.entry_interruption_write,
                    Some(TOY_UD_ENTRY_INTERRUPTION_INFO)
                );
            }
        }

        for field in [
            VmcsField::VM_ENTRY_EXCEPTION_ERROR_CODE,
            VmcsField::VM_ENTRY_INSTRUCTION_LENGTH,
            VmcsField::VM_ENTRY_INTERRUPTION_INFO,
        ] {
            let mut access = MockAccess::preemption();
            access.reason = 0;
            access.rip = contract().simd_guard_rip;
            access.fail_write = Some((field, 1));
            let mut registers = pat_registers();
            let mut sequence = ToyVmxExitSequence::AwaitingSimdGuard;

            assert_eq!(
                dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                    .unwrap_err(),
                ToyVmxExitError::VmcsWrite {
                    field,
                    kind: VmxErrorKind::InstructionFailed,
                }
            );
            assert_eq!(sequence, ToyVmxExitSequence::AwaitingSimdGuard);
        }
    }

    #[test]
    fn awaiting_ud_rejects_a_natural_exception_without_reading_its_length() {
        let (mut access, mut registers, mut sequence) = ready_ud_delivery();
        access.reason = 0;
        access.rip = contract().ud2_rip;
        access.fail_read = Some((VmcsField::VM_EXIT_INSTRUCTION_LENGTH, 1));

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::ExceptionDeliveryFailed(VmxExitReason::ExceptionOrNmi)
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingUdDelivery);
        assert_eq!(access.entry_interruption_write, None);

        let (mut access, mut registers, mut sequence) = ready_ud_delivery();
        access.reason = 52;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::ExceptionDeliveryFailed(VmxExitReason::PreemptionTimer)
        );
    }

    #[test]
    fn delivered_ud_requires_the_exact_hlt_boundary() {
        let (mut access, mut registers, mut sequence) = ready_ud_delivery();
        access.rip ^= 1;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidGuestRip
        );

        let (mut access, mut registers, mut sequence) = ready_ud_delivery();
        access.length = 2;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidInstructionLength
        );
    }

    #[test]
    fn delivered_ud_requires_the_exact_iret_frame_and_vmcs_state() {
        let (mut access, mut registers, mut sequence) = ready_ud_delivery();
        registers.r15 ^= 1;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidGuestCookie
        );

        let (mut access, mut registers, mut sequence) = ready_ud_delivery();
        access.rsp ^= 8;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidGuestStack
        );

        for field in 0..3 {
            let (mut access, mut registers, mut sequence) = ready_ud_delivery();
            match field {
                0 => access.guest_cs ^= 8,
                1 => access.guest_ss ^= 8,
                _ => access.guest_rflags ^= 1,
            }
            assert_eq!(
                dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                    .unwrap_err(),
                ToyVmxExitError::InvalidGuestReturnState
            );
        }

        for field in 0..4 {
            let (mut access, mut registers, mut sequence) = ready_ud_delivery();
            match field {
                0 => access.guest_gdtr_base ^= 8,
                1 => access.guest_gdtr_limit ^= 1,
                2 => access.guest_idtr_base ^= 8,
                _ => access.guest_idtr_limit ^= 1,
            }
            assert_eq!(
                dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                    .unwrap_err(),
                ToyVmxExitError::InvalidDescriptorTableState
            );
        }
    }

    #[test]
    fn delivered_ud_requires_consumed_entry_and_no_vectoring_event() {
        let (mut access, mut registers, mut sequence) = ready_ud_delivery();
        access.entry_interruption_info |= INTERRUPTION_INFO_VALID;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidExceptionInjection
        );

        let (mut access, mut registers, mut sequence) = ready_ud_delivery();
        access.idt_vectoring_info = INTERRUPTION_INFO_VALID | 6;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidIdtVectoringState
        );

        let (mut access, mut registers, mut sequence) = ready_ud_delivery();
        registers.rax ^= 1;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidGuestPat
        );
    }

    #[test]
    fn pat_mismatch_hlt_is_a_typed_terminal_failure() {
        let mut access = MockAccess::preemption();
        access.reason = 12;
        access.rip = contract().pat_mismatch_hlt_rip;
        access.length = 1;
        let mut registers = GeneralRegisters::default();
        let mut sequence = ToyVmxExitSequence::AwaitingX87Guard;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::GuestPatMismatch
        );
        assert_eq!(sequence, ToyVmxExitSequence::AwaitingX87Guard);
    }

    #[test]
    fn rdmsr_contract_accepts_only_synthetic_ia32_efer_read() {
        let mut access = MockAccess::preemption();
        access.reason = 31;
        access.rip = 0x102e;
        access.length = 2;
        let mut registers = GeneralRegisters {
            rcx: 0x10,
            ..GeneralRegisters::default()
        };
        let mut sequence = ToyVmxExitSequence::AwaitingRdmsr;

        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::Rdmsr(VmxErrorKind::UnsupportedMsr)
        );

        registers.rcx = u64::from(TOY_RDMSR_IA32_EFER);
        access.reason = 32;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::UnexpectedReason(VmxExitReason::Wrmsr)
        );

        let mut access = MockAccess::preemption();
        access.reason = 31;
        access.rip = contract().pat_rdmsr_rip;
        access.length = 2;
        let mut registers = GeneralRegisters {
            rcx: u64::from(TOY_RDMSR_IA32_PAT),
            ..GeneralRegisters::default()
        };
        let mut sequence = ToyVmxExitSequence::AwaitingX87Guard;
        assert_eq!(
            dispatch_toy_vmx_exit(&mut access, &mut registers, &mut sequence, contract())
                .unwrap_err(),
            ToyVmxExitError::InvalidGuestPat
        );
    }
}
