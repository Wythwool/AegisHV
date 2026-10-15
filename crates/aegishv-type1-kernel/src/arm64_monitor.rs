use core::arch::asm;
use core::fmt::{self, Write};
use core::ptr::{addr_of_mut, read_volatile};

use aegishv_type1_kernel::events::{Access, Action, Backend, Events};
use aegishv_type1_kernel::inspection::Vectors;

use crate::kernel::Frame;

const UART: u64 = 0x0904_0000;
const BASE: u64 = 0x4000_0000;
const BACKING: u64 = 0x5000_0000;

struct Serial;
impl Write for Serial {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            let mut remaining = 1_000_000;
            unsafe {
                while ((UART + 0x18) as *const u32).read_volatile() & (1 << 5) != 0 {
                    remaining -= 1;
                    if remaining == 0 {
                        return Err(fmt::Error);
                    }
                    core::hint::spin_loop();
                }
                (UART as *mut u32).write_volatile(u32::from(byte));
            }
        }
        Ok(())
    }
}

// The Linux exit lock covers this state and the telemetry UART.
static mut EVENTS: Events<Serial> = Events::new(Serial, Backend::El2);
static mut PMU: [Option<u64>; 4] = [None; 4];
static mut VECTORS: [Vectors; 4] = [const { Vectors::new() }; 4];
static mut IMAGE: (u64, u64) = (0, 0);

pub unsafe fn init(start: u64, size: u64) {
    unsafe {
        IMAGE = (start, start + size);
        assert_eq!(((UART + 0xfe0) as *const u32).read_volatile() & 255, 0x11);
        for (offset, value) in [
            (0x30, 0),
            (0x38, 0),
            (0x44, 0x7ff),
            (0x24, 13),
            (0x28, 1),
            (0x2c, 0x70),
            (0x30, 0x101),
        ] {
            ((UART + offset) as *mut u32).write_volatile(value);
        }
        asm!("dsb sy", options(nostack));
    }
}

pub unsafe fn cpu(cpu: usize) {
    unsafe { (*addr_of_mut!(EVENTS)).cpu(cpu) };
}

pub unsafe fn start(cpu: usize) {
    let dfr: u64;
    unsafe {
        asm!("mrs {}, id_aa64dfr0_el1", out(reg) dfr, options(nomem, nostack));
        (*addr_of_mut!(VECTORS))[cpu] = Vectors::new();
        self::cpu(cpu);
    }
    let present = !matches!((dfr >> 8) & 15, 0 | 15);
    if present {
        let pmcr: u64;
        unsafe {
            asm!("mrs {}, pmcr_el0", out(reg) pmcr, options(nomem, nostack));
            // Guest PMU accesses trap; only EL0/EL1 cycles are counted.
            asm!("msr mdcr_el2, {traps}", "msr pmcntenclr_el0, {mask}",
                "msr pmintenclr_el1, {mask}", "msr pmovsclr_el0, {mask}",
                "msr pmuserenr_el0, xzr", "msr pmccfiltr_el0, xzr",
                "msr pmcr_el0, {control}", "msr pmcntenset_el0, {enable}", "isb",
                traps = in(reg) ((pmcr >> 11) & 31) | 0x60,
                mask = in(reg) 0xffff_ffffu64, control = in(reg) 0x47u64,
                enable = in(reg) 1u64 << 31, options(nostack));
            let cycles: u64;
            asm!("mrs {}, pmccntr_el0", out(reg) cycles, options(nomem, nostack));
            PMU[cpu] = Some(cycles);
        }
    } else {
        unsafe { PMU[cpu] = None };
    }
    unsafe { (*addr_of_mut!(EVENTS)).pmu_status(present).unwrap() };
}

pub unsafe fn page(pc: u64, address: u64, access: Access, action: Action) {
    unsafe {
        (*addr_of_mut!(EVENTS))
            .page(pc, address, access, action)
            .unwrap()
    };
}

pub unsafe fn sample(cpu: usize, pc: u64) {
    unsafe { self::cpu(cpu) };
    let events = unsafe { &mut *addr_of_mut!(EVENTS) };
    if let Some(previous) = unsafe { PMU[cpu] } {
        let cycles: u64;
        unsafe { asm!("isb", "mrs {}, pmccntr_el0", out(reg) cycles, options(nomem, nostack)) };
        events.pmu_cycles(cycles.wrapping_sub(previous)).unwrap();
        unsafe { PMU[cpu] = Some(cycles) };
    }
    let target: u64;
    unsafe { asm!("mrs {}, vbar_el1", out(reg) target, options(nomem, nostack)) };
    if target == 0 {
        return;
    }
    let address = unsafe { translate(target) }.unwrap_or(0);
    let (start, end) = unsafe { IMAGE };
    let valid = target & 2047 == 0 && address >= start && address + 2048 <= end;
    let changed = if valid {
        let mut bytes = [0u8; 2048];
        for (offset, byte) in bytes.iter_mut().enumerate() {
            *byte =
                unsafe { read_volatile((address + BACKING - BASE + offset as u64) as *const u8) };
        }
        unsafe { (*addr_of_mut!(VECTORS))[cpu].sample(target, address, &bytes) }
    } else {
        false
    };
    events.vectors(pc, target, address, valid, changed).unwrap();
}

pub unsafe fn translate(address: u64) -> Result<u64, u64> {
    let result: u64;
    unsafe {
        asm!("mrs {saved}, par_el1", "at s1e1r, {address}", "isb",
            "mrs {result}, par_el1", "msr par_el1, {saved}",
            saved = out(reg) _, result = out(reg) result, address = in(reg) address,
            options(nostack));
    }
    if result & 1 != 0 {
        Err(result)
    } else {
        Ok((result & 0x000f_ffff_ffff_f000) | (address & 4095))
    }
}

pub fn pmu_access(frame: &mut Frame, esr: u64) -> bool {
    let op0 = (esr >> 20) & 3;
    let op1 = (esr >> 14) & 7;
    let crn = (esr >> 10) & 15;
    let crm = (esr >> 1) & 15;
    if op0 != 3
        || !((op1 == 3 && crn == 9 && (12..=14).contains(&crm))
            || (op1 == 0 && crn == 9 && crm == 14)
            || (op1 == 3 && crn == 14 && crm >= 8))
    {
        return false;
    }
    let rt = ((esr >> 5) & 31) as usize;
    if esr & 1 != 0 && rt != 31 {
        frame.registers[rt] = 0;
    }
    frame.pc += 4;
    true
}
