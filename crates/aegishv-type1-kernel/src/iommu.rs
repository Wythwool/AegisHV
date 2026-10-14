use core::arch::asm;
use core::fmt::Write;
use core::ptr::{addr_of, addr_of_mut, NonNull};

use aegishv_arch_x86::vtd::Unit;
use aegishv_type1_boot::LimineMemmapEntry;
use aegishv_type1_kernel::LimineMinimalHandoff;

#[repr(C, align(4096))]
struct Page([u64; 512]);
static mut ROOT: Page = Page([0; 512]);
static mut MMIO: [Page; 3] = [const { Page([0; 512]) }; 3];
static mut MMIO_COUNT: usize = 0;
static mut REGISTER_BASE: u64 = 0;
const WINDOW: u64 = 0xffff_8000_0000_0000;

fn fail(message: &str) -> ! {
    writeln!(crate::Serial, "aegishv:iommu:error {message}").unwrap();
    crate::halt_loop()
}

fn physical(handoff: LimineMinimalHandoff, address: u64) -> u64 {
    crate::linked_host_physical_address(handoff, address)
        .unwrap_or_else(|_| fail("physical address"))
}

unsafe fn acpi_bytes(
    handoff: LimineMinimalHandoff,
    entries: &[LimineMemmapEntry],
    address: u64,
    size: usize,
) -> &'static [u8] {
    let end = address
        .checked_add(size as u64)
        .unwrap_or_else(|| fail("ACPI range overflow"));
    if size == 0
        || size > 65536
        || !entries.iter().any(|entry| {
            matches!(entry.kind, 2 | 3 | 5 | 8)
                && address >= entry.base
                && entry
                    .base
                    .checked_add(entry.length)
                    .is_some_and(|limit| end <= limit)
        })
    {
        fail("ACPI range is not mapped");
    }
    let virtual_address = handoff
        .hhdm_offset
        .checked_add(address)
        .unwrap_or_else(|| fail("ACPI address"));
    unsafe { core::slice::from_raw_parts(virtual_address as *const u8, size) }
}

unsafe fn table(
    handoff: LimineMinimalHandoff,
    entries: &[LimineMemmapEntry],
    address: u64,
) -> &'static [u8] {
    let header = unsafe { acpi_bytes(handoff, entries, address, 36) };
    let size = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
    if size < 36 {
        fail("short ACPI table");
    }
    let bytes = unsafe { acpi_bytes(handoff, entries, address, size) };
    if bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0 {
        fail("ACPI checksum");
    }
    bytes
}

pub unsafe fn discover(handoff: LimineMinimalHandoff) {
    let response = unsafe { addr_of!(crate::LIMINE_RSDP_REQUEST.response).read_volatile() };
    if response == 0 {
        return;
    }
    crate::validate_limine_object_range(response, 16, 8).unwrap_or_else(|_| fail("RSDP response"));
    let pointer = unsafe { ((response + 8) as *const u64).read_volatile() };
    let address = pointer
        .checked_sub(handoff.hhdm_offset)
        .unwrap_or_else(|| fail("RSDP address"));
    let (entries, count) =
        crate::copy_limine_memory_entries(handoff).unwrap_or_else(|_| fail("memory map"));
    let entries = &entries[..count];
    let rsdp = unsafe { acpi_bytes(handoff, entries, address, 20) };
    if &rsdp[..8] != b"RSD PTR " || rsdp.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0
    {
        fail("RSDP checksum");
    }
    let (root, stride) = if rsdp[15] >= 2 {
        let rsdp = unsafe { acpi_bytes(handoff, entries, address, 36) };
        let size = u32::from_le_bytes(rsdp[20..24].try_into().unwrap()) as usize;
        if size < 36 {
            fail("RSDP length");
        }
        let extended = unsafe { acpi_bytes(handoff, entries, address, size) };
        if extended
            .iter()
            .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
            != 0
        {
            fail("extended RSDP checksum");
        }
        (u64::from_le_bytes(rsdp[24..32].try_into().unwrap()), 8)
    } else {
        (
            u32::from_le_bytes(rsdp[16..20].try_into().unwrap()) as u64,
            4,
        )
    };
    let root = unsafe { table(handoff, entries, root) };
    if &root[..4] != (if stride == 8 { b"XSDT" } else { b"RSDT" })
        || (root.len() - 36) % stride != 0
    {
        fail("ACPI root");
    }
    for slot in root[36..].chunks_exact(stride) {
        let address = if stride == 8 {
            u64::from_le_bytes(slot.try_into().unwrap())
        } else {
            u32::from_le_bytes(slot.try_into().unwrap()) as u64
        };
        let header = unsafe { acpi_bytes(handoff, entries, address, 36) };
        if &header[..4] != b"DMAR" {
            continue;
        }
        let bytes = unsafe { table(handoff, entries, address) };
        let dmar =
            aegishv_arch_x86::acpi::parse_dmar::<4>(bytes).unwrap_or_else(|_| fail("DMAR table"));
        if dmar.units().len() != 1 || dmar.units()[0].segment != 0 {
            fail("requires one VT-d unit in segment 0");
        }
        let include_all = dmar.units()[0].flags & 1 != 0;
        let mut covered = [false; 256];
        let mut offset = 48;
        while offset < bytes.len() {
            let length =
                u16::from_le_bytes(bytes[offset + 2..offset + 4].try_into().unwrap()) as usize;
            if bytes[offset..offset + 2] == [1, 0] {
                fail("RMRR needs device-specific handling");
            }
            if bytes[offset..offset + 2] == [0, 0] {
                let mut scope = offset + 16;
                while scope < offset + length {
                    if scope + 8 > offset + length || bytes[scope + 1] != 8 {
                        fail("unsupported DMAR device path");
                    }
                    match bytes[scope] {
                        1 if bytes[scope + 5] == 0
                            && bytes[scope + 6] < 32
                            && bytes[scope + 7] < 8 =>
                        {
                            covered[usize::from(bytes[scope + 6]) * 8
                                + usize::from(bytes[scope + 7])] = true;
                        }
                        3 | 4 => (),
                        _ => fail("unsupported DMAR scope"),
                    }
                    scope += 8;
                }
            }
            offset += length;
        }
        if !include_all {
            for (device, covered) in covered.into_iter().enumerate() {
                if unsafe { pci_read(device as u16, 0) } & 0xffff != 0xffff && !covered {
                    fail("PCI device outside DMAR scopes");
                }
            }
        }
        let base = dmar.units()[0].register_base;
        if base == 0 || base & 4095 != 0 || base >> 52 != 0 || unsafe { REGISTER_BASE != 0 } {
            fail("VT-d register range");
        }
        unsafe {
            REGISTER_BASE = base;
        }
        writeln!(crate::Serial, "aegishv:iommu:dmar base={base:#x}").unwrap();
    }
}

