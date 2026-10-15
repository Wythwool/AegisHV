#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod arm64_paging;

#[cfg(all(target_os = "none", target_arch = "aarch64", linux_guest))]
mod arm64_linux;

#[cfg(all(target_os = "none", target_arch = "aarch64", linux_guest))]
mod arm64_monitor;

#[cfg(all(target_os = "none", target_arch = "aarch64", not(linux_guest)))]
mod arm64_probe;

#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod kernel {
    use core::arch::{asm, global_asm};
    use core::fmt::{self, Write};
    global_asm!(include_str!("../../../boot/arm64/entry.S"));

    #[repr(C)]
    pub struct Frame {
        pub(crate) registers: [u64; 31],
        pub(crate) pc: u64,
        pub(crate) pstate: u64,
        pub(crate) sp: u64,
        pub(crate) fpsr: u64,
        pub(crate) fpcr: u64,
        pub(crate) vectors: [u64; 64],
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
        pub(crate) fn aegishv_el2_enter(frame: *const Frame) -> !;
    }

    pub(crate) fn stop(code: u64) -> ! {
        unsafe { asm!("msr cnthp_ctl_el2, xzr", "isb", options(nostack)) };
        let result = [0x20026u64, code];
        unsafe {
            asm!("hlt #0xf000", in("x0") 0x20u64, in("x1") result.as_ptr(), options(nostack))
        };
        loop {
            unsafe { asm!("wfe", options(nomem, nostack)) };
        }
    }

    pub(crate) unsafe fn timer_reload() {
        let frequency: u64;
        unsafe {
            asm!("mrs {}, cntfrq_el0", out(reg) frequency, options(nomem, nostack));
            asm!("msr cnthp_tval_el2, {}", "isb", in(reg) frequency / 100, options(nostack));
        }
    }

    pub(crate) unsafe fn start_timer() {
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

    #[no_mangle]
    pub unsafe extern "C" fn aegishv_el2_main() -> ! {
        #[cfg(linux_guest)]
        unsafe {
            crate::arm64_linux::run()
        }
        #[cfg(not(linux_guest))]
        unsafe {
            crate::arm64_probe::run()
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn aegishv_el2_secondary(cpu: usize) -> ! {
        #[cfg(linux_guest)]
        unsafe {
            crate::arm64_linux::secondary(cpu)
        }
        #[cfg(not(linux_guest))]
        {
            let _ = cpu;
            loop {
                unsafe {
                    asm!("wfe", options(nomem, nostack));
                }
            }
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn aegishv_el2_exit(frame: &mut Frame, vector: u64) {
        #[cfg(linux_guest)]
        unsafe {
            crate::arm64_linux::exit(frame, vector)
        }
        #[cfg(not(linux_guest))]
        unsafe {
            crate::arm64_probe::exit(frame, vector)
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
