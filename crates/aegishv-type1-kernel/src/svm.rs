use core::arch::{asm, global_asm};
use core::ptr::{addr_of, addr_of_mut};

use aegishv_arch_x86::svm::exits::GeneralRegisters;
use aegishv_arch_x86::svm::vmcb::{
    Segment, Vmcb, INTERCEPT_CPUID, INTERCEPT_HLT, INTERCEPT_IOIO, INTERCEPT_MSR,
};
use aegishv_hypervisor_core::ids::HostPhysical;
use aegishv_type1_kernel::events::Access;
use aegishv_type1_kernel::LimineMinimalHandoff;

use crate::{
    halt_loop, linked_host_physical_address, read_cr0, read_cr3, read_cr4, read_msr,
    serial_write_hex_u64, serial_write_line, write_cr0, write_cr4, write_msr,
};

global_asm!(include_str!("../../../boot/x86_64/svm_entry.S"));

#[repr(C, align(4096))]
struct Page([u64; 512]);
#[repr(C, align(16))]
struct FxState([u8; 512]);

static mut GUEST: Vmcb = Vmcb::zeroed();
static mut HOST: Vmcb = Vmcb::zeroed();
static mut HSAVE: Page = Page([0; 512]);
static mut NPT: [Page; 4] = [const { Page([0; 512]) }; 4];
static mut RAM: [Page; 32] = [const { Page([0; 512]) }; 32];
static mut IOPM: [Page; 3] = [const { Page([u64::MAX; 512]) }; 3];
static mut MSRPM: [Page; 2] = [const { Page([u64::MAX; 512]) }; 2];
static mut HOST_FX: FxState = FxState([0; 512]);
static mut GUEST_FX: FxState = FxState([0; 512]);

extern "C" {
    fn aegishv_svm_enter(
        guest: u64,
        host: u64,
        regs: *mut GeneralRegisters,
        host_fx: *mut FxState,
        guest_fx: *mut FxState,
    );
    static aegishv_svm_guest_start: u8;
    static aegishv_svm_guest_end: u8;
    static aegishv_svm_guest_write_exec: u8;
    static aegishv_svm_guest_after_timer: u8;
}

const NX: u64 = 1 << 63;
const EFER: u32 = 0xc000_0080;
const LSTAR: u32 = 0xc000_0082;

fn finish(code: u32) -> ! {
    unsafe {
        crate::outb(0x21, 0xff);
        asm!("out dx, eax", in("dx") 0xf4u16, in("eax") code, options(nostack));
    }
    halt_loop()
}

unsafe fn start_timer() {
    // V_INTR_MASKING keeps guest CLI from masking the host PIT interrupt.
    // This BSP-only path uses the legacy PIC, not the bootloader's masked LAPIC.
    unsafe { write_msr(0x1b, read_msr(0x1b) & !((1 << 11) | (1 << 10))) };
    for (port, value) in [
        (0x20, 0x11),
        (0xa0, 0x11),
        (0x21, 0x20),
        (0xa1, 0x28),
        (0x21, 4),
        (0xa1, 2),
        (0x21, 1),
        (0xa1, 1),
        (0x21, 0xfe),
        (0xa1, 0xff),
        (0x43, 0x34),
        (0x40, 0x9c),
        (0x40, 0x2e),
    ] {
        unsafe {
            crate::outb(port, value);
            crate::outb(0x80, 0);
        }
    }
}

fn fail(reason: &str, value: u64) -> ! {
    serial_write_line(reason);
    serial_write_hex_u64("aegishv:svm:detail=0x", value);
    halt_loop()
}

unsafe fn physical(handoff: LimineMinimalHandoff, pointer: *const u8) -> u64 {
    linked_host_physical_address(handoff, pointer as u64)
        .unwrap_or_else(|_| fail("aegishv:svm:address-error", pointer as u64))
}

