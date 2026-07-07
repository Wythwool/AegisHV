use crate::error::{CoreError, CoreErrorKind};
use crate::ids::HostPhysical;
use crate::memory::MemoryMap;

pub const PAGE_SIZE_4K: u64 = 4096;

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
    pub fn from_memory_map(map: &MemoryMap<R>) -> Result<Self, CoreError> {
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

    pub fn allocate(&mut self) -> Result<HostPhysical, CoreError> {
        if self.allocated_len >= A {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "physical allocator allocation tracking table is full",
            ));
        }
        if self.free_len == 0 {
            return Err(CoreError::new(
                CoreErrorKind::OutOfMemory,
                "physical allocator has no free 4K pages",
            ));
        }

        let page = self.free[0].start;
        self.free[0].start += PAGE_SIZE_4K;
        self.free[0].count -= 1;
        if self.free[0].count == 0 {
            remove_run(&mut self.free, &mut self.free_len, 0);
        }

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
            let _ = self.free(page);
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

        remove_allocated(&mut self.allocated, &mut self.allocated_len, index);
        self.insert_free_run(PageRun {
            start: page.get(),
            count: 1,
        })
    }

    fn insert_free_run(&mut self, run: PageRun) -> Result<(), CoreError> {
        if run.count == 0 {
            return Ok(());
        }
        if self.free_len >= R {
            return Err(CoreError::new(
                CoreErrorKind::CapacityExceeded,
                "physical allocator free-run table is full",
            ));
        }

        let mut index = self.free_len;
        while index > 0 && self.free[index - 1].start > run.start {
            self.free[index] = self.free[index - 1];
            index -= 1;
        }
        self.free[index] = run;
        self.free_len += 1;
        self.merge_free_runs()
    }

    fn merge_free_runs(&mut self) -> Result<(), CoreError> {
        let mut index = 0;
        while index + 1 < self.free_len {
            let current = self.free[index];
            let next = self.free[index + 1];
            let current_end = current.end()?;
            if current_end > next.start {
                return Err(CoreError::new(
                    CoreErrorKind::Overlap,
                    "physical allocator free list overlaps",
                ));
            }
            if current_end == next.start {
                self.free[index].count += next.count;
                remove_run(&mut self.free, &mut self.free_len, index + 1);
            } else {
                index += 1;
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
}
