use core::arch::asm;
use core::fmt::Write;
use core::ptr::{addr_of, addr_of_mut};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use crate::kernel::{aegishv_el2_enter, start_timer, stop, timer_reload, Frame, Serial};

const BASE: u64 = 0x4000_0000;
const BACKING: u64 = 0x5000_0000;
const SIZE: u64 = 0x1000_0000;
const NX: u64 = 1 << 54;
const HCR: u64 = (1 << 31) | (1 << 19) | (1 << 14) | (1 << 13) | 0x39;

#[repr(C, align(4096))]
struct Table([u64; 512]);
static mut TABLES: [Table; 132] = [const { Table([0; 512]) }; 132];
static mut TICKS: [u64; 4] = [0; 4];
static CPUS: AtomicUsize = AtomicUsize::new(1);
static DEADLINE: AtomicU64 = AtomicU64::new(0);
static STATES: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
static ENTRIES: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
static CONTEXTS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
static DISTRIBUTOR: AtomicBool = AtomicBool::new(false);
static LOCK: AtomicBool = AtomicBool::new(false);

struct Guard;
impl Guard {
    fn lock() -> Self {
        while LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        LOCK.store(false, Ordering::Release);
    }
}

fn cpu_id() -> usize {
    let value: u64;
    unsafe {
        asm!("mrs {}, mpidr_el1", out(reg) value, options(nomem, nostack));
    }
    (value & 255) as usize
}

static mut TRANSITIONS: [u64; 2] = [0; 2];
static mut GICS: [Gic; 4] = [const {
    Gic {
        interface: false,
        mask: 0,
        enabled: 0,
        pending: 0,
        active: 0,
        priority: [0xa0; 64],
        targets: [1; 64],
    }
}; 4];

struct Gic {
    interface: bool,
    mask: u32,
    enabled: u64,
    pending: u64,
    active: u64,
    priority: [u8; 64],
    targets: [u8; 64],
}

impl Gic {
    fn next(&self) -> u32 {
        if !DISTRIBUTOR.load(Ordering::Relaxed) || !self.interface {
            return 1023;
        }
        let pending = self.pending & self.enabled & !self.active;
        (0..64)
            .find(|id| pending & (1 << id) != 0 && u32::from(self.priority[*id]) < self.mask)
            .map_or(1023, |id| id as u32)
    }

    fn access(&mut self, address: u64, value: Option<u32>, bytes: usize) -> u32 {
        let offset = address & 0xffff;
        if address & !0xffff == 0x0801_0000 {
            return match (offset, value) {
                (0, Some(value)) => {
                    self.interface = value & 1 != 0;
                    0
                }
                (0, None) => u32::from(self.interface),
                (4, Some(value)) => {
                    self.mask = value & 255;
                    0
                }
                (4, None) => self.mask,
                (0xc, None) => {
                    let id = self.next();
                    if id < 64 {
                        self.pending &= !(1 << id);
                        self.active |= 1 << id;
                    }
                    id
                }
                (0x10 | 0x1000, Some(value)) => {
                    if value & 1023 < 64 {
                        self.active &= !(1 << (value & 1023));
                    }
                    0
                }
                (0xfc, None) => 0x0202_043b,
                _ => 0,
            };
        }
        match (offset, value) {
            (0, Some(value)) => {
                DISTRIBUTOR.store(value & 1 != 0, Ordering::Relaxed);
                0
            }
            (0, None) => u32::from(DISTRIBUTOR.load(Ordering::Relaxed)),
            (4, None) => 1 | ((CPUS.load(Ordering::Relaxed) as u32 - 1) << 5),
            (8, None) => 0x0102_043b,
            (0x100..=0x107, Some(value)) => {
                self.enabled |= u64::from(value) << ((offset & 4) * 8);
                0
            }
            (0x180..=0x187, Some(value)) => {
                self.enabled &= !(u64::from(value) << ((offset & 4) * 8));
                0
            }
            (0x100..=0x107 | 0x180..=0x187, None) => (self.enabled >> ((offset & 4) * 8)) as u32,
            (0x200..=0x207, Some(value)) => {
                self.pending |= u64::from(value) << ((offset & 4) * 8);
                0
            }
            (0x280..=0x287, Some(value)) => {
                self.pending &= !(u64::from(value) << ((offset & 4) * 8));
                0
            }
            (0x200..=0x207 | 0x280..=0x287, None) => (self.pending >> ((offset & 4) * 8)) as u32,
            (0x400..=0x43f | 0x800..=0x83f, value) => {
                let array = if offset < 0x800 {
                    &mut self.priority
                } else {
                    &mut self.targets
                };
                let start = (offset & 63) as usize;
                if start + bytes > 64 {
                    stop(1);
                }
                let mut result = 0;
                for index in 0..bytes {
                    if let Some(value) = value {
                        array[start + index] = (value >> (index * 8)) as u8;
                    }
                    result |= u32::from(array[start + index]) << (index * 8);
                }
                result
            }

            (0xfe8, None) => 0x2b,
            _ => 0,
        }
    }
}

