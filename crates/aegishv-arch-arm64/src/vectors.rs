use aegishv_hypervisor_core::ids::HostPhysical;

use crate::features::{Arm64Error, Arm64ErrorKind};

pub const EL2_VECTOR_ALIGNMENT: u64 = 2048;
pub const EL2_VECTOR_SLOTS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum El2VectorSlot {
    CurrentSp0Sync,
    CurrentSp0Irq,
    CurrentSp0Fiq,
    CurrentSp0SError,
    CurrentSpxSync,
    CurrentSpxIrq,
    CurrentSpxFiq,
    CurrentSpxSError,
    LowerAarch64Sync,
    LowerAarch64Irq,
    LowerAarch64Fiq,
    LowerAarch64SError,
    LowerAarch32Sync,
    LowerAarch32Irq,
    LowerAarch32Fiq,
    LowerAarch32SError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct El2VectorTable {
    pub base: HostPhysical,
    pub slots: [El2VectorSlot; EL2_VECTOR_SLOTS],
}

impl El2VectorTable {
    pub fn new(base: HostPhysical) -> Result<Self, Arm64Error> {
        if base.get() == 0 || base.get() % EL2_VECTOR_ALIGNMENT != 0 {
            return Err(Arm64Error::new(
                Arm64ErrorKind::InvalidAddress,
                "ARM64 EL2 vector base must be non-zero and 2K-aligned",
            ));
        }
        Ok(Self {
            base,
            slots: [
                El2VectorSlot::CurrentSp0Sync,
                El2VectorSlot::CurrentSp0Irq,
                El2VectorSlot::CurrentSp0Fiq,
                El2VectorSlot::CurrentSp0SError,
                El2VectorSlot::CurrentSpxSync,
                El2VectorSlot::CurrentSpxIrq,
                El2VectorSlot::CurrentSpxFiq,
                El2VectorSlot::CurrentSpxSError,
                El2VectorSlot::LowerAarch64Sync,
                El2VectorSlot::LowerAarch64Irq,
                El2VectorSlot::LowerAarch64Fiq,
                El2VectorSlot::LowerAarch64SError,
                El2VectorSlot::LowerAarch32Sync,
                El2VectorSlot::LowerAarch32Irq,
                El2VectorSlot::LowerAarch32Fiq,
                El2VectorSlot::LowerAarch32SError,
            ],
        })
    }

    pub const fn target_gate_enabled() -> bool {
        cfg!(target_arch = "aarch64")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el2_vector_table_requires_2k_aligned_base() {
        assert_eq!(
            El2VectorTable::new(HostPhysical::new(0x2100).unwrap())
                .unwrap_err()
                .kind,
            Arm64ErrorKind::InvalidAddress
        );
    }

    #[test]
    fn el2_vector_table_has_all_architectural_slots() {
        let table = El2VectorTable::new(HostPhysical::new(0x4000).unwrap()).unwrap();

        assert_eq!(table.slots.len(), EL2_VECTOR_SLOTS);
        assert_eq!(table.slots[8], El2VectorSlot::LowerAarch64Sync);
    }
}
