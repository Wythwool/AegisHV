use core::ptr::NonNull;
use core::sync::atomic::{fence, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Address,
    Unsupported,
    Busy,
    Timeout,
    Invalidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fault {
    pub address: u64,
    pub source: u16,
    pub reason: u8,
    pub read: bool,
}

pub struct Unit {
    registers: NonNull<u8>,
    cap: u64,
    iotlb: usize,
    faults: usize,
    fault_count: usize,
    active: bool,
}

impl Unit {
    /// # Safety
    /// `registers` must be an exclusively owned, uncached mapping of a complete
    /// 4 KiB VT-d register page. It must outlive this object.
    pub unsafe fn new(registers: NonNull<u8>) -> Result<Self, Error> {
        if registers.as_ptr() as usize & 4095 != 0 {
            return Err(Error::Address);
        }
        let mut unit = Self {
            registers,
            cap: 0,
            iotlb: 0,
            faults: 0,
            fault_count: 0,
            active: false,
        };
        unit.cap = unit.read64(8);
        let ecap = unit.read64(0x10);
        unit.iotlb = ((ecap >> 8) & 0x3ff) as usize * 16 + 8;
        unit.faults = ((unit.cap >> 24) & 0x3ff) as usize * 16;
        unit.fault_count = ((unit.cap >> 40) & 255) as usize + 1;
        if unit.cap & (1 << 9) == 0
            || unit.iotlb < 0x80
            || unit.iotlb + 8 > 4096
            || unit.faults < 0x80
            || unit.faults + unit.fault_count * 16 > 4096
        {
            return Err(Error::Unsupported);
        }
        // Limine must leave translation, queued invalidation and interrupt remapping off.
        if unit.read32(0x1c) & ((1 << 31) | (1 << 28) | (1 << 26) | (1 << 25)) != 0 {
            return Err(Error::Busy);
        }
        unit.write32(0x38, 1 << 31);
        Ok(unit)
    }

    /// # Safety
    /// `root` is a pinned, zeroed legacy root table. Publish it to memory before
    /// enabling translation. Later updates require cache writeback and invalidate().
    pub unsafe fn enable(&mut self, root: u64) -> Result<(), Error> {
        let width = ((self.cap >> 16) & 63) + 1;
        if root == 0 || root & 4095 != 0 || root >> 52 != 0 || (width < 64 && root >> width != 0) {
            return Err(Error::Address);
        }
        if self.active {
            return Err(Error::Busy);
        }
        self.write64(0x20, root);
        self.write32(0x18, 1 << 30);
        self.wait32(0x1c, 1 << 30, 1 << 30)?;
        self.invalidate()?;
        self.write32(0x18, 1 << 31);
        self.wait32(0x1c, 1 << 31, 1 << 31)?;
        self.active = true;
        Ok(())
    }

    pub fn invalidate(&mut self) -> Result<(), Error> {
        fence(Ordering::SeqCst);
        self.write64(0x28, (1 << 63) | (1 << 61));
        self.wait64(0x28, 1 << 63)?;
        if self.read64(0x28) & (3 << 59) != 1 << 59 {
            return Err(Error::Invalidation);
        }
        let drain_write = if self.cap & (1 << 54) != 0 {
            1 << 48
        } else {
            0
        };
        let drain_read = if self.cap & (1 << 55) != 0 {
            1 << 49
        } else {
            0
        };
        self.write64(self.iotlb, (1 << 63) | (1 << 60) | drain_write | drain_read);
        self.wait64(self.iotlb, 1 << 63)?;
        if self.read64(self.iotlb) & (3 << 57) != 1 << 57 {
            return Err(Error::Invalidation);
        }
        if self.cap & (1 << 4) != 0 {
            self.write32(0x18, (u32::from(self.active) << 31) | (1 << 27));
            self.wait32(0x1c, 1 << 27, 0)?;
        }
        fence(Ordering::SeqCst);
        Ok(())
    }

    pub fn take_fault(&mut self) -> Option<Fault> {
        for index in 0..self.fault_count {
            let offset = self.faults + index * 16;
            let high = self.read64(offset + 8);
            if high & (1 << 63) == 0 {
                continue;
            }
            let fault = Fault {
                address: self.read64(offset) & !4095,
                source: high as u16,
                reason: (high >> 32) as u8,
                read: high & (1 << 62) != 0,
            };
            self.write64(offset + 8, 1 << 63);
            self.write32(0x34, 1);
            return Some(fault);
        }
        None
    }

    fn read32(&self, offset: usize) -> u32 {
        unsafe {
            self.registers
                .as_ptr()
                .add(offset)
                .cast::<u32>()
                .read_volatile()
        }
    }
    fn read64(&self, offset: usize) -> u64 {
        unsafe {
            self.registers
                .as_ptr()
                .add(offset)
                .cast::<u64>()
                .read_volatile()
        }
    }
    fn write32(&mut self, offset: usize, value: u32) {
        unsafe {
            self.registers
                .as_ptr()
                .add(offset)
                .cast::<u32>()
                .write_volatile(value);
        }
    }
    fn write64(&mut self, offset: usize, value: u64) {
        unsafe {
            self.registers
                .as_ptr()
                .add(offset)
                .cast::<u64>()
                .write_volatile(value);
        }
    }
    fn wait32(&self, offset: usize, mask: u32, value: u32) -> Result<(), Error> {
        for _ in 0..1_000_000 {
            if self.read32(offset) & mask == value {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(Error::Timeout)
    }
    fn wait64(&self, offset: usize, mask: u64) -> Result<(), Error> {
        for _ in 0..1_000_000 {
            if self.read64(offset) & mask == 0 {
                return Ok(());
            }
            core::hint::spin_loop();
        }
        Err(Error::Timeout)
    }
}
