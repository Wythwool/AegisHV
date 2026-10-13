use crate::features::{Arm64Error, Arm64ErrorKind, GicVirtualization};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GicVersion {
    V2,
    V3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtualInterrupt {
    pub intid: u32,
    pub priority: u8,
    pub group1: bool,
}

impl VirtualInterrupt {
    pub const fn new(intid: u32, priority: u8, group1: bool) -> Result<Self, Arm64Error> {
        if intid < 16 || intid > 1019 {
            Err(Arm64Error::new(
                Arm64ErrorKind::MissingGicVirtualization,
                "ARM64 virtual interrupt id is outside the GIC SPI/PPI range",
            ))
        } else {
            Ok(Self {
                intid,
                priority,
                group1,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GicVirtualizationPlan {
    pub version: GicVersion,
    pub list_registers: u8,
    pub maintenance_irq: Option<u32>,
}

impl GicVirtualizationPlan {
    pub const fn new(
        gic: GicVirtualization,
        list_registers: u8,
        maintenance_irq: Option<u32>,
    ) -> Result<Self, Arm64Error> {
        let version = match gic {
            GicVirtualization::Gicv2 => GicVersion::V2,
            GicVirtualization::Gicv3 => GicVersion::V3,
            GicVirtualization::None => {
                return Err(Arm64Error::new(
                    Arm64ErrorKind::MissingGicVirtualization,
                    "ARM64 GIC virtualization is not available",
                ))
            }
        };
        if list_registers == 0 {
            return Err(Arm64Error::new(
                Arm64ErrorKind::MissingGicVirtualization,
                "ARM64 VGIC needs at least one list register",
            ));
        }
        Ok(Self {
            version,
            list_registers,
            maintenance_irq,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gic_plan_rejects_missing_virtualization_and_zero_lrs() {
        assert_eq!(
            GicVirtualizationPlan::new(GicVirtualization::None, 4, None)
                .unwrap_err()
                .kind,
            Arm64ErrorKind::MissingGicVirtualization
        );
        assert_eq!(
            GicVirtualizationPlan::new(GicVirtualization::Gicv3, 0, None)
                .unwrap_err()
                .kind,
            Arm64ErrorKind::MissingGicVirtualization
        );
    }

    #[test]
    fn virtual_interrupt_rejects_sgi_range_for_lab_injection() {
        assert_eq!(
            VirtualInterrupt::new(2, 0x80, true).unwrap_err().kind,
            Arm64ErrorKind::MissingGicVirtualization
        );
    }
}