pub unsafe fn run(handoff: LimineMinimalHandoff) -> ! {
    serial_write_line("aegishv:type1:backend-svm");
    let features = unsafe { core::arch::x86_64::__cpuid_count(0x8000_000a, 0) };
    if features.edx & 1 == 0 || features.ebx < 2 || unsafe { read_msr(0xc001_0114) } & (1 << 4) != 0
    {
        fail("aegishv:svm:unavailable", u64::from(features.edx));
    }

    // All backing pages are in the linked kernel allocation. NPT only exposes RAM.
    let guest_pa = unsafe { physical(handoff, addr_of!(GUEST).cast()) };
    let host_pa = unsafe { physical(handoff, addr_of!(HOST).cast()) };
    let hsave_pa = unsafe { physical(handoff, addr_of!(HSAVE).cast()) };
    let npt_pa = unsafe { physical(handoff, addr_of!(NPT).cast()) };
    let ram_pa = unsafe { physical(handoff, addr_of!(RAM).cast()) };
    let iopm_pa = unsafe { physical(handoff, addr_of!(IOPM).cast()) };
    let msrpm_pa = unsafe { physical(handoff, addr_of!(MSRPM).cast()) };
    let guest = unsafe { &mut *addr_of_mut!(GUEST) };
    let ram = unsafe { &mut *addr_of_mut!(RAM) };
    let npt = unsafe { &mut *addr_of_mut!(NPT) };
    for (level, table) in npt.iter_mut().enumerate().take(3) {
        table.0[0] = (npt_pa + (level as u64 + 1) * 4096) | 7;
    }
    let mut flags = [6u8; 32];
    flags[0] = 0;
    flags[1] = 5;
    let start = addr_of!(aegishv_svm_guest_start);
    let length = addr_of!(aegishv_svm_guest_end) as usize - start as usize;
    assert!(length < 4096);
    let image = aegishv_type1_kernel::GUEST_IMAGE;
    let entry = if image.is_empty() {
        unsafe { core::ptr::copy_nonoverlapping(start, addr_of_mut!(ram[1]).cast(), length) };
        0x1000
    } else {
        let memory = unsafe {
            core::slice::from_raw_parts_mut(addr_of_mut!(ram[1]).cast::<u8>(), 23 * 4096)
        };
        aegishv_type1_kernel::elf::load(image, 62, 0x1000, memory, &mut flags[1..24])
            .unwrap_or_else(|_| fail("aegishv:svm:guest-elf-error", 0))
    };
    // Table pages are read-only in the guest map. NPT permits hardware A/D updates.
    ram[28].0[0] = 0x1d000 | 0x67;
    ram[29].0[0] = 0x1e000 | 0x67;
    ram[30].0[0] = 0x1f000 | 0x67;
    for (page, flag) in flags.iter().copied().enumerate().skip(1) {
        ram[31].0[page] = (page as u64 * 4096) | if page >= 28 { 0x65 } else { 0x67 };
        if flag != 0 {
            let permissions = 5 | u64::from(flag & 2) | if flag & 1 == 0 { NX } else { 0 };
            npt[3].0[page] = (ram_pa + page as u64 * 4096) | permissions;
        }
    }

    guest.control.set_misc_intercepts(
        0xf | (1 << 15)
            | INTERCEPT_CPUID
            | INTERCEPT_HLT
            | INTERCEPT_IOIO
            | INTERCEPT_MSR
            | (0x3fff << 32),
    );
    guest.control.mask_guest_interrupts();
    guest.control.set_intercept_cr_write(u16::MAX);
    guest.control.intercept_debug_registers();
    guest.control.set_exception_intercepts(u32::MAX);
    guest
        .control
        .set_io_permission_map(HostPhysical::new(iopm_pa).unwrap());
    guest
        .control
        .set_msr_permission_map(HostPhysical::new(msrpm_pa).unwrap());
    guest.control.set_guest_asid(1);
    guest
        .control
        .set_nested_paging(true, HostPhysical::new(npt_pa).unwrap());
    guest.state.set_segment(Segment::Cs, 8, 0xa9b, u32::MAX, 0);
    for segment in [
        Segment::Es,
        Segment::Ss,
        Segment::Ds,
        Segment::Fs,
        Segment::Gs,
    ] {
        guest.state.set_segment(segment, 16, 0xc93, u32::MAX, 0);
    }
    guest.state.set_segment(Segment::Tr, 24, 0x8b, 103, 0);
    guest.state.set_cr0(0x8001_0033);
    guest.state.set_cr3(0x1c000);
    guest.state.set_cr4(0x620);
    guest.state.set_efer(0x1d01); // SCE, LME, LMA, NXE, SVME.
    guest.state.set_debug_registers(0xffff_0ff0, 0x400);
    guest.state.set_rflags(2);
    guest.state.set_rip(entry);
    guest.state.set_rsp(0x1c000);
    guest.state.set_pat(0x0007_0406_0007_0406);
    guest.state.set_star(8 << 32);
    unsafe {
        (*addr_of_mut!(GUEST_FX)).0[0..2].copy_from_slice(&0x037fu16.to_le_bytes());
        (*addr_of_mut!(GUEST_FX)).0[24..28].copy_from_slice(&0x1f80u32.to_le_bytes());
        write_cr0((read_cr0() | 2) & !12);
        write_cr4((read_cr4() | 0x600) & !(1 << 18));
        write_msr(EFER, read_msr(EFER) | (1 << 12));
        write_msr(0xc001_0117, hsave_pa);
    }
    let inherited = unsafe { read_cr3() };
    let tables = unsafe { crate::prepare_owned_host_page_tables(handoff) }
        .unwrap_or_else(|_| fail("aegishv:svm:host-paging-error", 0));
    unsafe { crate::activate_owned_host_paging(&tables, inherited) }
        .unwrap_or_else(|_| fail("aegishv:svm:host-paging-error", 1));
    serial_write_line("aegishv:svm:guest-enter");
    let mut regs = GeneralRegisters::default();
    let mut events = aegishv_type1_kernel::events::Events::new(
        crate::Serial,
        aegishv_type1_kernel::events::Backend::Svm,
    );
    let mut syscall_entry = 0u64;
    let mut write_blocked = false;
    let mut ticks = 0u64;
    unsafe { start_timer() };
    for sequence in 0..4096u64 {
        guest.control.set_tlb_control(1);
        unsafe {
            aegishv_svm_enter(
                guest_pa,
                host_pa,
                &mut regs,
                addr_of_mut!(HOST_FX),
                addr_of_mut!(GUEST_FX),
            )
        };
        regs.rax = guest.state.rax();
        let host_tr = unsafe { (*addr_of!(HOST)).state.segment_base(Segment::Tr) };
        if host_tr != addr_of!(crate::__aegishv_host_tss) as u64 {
            fail("aegishv:svm:host-tss-error", host_tr);
        }
        regs.rsp = guest.state.rsp();
        regs.rip = guest.state.rip();
        match guest.control.exit_code() {
            code @ 0x40..=0x5f if !image.is_empty() => {
                events
                    .exception(regs.rip, code - 0x40, guest.control.exit_info2())
                    .unwrap();
                finish(0x12);
            }
            0x60 => {
                unsafe {
                    crate::outb(0x20, 0x0c);
                    let pending = crate::inb(0x20);
                    if pending != 0x80 {
                        fail("aegishv:svm:unexpected-irq", u64::from(pending));
                    }
                    crate::outb(0x20, 0x20);
                }
                ticks += 1;
                if image.is_empty() && ticks == 1 {
                    if !(0x1000..0x1003).contains(&regs.rip) {
                        fail("aegishv:svm:timer-rip", regs.rip);
                    }
                    guest.state.set_rip(
                        0x1000 + addr_of!(aegishv_svm_guest_after_timer) as u64 - start as u64,
                    );
                    serial_write_line("aegishv:svm:preempt-exit");
                }
                if ticks >= 20 {
                    events.stopped(regs.rip, ticks, true).unwrap();
                    finish(0x11);
                }
            }
            0x78 if !image.is_empty() => {
                events.stopped(regs.rip, regs.rax, false).unwrap();
                finish(0x10);
            }
            0x72 => {
                let leaf = regs.rax as u32;
                let values = match leaf {
                    0 | 0x4000_0000 => [
                        if leaf == 0 { 1 } else { leaf },
                        u32::from_le_bytes(*b"Aegi"),
                        u32::from_le_bytes(*b"    "),
                        u32::from_le_bytes(*b"sHV "),
                    ],
                    // No nested SVM, XSAVE or AVX: entry preserves the FXSAVE state.
                    1 => [
                        0x600,
                        0,
                        1 << 31,
                        (1 << 26)
                            | (1 << 25)
                            | (1 << 24)
                            | (1 << 23)
                            | (1 << 15)
                            | (1 << 8)
                            | (1 << 6)
                            | (1 << 5)
                            | (1 << 4)
                            | 1,
                    ],
                    0x8000_0000 => [0x8000_0001, 0, 0, 0],
                    0x8000_0001 => [0, 0, 0, (1 << 29) | (1 << 20) | (1 << 11)],
                    _ => [0; 4],
                };
                [regs.rax, regs.rbx, regs.rcx, regs.rdx] = values.map(u64::from);
                guest.state.set_rip(regs.rip + 2);
                serial_write_line("aegishv:svm:cpuid-exit");
            }
            0x7c if regs.rcx == u64::from(LSTAR) => {
                if guest.control.exit_info1() & 1 != 0 {
                    let target = (regs.rdx & 0xffff_ffff) << 32 | (regs.rax & 0xffff_ffff);
                    let allowed = flags.get((target >> 12) as usize) == Some(&5);
                    events.syscall(regs.rip, LSTAR, target, allowed).unwrap();
                    if !allowed {
                        finish(0x12);
                    }
                    syscall_entry = target;
                    guest.state.set_lstar(target);
                } else {
                    regs.rax = syscall_entry & 0xffff_ffff;
                    regs.rdx = syscall_entry >> 32;
                }
                guest.state.set_rip(regs.rip + 2);
            }
            0x400 => {
                let info = guest.control.exit_info1();
                let address = guest.control.exit_info2();
                serial_write_hex_u64("aegishv:svm:npf-info=0x", info);
                serial_write_hex_u64("aegishv:svm:npf-gpa=0x", address);
                if info & (1 << 3) != 0 {
                    fail("aegishv:svm:npt-reserved-bit", info);
                }
                if !image.is_empty() {
                    let access = if info & (1 << 4) != 0 {
                        Access::Execute
                    } else if info & 2 != 0 {
                        Access::Write
                    } else {
                        Access::Read
                    };
                    events.denied(regs.rip, address, access).unwrap();
                    finish(0x12);
                }
                if !write_blocked && address == 0x1800 && info & 2 != 0 {
                    if ram[1].0[256] != 0 {
                        fail("aegishv:svm:write-escaped", ram[1].0[256]);
                    }
                    events.denied(regs.rip, address, Access::Write).unwrap();
                    write_blocked = true;
                    // Start the independent NX probe without executing the denied store.
                    guest.state.set_rip(
                        0x1000 + addr_of!(aegishv_svm_guest_write_exec) as u64 - start as u64,
                    );
                } else if write_blocked && address == 0x3000 && info & (1 << 4) != 0 {
                    if ram[3].0[0] & 0xff != 0xf4 {
                        fail("aegishv:svm:write-not-visible", ram[3].0[0]);
                    }
                    events.denied(regs.rip, address, Access::Execute).unwrap();
                    serial_write_line("aegishv:svm:guest-run-ok");
                    // QEMU isa-debug-exit. Real hardware falls through to HLT.
                    finish(0x10);
                } else {
                    fail("aegishv:svm:unexpected-npf", address);
                }
            }
            other => {
                serial_write_hex_u64("aegishv:svm:rip=0x", regs.rip);
                fail("aegishv:svm:unexpected-exit", other);
            }
        }
        guest.state.set_rax(regs.rax);
        if sequence == 4095 {
            events.stopped(regs.rip, sequence + 1, true).unwrap();
            finish(0x11);
        }
    }
    halt_loop()
}
