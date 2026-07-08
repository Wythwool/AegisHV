use aegishv_hypervisor_core::ids::HostPhysical;

use super::features::{EferValue, SvmError, SvmErrorKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvmInstruction {
    EnableSvme,
    Vmrun,
    Vmload,
    Vmsave,
    Invlpga,
}

impl SvmInstruction {
    pub const fn name(self) -> &'static str {
        match self {
            Self::EnableSvme => "EFER.SVME",
            Self::Vmrun => "VMRUN",
            Self::Vmload => "VMLOAD",
            Self::Vmsave => "VMSAVE",
            Self::Invlpga => "INVLPGA",
        }
    }
}

pub trait SvmInstructionExecutor {
    /// # Safety
    ///
    /// The caller must be running at the required privilege level and must make
    /// sure the new EFER value is written on the same CPU that will execute SVM.
    unsafe fn enable_svme(&mut self, efer: EferValue) -> Result<EferValue, SvmError>;

    /// # Safety
    ///
    /// The caller must enable EFER.SVME, prepare a valid 4K-aligned VMCB, and
    /// preserve host state according to the AMD64 architecture rules.
    unsafe fn vmrun(&mut self, vmcb: HostPhysical) -> Result<(), SvmError>;

    /// # Safety
    ///
    /// The caller must pass the physical address of a VMCB owned by this CPU.
    unsafe fn vmload(&mut self, vmcb: HostPhysical) -> Result<(), SvmError>;

    /// # Safety
    ///
    /// The caller must pass the physical address of a VMCB owned by this CPU.
    unsafe fn vmsave(&mut self, vmcb: HostPhysical) -> Result<(), SvmError>;

    /// # Safety
    ///
    /// The caller must pass an ASID that is valid on the current CPU and a guest
    /// address whose translation belongs to that ASID.
    unsafe fn invlpga(&mut self, guest_virtual: u64, asid: u32) -> Result<(), SvmError>;
}

#[derive(Default)]
pub struct UnsupportedSvmInstructions;

impl SvmInstructionExecutor for UnsupportedSvmInstructions {
    unsafe fn enable_svme(&mut self, _efer: EferValue) -> Result<EferValue, SvmError> {
        unsupported(SvmInstruction::EnableSvme)
    }

    unsafe fn vmrun(&mut self, _vmcb: HostPhysical) -> Result<(), SvmError> {
        unsupported(SvmInstruction::Vmrun)
    }

    unsafe fn vmload(&mut self, _vmcb: HostPhysical) -> Result<(), SvmError> {
        unsupported(SvmInstruction::Vmload)
    }

    unsafe fn vmsave(&mut self, _vmcb: HostPhysical) -> Result<(), SvmError> {
        unsupported(SvmInstruction::Vmsave)
    }

    unsafe fn invlpga(&mut self, _guest_virtual: u64, _asid: u32) -> Result<(), SvmError> {
        unsupported(SvmInstruction::Invlpga)
    }
}

const fn unsupported<T>(instruction: SvmInstruction) -> Result<T, SvmError> {
    let message = match instruction {
        SvmInstruction::EnableSvme => "EFER.SVME cannot be changed in this build",
        SvmInstruction::Vmrun => "VMRUN execution is not available in this build",
        SvmInstruction::Vmload => "VMLOAD execution is not available in this build",
        SvmInstruction::Vmsave => "VMSAVE execution is not available in this build",
        SvmInstruction::Invlpga => "INVLPGA execution is not available in this build",
    };
    Err(SvmError::new(SvmErrorKind::UnsupportedCapability, message))
}

#[cfg(test)]
pub mod tests_support {
    use super::*;

    #[derive(Default)]
    pub struct MockSvmInstructions {
        pub svme_enabled: bool,
        pub last_vmrun: Option<HostPhysical>,
        pub last_vmload: Option<HostPhysical>,
        pub last_vmsave: Option<HostPhysical>,
        pub last_invlpga: Option<(u64, u32)>,
        pub enable_count: u64,
        pub vmrun_count: u64,
        pub vmload_count: u64,
        pub vmsave_count: u64,
        pub invlpga_count: u64,
        pub next_failure: Option<SvmInstruction>,
    }

    impl MockSvmInstructions {
        pub fn fail_next(&mut self, instruction: SvmInstruction) {
            self.next_failure = Some(instruction);
        }

        fn maybe_fail(&mut self, instruction: SvmInstruction) -> Result<(), SvmError> {
            if self.next_failure == Some(instruction) {
                self.next_failure = None;
                return Err(SvmError::new(
                    SvmErrorKind::InstructionFailed,
                    "mock SVM instruction failed",
                ));
            }
            Ok(())
        }
    }

    impl SvmInstructionExecutor for MockSvmInstructions {
        unsafe fn enable_svme(&mut self, efer: EferValue) -> Result<EferValue, SvmError> {
            self.maybe_fail(SvmInstruction::EnableSvme)?;
            self.svme_enabled = true;
            self.enable_count += 1;
            Ok(efer.with_svme())
        }

        unsafe fn vmrun(&mut self, vmcb: HostPhysical) -> Result<(), SvmError> {
            self.maybe_fail(SvmInstruction::Vmrun)?;
            self.last_vmrun = Some(vmcb);
            self.vmrun_count += 1;
            Ok(())
        }

        unsafe fn vmload(&mut self, vmcb: HostPhysical) -> Result<(), SvmError> {
            self.maybe_fail(SvmInstruction::Vmload)?;
            self.last_vmload = Some(vmcb);
            self.vmload_count += 1;
            Ok(())
        }

        unsafe fn vmsave(&mut self, vmcb: HostPhysical) -> Result<(), SvmError> {
            self.maybe_fail(SvmInstruction::Vmsave)?;
            self.last_vmsave = Some(vmcb);
            self.vmsave_count += 1;
            Ok(())
        }

        unsafe fn invlpga(&mut self, guest_virtual: u64, asid: u32) -> Result<(), SvmError> {
            self.maybe_fail(SvmInstruction::Invlpga)?;
            self.last_invlpga = Some((guest_virtual, asid));
            self.invlpga_count += 1;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::MockSvmInstructions;
    use super::*;

    #[test]
    fn unsupported_executor_returns_typed_error() {
        let mut executor = UnsupportedSvmInstructions;
        let err = unsafe { executor.vmrun(HostPhysical::new(0x4000).unwrap()) }.unwrap_err();

        assert_eq!(err.kind, SvmErrorKind::UnsupportedCapability);
    }

    #[test]
    fn mock_executor_records_vmrun_and_invlpga() {
        let mut executor = MockSvmInstructions::default();

        unsafe { executor.vmrun(HostPhysical::new(0x8000).unwrap()) }.unwrap();
        unsafe { executor.invlpga(0xfeed_0000, 9) }.unwrap();

        assert_eq!(executor.last_vmrun.unwrap().get(), 0x8000);
        assert_eq!(executor.last_invlpga, Some((0xfeed_0000, 9)));
        assert_eq!(executor.vmrun_count, 1);
        assert_eq!(executor.invlpga_count, 1);
    }

    #[test]
    fn mock_executor_can_fail_next_instruction() {
        let mut executor = MockSvmInstructions::default();
        executor.fail_next(SvmInstruction::Vmload);

        let err = unsafe { executor.vmload(HostPhysical::new(0x8000).unwrap()) }.unwrap_err();

        assert_eq!(err.kind, SvmErrorKind::InstructionFailed);
    }
}
