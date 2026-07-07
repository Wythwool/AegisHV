use aegishv_hypervisor_core::ids::HostPhysical;

use super::features::{VmxError, VmxErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VmxInstruction {
    Vmxon,
    Vmxoff,
    Vmptrld,
    Vmclear,
    Vmread,
    Vmwrite,
}

impl VmxInstruction {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Vmxon => "VMXON",
            Self::Vmxoff => "VMXOFF",
            Self::Vmptrld => "VMPTRLD",
            Self::Vmclear => "VMCLEAR",
            Self::Vmread => "VMREAD",
            Self::Vmwrite => "VMWRITE",
        }
    }
}

pub trait VmxInstructionExecutor {
    /// # Safety
    ///
    /// The caller must already be running at the required privilege level, must
    /// have enabled CR4.VMXE, and must pass a 4K-aligned physical address for a
    /// VMXON region initialized with the CPU's VMCS revision id.
    unsafe fn vmxon(&mut self, region: HostPhysical) -> Result<(), VmxError>;

    /// # Safety
    ///
    /// The caller must ensure the current CPU is in VMX operation and that no
    /// VMCS state owned by this CPU is still live.
    unsafe fn vmxoff(&mut self) -> Result<(), VmxError>;

    /// # Safety
    ///
    /// The caller must pass a 4K-aligned physical address for an initialized,
    /// cleared VMCS region belonging to the current CPU.
    unsafe fn vmptrld(&mut self, vmcs: HostPhysical) -> Result<(), VmxError>;

    /// # Safety
    ///
    /// The caller must pass a 4K-aligned physical address for a VMCS region that
    /// is not concurrently used by another CPU.
    unsafe fn vmclear(&mut self, vmcs: HostPhysical) -> Result<(), VmxError>;

    /// # Safety
    ///
    /// The caller must ensure a current VMCS is loaded and that `field` is a
    /// supported VMCS field encoding for the current processor.
    unsafe fn vmread(&mut self, field: u64) -> Result<u64, VmxError>;

    /// # Safety
    ///
    /// The caller must ensure a current VMCS is loaded and that `field` accepts
    /// `value` under the active VMX control MSRs.
    unsafe fn vmwrite(&mut self, field: u64, value: u64) -> Result<(), VmxError>;
}

#[derive(Default)]
pub struct UnsupportedVmxInstructions;

impl VmxInstructionExecutor for UnsupportedVmxInstructions {
    unsafe fn vmxon(&mut self, _region: HostPhysical) -> Result<(), VmxError> {
        unsupported(VmxInstruction::Vmxon)
    }

    unsafe fn vmxoff(&mut self) -> Result<(), VmxError> {
        unsupported(VmxInstruction::Vmxoff)
    }

    unsafe fn vmptrld(&mut self, _vmcs: HostPhysical) -> Result<(), VmxError> {
        unsupported(VmxInstruction::Vmptrld)
    }

    unsafe fn vmclear(&mut self, _vmcs: HostPhysical) -> Result<(), VmxError> {
        unsupported(VmxInstruction::Vmclear)
    }

    unsafe fn vmread(&mut self, _field: u64) -> Result<u64, VmxError> {
        unsupported(VmxInstruction::Vmread)
    }

    unsafe fn vmwrite(&mut self, _field: u64, _value: u64) -> Result<(), VmxError> {
        unsupported(VmxInstruction::Vmwrite)
    }
}

const fn unsupported<T>(instruction: VmxInstruction) -> Result<T, VmxError> {
    let message = match instruction {
        VmxInstruction::Vmxon => "VMXON execution is not available in this build",
        VmxInstruction::Vmxoff => "VMXOFF execution is not available in this build",
        VmxInstruction::Vmptrld => "VMPTRLD execution is not available in this build",
        VmxInstruction::Vmclear => "VMCLEAR execution is not available in this build",
        VmxInstruction::Vmread => "VMREAD execution is not available in this build",
        VmxInstruction::Vmwrite => "VMWRITE execution is not available in this build",
    };
    Err(VmxError::new(VmxErrorKind::UnsupportedCapability, message))
}

#[cfg(test)]
pub mod tests_support {
    use super::*;

    #[derive(Default)]
    pub struct MockVmxInstructions {
        pub vmxon_region: Option<HostPhysical>,
        pub current_vmcs: Option<HostPhysical>,
        pub cleared_vmcs: Option<HostPhysical>,
        pub last_write: Option<(u64, u64)>,
        pub next_failure: Option<VmxInstruction>,
        pub read_value: u64,
    }

    impl MockVmxInstructions {
        pub fn fail_next(&mut self, instruction: VmxInstruction) {
            self.next_failure = Some(instruction);
        }

        fn maybe_fail(&mut self, instruction: VmxInstruction) -> Result<(), VmxError> {
            if self.next_failure == Some(instruction) {
                self.next_failure = None;
                return Err(VmxError::new(
                    VmxErrorKind::InstructionFailed,
                    "mock VMX instruction failed",
                ));
            }
            Ok(())
        }
    }

    impl VmxInstructionExecutor for MockVmxInstructions {
        unsafe fn vmxon(&mut self, region: HostPhysical) -> Result<(), VmxError> {
            self.maybe_fail(VmxInstruction::Vmxon)?;
            self.vmxon_region = Some(region);
            Ok(())
        }

        unsafe fn vmxoff(&mut self) -> Result<(), VmxError> {
            self.maybe_fail(VmxInstruction::Vmxoff)?;
            self.vmxon_region = None;
            self.current_vmcs = None;
            Ok(())
        }

        unsafe fn vmptrld(&mut self, vmcs: HostPhysical) -> Result<(), VmxError> {
            self.maybe_fail(VmxInstruction::Vmptrld)?;
            self.current_vmcs = Some(vmcs);
            Ok(())
        }

        unsafe fn vmclear(&mut self, vmcs: HostPhysical) -> Result<(), VmxError> {
            self.maybe_fail(VmxInstruction::Vmclear)?;
            self.cleared_vmcs = Some(vmcs);
            Ok(())
        }

        unsafe fn vmread(&mut self, _field: u64) -> Result<u64, VmxError> {
            self.maybe_fail(VmxInstruction::Vmread)?;
            Ok(self.read_value)
        }

        unsafe fn vmwrite(&mut self, field: u64, value: u64) -> Result<(), VmxError> {
            self.maybe_fail(VmxInstruction::Vmwrite)?;
            self.last_write = Some((field, value));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vmx::instructions::tests_support::MockVmxInstructions;

    #[test]
    fn unsupported_executor_returns_typed_error() {
        let mut executor = UnsupportedVmxInstructions;
        let err = unsafe { executor.vmxon(HostPhysical::new(0x1000).unwrap()) }.unwrap_err();

        assert_eq!(err.kind, VmxErrorKind::UnsupportedCapability);
    }

    #[test]
    fn mock_executor_records_vmcs_writes_and_failures() {
        let mut executor = MockVmxInstructions::default();

        unsafe { executor.vmwrite(0x6800, 0xfeed) }.unwrap();
        assert_eq!(executor.last_write, Some((0x6800, 0xfeed)));

        executor.fail_next(VmxInstruction::Vmread);
        let err = unsafe { executor.vmread(0x6800) }.unwrap_err();
        assert_eq!(err.kind, VmxErrorKind::InstructionFailed);
    }
}
