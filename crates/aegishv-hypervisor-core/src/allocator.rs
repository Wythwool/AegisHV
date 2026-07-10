use crate::error::{CoreError, CoreErrorKind};
use crate::ids::HostPhysical;
use crate::memory::MemoryMap;

pub const PAGE_SIZE_4K: u64 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageAllocationConstraint {
    pub min_address: u64,
    pub max_address_exclusive: u64,
}

impl PageAllocationConstraint {
    pub const fn new(min_address: u64, max_address_exclusive: u64) -> Result<Self, CoreError> {
        if min_address >= max_address_exclusive {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "physical allocation constraint has an empty address range",
            ));
        }
        Ok(Self {
            min_address,
            max_address_exclusive,
        })
    }

    pub const fn any() -> Self {
        Self {
            min_address: 0,
            max_address_exclusive: u64::MAX,
        }
    }
}

pub trait PageZeroer {
    fn zero_page(&mut self, page: HostPhysical) -> Result<(), CoreError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PageRun {
    start: u64,
    count: u64,
}

impl PageRun {
    const fn empty() -> Self {
        Self { start: 0, count: 0 }
    }

    fn end(self) -> Result<u64, CoreError> {
        self.start
            .checked_add(self.count.saturating_mul(PAGE_SIZE_4K))
            .ok_or(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "page run end address overflowed",
            ))
    }
}

pub struct PhysicalPageAllocator<const R: usize, const A: usize> {
    free: [PageRun; R],
    free_len: usize,
    allocated: [u64; A],
    allocated_len: usize,
}

impl<const R: usize, const A: usize> PhysicalPageAllocator<R, A> {
    pub fn from_memory_map<const M: usize>(map: &MemoryMap<M>) -> Result<Self, CoreError> {
        let mut allocator = Self {
            free: [PageRun::empty(); R],
            free_len: 0,
            allocated: [0; A],
            allocated_len: 0,
        };

        for region in map.usable_regions() {
            let start = align_up(region.base, PAGE_SIZE_4K)?;
            let end = align_down(region.end()?, PAGE_SIZE_4K);
            if end <= start {
                continue;
            }
            allocator.insert_free_run(PageRun {
                start,
                count: (end - start) / PAGE_SIZE_4K,
            })?;
        }

        Ok(allocator)
    }

    pub const fn free_pages(&self) -> u64 {
        let mut pages = 0;
        let mut index = 0;
        while index < self.free_len {
            pages += self.free[index].count;
            index += 1;
        }
        pages
    }

    pub const fn allocated_pages(&self) -> usize {
        self.allocated_len
    }

