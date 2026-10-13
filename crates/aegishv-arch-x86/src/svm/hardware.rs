use aegishv_hypervisor_core::ids::HostPhysical;

#[cfg(target_arch = "x86_64")]
use core::arch::asm;

use super::features::{EferValue, SvmError};
use super::instructions::SvmInstructionExecutor;

const IA32_EFER: u32 = 0xc000_0080;

#[derive(Default)]
pub struct HardwareSvmInstructions;

impl HardwareSvmInstructions {
    pub const fn new() -> Self {
        Self
    }
}

#[cfg(target_arch = "x86_64")]
impl SvmInstructionExecutor for HardwareSvmInstructions {
    unsafe fn enable_svme(&mut self, efer: EferValue) -> Result<EferValue, SvmError> {
        let enabled = efer.with_svme();
        unsafe { write_msr(IA32_EFER, enabled.raw()) };
        Ok(enabled)
    }

    unsafe fn vmrun(&mut self, vmcb: HostPhysical) -> Result<(), SvmError> {
        unsafe {
            asm!(
                "vmrun",
                in("rax") vmcb.get(),
                options(nostack),
            );
        }
        Ok(())
    }

    unsafe fn vmload(&mut self, vmcb: HostPhysical) -> Result<(), SvmError> {
        unsafe {
            asm!(
                "vmload",
                in("rax") vmcb.get(),
                options(nostack),
            );
        }
        Ok(())
    }

    unsafe fn vmsave(&mut self, vmcb: HostPhysical) -> Result<(), SvmError> {
        unsafe {
            asm!(
                "vmsave",
                in("rax") vmcb.get(),
                options(nostack),
            );
        }
        Ok(())
    }

    unsafe fn invlpga(&mut self, guest_virtual: u64, asid: u32) -> Result<(), SvmError> {
        unsafe {
            asm!(
                "invlpga",
                in("rax") guest_virtual,
                in("ecx") asid,
                options(nostack),
            );
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "x86_64"))]
impl SvmInstructionExecutor for HardwareSvmInstructions {
    unsafe fn enable_svme(&mut self, _efer: EferValue) -> Result<EferValue, SvmError> {
        unsupported_arch()
    }

    unsafe fn vmrun(&mut self, _vmcb: HostPhysical) -> Result<(), SvmError> {
        unsupported_arch()
    }

    unsafe fn vmload(&mut self, _vmcb: HostPhysical) -> Result<(), SvmError> {
        unsupported_arch()
    }

    unsafe fn vmsave(&mut self, _vmcb: HostPhysical) -> Result<(), SvmError> {
        unsupported_arch()
    }

    unsafe fn invlpga(&mut self, _guest_virtual: u64, _asid: u32) -> Result<(), SvmError> {
        unsupported_arch()
    }
}

#[cfg(target_arch = "x86_64")]
unsafe fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") low,
            in("edx") high,
            options(nostack, preserves_flags),
        );
    }
}

#[cfg(not(target_arch = "x86_64"))]
const fn unsupported_arch<T>() -> Result<T, SvmError> {
    Err(SvmError::new(
        super::features::SvmErrorKind::UnsupportedCapability,
        "hardware SVM instructions require x86_64",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svm::features::EFER_SVME;

    #[test]
    fn hardware_executor_can_be_constructed_without_host_access() {
        let _executor = HardwareSvmInstructions::new();
    }

    #[test]
    fn svme_enable_value_preserves_existing_efer_bits() {
        let efer = EferValue::new(0x500).with_svme();

        assert_eq!(efer.raw(), 0x500 | EFER_SVME);
    }
}
