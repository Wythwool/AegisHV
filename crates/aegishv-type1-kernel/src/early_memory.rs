use aegishv_hypervisor_core::allocator::{PageAllocationConstraint, PhysicalPageAllocator};
use aegishv_hypervisor_core::error::CoreErrorKind;
use aegishv_hypervisor_core::ids::HostPhysical;
use aegishv_hypervisor_core::memory::{MemoryMap, MemoryRegion, MemoryRegionKind};
use aegishv_type1_boot::{LimineMemmapEntry, LimineMemoryMapError, LimineUsableMemory};

use crate::{Type1RuntimeBackend, Type1RuntimeMemoryPlan};

pub const TYPE1_RUNTIME_MIN_PHYSICAL: u64 = 0x10_0000;
pub const TYPE1_RUNTIME_MAX_PHYSICAL_EXCLUSIVE: u64 = 0x1_0000_0000;
pub const TYPE1_MAX_MEMORY_MAP_ENTRIES: usize = 256;
pub const TYPE1_MAX_PHYSICAL_RESERVATIONS: usize = 8;
const TYPE1_EARLY_ALLOCATOR_RUNS: usize = TYPE1_MAX_MEMORY_MAP_ENTRIES * 2;
const TYPE1_EARLY_ALLOCATOR_ALLOCATIONS: usize = 64;
const TYPE1_TOY_GUEST_PAGE_COUNT: usize = 13;
const PAGE_SIZE_4K: u64 = 4096;
const X86_64_MAX_PHYSICAL_ADDRESS_EXCLUSIVE: u64 = 1_u64 << 52;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type1EarlyMemoryError {
    Limine(LimineMemoryMapError),
    Core(CoreErrorKind),
    RollbackFailed(CoreErrorKind),
    BackendMismatch,
    ToyGuestAlreadyAllocated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1PhysicalReservation {
    base: u64,
    length: u64,
}

impl Type1PhysicalReservation {
    pub const fn new(base: u64, length: u64) -> Result<Self, Type1EarlyMemoryError> {
        if length == 0 || base.checked_add(length).is_none() {
            return Err(Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress));
        }
        Ok(Self { base, length })
    }

    pub const fn base(self) -> u64 {
        self.base
    }

    pub const fn length(self) -> u64 {
        self.length
    }
}

const EMPTY_PHYSICAL_RESERVATION: Type1PhysicalReservation =
    Type1PhysicalReservation { base: 0, length: 0 };

pub const fn linked_kernel_reservation(
    executable_physical_base: u64,
    executable_virtual_base: u64,
    linked_virtual_start: u64,
    linked_virtual_end: u64,
) -> Result<Type1PhysicalReservation, Type1EarlyMemoryError> {
    if linked_virtual_start != executable_virtual_base
        || executable_physical_base % PAGE_SIZE_4K != 0
        || linked_virtual_start % PAGE_SIZE_4K != 0
        || linked_virtual_end % PAGE_SIZE_4K != 0
        || !aegishv_arch_x86::vmx::features::is_canonical_u64(linked_virtual_start)
    {
        return Err(Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress));
    }
    let length = match linked_virtual_end.checked_sub(linked_virtual_start) {
        Some(length) if length != 0 => length,
        _ => return Err(Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress)),
    };
    if !aegishv_arch_x86::vmx::features::is_canonical_u64(linked_virtual_end - 1) {
        return Err(Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress));
    }
    match executable_physical_base.checked_add(length) {
        Some(end) if end <= X86_64_MAX_PHYSICAL_ADDRESS_EXCLUSIVE => {}
        _ => return Err(Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress)),
    }
    Type1PhysicalReservation::new(executable_physical_base, length)
}

pub const fn inherited_x86_cr3_root_reservation(
    raw_cr3: u64,
) -> Result<Type1PhysicalReservation, Type1EarlyMemoryError> {
    let root = raw_cr3 & !(PAGE_SIZE_4K - 1);
    if root == 0 || root >= X86_64_MAX_PHYSICAL_ADDRESS_EXCLUSIVE {
        return Err(Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress));
    }
    Type1PhysicalReservation::new(root, PAGE_SIZE_4K)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Type1ToyGuestHostPages {
    pub code: HostPhysical,
    pub stack: HostPhysical,
    pub guest_pml4: HostPhysical,
    pub guest_pdpt: HostPhysical,
    pub guest_pd: HostPhysical,
    pub guest_pt: HostPhysical,
    pub ept_pml4: HostPhysical,
    pub ept_pdpt: HostPhysical,
    pub ept_pd: HostPhysical,
    pub ept_pt: HostPhysical,
    pub io_bitmap_a: HostPhysical,
    pub io_bitmap_b: HostPhysical,
    pub msr_bitmap: HostPhysical,
}

