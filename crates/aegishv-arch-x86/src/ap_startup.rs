use aegishv_hypervisor_core::error::{CoreError, CoreErrorKind};
use aegishv_hypervisor_core::ids::{HostPhysical, PhysicalCpuId};

const LOW_MEMORY_LIMIT: u64 = 0x0010_0000;
const MIN_AP_STACK_SIZE: u64 = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApStartupPlan {
    pub bootstrap_cpu: PhysicalCpuId,
    pub trampoline: HostPhysical,
    pub entry_point: u64,
    pub stack_top: HostPhysical,
    pub stack_size: u64,
    pub ap_count: u16,
}

impl ApStartupPlan {
    pub fn new(
        bootstrap_cpu: PhysicalCpuId,
        trampoline: HostPhysical,
        entry_point: u64,
        stack_top: HostPhysical,
        stack_size: u64,
        ap_count: u16,
    ) -> Result<Self, CoreError> {
        let plan = Self {
            bootstrap_cpu,
            trampoline,
            entry_point,
            stack_top,
            stack_size,
            ap_count,
        };
        plan.validate()?;
        Ok(plan)
    }

    pub fn validate(self) -> Result<(), CoreError> {
        if self.trampoline.get() >= LOW_MEMORY_LIMIT || self.trampoline.get() % 4096 != 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "AP startup trampoline must be 4K-aligned below 1 MiB",
            ));
        }
        if self.entry_point == 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "AP startup entry point must not be zero",
            ));
        }
        if self.stack_size < MIN_AP_STACK_SIZE || self.stack_size % 4096 != 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "AP startup stack size must be at least 16 KiB and 4K aligned",
            ));
        }
        if self.stack_top.get() % 16 != 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "AP startup stack top must be 16-byte aligned",
            ));
        }
        if self.ap_count == 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidArgument,
                "AP startup plan must include at least one application processor",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ap_startup_plan_accepts_low_aligned_trampoline_and_stacks() {
        let plan = ApStartupPlan::new(
            PhysicalCpuId::new(0).unwrap(),
            HostPhysical::new(0x8000).unwrap(),
            0xffff_8000_0000_2000,
            HostPhysical::new(0x90000).unwrap(),
            16 * 1024,
            3,
        )
        .unwrap();

        assert_eq!(plan.ap_count, 3);
    }

    #[test]
    fn ap_startup_plan_rejects_bad_trampoline_or_stack() {
        assert_eq!(
            ApStartupPlan::new(
                PhysicalCpuId::new(0).unwrap(),
                HostPhysical::new(0x100000).unwrap(),
                0x2000,
                HostPhysical::new(0x90000).unwrap(),
                16 * 1024,
                1,
            )
            .unwrap_err()
            .kind,
            CoreErrorKind::InvalidAddress
        );

        assert_eq!(
            ApStartupPlan::new(
                PhysicalCpuId::new(0).unwrap(),
                HostPhysical::new(0x8000).unwrap(),
                0x2000,
                HostPhysical::new(0x90008).unwrap(),
                4096,
                1,
            )
            .unwrap_err()
            .kind,
            CoreErrorKind::InvalidArgument
        );
    }
}
