use aegishv_hypervisor_core::allocator::{PageAllocationConstraint, PhysicalPageAllocator};
use aegishv_hypervisor_core::error::CoreErrorKind;
use aegishv_hypervisor_core::memory::{MemoryMap, MemoryRegion, MemoryRegionKind};
use aegishv_type1_boot::{LimineMemmapEntry, LimineMemoryMapError, LimineUsableMemory};

use crate::{Type1RuntimeBackend, Type1RuntimeMemoryPlan};

pub const TYPE1_RUNTIME_MIN_PHYSICAL: u64 = 0x10_0000;
pub const TYPE1_RUNTIME_MAX_PHYSICAL_EXCLUSIVE: u64 = 0x1_0000_0000;
pub const TYPE1_MAX_MEMORY_MAP_ENTRIES: usize = 256;
const TYPE1_EARLY_ALLOCATOR_RUNS: usize = TYPE1_MAX_MEMORY_MAP_ENTRIES * 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1EarlyMemoryError {
    Limine(LimineMemoryMapError),
    Core(CoreErrorKind),
    RollbackFailed(CoreErrorKind),
}

pub struct Type1RuntimeMemoryAllocation {
    plan: Type1RuntimeMemoryPlan,
    allocator: PhysicalPageAllocator<TYPE1_EARLY_ALLOCATOR_RUNS, 2>,
}

impl Type1RuntimeMemoryAllocation {
    pub const fn plan(&self) -> Type1RuntimeMemoryPlan {
        self.plan
    }

    pub const fn allocated_pages(&self) -> usize {
        self.allocator.allocated_pages()
    }
}

pub fn allocate_type1_runtime_memory<const N: usize>(
    entries: &[LimineMemmapEntry],
    backend: Type1RuntimeBackend,
) -> Result<Type1RuntimeMemoryAllocation, Type1EarlyMemoryError> {
    let usable =
        LimineUsableMemory::<N>::from_entries(entries).map_err(Type1EarlyMemoryError::Limine)?;
    let mut normalized = [MemoryRegion::empty(); N];
    for (index, region) in usable.regions().iter().copied().enumerate() {
        normalized[index] = MemoryRegion::new(region.base, region.length, MemoryRegionKind::Usable);
    }
    let map = MemoryMap::<N>::from_entries(&normalized[..usable.regions().len()])
        .map_err(|err| Type1EarlyMemoryError::Core(err.kind))?;
    let mut allocator =
        PhysicalPageAllocator::<TYPE1_EARLY_ALLOCATOR_RUNS, 2>::from_memory_map(&map)
            .map_err(|err| Type1EarlyMemoryError::Core(err.kind))?;
    let constraint = PageAllocationConstraint::new(
        TYPE1_RUNTIME_MIN_PHYSICAL,
        TYPE1_RUNTIME_MAX_PHYSICAL_EXCLUSIVE,
    )
    .map_err(|err| Type1EarlyMemoryError::Core(err.kind))?;

    let plan = match backend {
        Type1RuntimeBackend::None => Type1RuntimeMemoryPlan {
            runtime_base: 0,
            vmxon_physical: 0,
            vmcs_physical: 0,
            svm_vmcb_physical: 0,
        },
        Type1RuntimeBackend::IntelVmx => {
            let vmxon = allocator
                .allocate_within(constraint)
                .map_err(|err| Type1EarlyMemoryError::Core(err.kind))?;
            let vmcs = match allocator.allocate_within(constraint) {
                Ok(page) => page,
                Err(err) => {
                    if let Err(rollback) = allocator.free(vmxon) {
                        return Err(Type1EarlyMemoryError::RollbackFailed(rollback.kind));
                    }
                    return Err(Type1EarlyMemoryError::Core(err.kind));
                }
            };
            Type1RuntimeMemoryPlan {
                runtime_base: vmxon.get(),
                vmxon_physical: vmxon.get(),
                vmcs_physical: vmcs.get(),
                svm_vmcb_physical: 0,
            }
        }
        Type1RuntimeBackend::AmdSvm => {
            let vmcb = allocator
                .allocate_within(constraint)
                .map_err(|err| Type1EarlyMemoryError::Core(err.kind))?;
            Type1RuntimeMemoryPlan {
                runtime_base: vmcb.get(),
                vmxon_physical: 0,
                vmcs_physical: 0,
                svm_vmcb_physical: vmcb.get(),
            }
        }
    };

    Ok(Type1RuntimeMemoryAllocation { plan, allocator })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(base: u64, length: u64, kind: u64) -> LimineMemmapEntry {
        LimineMemmapEntry::new(base, length, kind)
    }

    #[test]
    fn intel_runtime_pages_come_from_usable_memory() {
        let entries = [
            entry(0, 0x10_0000, 0),
            entry(0x10_0000, 0x20_0000, 0),
            entry(0x40_0000, 0x10_0000, 5),
        ];

        let allocation =
            allocate_type1_runtime_memory::<8>(&entries, Type1RuntimeBackend::IntelVmx).unwrap();
        let plan = allocation.plan();

        assert_eq!(allocation.allocated_pages(), 2);
        assert_eq!(plan.vmxon_physical, 0x10_0000);
        assert_eq!(plan.vmcs_physical, 0x10_1000);
        assert_ne!(plan.vmxon_physical, plan.vmcs_physical);
        assert_eq!(plan.svm_vmcb_physical, 0);
    }

    #[test]
    fn backend_specific_allocation_does_not_reserve_unused_pages() {
        let entries = [entry(0x20_0000, 0x10_0000, 0)];

        let svm =
            allocate_type1_runtime_memory::<4>(&entries, Type1RuntimeBackend::AmdSvm).unwrap();
        assert_eq!(svm.allocated_pages(), 1);
        assert_eq!(svm.plan().svm_vmcb_physical, 0x20_0000);
        assert_eq!(svm.plan().vmxon_physical, 0);

        let none = allocate_type1_runtime_memory::<4>(&entries, Type1RuntimeBackend::None).unwrap();
        assert_eq!(none.allocated_pages(), 0);
        assert_eq!(none.plan().runtime_base, 0);
    }

    #[test]
    fn bootloader_reclaimable_memory_cannot_back_runtime_pages() {
        let entries = [entry(0x20_0000, 0x10_0000, 5)];

        let error =
            match allocate_type1_runtime_memory::<4>(&entries, Type1RuntimeBackend::IntelVmx) {
                Ok(_) => panic!("bootloader-reclaimable memory was allocated"),
                Err(error) => error,
            };
        assert_eq!(
            error,
            Type1EarlyMemoryError::Core(CoreErrorKind::OutOfMemory)
        );
    }
}