unsafe fn pci_read(device: u16, register: u32) -> u32 {
    let value: u32;
    unsafe {
        asm!("out dx, eax", in("dx") 0xcf8u16, in("eax") 0x8000_0000 | (u32::from(device) << 8) | register, options(nostack));
        asm!("in eax, dx", in("dx") 0xcfcu16, out("eax") value, options(nostack));
    }
    value
}

unsafe fn map_mmio(handoff: LimineMinimalHandoff, address: u64) -> NonNull<u8> {
    assert_eq!(address & 4095, 0);
    assert!(address >> 52 == 0);
    // PAT entry 3 must still be UC when selecting PCD|PWT.
    assert_eq!((unsafe { crate::read_msr(0x277) } >> 24) & 255, 0);
    let tables = unsafe { &mut *addr_of_mut!(MMIO) };
    let count = unsafe { MMIO_COUNT };
    assert!(count < 512);
    if count == 0 {
        tables[0].0[0] = physical(handoff, addr_of!(tables[1]) as u64) | 3;
        tables[1].0[0] = physical(handoff, addr_of!(tables[2]) as u64) | 3;
        let root = addr_of_mut!(crate::__aegishv_host_page_tables_start).cast::<u64>();
        unsafe {
            assert_eq!(root.add(256).read_volatile(), 0);
            root.add(256)
                .write_volatile(physical(handoff, addr_of!(MMIO) as u64) | 3);
        }
    }
    tables[2].0[count] = address | 0x1b | (1 << 63);
    let mapped = WINDOW + count as u64 * 4096;
    unsafe {
        MMIO_COUNT += 1;
        asm!("mfence", "invlpg [{}]", in(reg) mapped, options(nostack));
    }
    NonNull::new(mapped as *mut u8).unwrap()
}

unsafe fn publish(address: *const u8, bytes: usize) {
    let features = unsafe { core::arch::x86_64::__cpuid_count(1, 0) };
    assert!(features.edx & (1 << 19) != 0 && ((features.ebx >> 8) & 255) * 8 == 64);
    for offset in (0..bytes).step_by(64) {
        unsafe {
            asm!("clflush [{}]", in(reg) address.add(offset), options(nostack));
        }
    }
    unsafe {
        asm!("mfence", options(nostack));
    }
}

pub unsafe fn activate(handoff: LimineMinimalHandoff) {
    let base = unsafe { REGISTER_BASE };
    if base == 0 {
        crate::serial_write_line("aegishv:iommu:unavailable");
        #[cfg(iommu_test)]
        fail("DMA test requires VT-d");
        #[cfg(not(iommu_test))]
        return;
    }
    let mut unit = unsafe { Unit::new(map_mmio(handoff, base)) }.unwrap_or_else(|error| {
        writeln!(crate::Serial, "aegishv:iommu:register-error {error:?}").unwrap();
        fail("VT-d capabilities")
    });
    unsafe {
        publish(addr_of!(ROOT).cast(), 4096);
        unit.enable(physical(handoff, addr_of!(ROOT) as u64))
    }
    .unwrap_or_else(|error| {
        writeln!(crate::Serial, "aegishv:iommu:enable-error {error:?}").unwrap();
        fail("VT-d enable")
    });
    crate::serial_write_line("aegishv:iommu:quarantine-on");
    #[cfg(iommu_test)]
    unsafe {
        dma_test(handoff, &mut unit);
    }
}

#[cfg(iommu_test)]
#[path = "iommu_test.rs"]
mod test;
#[cfg(iommu_test)]
use test::dma_test;
