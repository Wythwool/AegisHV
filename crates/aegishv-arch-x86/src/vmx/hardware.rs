use aegishv_hypervisor_core::ids::HostPhysical;

#[cfg(target_arch = "x86_64")]
use core::arch::asm;

use super::features::{VmxError, VmxErrorKind};
use super::instructions::{VmxInstruction, VmxInstructionExecutor};

const RFLAGS_CF: u64 = 1 << 0;
const RFLAGS_ZF: u64 = 1 << 6;

#[derive(Default)]
pub struct HardwareVmxInstructions;

impl HardwareVmxInstructions {
    pub const fn new() -> Self {
        Self
    }
}

#[cfg(target_arch = "x86_64")]
impl VmxInstructionExecutor for HardwareVmxInstructions {
    unsafe fn vmxon(&mut self, region: HostPhysical) -> Result<(), VmxError> {
        let operand = region.get();
        let flags = unsafe {
            let flags: u64;
            asm!(
                "vmxon qword ptr [{operand}]",
                "pushfq",
                "pop {flags}",
                operand = in(reg) &operand,
                flags = lateout(reg) flags,
            );
            flags
        };
        finish_instruction_status(VmxInstruction::Vmxon, flags)
    }

    unsafe fn vmxoff(&mut self) -> Result<(), VmxError> {
        let flags = unsafe {
            let flags: u64;
            asm!(
                "vmxoff",
                "pushfq",
                "pop {flags}",
                flags = lateout(reg) flags,
            );
            flags
        };
        finish_instruction_status(VmxInstruction::Vmxoff, flags)
    }

    unsafe fn vmptrld(&mut self, vmcs: HostPhysical) -> Result<(), VmxError> {
        let operand = vmcs.get();
        let flags = unsafe {
            let flags: u64;
            asm!(
                "vmptrld qword ptr [{operand}]",
                "pushfq",
                "pop {flags}",
                operand = in(reg) &operand,
                flags = lateout(reg) flags,
            );
            flags
        };
        finish_instruction_status(VmxInstruction::Vmptrld, flags)
    }

    unsafe fn vmclear(&mut self, vmcs: HostPhysical) -> Result<(), VmxError> {
        let operand = vmcs.get();
        let flags = unsafe {
            let flags: u64;
            asm!(
                "vmclear qword ptr [{operand}]",
                "pushfq",
                "pop {flags}",
                operand = in(reg) &operand,
                flags = lateout(reg) flags,
            );
            flags
        };
        finish_instruction_status(VmxInstruction::Vmclear, flags)
    }

    unsafe fn vmlaunch(&mut self) -> Result<(), VmxError> {
        let flags = unsafe {
            let flags: u64;
            asm!(
                "vmlaunch",
                "pushfq",
                "pop {flags}",
                flags = lateout(reg) flags,
            );
            flags
        };
        finish_instruction_status(VmxInstruction::Vmlaunch, flags)
    }

    unsafe fn vmresume(&mut self) -> Result<(), VmxError> {
        let flags = unsafe {
            let flags: u64;
            asm!(
                "vmresume",
                "pushfq",
                "pop {flags}",
                flags = lateout(reg) flags,
            );
            flags
        };
        finish_instruction_status(VmxInstruction::Vmresume, flags)
    }

    unsafe fn vmread(&mut self, field: u64) -> Result<u64, VmxError> {
        let (value, flags) = unsafe {
            let value: u64;
            let flags: u64;
            asm!(
                "vmread {value}, {field}",
                "pushfq",
                "pop {flags}",
                field = in(reg) field,
                value = lateout(reg) value,
                flags = lateout(reg) flags,
            );
            (value, flags)
        };
        finish_instruction_status(VmxInstruction::Vmread, flags)?;
        Ok(value)
    }

    unsafe fn vmwrite(&mut self, field: u64, value: u64) -> Result<(), VmxError> {
        let flags = unsafe {
            let flags: u64;
            // Intel syntax places the register-only VMCS field encoding first
            // and the register-or-memory value source second.
            asm!(
                "vmwrite {field}, {value}",
                "pushfq",
                "pop {flags}",
                value = in(reg) value,
                field = in(reg) field,
                flags = lateout(reg) flags,
            );
            flags
        };
        finish_instruction_status(VmxInstruction::Vmwrite, flags)
    }
}