    pub fn reserve_range(&mut self, base: u64, length: u64) -> Result<u64, CoreError> {
        if length == 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "physical reservation has an empty address range",
            ));
        }
        let raw_end = base.checked_add(length).ok_or(CoreError::new(
            CoreErrorKind::InvalidAddress,
            "physical reservation end address overflowed",
        ))?;
        let start = align_down(base, PAGE_SIZE_4K);
        let end = align_up(raw_end, PAGE_SIZE_4K)?;

        for page in self.allocated.iter().take(self.allocated_len).copied() {
            if page < end && page + PAGE_SIZE_4K > start {
                return Err(CoreError::new(
                    CoreErrorKind::Overlap,
                    "physical reservation overlaps an allocated page",
                ));
            }
        }

        for run in self.free.iter().take(self.free_len).copied() {
            let run_end = run.end()?;
            if start > run.start && end < run_end && self.free_len >= R {
                return Err(CoreError::new(
                    CoreErrorKind::CapacityExceeded,
                    "physical allocator cannot split a free run for a reservation",
                ));
            }
        }

        let free_before = self.free_pages();
        let mut index = 0;
        while index < self.free_len {
            let run = self.free[index];
            let run_end = run.end()?;
            if end <= run.start {
                break;
            }
            if start >= run_end {
                index += 1;
                continue;
            }

            let left_count = if start > run.start {
                (start - run.start) / PAGE_SIZE_4K
            } else {
                0
            };
            let right_count = if end < run_end {
                (run_end - end) / PAGE_SIZE_4K
            } else {
                0
            };
            match (left_count, right_count) {
                (0, 0) => remove_run(&mut self.free, &mut self.free_len, index),
                (0, right) => {
                    self.free[index] = PageRun {
                        start: end,
                        count: right,
                    };
                    index += 1;
                }
                (left, 0) => {
                    self.free[index].count = left;
                    index += 1;
                }
                (left, right) => {
                    let mut cursor = self.free_len;
                    while cursor > index + 1 {
                        self.free[cursor] = self.free[cursor - 1];
                        cursor -= 1;
                    }
                    self.free[index].count = left;
                    self.free[index + 1] = PageRun {
                        start: end,
                        count: right,
                    };
                    self.free_len += 1;
                    index += 2;
                }
            }
        }
        Ok(free_before - self.free_pages())
    }

    pub fn allocate(&mut self) -> Result<HostPhysical, CoreError> {
        self.allocate_within(PageAllocationConstraint::any())
    }

    pub fn allocate_within(
        &mut self,
        constraint: PageAllocationConstraint,
    ) -> Result<HostPhysical, CoreError> {
        if constraint.min_address >= constraint.max_address_exclusive {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "physical allocation constraint has an empty address range",
            ));
        }
        if self.allocated_len >= A {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "physical allocator allocation tracking table is full",
            ));
        }
        let mut selected = None;
        for index in 0..self.free_len {
            let run = self.free[index];
            let run_end = run.end()?;
            let candidate = align_up(run.start.max(constraint.min_address), PAGE_SIZE_4K)?;
            let candidate_end = match candidate.checked_add(PAGE_SIZE_4K) {
                Some(value) => value,
                None => continue,
            };
            let allowed_end = run_end.min(constraint.max_address_exclusive);
            if candidate_end <= allowed_end {
                selected = Some((index, candidate, run_end));
                break;
            }
        }

        let (run_index, page, run_end) = selected.ok_or(CoreError::new(
            CoreErrorKind::OutOfMemory,
            "physical allocator has no free 4K page in the requested address range",
        ))?;
        self.take_page_from_run(run_index, page, run_end)?;

        self.allocated[self.allocated_len] = page;
        self.allocated_len += 1;
        HostPhysical::new(page)
    }

    pub fn allocate_zeroed(
        &mut self,
        zeroer: &mut impl PageZeroer,
    ) -> Result<HostPhysical, CoreError> {
        let page = self.allocate()?;
        if let Err(err) = zeroer.zero_page(page) {
            self.free(page)?;
            return Err(err);
        }
        Ok(page)
    }

    pub fn free(&mut self, page: HostPhysical) -> Result<(), CoreError> {
        if page.get() % PAGE_SIZE_4K != 0 {
            return Err(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "physical page free address is not 4K aligned",
            ));
        }

        let index = self
            .allocated
            .iter()
            .take(self.allocated_len)
            .position(|&entry| entry == page.get())
            .ok_or(CoreError::new(
                CoreErrorKind::DoubleFree,
                "physical page was not allocated by this allocator",
            ))?;

        self.insert_free_run(PageRun {
            start: page.get(),
            count: 1,
        })?;
        remove_allocated(&mut self.allocated, &mut self.allocated_len, index);
        Ok(())
    }

    fn take_page_from_run(
        &mut self,
        index: usize,
        page: u64,
        run_end: u64,
    ) -> Result<(), CoreError> {
        let run = self.free[index];
        let page_end = page + PAGE_SIZE_4K;
        let before_count = (page - run.start) / PAGE_SIZE_4K;
        let after_count = (run_end - page_end) / PAGE_SIZE_4K;

        match (before_count, after_count) {
            (0, 0) => remove_run(&mut self.free, &mut self.free_len, index),
            (0, after) => {
                self.free[index] = PageRun {
                    start: page_end,
                    count: after,
                };
            }
            (before, 0) => self.free[index].count = before,
            (before, after) => {
                if self.free_len >= R {
                    return Err(CoreError::new(
                        CoreErrorKind::CapacityExceeded,
                        "physical allocator cannot split a free run at the requested page",
                    ));
                }
                let mut cursor = self.free_len;
                while cursor > index + 1 {
                    self.free[cursor] = self.free[cursor - 1];
                    cursor -= 1;
                }
                self.free[index].count = before;
                self.free[index + 1] = PageRun {
                    start: page_end,
                    count: after,
                };
                self.free_len += 1;
            }
        }
        Ok(())
    }

    fn insert_free_run(&mut self, run: PageRun) -> Result<(), CoreError> {
        if run.count == 0 {
            return Ok(());
        }
        let run_end = run.end()?;
        let mut index = 0;
        while index < self.free_len && self.free[index].start < run.start {
            index += 1;
        }

        let merge_left = if index > 0 {
            let previous_end = self.free[index - 1].end()?;
            if previous_end > run.start {
                return Err(CoreError::new(
                    CoreErrorKind::Overlap,
                    "physical allocator free list overlaps",
                ));
            }
            previous_end == run.start
        } else {
            false
        };
        let merge_right = if index < self.free_len {
            if run_end > self.free[index].start {
                return Err(CoreError::new(
                    CoreErrorKind::Overlap,
                    "physical allocator free list overlaps",
                ));
            }
            run_end == self.free[index].start
        } else {
            false
        };

        let merged_count = |left: u64, right: u64| {
            left.checked_add(right).ok_or(CoreError::new(
                CoreErrorKind::InvalidAddress,
                "physical allocator page-run count overflowed",
            ))
        };
        match (merge_left, merge_right) {
            (true, true) => {
                let count = merged_count(self.free[index - 1].count, run.count)?;
                let count = merged_count(count, self.free[index].count)?;
                self.free[index - 1].count = count;
                remove_run(&mut self.free, &mut self.free_len, index);
            }
            (true, false) => {
                self.free[index - 1].count = merged_count(self.free[index - 1].count, run.count)?;
            }
            (false, true) => {
                self.free[index] = PageRun {
                    start: run.start,
                    count: merged_count(run.count, self.free[index].count)?,
                };
            }
            (false, false) => {
                if self.free_len >= R {
                    return Err(CoreError::new(
                        CoreErrorKind::CapacityExceeded,
                        "physical allocator free-run table is full",
                    ));
                }
                let mut cursor = self.free_len;
                while cursor > index {
                    self.free[cursor] = self.free[cursor - 1];
                    cursor -= 1;
                }
                self.free[index] = run;
                self.free_len += 1;
            }
        }
        Ok(())
    }
}