unsafe fn inject() {
    let ctl: u64;
    unsafe {
        asm!("mrs {}, cntv_ctl_el0", out(reg) ctl, options(nomem, nostack));
    }
    let gic = unsafe { &mut (*addr_of_mut!(GICS))[cpu_id()] };
    if ctl & 7 == 5 {
        gic.pending |= 1 << 27;
    } else {
        gic.pending &= !(1 << 27);
    }
    let irq = if gic.next() < 64 { 1 << 7 } else { 0 };
    unsafe {
        asm!("msr hcr_el2, {}", "isb", in(reg) HCR | irq, options(nostack));
    }
}

pub unsafe fn run() -> ! {
    let config = unsafe { &*(BACKING as *const [u64; 8]) };
    assert_eq!(config[0].to_le_bytes(), *b"AEGLNX01");
    let entry = config[1];
    assert!(matches!(entry, 0x4020_0000 | 0x4028_0000));
    assert_eq!(config[2], BASE + 0x10000);
    assert_eq!(config[3], SIZE);
    assert!((1..=4).contains(&config[4]));
    CPUS.store(config[4] as usize, Ordering::Release);
    DEADLINE.store(config[7], Ordering::Relaxed);
    STATES[0].store(2, Ordering::Release);
    assert!(config[5] > 0 && config[5] < SIZE / 2);
    assert!(config[6] > 64 && config[6] <= BASE + SIZE / 2 - entry);
    assert_eq!(
        unsafe { ((entry + BACKING - BASE + 56) as *const u32).read_volatile() },
        0x644d_5241
    );
    writeln!(Serial, "aegishv:linux:boot entry={entry:#x}").unwrap();
    let tables = unsafe { &mut *addr_of_mut!(TABLES) };
    tables[0].0[1] = addr_of!(tables[1]) as u64 | 3;
    for block in 0..128 {
        tables[1].0[block] = addr_of!(tables[2 + block]) as u64 | 3;
        for page in 0..512 {
            tables[2 + block].0[page] = (BACKING + (block * 512 + page) as u64 * 4096)
                | 3
                | (15 << 2)
                | (3 << 8)
                | (1 << 10)
                | (3 << 6)
                | NX;
        }
    }
    tables[0].0[0] = addr_of!(tables[130]) as u64 | 3;
    tables[130].0[72] = addr_of!(tables[131]) as u64 | 3;
    tables[131].0[0] = 0x0900_0000 | 3 | (1 << 10) | (3 << 6) | NX;
    unsafe {
        crate::arm64_paging::enable();
        enter(0, entry, BASE + 0x10000)
    }
}

