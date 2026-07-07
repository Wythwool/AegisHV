use aegishv_hypervisor_core::ids::GuestVirtual;

use super::features::{SvmError, SvmErrorKind};
use super::instructions::SvmInstructionExecutor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SvmAsid(u32);

impl SvmAsid {
    pub const fn new(raw: u32) -> Result<Self, SvmError> {
        if raw == 0 {
            Err(SvmError::new(
                SvmErrorKind::InvalidAsid,
                "SVM ASID 0 is reserved",
            ))
        } else {
            Ok(Self(raw))
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

pub struct SvmAsidAllocator<const N: usize> {
    max_asid: u32,
    used: [bool; N],
}

impl<const N: usize> SvmAsidAllocator<N> {
    pub const fn new(max_asid: u32) -> Self {
        Self {
            max_asid,
            used: [false; N],
        }
    }

    pub fn allocate(&mut self) -> Result<SvmAsid, SvmError> {
        let limit = core::cmp::min(self.max_asid as usize, N);
        for index in 1..limit {
            if !self.used[index] {
                self.used[index] = true;
                return SvmAsid::new(index as u32);
            }
        }
        Err(SvmError::new(
            SvmErrorKind::MissingAsidCapacity,
            "SVM ASID allocator has no free ASID",
        ))
    }

    pub fn release(&mut self, asid: SvmAsid) -> Result<(), SvmError> {
        let index = asid.get() as usize;
        if index >= N || index > self.max_asid as usize || !self.used[index] {
            return Err(SvmError::new(
                SvmErrorKind::InvalidAsid,
                "SVM ASID release does not match an allocated ASID",
            ));
        }
        self.used[index] = false;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SvmInvalidation {
    WholeAsid(SvmAsid),
    SingleAddress {
        asid: SvmAsid,
        guest_virtual: GuestVirtual,
    },
}

impl SvmInvalidation {
    /// # Safety
    ///
    /// The caller must run the invalidation on the CPU that owns the ASID.
    pub unsafe fn execute_with<E: SvmInstructionExecutor>(
        self,
        executor: &mut E,
    ) -> Result<(), SvmError> {
        match self {
            Self::WholeAsid(asid) => unsafe { executor.invlpga(0, asid.get()) },
            Self::SingleAddress {
                asid,
                guest_virtual,
            } => unsafe { executor.invlpga(guest_virtual.get(), asid.get()) },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svm::instructions::tests_support::MockSvmInstructions;

    #[test]
    fn asid_allocator_skips_reserved_zero_and_reuses_released_slot() {
        let mut allocator = SvmAsidAllocator::<4>::new(4);
        let first = allocator.allocate().unwrap();
        let second = allocator.allocate().unwrap();

        assert_eq!(first.get(), 1);
        assert_eq!(second.get(), 2);

        allocator.release(first).unwrap();
        assert_eq!(allocator.allocate().unwrap().get(), 1);
    }

    #[test]
    fn asid_allocator_rejects_double_release() {
        let mut allocator = SvmAsidAllocator::<4>::new(4);
        let asid = allocator.allocate().unwrap();
        allocator.release(asid).unwrap();

        assert_eq!(
            allocator.release(asid).unwrap_err().kind,
            SvmErrorKind::InvalidAsid
        );
    }

    #[test]
    fn invalidation_uses_invlpga_wrapper() {
        let mut executor = MockSvmInstructions::default();
        let asid = SvmAsid::new(7).unwrap();

        unsafe {
            SvmInvalidation::SingleAddress {
                asid,
                guest_virtual: GuestVirtual::new(0x4000),
            }
            .execute_with(&mut executor)
        }
        .unwrap();

        assert_eq!(executor.last_invlpga, Some((0x4000, 7)));
    }
}