impl Type1ToyGuestHostPages {
    pub const fn all(self) -> [HostPhysical; TYPE1_TOY_GUEST_PAGE_COUNT] {
        [
            self.code,
            self.stack,
            self.guest_pml4,
            self.guest_pdpt,
            self.guest_pd,
            self.guest_pt,
            self.ept_pml4,
            self.ept_pdpt,
            self.ept_pd,
            self.ept_pt,
            self.io_bitmap_a,
            self.io_bitmap_b,
            self.msr_bitmap,
        ]
    }

    pub const fn interception_bitmaps(self) -> [HostPhysical; 3] {
        [self.io_bitmap_a, self.io_bitmap_b, self.msr_bitmap]
    }
}

pub struct Type1RuntimeMemoryAllocation {
    plan: Type1RuntimeMemoryPlan,
    allocator: PhysicalPageAllocator<TYPE1_EARLY_ALLOCATOR_RUNS, TYPE1_EARLY_ALLOCATOR_ALLOCATIONS>,
    toy_guest: Option<Type1ToyGuestHostPages>,
    excluded_usable_pages: u64,
    reservations: [Type1PhysicalReservation; TYPE1_MAX_PHYSICAL_RESERVATIONS],
    reservation_count: usize,
}

impl Type1RuntimeMemoryAllocation {
    pub const fn plan(&self) -> Type1RuntimeMemoryPlan {
        self.plan
    }

    pub const fn allocated_pages(&self) -> usize {
        self.allocator.allocated_pages()
    }

    pub const fn toy_guest_pages(&self) -> Option<Type1ToyGuestHostPages> {
        self.toy_guest
    }

    pub const fn excluded_usable_pages(&self) -> u64 {
        self.excluded_usable_pages
    }

    pub fn reservations(&self) -> &[Type1PhysicalReservation] {
        &self.reservations[..self.reservation_count]
    }

    pub fn allocate_intel_toy_guest(
        &mut self,
    ) -> Result<Type1ToyGuestHostPages, Type1EarlyMemoryError> {
        if self.plan.vmxon_physical == 0 || self.plan.vmcs_physical == 0 {
            return Err(Type1EarlyMemoryError::BackendMismatch);
        }
        if self.toy_guest.is_some() {
            return Err(Type1EarlyMemoryError::ToyGuestAlreadyAllocated);
        }
        let constraint = runtime_page_constraint()?;
        let mut pages = [HostPhysical::ZERO; TYPE1_TOY_GUEST_PAGE_COUNT];
        let mut allocated = 0;
        while allocated < pages.len() {
            match self.allocator.allocate_within(constraint) {
                Ok(page) => {
                    pages[allocated] = page;
                    allocated += 1;
                }
                Err(error) => {
                    let mut rollback_error = None;
                    while allocated > 0 {
                        allocated -= 1;
                        if let Err(rollback) = self.allocator.free(pages[allocated]) {
                            if rollback_error.is_none() {
                                rollback_error = Some(rollback.kind);
                            }
                        }
                    }
                    if let Some(kind) = rollback_error {
                        return Err(Type1EarlyMemoryError::RollbackFailed(kind));
                    }
                    return Err(Type1EarlyMemoryError::Core(error.kind));
                }
            }
        }

        let toy_guest = Type1ToyGuestHostPages {
            code: pages[0],
            stack: pages[1],
            guest_pml4: pages[2],
            guest_pdpt: pages[3],
            guest_pd: pages[4],
            guest_pt: pages[5],
            ept_pml4: pages[6],
            ept_pdpt: pages[7],
            ept_pd: pages[8],
            ept_pt: pages[9],
            io_bitmap_a: pages[10],
            io_bitmap_b: pages[11],
            msr_bitmap: pages[12],
        };
        self.toy_guest = Some(toy_guest);
        Ok(toy_guest)
    }
}

fn runtime_page_constraint() -> Result<PageAllocationConstraint, Type1EarlyMemoryError> {
    PageAllocationConstraint::new(
        TYPE1_RUNTIME_MIN_PHYSICAL,
        TYPE1_RUNTIME_MAX_PHYSICAL_EXCLUSIVE,
    )
    .map_err(|err| Type1EarlyMemoryError::Core(err.kind))
}

pub fn allocate_type1_runtime_memory<const N: usize>(
    entries: &[LimineMemmapEntry],
    backend: Type1RuntimeBackend,
) -> Result<Type1RuntimeMemoryAllocation, Type1EarlyMemoryError> {
    allocate_type1_runtime_memory_with_reservations::<N>(entries, backend, &[])
}