pub unsafe fn secondary(cpu: usize) -> ! {
    assert!(cpu < CPUS.load(Ordering::Acquire));
    unsafe {
        crate::arm64_paging::activate();
    }
    STATES[cpu].store(2, Ordering::Release);
    unsafe {
        enter(
            cpu,
            ENTRIES[cpu].load(Ordering::Acquire),
            CONTEXTS[cpu].load(Ordering::Acquire),
        )
    }
}

unsafe fn enter(cpu: usize, entry: u64, context: u64) -> ! {
    {
        let _guard = Guard::lock();
        unsafe {
            (*addr_of_mut!(GICS))[cpu].targets.fill(1 << cpu);
        }
        writeln!(Serial, "aegishv:linux:cpu-online {cpu}").unwrap();
    }
    unsafe {
        asm!("msr vmpidr_el2, {}", in(reg) (1u64 << 31) | cpu as u64, options(nostack));
        asm!("dsb sy", "msr vtcr_el2, {vtcr}", "msr vttbr_el2, {root}", "isb", "tlbi vmalls12e1is", "dsb sy", "isb",
            vtcr = in(reg) aegishv_arch_arm64::vtcr::VtcrConfig::new(36, aegishv_arch_arm64::features::Granule::Size4K, aegishv_arch_arm64::features::VmidBits::Bits8).unwrap().encode(),
            root = in(reg) addr_of!(TABLES) as u64 | (1 << 48), options(nostack));
        asm!("msr sctlr_el1, {sctlr}", "msr cpacr_el1, {cpacr}", "msr cnthctl_el2, {timer}",
            "msr cntvoff_el2, xzr", "msr cntv_ctl_el0, xzr", "msr mdcr_el2, xzr", "msr hcr_el2, {hcr}",
            "msr vbar_el1, xzr", "msr sp_el0, xzr", "msr tpidr_el1, xzr", "msr tpidr_el0, xzr", "msr tpidrro_el0, xzr", "isb",
            sctlr = in(reg) 0x30d0_0800u64, cpacr = in(reg) 3u64 << 20, timer = in(reg) 3u64, hcr = in(reg) HCR, options(nostack));
    }
    let mut frame = Frame {
        registers: [0; 31],
        pc: entry,
        pstate: 0x3c5,
        sp: 0,
        fpsr: 0,
        fpcr: 0,
        vectors: [0; 64],
    };
    frame.registers[0] = context;
    unsafe {
        start_timer();
        aegishv_el2_enter(&frame)
    }
}

