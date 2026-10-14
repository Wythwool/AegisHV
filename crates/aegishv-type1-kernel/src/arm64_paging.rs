use core::arch::asm;
use core::ptr::{addr_of, addr_of_mut};

#[repr(C, align(4096))]
struct Table([u64; 512]);

static mut TABLES: [Table; 6] = [const { Table([0; 512]) }; 6];

extern "C" {
    static __image_start: u8;
    static __image_end: u8;
    static __text_end: u8;
    static __rodata_end: u8;
    static __stack_guard: u8;
}

const NX: u64 = 1 << 54;
const AF: u64 = 1 << 10;
const RO: u64 = 1 << 7;

unsafe fn translate(address: u64, write: bool) -> u64 {
    let result;
    unsafe {
        if write {
            asm!("at s1e2w, {}", in(reg) address, options(nostack));
        } else {
            asm!("at s1e2r, {}", in(reg) address, options(nostack));
        }
        asm!("isb", "mrs {}, par_el1", out(reg) result, options(nostack));
    }
    result
}

/// # Safety
/// Called once on the boot CPU with IRQs masked and EL2 stage 1 disabled.
pub unsafe fn enable() {
    let tables = unsafe { &mut *addr_of_mut!(TABLES) };
    let start = addr_of!(__image_start) as u64;
    let end = addr_of!(__image_end) as u64;
    let text_end = addr_of!(__text_end) as u64;
    let rodata_end = addr_of!(__rodata_end) as u64;
    let guard = addr_of!(__stack_guard) as u64;
    assert!(start == 0x4008_0000 && end <= 0x4020_0000);
    assert!(start < text_end && text_end <= rodata_end && rodata_end < guard && guard < end);
    tables[0].0[1] = addr_of!(tables[1]) as u64 | 3;
    tables[1].0[0] = addr_of!(tables[2]) as u64 | 3;
    #[cfg(linux_guest)]
    for index in 0..128 {
        tables[1].0[128 + index] =
            (0x5000_0000 + index as u64 * 0x20_0000) | 1 | AF | (3 << 8) | NX;
    }
    for address in (start..end).step_by(4096) {
        if address >= guard && (address - guard) % 0x11000 == 0 {
            continue;
        }
        let permissions = if address < text_end {
            RO
        } else if address < rodata_end {
            RO | NX
        } else {
            NX
        };
        tables[2].0[((address >> 12) & 511) as usize] = address | 3 | AF | (3 << 8) | permissions;
    }

    // Only the GIC distributor/CPU interface and PL011 are mapped as device memory.
    tables[0].0[0] = addr_of!(tables[3]) as u64 | 3;
    tables[3].0[0x40] = addr_of!(tables[4]) as u64 | 3;
    tables[3].0[0x48] = addr_of!(tables[5]) as u64 | 3;
    let device = 3 | (1 << 2) | AF | NX;
    tables[4].0[0] = 0x0800_0000 | device;
    tables[4].0[16] = 0x0801_0000 | device;
    tables[5].0[0] = 0x0900_0000 | device;

    unsafe {
        activate();
    }
}

pub unsafe fn activate() {
    let root = addr_of!(TABLES) as u64;
    let start = addr_of!(__image_start) as u64;
    let rodata_end = addr_of!(__rodata_end) as u64;
    let guard = addr_of!(__stack_guard) as u64;

    let control: u64;
    // 39-bit VA, 36-bit PA, 4K granule, inner-shareable WB walks, EL2 RES1 bits.
    let tcr = (1u64 << 31) | (1 << 23) | (1 << 16) | (3 << 12) | (1 << 10) | (1 << 8) | 25;
    unsafe {
        asm!("mrs {}, sctlr_el2", out(reg) control, options(nomem, nostack));
        assert!(control & 1 == 0);
        asm!("dsb sy", "msr mair_el2, {mair}", "msr tcr_el2, {tcr}", "msr ttbr0_el2, {root}",
            "isb", "tlbi alle2", "dsb sy", "isb", "msr sctlr_el2, {control}", "isb",
            mair = in(reg) 0x04ffu64, tcr = in(reg) tcr, root = in(reg) root,
            control = in(reg) control | (1 << 19) | (1 << 12) | (1 << 2) | 1, options(nostack));
        assert!(translate(start, false) & 1 == 0);
        assert!(translate(start, true) & 1 != 0);
        assert!(translate(rodata_end, true) & 1 == 0);
        assert!(translate(guard, false) & 1 != 0);
        assert!(translate(0, false) & 1 != 0);
    }
}
