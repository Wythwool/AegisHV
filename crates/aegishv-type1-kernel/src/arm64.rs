#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod kernel {
    use core::arch::{asm, global_asm};
    use core::fmt::{self, Write};
    use core::ptr::{addr_of, addr_of_mut};

    use aegishv_arch_arm64::features::{Granule, VmidBits};
    use aegishv_arch_arm64::vtcr::{VtcrConfig, Vttbr};
    use aegishv_hypervisor_core::ids::HostPhysical;
    use aegishv_type1_kernel::events::{Access, Backend, Events};

    global_asm!(include_str!("../../../boot/arm64/entry.S"));

    #[repr(C, align(4096))]
    struct Page([u64; 512]);
    static mut TABLES: [Page; 3] = [const { Page([0; 512]) }; 3];
    static mut RAM: [Page; 32] = [const { Page([0; 512]) }; 32];
    static mut EVENTS: Events<Serial> = Events::new(Serial, Backend::El2);
    static mut WRITE_BLOCKED: bool = false;
    static mut PMU_START: u64 = 0;
    static mut PMU_PRESENT: bool = false;
    static mut TICKS: u64 = 0;
    static mut EXITS: u64 = 0;
    static mut PAGE_FLAGS: [u8; 32] = [0; 32];
    use aegishv_type1_kernel::GUEST_IMAGE as IMAGE;
    const BASE: u64 = 0x4000_0000;

    #[repr(C)]
    pub struct Frame {
        registers: [u64; 31],
        pc: u64,
        pstate: u64,
        sp: u64,
        fpsr: u64,
        fpcr: u64,
        vectors: [u64; 64],
    }
    const _: () = assert!(core::mem::size_of::<Frame>() == 800);

    pub struct Serial;
    impl Write for Serial {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            for byte in text.bytes() {
                unsafe {
                    while (0x0900_0018 as *const u32).read_volatile() & (1 << 5) != 0 {}
                    (0x0900_0000 as *mut u32).write_volatile(u32::from(byte));
                }
            }
            Ok(())
        }
    }

    extern "C" {
        fn aegishv_el2_enter(frame: *const Frame) -> !;
        static aegishv_el1_start: u8;
        static aegishv_el1_end: u8;
        static aegishv_el1_exec_probe: u8;
        static aegishv_el1_after_timer: u8;
    }

    fn stop(code: u64) -> ! {
        unsafe { asm!("msr cnthp_ctl_el2, xzr", "isb", options(nostack)) };
        let result = [0x20026u64, code];
        unsafe {
            asm!("hlt #0xf000", in("x0") 0x20u64, in("x1") result.as_ptr(), options(nostack))
        };
        loop {
            unsafe { asm!("wfe", options(nomem, nostack)) };
        }
    }

    unsafe fn timer_reload() {
        let frequency: u64;
        unsafe {
            asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nomem, nostack));
            asm!("msr cnthp_tval_el2, {}", "isb", in(reg) frequency / 100, options(nostack));
        }
    }

    unsafe fn start_timer() {
        // QEMU virt GICv2 without security extensions, EL2 physical timer PPI 26.
        unsafe {
            assert!((0x0800_0004 as *const u32).read_volatile() & (1 << 10) == 0);
            (0x0800_0080 as *mut u32).write_volatile(0);
            (0x0800_0100 as *mut u32).write_volatile(1 << 26);
            (0x0800_041a as *mut u8).write_volatile(0x80);
            (0x0800_0000 as *mut u32).write_volatile(1);
            (0x0801_0004 as *mut u32).write_volatile(0xff);
            (0x0801_0000 as *mut u32).write_volatile(1);
            asm!("dsb sy", "isb", options(nostack));
            timer_reload();
            asm!("msr cnthp_ctl_el2, {}", "isb", in(reg) 1u64, options(nostack));
        }
    }

    fn sample_pmu(events: &mut Events<Serial>) {
        if unsafe { PMU_PRESENT } {
            let cycles: u64;
            unsafe { asm!("isb", "mrs {}, pmccntr_el0", out(reg) cycles, options(nomem, nostack)) };
            events
                .pmu_cycles(cycles.wrapping_sub(unsafe { PMU_START }))
                .unwrap();
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn aegishv_el2_main() -> ! {
        writeln!(Serial, "aegishv:el2:boot").unwrap();
        let tables = unsafe { &mut *addr_of_mut!(TABLES) };
        let ram = unsafe { &mut *addr_of_mut!(RAM) };
        tables[0].0[1] = addr_of!(tables[1]) as u64 | 3;
        tables[1].0[0] = addr_of!(tables[2]) as u64 | 3;
        let flags = unsafe { &mut *addr_of_mut!(PAGE_FLAGS) };
        let length = addr_of!(aegishv_el1_end) as usize - addr_of!(aegishv_el1_start) as usize;
        assert!(length < 4096);
        let entry = if IMAGE.is_empty() {
            flags[..3].copy_from_slice(&[5, 6, 6]);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    addr_of!(aegishv_el1_start),
                    addr_of_mut!(ram[0]).cast(),
                    length,
                )
            };
            BASE
        } else {
            let memory = unsafe {
                core::slice::from_raw_parts_mut(addr_of_mut!(ram[0]).cast::<u8>(), 28 * 4096)
            };
            aegishv_type1_kernel::elf::load(IMAGE, 183, BASE, memory, &mut flags[..28]).unwrap()
        };
        flags[28..].fill(6); // Private stack at the end of the guest allocation.
        for (index, flag) in flags.iter().copied().enumerate() {
            if flag == 0 {
                continue;
            }
            let access = (1 << 6)
                | if flag & 2 != 0 { 1 << 7 } else { 0 }
                | if flag & 1 == 0 { 1 << 54 } else { 0 };
            tables[2].0[index] =
                addr_of!(ram[index]) as u64 | 3 | (15 << 2) | (3 << 8) | (1 << 10) | access;
        }

        let dfr: u64;
        unsafe { asm!("mrs {}, id_aa64dfr0_el1", out(reg) dfr, options(nomem, nostack)) };
        let pmu_version = (dfr >> 8) & 15;
        unsafe { PMU_PRESENT = pmu_version != 0 && pmu_version != 15 };
        if unsafe { PMU_PRESENT } {
            let pmcr: u64;
            unsafe {
                asm!("mrs {}, pmcr_el0", out(reg) pmcr, options(nomem, nostack));
                asm!("msr mdcr_el2, {}", in(reg) ((pmcr >> 11) & 31) | (1 << 6) | (1 << 5), options(nostack));
                asm!("msr pmccfiltr_el0, xzr", "msr pmcr_el0, {control}", "msr pmcntenset_el0, {enable}", "isb",
                    control = in(reg) 0x47u64, enable = in(reg) 1u64 << 31, options(nostack));
            }
        }

        // 36-bit IPA, level-1 root, 4K pages. All unmapped IPA faults stay at EL2.
        let vtcr = VtcrConfig::new(36, Granule::Size4K, VmidBits::Bits8)
            .unwrap()
            .encode();
        let vttbr = Vttbr::new(
            1,
            HostPhysical::new(addr_of!(TABLES) as u64).unwrap(),
            VmidBits::Bits8,
        )
        .unwrap()
        .encode();
        unsafe {
            asm!("dsb sy", "msr vtcr_el2, {vtcr}", "msr vttbr_el2, {vttbr}", "isb", "tlbi vmalls12e1is", "dsb sy", "isb",
                vtcr = in(reg) vtcr, vttbr = in(reg) vttbr, options(nostack));
            asm!("msr sctlr_el1, {sctlr}", "msr cpacr_el1, {cpacr}", "msr cntvoff_el2, xzr", "msr cnthctl_el2, {timer}",
                sctlr = in(reg) 0x30d0_0800u64, cpacr = in(reg) 3u64 << 20, timer = in(reg) 3u64, options(nostack));
            // TVM traps changes to the fixed EL1 identity address space.
            asm!("msr hcr_el2, {}", in(reg) (1u64 << 31) | (1 << 26) | (1 << 19) | (1 << 14) | (1 << 13) | 0x39, options(nostack));
        }
        writeln!(Serial, "aegishv:el2:guest-enter").unwrap();
        if unsafe { PMU_PRESENT } {
            let cycles: u64;
            unsafe {
                asm!("isb", "mrs {}, pmccntr_el0", out(reg) cycles, options(nomem, nostack));
                PMU_START = cycles;
            }
        }
        let frame = Frame {
            registers: [0; 31],
            pc: entry,
            pstate: 0x3c5,
            sp: BASE + 32 * 4096,
            fpsr: 0,
            fpcr: 0,
            vectors: [0; 64],
        };
        unsafe {
            asm!(
                "msr sp_el0, xzr",
                "msr vbar_el1, xzr",
                "msr tpidr_el1, xzr",
                "msr tpidr_el0, xzr",
                "msr tpidrro_el0, xzr",
                options(nostack)
            );
            start_timer();
            aegishv_el2_enter(&frame)
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn aegishv_el2_exit(frame: &mut Frame, vector: u64) {
        let events = unsafe { &mut *addr_of_mut!(EVENTS) };
        unsafe { EXITS += 1 };
        if unsafe { EXITS } >= 4096 {
            events.stopped(frame.pc, unsafe { EXITS }, true).unwrap();
            sample_pmu(events);
            stop(2);
        }
        if vector == 9 {
            let irq = unsafe { (0x0801_000c as *const u32).read_volatile() };
            if irq & 1023 != 26 {
                writeln!(Serial, "aegishv:el2:unexpected-irq {irq}").unwrap();
                stop(1);
            }
            unsafe {
                TICKS += 1;
                timer_reload();
                (0x0801_0010 as *mut u32).write_volatile(irq);
                asm!("dsb sy", "isb", options(nostack));
            }
            if IMAGE.is_empty() && unsafe { TICKS } == 1 {
                assert!((BASE..BASE + 8).contains(&frame.pc));
                frame.pc = BASE + addr_of!(aegishv_el1_after_timer) as u64
                    - addr_of!(aegishv_el1_start) as u64;
                writeln!(Serial, "aegishv:el2:preempt-exit").unwrap();
            }
            if unsafe { TICKS } >= 20 {
                events.stopped(frame.pc, unsafe { TICKS }, true).unwrap();
                sample_pmu(events);
                stop(2);
            }
            return;
        }
        if vector != 8 {
            writeln!(
                Serial,
                "aegishv:el2:host-exception vector={vector} pc={:#x}",
                frame.pc
            )
            .unwrap();
            stop(1);
        }
        let (esr, far, hpfar): (u64, u64, u64);
        unsafe {
            asm!("mrs {esr}, esr_el2", "mrs {far}, far_el2", "mrs {hpfar}, hpfar_el2",
            esr = out(reg) esr, far = out(reg) far, hpfar = out(reg) hpfar, options(nomem, nostack));
        }
        match esr >> 26 {
            0x16 if esr & 0xffff == 1 && !IMAGE.is_empty() => {
                events.stopped(frame.pc, frame.registers[0], false).unwrap();
                sample_pmu(events);
                stop(0);
            }
            0x16 if esr & 0xffff == 0 => {
                let vbar: u64;
                unsafe { asm!("mrs {}, vbar_el1", out(reg) vbar, options(nomem, nostack)) };
                let page = vbar.wrapping_sub(BASE) >> 12;
                let allowed = vbar & 2047 == 0
                    && unsafe { (*addr_of!(PAGE_FLAGS)).get(page as usize) } == Some(&5);
                events.syscall(frame.pc, 0xc600, vbar, allowed).unwrap();
                if !allowed {
                    stop(1);
                }
                writeln!(Serial, "aegishv:el2:hvc-exit").unwrap();
            }
            0x20 | 0x24 if !IMAGE.is_empty() => {
                let access = if esr >> 26 == 0x20 {
                    Access::Execute
                } else if esr & (1 << 6) != 0 {
                    Access::Write
                } else {
                    Access::Read
                };
                events.denied(frame.pc, far, access).unwrap();
                sample_pmu(events);
                stop(3);
            }
            0x24 if far == 0x4000_0800 && esr & 0x7f == 0x4f && !unsafe { WRITE_BLOCKED } => {
                if unsafe { (*addr_of!(RAM))[0].0[256] } != 0 {
                    stop(1);
                }
                events.denied(frame.pc, far, Access::Write).unwrap();
                unsafe {
                    WRITE_BLOCKED = true;
                }
                frame.pc = 0x4000_0000 + addr_of!(aegishv_el1_exec_probe) as u64
                    - addr_of!(aegishv_el1_start) as u64;
            }
            0x20 if far == 0x4000_1000 && esr & 0x3f == 0xf && unsafe { WRITE_BLOCKED } => {
                events.denied(frame.pc, far, Access::Execute).unwrap();
                sample_pmu(events);
                writeln!(Serial, "aegishv:el2:guest-run-ok").unwrap();
                stop(0);
            }
            _ => {
                writeln!(Serial, "aegishv:el2:unexpected-exit esr={esr:#x} far={far:#x} hpfar={hpfar:#x} pc={:#x}", frame.pc).unwrap();
                stop(1);
            }
        }
    }

    #[panic_handler]
    fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
        let _ = writeln!(Serial, "aegishv:el2:panic {info}");
        stop(1)
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    eprintln!("build this kernel for aarch64-unknown-none");
}