#[cfg(not(target_arch = "x86_64"))]
impl VmxInstructionExecutor for HardwareVmxInstructions {
    unsafe fn vmxon(&mut self, _region: HostPhysical) -> Result<(), VmxError> {
        unsupported_arch()
    }

    unsafe fn vmxoff(&mut self) -> Result<(), VmxError> {
        unsupported_arch()
    }

    unsafe fn vmptrld(&mut self, _vmcs: HostPhysical) -> Result<(), VmxError> {
        unsupported_arch()
    }

    unsafe fn vmclear(&mut self, _vmcs: HostPhysical) -> Result<(), VmxError> {
        unsupported_arch()
    }

    unsafe fn vmlaunch(&mut self) -> Result<(), VmxError> {
        unsupported_arch()
    }

    unsafe fn vmresume(&mut self) -> Result<(), VmxError> {
        unsupported_arch()
    }

    unsafe fn vmread(&mut self, _field: u64) -> Result<u64, VmxError> {
        unsupported_arch()
    }

    unsafe fn vmwrite(&mut self, _field: u64, _value: u64) -> Result<(), VmxError> {
        unsupported_arch()
    }
}

const fn finish_instruction_status(
    instruction: VmxInstruction,
    flags: u64,
) -> Result<(), VmxError> {
    if flags & RFLAGS_CF != 0 {
        return Err(VmxError::new(
            VmxErrorKind::InstructionFailed,
            instruction_fail_invalid_message(instruction),
        ));
    }
    if flags & RFLAGS_ZF != 0 {
        return Err(VmxError::new(
            VmxErrorKind::InstructionFailed,
            instruction_fail_valid_message(instruction),
        ));
    }
    Ok(())
}

const fn instruction_fail_invalid_message(instruction: VmxInstruction) -> &'static str {
    match instruction {
        VmxInstruction::Vmxon => "VMXON failed with VMfailInvalid",
        VmxInstruction::Vmxoff => "VMXOFF failed with VMfailInvalid",
        VmxInstruction::Vmptrld => "VMPTRLD failed with VMfailInvalid",
        VmxInstruction::Vmclear => "VMCLEAR failed with VMfailInvalid",
        VmxInstruction::Vmlaunch => "VMLAUNCH failed with VMfailInvalid",
        VmxInstruction::Vmresume => "VMRESUME failed with VMfailInvalid",
        VmxInstruction::Vmread => "VMREAD failed with VMfailInvalid",
        VmxInstruction::Vmwrite => "VMWRITE failed with VMfailInvalid",
    }
}

const fn instruction_fail_valid_message(instruction: VmxInstruction) -> &'static str {
    match instruction {
        VmxInstruction::Vmxon => "VMXON failed with VMfailValid",
        VmxInstruction::Vmxoff => "VMXOFF failed with VMfailValid",
        VmxInstruction::Vmptrld => "VMPTRLD failed with VMfailValid",
        VmxInstruction::Vmclear => "VMCLEAR failed with VMfailValid",
        VmxInstruction::Vmlaunch => "VMLAUNCH failed with VMfailValid",
        VmxInstruction::Vmresume => "VMRESUME failed with VMfailValid",
        VmxInstruction::Vmread => "VMREAD failed with VMfailValid",
        VmxInstruction::Vmwrite => "VMWRITE failed with VMfailValid",
    }
}

#[cfg(not(target_arch = "x86_64"))]
const fn unsupported_arch<T>() -> Result<T, VmxError> {
    Err(VmxError::new(
        VmxErrorKind::UnsupportedCapability,
        "hardware VMX instructions require x86_64",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vmx_status_accepts_clear_flags() {
        finish_instruction_status(VmxInstruction::Vmlaunch, 0).unwrap();
    }

    #[test]
    fn vmx_status_reports_vmfail_invalid_before_vmfail_valid() {
        let err =
            finish_instruction_status(VmxInstruction::Vmresume, RFLAGS_CF | RFLAGS_ZF).unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::InstructionFailed);
        assert_eq!(err.message, "VMRESUME failed with VMfailInvalid");
    }

    #[test]
    fn vmx_status_reports_vmfail_valid() {
        let err = finish_instruction_status(VmxInstruction::Vmlaunch, RFLAGS_ZF).unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::InstructionFailed);
        assert_eq!(err.message, "VMLAUNCH failed with VMfailValid");
    }
}
