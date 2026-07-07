use aegishv_hypervisor_core::ids::GuestPhysical;

use crate::features::{Arm64Error, Arm64ErrorKind, VmidBits};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vmid {
    raw: u16,
    width: VmidBits,
}

impl Vmid {
    pub const fn new(raw: u16, width: VmidBits) -> Result<Self, Arm64Error> {
        if raw == 0 || raw > width.max_vmid() {
            Err(Arm64Error::new(
                Arm64ErrorKind::UnsupportedVmidWidth,
                "ARM64 VMID is outside the supported VMID width",
            ))
        } else {
            Ok(Self { raw, width })
        }
    }

    pub const fn get(self) -> u16 {
        self.raw
    }

    pub const fn width(self) -> VmidBits {
        self.width
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TlbiKind {
    AllStage2,
    Vmid(Vmid),
    Ipa { vmid: Vmid, ipa: GuestPhysical },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TlbiPlan {
    pub kind: TlbiKind,
    pub needs_dsb_ish: bool,
    pub needs_isb: bool,
}

impl TlbiPlan {
    pub const fn all_stage2() -> Self {
        Self {
            kind: TlbiKind::AllStage2,
            needs_dsb_ish: true,
            needs_isb: true,
        }
    }

    pub const fn vmid(vmid: Vmid) -> Self {
        Self {
            kind: TlbiKind::Vmid(vmid),
            needs_dsb_ish: true,
            needs_isb: true,
        }
    }

    pub const fn ipa(vmid: Vmid, ipa: GuestPhysical) -> Self {
        Self {
            kind: TlbiKind::Ipa { vmid, ipa },
            needs_dsb_ish: true,
            needs_isb: true,
        }
    }
}

pub trait TlbiExecutor {
    /// # Safety
    ///
    /// The caller must execute the TLBI on the CPU context that owns the
    /// affected Stage-2 translations and must issue the required DSB/ISB
    /// barriers around the instruction sequence.
    unsafe fn execute_tlbi(&mut self, plan: TlbiPlan) -> Result<(), Arm64Error>;
}

#[derive(Default)]
pub struct UnsupportedTlbi;

impl TlbiExecutor for UnsupportedTlbi {
    unsafe fn execute_tlbi(&mut self, _plan: TlbiPlan) -> Result<(), Arm64Error> {
        Err(Arm64Error::new(
            Arm64ErrorKind::UnsupportedCapability,
            "ARM64 TLBI execution is not available in this build",
        ))
    }
}

#[cfg(test)]
pub mod tests_support {
    use super::*;

    #[derive(Default)]
    pub struct MockTlbi {
        pub last_plan: Option<TlbiPlan>,
    }

    impl TlbiExecutor for MockTlbi {
        unsafe fn execute_tlbi(&mut self, plan: TlbiPlan) -> Result<(), Arm64Error> {
            self.last_plan = Some(plan);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::MockTlbi;
    use super::*;

    #[test]
    fn tlbi_plan_keeps_barrier_requirements() {
        let vmid = Vmid::new(3, VmidBits::Bits8).unwrap();
        let plan = TlbiPlan::ipa(vmid, GuestPhysical::new(0x4000).unwrap());

        assert!(plan.needs_dsb_ish);
        assert!(plan.needs_isb);
    }

    #[test]
    fn vmid_validation_rejects_zero() {
        assert_eq!(
            Vmid::new(0, VmidBits::Bits8).unwrap_err().kind,
            Arm64ErrorKind::UnsupportedVmidWidth
        );
    }

    #[test]
    fn mock_executor_records_tlbi_plan() {
        let mut executor = MockTlbi::default();
        let vmid = Vmid::new(9, VmidBits::Bits16).unwrap();
        let plan = TlbiPlan::vmid(vmid);

        unsafe { executor.execute_tlbi(plan) }.unwrap();

        assert_eq!(executor.last_plan, Some(plan));
    }
}
