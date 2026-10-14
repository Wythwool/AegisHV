use super::*;

static mut TABLES: [Page; 4] = [const { Page([0; 512]) }; 4];
static mut BUFFER: [Page; 2] = [const { Page([0xa5a5_a5a5_a5a5_a5a5; 512]) }; 2];

unsafe fn pci_write(device: u16, register: u32, value: u32) {
    unsafe {
        asm!("out dx, eax", in("dx") 0xcf8u16, in("eax") 0x8000_0000 | (u32::from(device) << 8) | register, options(nostack));
        asm!("out dx, eax", in("dx") 0xcfcu16, in("eax") value, options(nostack));
    }
}

unsafe fn transfer(device: NonNull<u8>, from: u64, to: u64, size: u64, write: bool) {
    unsafe {
        device.as_ptr().add(0x80).cast::<u64>().write_volatile(from);
        device.as_ptr().add(0x88).cast::<u64>().write_volatile(to);
        device.as_ptr().add(0x90).cast::<u64>().write_volatile(size);
        asm!("mfence", options(nostack));
        device
            .as_ptr()
            .add(0x98)
            .cast::<u64>()
            .write_volatile(1 | (u64::from(write) << 1));
    }
    for _ in 0..10_000_000 {
        if unsafe { device.as_ptr().add(0x98).cast::<u64>().read_volatile() } & 1 == 0 {
            unsafe {
                asm!("mfence", options(nostack));
            }
            return;
        }
        core::hint::spin_loop();
    }
    fail("EDU DMA timeout");
}

fn fault(unit: &mut Unit, source: u16, address: u64, reason: u8, read: bool) {
    let fault = unit
        .take_fault()
        .unwrap_or_else(|| fail("missing DMA fault"));
    writeln!(
        crate::Serial,
        "aegishv:iommu:fault source={:#x} address={:#x} reason={} read={}",
        fault.source,
        fault.address,
        fault.reason,
        fault.read
    )
    .unwrap();
    assert_eq!(fault.source, source);
    assert_eq!(fault.address, address & !4095);
    assert_eq!(fault.reason, reason);
    assert_eq!(fault.read, read);
    assert!(unit.take_fault().is_none());
}

unsafe fn fill(value: u64) {
    for index in 0..512 {
        unsafe {
            addr_of_mut!(BUFFER)
                .cast::<u64>()
                .add(index)
                .write_volatile(value);
        }
    }
    unsafe {
        publish(addr_of!(BUFFER).cast(), 4096);
    }
}

unsafe fn unchanged(page: usize, value: u64) {
    for index in 0..512 {
        assert_eq!(
            unsafe {
                addr_of!(BUFFER)
                    .cast::<u64>()
                    .add(page * 512 + index)
                    .read_volatile()
            },
            value
        );
    }
}

pub(super) unsafe fn dma_test(handoff: LimineMinimalHandoff, unit: &mut Unit) {
    let source = (0..256)
        .find(|device| unsafe { pci_read(*device, 0) } == 0x11e8_1234)
        .unwrap_or_else(|| fail("no EDU device on bus 0"));
    let bar = unsafe { pci_read(source, 0x10) };
    assert!(bar & 15 == 0 && bar & !15 != 0);
    let command = unsafe { pci_read(source, 4) } & 0xffff;
    unsafe {
        pci_write(source, 4, (command | 0x402) & !4);
    }
    let device = unsafe { map_mmio(handoff, u64::from(bar & !15)) };
    assert_eq!(
        unsafe { device.as_ptr().cast::<u32>().read_volatile() },
        0x0100_00ed
    );
    let buffer = physical(handoff, addr_of!(BUFFER) as u64);
    assert!(buffer + 8192 < 1 << 28);
    unsafe {
        publish(addr_of!(BUFFER).cast(), 8192);
        pci_write(source, 4, command | 0x406);
        transfer(device, 0x40000, buffer, 4096, true);
        unchanged(0, 0xa5a5_a5a5_a5a5_a5a5);
    }
    fault(unit, source, buffer, 1, false);
    crate::serial_write_line("aegishv:iommu:blocked-before-map");

    let tables = unsafe { &mut *addr_of_mut!(TABLES) };
    tables[0].0[usize::from(source) * 2] = physical(handoff, addr_of!(tables[1]) as u64) | 1;
    tables[0].0[usize::from(source) * 2 + 1] = (1 << 8) | 1;
    tables[1].0[0] = physical(handoff, addr_of!(tables[2]) as u64) | 3;
    tables[2].0[0] = physical(handoff, addr_of!(tables[3]) as u64) | 3;
    tables[3].0[1] = buffer | 3;
    unsafe {
        publish(addr_of!(TABLES).cast(), 4 * 4096);
        (*addr_of_mut!(ROOT)).0[0] = physical(handoff, addr_of!(TABLES) as u64) | 1;
        publish(addr_of!(ROOT).cast(), 4096);
    }
    unit.invalidate().unwrap();
    for index in 0..512 {
        unsafe {
            addr_of_mut!(BUFFER)
                .cast::<u64>()
                .add(index)
                .write_volatile(0x5a00_0000 | index as u64);
        }
    }
    unsafe {
        publish(addr_of!(BUFFER).cast(), 4096);
        transfer(device, 0x1000, 0x40000, 4096, false);
        fill(0);
        transfer(device, 0x40000, 0x1000, 4096, true);
    }
    assert!(unit.take_fault().is_none());
    for index in 0..512 {
        assert_eq!(
            unsafe { addr_of!(BUFFER).cast::<u64>().add(index).read_volatile() },
            0x5a00_0000 | index as u64
        );
    }
    crate::serial_write_line("aegishv:iommu:page-roundtrip-ok");
    unsafe {
        transfer(device, 0x40000, 0x2000, 4096, true);
        unchanged(1, 0xa5a5_a5a5_a5a5_a5a5);
    }
    fault(unit, source, 0x2000, 5, false);
    crate::serial_write_line("aegishv:iommu:adjacent-page-blocked");

    tables[3].0[1] = buffer | 1;
    unsafe {
        publish(addr_of!(tables[3]).cast(), 4096);
        fill(0xabab_abab_abab_abab);
    }
    unit.invalidate().unwrap();
    unsafe {
        transfer(device, 0x40000, 0x1000, 4096, true);
        unchanged(0, 0xabab_abab_abab_abab);
    }
    fault(unit, source, 0x1000, 5, false);
    unsafe {
        transfer(device, 0x1000, 0x40000, 4096, false);
    }
    assert!(unit.take_fault().is_none());
    crate::serial_write_line("aegishv:iommu:read-only-ok");

    tables[3].0[1] = 0;
    unsafe {
        publish(addr_of!(tables[3]).cast(), 4096);
    }
    unit.invalidate().unwrap();
    unsafe {
        transfer(device, 0x40000, 0x1000, 4096, true);
        unchanged(0, 0xabab_abab_abab_abab);
    }
    fault(unit, source, 0x1000, 5, false);
    unsafe {
        transfer(device, 0x1000, 0x40000, 4096, false);
    }
    fault(unit, source, 0x1000, 6, true);
    unsafe {
        (*addr_of_mut!(ROOT)).0[0] = 0;
        publish(addr_of!(ROOT).cast(), 4096);
        pci_write(source, 4, (command | 0x400) & !4);
    }
    unit.invalidate().unwrap();
    crate::serial_write_line("aegishv:iommu:revoked-ok");
    crate::serial_write_line("aegishv:iommu:dma-test-ok");
}