fn align_up(value: u64, align: u64) -> Result<u64, CoreError> {
    let mask = align - 1;
    value
        .checked_add(mask)
        .map(|v| v & !mask)
        .ok_or(CoreError::new(
            CoreErrorKind::InvalidAddress,
            "address alignment overflowed",
        ))
}

const fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

fn remove_run<const R: usize>(runs: &mut [PageRun; R], len: &mut usize, index: usize) {
    let mut cursor = index;
    while cursor + 1 < *len {
        runs[cursor] = runs[cursor + 1];
        cursor += 1;
    }
    *len -= 1;
    runs[*len] = PageRun::empty();
}

fn remove_allocated<const A: usize>(pages: &mut [u64; A], len: &mut usize, index: usize) {
    let mut cursor = index;
    while cursor + 1 < *len {
        pages[cursor] = pages[cursor + 1];
        cursor += 1;
    }
    *len -= 1;
    pages[*len] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryRegion, MemoryRegionKind};

    struct CountingZeroer {
        pages: usize,
        fail: bool,
    }

    impl PageZeroer for CountingZeroer {
        fn zero_page(&mut self, _page: HostPhysical) -> Result<(), CoreError> {
            if self.fail {
                return Err(CoreError::new(
                    CoreErrorKind::ZeroingFailed,
                    "test zeroer refused the page",
                ));
            }
            self.pages += 1;
            Ok(())
        }
    }

    #[test]
    fn allocator_uses_usable_regions_and_skips_reserved_ranges() {
        let map = MemoryMap::<4>::from_entries(&[
            MemoryRegion::new(0x1000, 0x3000, MemoryRegionKind::Usable),
            MemoryRegion::new(0x4000, 0x1000, MemoryRegionKind::Reserved),
        ])
        .unwrap();
        let mut allocator = PhysicalPageAllocator::<4, 8>::from_memory_map(&map).unwrap();

        assert_eq!(allocator.free_pages(), 3);
        assert_eq!(allocator.allocate().unwrap().get(), 0x1000);
        assert_eq!(allocator.allocate().unwrap().get(), 0x2000);
        assert_eq!(allocator.allocate().unwrap().get(), 0x3000);
        assert_eq!(
            allocator.allocate().unwrap_err().kind,
            CoreErrorKind::OutOfMemory
        );
    }

    #[test]
    fn allocator_detects_double_free() {
        let map = MemoryMap::<2>::from_entries(&[MemoryRegion::new(
            0x1000,
            0x1000,
            MemoryRegionKind::Usable,
        )])
        .unwrap();
        let mut allocator = PhysicalPageAllocator::<2, 2>::from_memory_map(&map).unwrap();
        let page = allocator.allocate().unwrap();

        allocator.free(page).unwrap();
        assert_eq!(
            allocator.free(page).unwrap_err().kind,
            CoreErrorKind::DoubleFree
        );
    }

    #[test]
    fn zeroed_allocation_calls_zeroer_and_rolls_back_on_failure() {
        let map = MemoryMap::<2>::from_entries(&[MemoryRegion::new(
            0x1000,
            0x1000,
            MemoryRegionKind::Usable,
        )])
        .unwrap();
        let mut allocator = PhysicalPageAllocator::<2, 2>::from_memory_map(&map).unwrap();
        let mut zeroer = CountingZeroer {
            pages: 0,
            fail: false,
        };

        assert_eq!(
            allocator.allocate_zeroed(&mut zeroer).unwrap().get(),
            0x1000
        );
        assert_eq!(zeroer.pages, 1);

        let mut allocator = PhysicalPageAllocator::<2, 2>::from_memory_map(&map).unwrap();
        let mut failing = CountingZeroer {
            pages: 0,
            fail: true,
        };
        assert_eq!(
            allocator.allocate_zeroed(&mut failing).unwrap_err().kind,
            CoreErrorKind::ZeroingFailed
        );
        assert_eq!(allocator.free_pages(), 1);
    }

    #[test]
    fn constrained_allocation_skips_low_memory_and_splits_a_run() {
        let map = MemoryMap::<2>::from_entries(&[MemoryRegion::new(
            0,
            0x20_0000,
            MemoryRegionKind::Usable,
        )])
        .unwrap();
        let mut allocator = PhysicalPageAllocator::<2, 2>::from_memory_map(&map).unwrap();
        let constraint = PageAllocationConstraint::new(0x10_0000, 0x20_0000).unwrap();

        assert_eq!(
            allocator.allocate_within(constraint).unwrap().get(),
            0x10_0000
        );
        assert_eq!(allocator.free_pages(), 0x200 - 1);
    }

    #[test]
    fn reservation_removes_every_touched_page_and_splits_the_run() {
        let map = MemoryMap::<2>::from_entries(&[MemoryRegion::new(
            0x1000,
            0x5000,
            MemoryRegionKind::Usable,
        )])
        .unwrap();
        let mut allocator = PhysicalPageAllocator::<2, 4>::from_memory_map(&map).unwrap();

        assert_eq!(allocator.reserve_range(0x2800, 0x100).unwrap(), 1);
        assert_eq!(allocator.free_pages(), 4);
        assert_eq!(allocator.allocate().unwrap().get(), 0x1000);
        assert_eq!(allocator.allocate().unwrap().get(), 0x3000);
    }

    #[test]
    fn failed_reservation_split_keeps_the_free_list_intact() {
        let map = MemoryMap::<1>::from_entries(&[MemoryRegion::new(
            0x1000,
            0x5000,
            MemoryRegionKind::Usable,
        )])
        .unwrap();
        let mut allocator = PhysicalPageAllocator::<1, 2>::from_memory_map(&map).unwrap();

        assert_eq!(
            allocator.reserve_range(0x3000, 0x1000).unwrap_err().kind,
            CoreErrorKind::CapacityExceeded
        );
        assert_eq!(allocator.free_pages(), 5);
        assert_eq!(allocator.allocate().unwrap().get(), 0x1000);
    }

    #[test]
    fn reservation_rejects_overlap_with_an_allocated_page() {
        let map = MemoryMap::<1>::from_entries(&[MemoryRegion::new(
            0x1000,
            0x3000,
            MemoryRegionKind::Usable,
        )])
        .unwrap();
        let mut allocator = PhysicalPageAllocator::<2, 2>::from_memory_map(&map).unwrap();
        let page = allocator.allocate().unwrap();

        assert_eq!(
            allocator
                .reserve_range(page.get(), PAGE_SIZE_4K)
                .unwrap_err()
                .kind,
            CoreErrorKind::Overlap
        );
        assert_eq!(allocator.allocated_pages(), 1);
        assert_eq!(allocator.free_pages(), 2);
    }

    #[test]
    fn free_merges_with_a_neighbor_when_the_run_table_is_full() {
        let map = MemoryMap::<1>::from_entries(&[MemoryRegion::new(
            0x1000,
            0x3000,
            MemoryRegionKind::Usable,
        )])
        .unwrap();
        let mut allocator = PhysicalPageAllocator::<1, 1>::from_memory_map(&map).unwrap();
        let page = allocator.allocate().unwrap();

        allocator.free(page).unwrap();
        assert_eq!(allocator.allocated_pages(), 0);
        assert_eq!(allocator.free_pages(), 3);
    }

    #[test]
    fn failed_unmergeable_free_keeps_the_allocation_ledger_intact() {
        let map = MemoryMap::<2>::from_entries(&[
            MemoryRegion::new(0x1000, 0x1000, MemoryRegionKind::Usable),
            MemoryRegion::new(0x4000, 0x3000, MemoryRegionKind::Usable),
        ])
        .unwrap();
        let mut allocator = PhysicalPageAllocator::<2, 2>::from_memory_map(&map).unwrap();
        let isolated = allocator.allocate().unwrap();
        let middle = allocator
            .allocate_within(PageAllocationConstraint::new(0x5000, 0x6000).unwrap())
            .unwrap();

        assert_eq!(
            allocator.free(isolated).unwrap_err().kind,
            CoreErrorKind::CapacityExceeded
        );
        assert_eq!(allocator.allocated_pages(), 2);

        allocator.free(middle).unwrap();
        assert_eq!(allocator.allocated_pages(), 1);
    }
}