pub fn allocate_type1_runtime_memory_with_reservations<const N: usize>(
    entries: &[LimineMemmapEntry],
    backend: Type1RuntimeBackend,
    reservations: &[Type1PhysicalReservation],
) -> Result<Type1RuntimeMemoryAllocation, Type1EarlyMemoryError> {
    if reservations.len() > TYPE1_MAX_PHYSICAL_RESERVATIONS {
        return Err(Type1EarlyMemoryError::Core(CoreErrorKind::CapacityExceeded));
    }
    let usable =
        LimineUsableMemory::<N>::from_entries(entries).map_err(Type1EarlyMemoryError::Limine)?;
    let mut normalized = [MemoryRegion::empty(); N];
    for (index, region) in usable.regions().iter().copied().enumerate() {
        normalized[index] = MemoryRegion::new(region.base, region.length, MemoryRegionKind::Usable);
    }
    let map = MemoryMap::<N>::from_entries(&normalized[..usable.regions().len()])
        .map_err(|err| Type1EarlyMemoryError::Core(err.kind))?;
    let mut allocator = PhysicalPageAllocator::<
        TYPE1_EARLY_ALLOCATOR_RUNS,
        TYPE1_EARLY_ALLOCATOR_ALLOCATIONS,
    >::from_memory_map(&map)
    .map_err(|err| Type1EarlyMemoryError::Core(err.kind))?;
    let mut excluded_usable_pages = 0_u64;
    for reservation in reservations {
        let removed = allocator
            .reserve_range(reservation.base, reservation.length)
            .map_err(|err| Type1EarlyMemoryError::Core(err.kind))?;
        excluded_usable_pages = excluded_usable_pages
            .checked_add(removed)
            .ok_or(Type1EarlyMemoryError::Core(CoreErrorKind::CapacityExceeded))?;
    }
    let mut accepted_reservations = [EMPTY_PHYSICAL_RESERVATION; TYPE1_MAX_PHYSICAL_RESERVATIONS];
    accepted_reservations[..reservations.len()].copy_from_slice(reservations);
    let constraint = runtime_page_constraint()?;

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

    Ok(Type1RuntimeMemoryAllocation {
        plan,
        allocator,
        toy_guest: None,
        excluded_usable_pages,
        reservations: accepted_reservations,
        reservation_count: reservations.len(),
    })
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

    #[test]
    fn intel_toy_guest_reserves_fifteen_distinct_usable_pages() {
        let entries = [entry(0x20_0000, 0x20_0000, 0)];
        let mut allocation =
            allocate_type1_runtime_memory::<4>(&entries, Type1RuntimeBackend::IntelVmx).unwrap();

        let guest = allocation.allocate_intel_toy_guest().unwrap();
        let pages = guest.all();

        assert_eq!(allocation.allocated_pages(), 15);
        for (index, page) in pages.iter().enumerate() {
            assert!(page.get() >= 0x20_0000);
            assert!(page.get() < 0x40_0000);
            assert!(!pages[..index].contains(page));
            assert_ne!(page.get(), allocation.plan().vmxon_physical);
            assert_ne!(page.get(), allocation.plan().vmcs_physical);
        }
        assert_eq!(allocation.toy_guest_pages(), Some(guest));
        assert_eq!(
            allocation.allocate_intel_toy_guest().unwrap_err(),
            Type1EarlyMemoryError::ToyGuestAlreadyAllocated
        );
    }

    #[test]
    fn failed_toy_guest_allocation_rolls_back_only_guest_pages() {
        let entries = [entry(0x20_0000, 14 * 4096, 0)];
        let mut allocation =
            allocate_type1_runtime_memory::<4>(&entries, Type1RuntimeBackend::IntelVmx).unwrap();

        assert_eq!(
            allocation.allocate_intel_toy_guest().unwrap_err(),
            Type1EarlyMemoryError::Core(CoreErrorKind::OutOfMemory)
        );
        assert_eq!(allocation.allocated_pages(), 2);
        assert_eq!(allocation.toy_guest_pages(), None);
    }

    #[test]
    fn explicit_kernel_reservation_wins_over_a_usable_map_entry() {
        let entries = [entry(0x20_0000, 0x20_0000, 0)];
        let reservation = Type1PhysicalReservation::new(0x20_0000, 0x10_000).unwrap();

        let allocation = allocate_type1_runtime_memory_with_reservations::<4>(
            &entries,
            Type1RuntimeBackend::IntelVmx,
            &[reservation],
        )
        .unwrap();

        assert_eq!(allocation.excluded_usable_pages(), 0x10);
        assert_eq!(allocation.plan().vmxon_physical, 0x21_0000);
        assert_eq!(allocation.plan().vmcs_physical, 0x21_1000);
    }

    #[test]
    fn reservation_constructor_rejects_empty_and_wrapping_ranges() {
        assert_eq!(
            Type1PhysicalReservation::new(0x20_0000, 0).unwrap_err(),
            Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress)
        );
        assert_eq!(
            Type1PhysicalReservation::new(u64::MAX - 1, 4).unwrap_err(),
            Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress)
        );
    }

    #[test]
    fn linked_kernel_reservation_validates_the_loaded_image_bounds() {
        let virtual_base = 0xffff_ffff_8020_0000;
        let reservation =
            linked_kernel_reservation(0x20_0000, virtual_base, virtual_base, virtual_base + 0x4000)
                .unwrap();

        assert_eq!(reservation.base(), 0x20_0000);
        assert_eq!(reservation.length(), 0x4000);
        assert_eq!(
            linked_kernel_reservation(
                0x20_0000,
                virtual_base + 0x1000,
                virtual_base,
                virtual_base + 0x4000,
            )
            .unwrap_err(),
            Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress)
        );
        assert_eq!(
            linked_kernel_reservation(0x20_0000, virtual_base, virtual_base, virtual_base + 1,)
                .unwrap_err(),
            Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress)
        );
        assert_eq!(
            linked_kernel_reservation(
                u64::MAX - 0xfff,
                virtual_base,
                virtual_base,
                virtual_base + 0x2000,
            )
            .unwrap_err(),
            Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress)
        );
        let last_low_canonical_page = 0x0000_7fff_ffff_f000;
        assert_eq!(
            linked_kernel_reservation(
                0x20_0000,
                last_low_canonical_page,
                last_low_canonical_page,
                last_low_canonical_page + 0x2000,
            )
            .unwrap_err(),
            Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress)
        );
    }

    #[test]
    fn inherited_cr3_reservation_masks_pcid_and_rejects_invalid_roots() {
        let reservation = inherited_x86_cr3_root_reservation(0x1234_5abc).unwrap();

        assert_eq!(reservation.base(), 0x1234_5000);
        assert_eq!(reservation.length(), PAGE_SIZE_4K);
        assert_eq!(
            inherited_x86_cr3_root_reservation(0xabc).unwrap_err(),
            Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress)
        );
        assert_eq!(
            inherited_x86_cr3_root_reservation(1_u64 << 52).unwrap_err(),
            Type1EarlyMemoryError::Core(CoreErrorKind::InvalidAddress)
        );
    }

    #[test]
    fn one_owner_keeps_reserved_pages_out_of_runtime_and_guest_allocations() {
        let entries = [entry(0x10_0000, 0x40_0000, 0)];
        let virtual_base = 0xffff_ffff_8020_0000;
        let kernel =
            linked_kernel_reservation(0x10_0000, virtual_base, virtual_base, virtual_base + 0x4000)
                .unwrap();
        let active_root = inherited_x86_cr3_root_reservation(0x10_4abc).unwrap();
        let mut allocation = allocate_type1_runtime_memory_with_reservations::<4>(
            &entries,
            Type1RuntimeBackend::IntelVmx,
            &[kernel, active_root],
        )
        .unwrap();
        let plan_before_guest = allocation.plan();

        assert_eq!(allocation.reservations(), &[kernel, active_root]);
        assert_eq!(allocation.excluded_usable_pages(), 5);
        assert_eq!(plan_before_guest.vmxon_physical, 0x10_5000);
        assert_eq!(plan_before_guest.vmcs_physical, 0x10_6000);
        let guest = allocation.allocate_intel_toy_guest().unwrap();
        assert_eq!(allocation.plan(), plan_before_guest);
        for page in guest.all() {
            assert!(page.get() >= 0x10_7000);
        }
    }

    #[test]
    fn reservations_are_recorded_even_when_the_map_already_excludes_them() {
        let entries = [entry(0x20_0000, 0x20_0000, 0)];
        let kernel = Type1PhysicalReservation::new(0x10_0000, 0x4000).unwrap();
        let active_root = inherited_x86_cr3_root_reservation(0x80_abc).unwrap();

        let allocation = allocate_type1_runtime_memory_with_reservations::<4>(
            &entries,
            Type1RuntimeBackend::IntelVmx,
            &[kernel, active_root],
        )
        .unwrap();

        assert_eq!(allocation.excluded_usable_pages(), 0);
        assert_eq!(allocation.reservations(), &[kernel, active_root]);
        assert_eq!(allocation.plan().vmxon_physical, 0x20_0000);
    }

    #[test]
    fn reservation_ledger_has_a_hard_capacity() {
        let entries = [entry(0x20_0000, 0x20_0000, 0)];
        let reservations = [Type1PhysicalReservation::new(0x1000, 0x1000).unwrap();
            TYPE1_MAX_PHYSICAL_RESERVATIONS + 1];

        let result = allocate_type1_runtime_memory_with_reservations::<4>(
            &entries,
            Type1RuntimeBackend::IntelVmx,
            &reservations,
        );
        assert!(matches!(
            result,
            Err(Type1EarlyMemoryError::Core(CoreErrorKind::CapacityExceeded))
        ));
    }
}
