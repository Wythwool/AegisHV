#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod kernel {
    use core::arch::{asm, global_asm};
    use core::fmt::{self, Write};
    use core::ptr::{addr_of, addr_of_mut};

    use aegishv_type1_kernel::events::{Backend, Events};

    global_asm!(include_str!("../../../boot/arm64/entry.S"));

    #[repr(C, align(4096))]
    struct Page([u64; 512]);
    static mut TABLES: [Page; 3] = [const { Page([0; 512]) }; 3];
    static mut RAM: [Page; 3] = [const { Page([0; 512]) }; 3];
    static mut EVENTS: Events<Serial> = Events::new(Serial, Backend::El2);
    static mut WRITE_BLOCKED: bool = false;
    static mut PMU_START: u64 = 0;
    static mut PMU_PRESENT: bool = false;

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
        static aegishv_el1_start: u8;
        static aegishv_el1_end: u8;
        static aegishv_el1_exec_probe: u8;
    }

    fn stop(code: u64) -> ! {
        let result = [0x20026u64, code];
        unsafe {
            asm!("hlt #0xf000", in("x0") 0x20u64, in("x1") result.as_ptr(), options(nostack))
        };
        loop {
            unsafe { asm!("wfe", options(nomem, nostack)) };
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn aegishv_el2_main() -> ! {
        writeln!(Serial, "aegishv:el2:boot").unwrap();
        let tables = unsafe { &mut *addr_of_mut!(TABLES) };
        let ram = unsafe { &mut *addr_of_mut!(RAM) };
        tables[0].0[1] = addr_of!(tables[1]) as u64 | 3;
        tables[1].0[0] = addr_of!(tables[2]) as u64 | 3;
        for index in 0..3 {
            let access = if index == 0 {
                1 << 6
            } else {
                (3 << 6) | (1 << 54)
            };
            tables[2].0[index] =
                addr_of!(ram[index]) as u64 | 3 | (15 << 2) | (3 << 8) | (1 << 10) | access;
        }
        let length = addr_of!(aegishv_el1_end) as usize - addr_of!(aegishv_el1_start) as usize;
        assert!(length < 4096);
        unsafe {
            core::ptr::copy_nonoverlapping(
                addr_of!(aegishv_el1_start),
                addr_of_mut!(ram[0]).cast(),
                length,
            )
        };

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
        let vtcr = (1u64 << 31) | (1 << 16) | (3 << 12) | (1 << 10) | (1 << 8) | (1 << 6) | 28;
        let vttbr = (1u64 << 48) | addr_of!(TABLES) as u64;
        unsafe {
            asm!("dsb sy", "msr vtcr_el2, {vtcr}", "msr vttbr_el2, {vttbr}", "isb", "tlbi vmalls12e1is", "dsb sy", "isb",
                vtcr = in(reg) vtcr, vttbr = in(reg) vttbr, options(nostack));
            asm!("msr sctlr_el1, {sctlr}", "msr cpacr_el1, {cpacr}", "msr cntvoff_el2, xzr", "msr cnthctl_el2, {timer}",
                sctlr = in(reg) 0x30d0_0800u64, cpacr = in(reg) 3u64 << 20, timer = in(reg) 3u64, options(nostack));
            asm!("msr hcr_el2, {}", in(reg) (1u64 << 31) | (1 << 19) | (1 << 14) | (1 << 13) | 0x39, options(nostack));
        }
        writeln!(Serial, "aegishv:el2:guest-enter").unwrap();
        if unsafe { PMU_PRESENT } {
            let cycles: u64;
            unsafe {
                asm!("isb", "mrs {}, pmccntr_el0", out(reg) cycles, options(nomem, nostack));
                PMU_START = cycles;
            }
        }
        unsafe {
            asm!("msr sp_el1, {stack}", "msr elr_el2, {entry}", "msr spsr_el2, {state}", "isb", "eret",
                stack = in(reg) 0x4000_3000u64, entry = in(reg) 0x4000_0000u64, state = in(reg) 0x3c5u64, options(noreturn));
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn aegishv_el2_exit(frame: &mut Frame) {
        let (esr, far, hpfar): (u64, u64, u64);
        unsafe {
            asm!("mrs {esr}, esr_el2", "mrs {far}, far_el2", "mrs {hpfar}, hpfar_el2",
            esr = out(reg) esr, far = out(reg) far, hpfar = out(reg) hpfar, options(nomem, nostack));
        }
        let events = unsafe { &mut *addr_of_mut!(EVENTS) };
        match esr >> 26 {
            0x16 if esr & 0xffff == 0 => {
                let vbar: u64;
                unsafe { asm!("mrs {}, vbar_el1", out(reg) vbar, options(nomem, nostack)) };
                events
                    .syscall(frame.pc, 0, vbar, vbar == 0x4000_0000)
                    .unwrap();
                if vbar != 0x4000_0000 {
                    stop(1);
                }
                writeln!(Serial, "aegishv:el2:hvc-exit").unwrap();
            }
            0x24 if far == 0x4000_0800 && esr & 0x7f == 0x4f && !unsafe { WRITE_BLOCKED } => {
                if unsafe { (*addr_of!(RAM))[0].0[256] } != 0 {
                    stop(1);
                }
                events.denied(frame.pc, far, true).unwrap();
                unsafe {
                    WRITE_BLOCKED = true;
                }
                frame.pc = 0x4000_0000 + addr_of!(aegishv_el1_exec_probe) as u64
                    - addr_of!(aegishv_el1_start) as u64;
            }
            0x20 if far == 0x4000_1000 && esr & 0x3f == 0xf && unsafe { WRITE_BLOCKED } => {
                events.denied(frame.pc, far, false).unwrap();
                if unsafe { PMU_PRESENT } {
                    let cycles: u64;
                    unsafe {
                        asm!("isb", "mrs {}, pmccntr_el0", out(reg) cycles, options(nomem, nostack))
                    };
                    events
                        .pmu_cycles(cycles.wrapping_sub(unsafe { PMU_START }))
                        .unwrap();
                }
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
