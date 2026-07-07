use aegishv_hypervisor_core::ids::HostPhysical;

use crate::esr::{EsrEl2, ExceptionClass};
use crate::features::{Arm64Error, Arm64ErrorKind, Arm64FeatureSet};
use crate::traps::{handle_sync_trap, SmcPolicy, TrapAction};
use crate::vectors::El2VectorTable;
use crate::vtcr::{VtcrConfig, Vttbr};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RequiredExitCoverage {
    pub hvc: bool,
    pub wfi: bool,
    pub wfe: bool,
    pub instruction_abort: bool,
    pub data_abort: bool,
}

impl RequiredExitCoverage {
    pub const fn toy_guest() -> Self {
        Self {
            hvc: true,
            wfi: true,
            wfe: true,
            instruction_abort: true,
            data_abort: true,
        }
    }

    pub fn mark(&mut self, esr: EsrEl2) {
        match esr.exception_class() {
            ExceptionClass::Hvc64 => self.hvc = true,
            ExceptionClass::WfiWfe if esr.iss() & 1 == 0 => self.wfi = true,
            ExceptionClass::WfiWfe => self.wfe = true,
            ExceptionClass::InstructionAbortLowerEl => self.instruction_abort = true,
            ExceptionClass::DataAbortLowerEl => self.data_abort = true,
            _ => {}
        }
    }

    pub fn contains(self, required: Self) -> bool {
        (!required.hvc || self.hvc)
            && (!required.wfi || self.wfi)
            && (!required.wfe || self.wfe)
            && (!required.instruction_abort || self.instruction_abort)
            && (!required.data_abort || self.data_abort)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arm64ToyGuestPlan {
    pub features: Arm64FeatureSet,
    pub vectors: El2VectorTable,
    pub vtcr: u64,
    pub vttbr: Vttbr,
    pub exits: RequiredExitCoverage,
}

impl Arm64ToyGuestPlan {
    pub fn new(
        features: Arm64FeatureSet,
        vector_base: HostPhysical,
        vtcr_config: VtcrConfig,
        vttbr: Vttbr,
        exits: RequiredExitCoverage,
    ) -> Result<Self, Arm64Error> {
        features.validate_stage2_4k()?;
        let vectors = El2VectorTable::new(vector_base)?;
        if !exits.contains(RequiredExitCoverage::toy_guest()) {
            return Err(Arm64Error::new(
                Arm64ErrorKind::UnsupportedTrap,
                "ARM64 toy guest lab has not covered required exits",
            ));
        }
        Ok(Self {
            features,
            vectors,
            vtcr: vtcr_config.encode(),
            vttbr,
            exits,
        })
    }

    pub fn handle_controlled_exit(&self, esr: EsrEl2) -> Result<TrapAction, Arm64Error> {
        handle_sync_trap(esr, SmcPolicy::Deny)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{
        features_from_id_registers, GicVirtualization, Granule, IdAa64Mmfr0El1, IdAa64Mmfr1El1,
        IdAa64Pfr0El1, SmmuCapability, VmidBits,
    };

    fn features() -> Arm64FeatureSet {
        features_from_id_registers(
            IdAa64Pfr0El1::new(1 << 8),
            IdAa64Mmfr0El1::new(5),
            IdAa64Mmfr1El1::new(0),
            GicVirtualization::Gicv3,
            SmmuCapability::Smmuv3,
            false,
        )
    }

    #[test]
    fn toy_guest_plan_rejects_missing_exit_coverage() {
        let err = Arm64ToyGuestPlan::new(
            features(),
            HostPhysical::new(0x4000).unwrap(),
            VtcrConfig::new(48, Granule::Size4K, VmidBits::Bits8).unwrap(),
            Vttbr::new(1, HostPhysical::new(0x8000).unwrap(), VmidBits::Bits8).unwrap(),
            RequiredExitCoverage::default(),
        )
        .unwrap_err();

        assert_eq!(err.kind, Arm64ErrorKind::UnsupportedTrap);
    }

    #[test]
    fn toy_guest_plan_handles_hvc_exit() {
        let plan = Arm64ToyGuestPlan::new(
            features(),
            HostPhysical::new(0x4000).unwrap(),
            VtcrConfig::new(48, Granule::Size4K, VmidBits::Bits8).unwrap(),
            Vttbr::new(1, HostPhysical::new(0x8000).unwrap(), VmidBits::Bits8).unwrap(),
            RequiredExitCoverage::toy_guest(),
        )
        .unwrap();

        assert_eq!(
            plan.handle_controlled_exit(EsrEl2::new(0x16 << 26))
                .unwrap(),
            TrapAction::Resume
        );
    }
}