pub unsafe fn exit(frame: &mut Frame, vector: u64) {
    let guard = Guard::lock();
    let cpu = cpu_id();
    if vector == 9 {
        let irq = unsafe { (0x0801_000c as *const u32).read_volatile() };
        if !matches!(irq & 1023, 0 | 26) {
            writeln!(Serial, "aegishv:linux:irq {irq}").unwrap();
            stop(1);
        }
        unsafe {
            if irq & 1023 == 26 {
                TICKS[cpu] += 1;
                timer_reload();
            }
            (0x0801_0010 as *mut u32).write_volatile(irq);
            asm!("dsb sy", options(nostack));
            let deadline = DEADLINE.load(Ordering::Relaxed);
            if deadline != 0 && TICKS[cpu] >= deadline {
                writeln!(Serial, "aegishv:linux:deadline").unwrap();
                stop(2);
            }
            inject();
        }
        return;
    }
    let (esr, far, hpfar): (u64, u64, u64);
    unsafe {
        asm!("mrs {e}, esr_el2", "mrs {f}, far_el2", "mrs {h}, hpfar_el2", e=out(reg) esr, f=out(reg) far, h=out(reg) hpfar, options(nostack));
    }
    if vector != 8 {
        writeln!(
            Serial,
            "aegishv:linux:host-fault v={vector} pc={:#x} esr={esr:#x} far={far:#x}",
            frame.pc
        )
        .unwrap();
        stop(1);
    }
    match esr >> 26 {
        1 => {
            frame.pc += 4;
            unsafe {
                inject();
            }
            drop(guard);
            unsafe {
                asm!("wfi", options(nostack));
            }
            return;
        }
        0x16 => {
            frame.registers[0] = match frame.registers[0] as u32 {
                0x8400_0000 => 0x0001_0000,
                0x8400_000a => match frame.registers[1] as u32 {
                    0x8400_0000 | 0x8400_0002 | 0xc400_0003 | 0xc400_0004 | 0x8400_0008
                    | 0x8400_0009 | 0x8400_000a => 0,
                    _ => u64::MAX,
                },
                0x8400_0008 | 0x8400_0009 => {
                    writeln!(
                        Serial,
                        "aegishv:linux:poweroff ticks={} exec={} write={}",
                        unsafe { TICKS[0] },
                        unsafe { TRANSITIONS[0] },
                        unsafe { TRANSITIONS[1] }
                    )
                    .unwrap();
                    stop(0)
                }
                0xc400_0003 => unsafe {
                    cpu_on(frame.registers[1], frame.registers[2], frame.registers[3])
                },
                0xc400_0004 => {
                    let target = frame.registers[1] as usize;
                    if target >= CPUS.load(Ordering::Relaxed) {
                        (-2i64) as u64
                    } else {
                        match STATES[target].load(Ordering::Acquire) {
                            2 => 0,
                            1 => 2,
                            _ => 1,
                        }
                    }
                }
                0x8400_0002 => {
                    STATES[cpu].store(0, Ordering::Release);
                    drop(guard);
                    unsafe {
                        asm!("smc #0", in("x0") 0x8400_0002u64, options(noreturn));
                    }
                }
                0x8000_0000 => 0x0001_0001,
                _ => u64::MAX,
            };
        }
        0x20 | 0x24 => {
            let permission = esr & 0x3c == 0xc;
            let address = if permission && esr & (1 << 7) == 0 {
                let par: u64;
                unsafe {
                    asm!("at s1e1r, {}", "isb", "mrs {}, par_el1", in(reg) far, out(reg) par, options(nostack));
                }
                if par & 1 != 0 {
                    writeln!(Serial, "aegishv:linux:walk-failed {par:#x}").unwrap();
                    stop(1);
                }
                (par & 0x000f_ffff_ffff_f000) | (far & 4095)
            } else {
                ((hpfar & 0x0000_0fff_ffff_fff0) << 8) | (far & 4095)
            };
            if (BASE..BASE + SIZE).contains(&address) && permission {
                let page = ((address - BASE) >> 12) as usize;
                let execute = esr >> 26 == 0x20;
                let write = esr & (1 << 6) != 0;
                if !execute && !write {
                    stop(1);
                }
                if write && esr & (1 << 24) != 0 {
                    let pc: u64;
                    unsafe {
                        asm!("at s1e1r, {}", "isb", "mrs {}, par_el1", in(reg) frame.pc, out(reg) pc, options(nostack));
                    }
                    if pc & 1 == 0 && pc & 0x000f_ffff_ffff_f000 == address & !4095 {
                        let count = 1usize << ((esr >> 22) & 3);
                        let rt = ((esr >> 16) & 31) as usize;
                        let value = if rt == 31 { 0 } else { frame.registers[rt] };
                        assert!(address + count as u64 <= BASE + SIZE);
                        for byte in 0..count {
                            unsafe {
                                ((address + BACKING - BASE + byte as u64) as *mut u8)
                                    .write_volatile((value >> (byte * 8)) as u8);
                            }
                        }
                        unsafe {
                            TRANSITIONS[1] += 1;
                            asm!("dc civac, {}", "dsb ish", "ic ialluis", "dsb ish", "isb", in(reg) address + BACKING - BASE, options(nostack));
                        }
                        frame.pc += 4;
                        unsafe {
                            inject();
                        }
                        return;
                    }
                }
                let entry = unsafe { &mut (*addr_of_mut!(TABLES))[2 + page / 512].0[page % 512] };
                *entry = (*entry & !(NX | (1 << 7))) | if execute { 0 } else { NX | (1 << 7) };
                unsafe {
                    TRANSITIONS[usize::from(!execute)] += 1;
                    asm!("dsb ishst", "tlbi ipas2e1is, {}", "dsb ish", "tlbi vmalle1is", "dsb ish", "isb", in(reg) address >> 12, options(nostack));
                }
            } else if esr >> 26 == 0x24
                && esr & (1 << 24) != 0
                && (0x0800_0000..0x0802_0000).contains(&address)
            {
                let rt = ((esr >> 16) & 31) as usize;
                let bytes = 1usize << ((esr >> 22) & 3);
                if bytes > 4 || address as usize % bytes != 0 {
                    stop(1);
                }
                let value = if esr & (1 << 6) != 0 {
                    Some(if rt == 31 {
                        0
                    } else {
                        frame.registers[rt] as u32
                    })
                } else {
                    None
                };
                let result = if let (0x0800_0f00, Some(value)) = (address, value) {
                    unsafe {
                        send_sgi(value, cpu);
                    }
                    0
                } else {
                    unsafe { (*addr_of_mut!(GICS))[cpu].access(address, value, bytes) }
                };
                if value.is_none() && rt != 31 {
                    frame.registers[rt] = u64::from(result);
                }
                frame.pc += 4;
            } else {
                writeln!(
                    Serial,
                    "aegishv:linux:abort esr={esr:#x} ipa={address:#x} far={far:#x} pc={:#x}",
                    frame.pc
                )
                .unwrap();
                stop(1);
            }
        }
        _ => {
            writeln!(
                Serial,
                "aegishv:linux:exit esr={esr:#x} far={far:#x} pc={:#x} x0={:#x}",
                frame.pc, frame.registers[0]
            )
            .unwrap();
            stop(1);
        }
    }
    unsafe {
        inject();
    }
}

unsafe fn cpu_on(cpu: u64, entry: u64, context: u64) -> u64 {
    if cpu >= CPUS.load(Ordering::Relaxed) as u64
        || entry & 3 != 0
        || !(BASE..BASE + SIZE).contains(&entry)
    {
        return (-2i64) as u64;
    }
    let cpu = cpu as usize;
    if let Err(state) = STATES[cpu].compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire) {
        return if state == 2 { -4i64 } else { -5i64 } as u64;
    }
    ENTRIES[cpu].store(entry, Ordering::Release);
    CONTEXTS[cpu].store(context, Ordering::Release);
    extern "C" {
        fn aegishv_el2_start();
    }
    let result: u64;
    unsafe {
        asm!("dsb sy", "smc #0", inlateout("x0") 0xc400_0003u64 => result,
             in("x1") cpu as u64, in("x2") aegishv_el2_start as usize as u64, in("x3") cpu as u64, clobber_abi("C"), options(nostack));
    }
    if result != 0 {
        STATES[cpu].store(0, Ordering::Release);
    }
    result
}

unsafe fn send_sgi(value: u32, source: usize) {
    let filter = (value >> 24) & 3;
    let mut targets = 0;
    for (cpu, state) in STATES.iter().enumerate().take(CPUS.load(Ordering::Relaxed)) {
        let selected = match filter {
            0 => value & (1 << (16 + cpu)) != 0,
            1 => cpu != source,
            2 => cpu == source,
            _ => false,
        };
        if selected && state.load(Ordering::Acquire) == 2 {
            unsafe {
                (*addr_of_mut!(GICS))[cpu].pending |= 1 << (value & 15);
            }
            targets |= 1 << (16 + cpu);
        }
    }
    unsafe {
        asm!("dsb ishst", options(nostack));
        (0x0800_0f00 as *mut u32).write_volatile(targets);
    }
}
