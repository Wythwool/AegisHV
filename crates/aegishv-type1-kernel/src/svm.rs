use core::arch::{asm, global_asm};
use core::ptr::{addr_of, addr_of_mut};

use aegishv_arch_x86::svm::exits::GeneralRegisters;
use aegishv_arch_x86::svm::vmcb::{
    Segment, Vmcb, INTERCEPT_CPUID, INTERCEPT_HLT, INTERCEPT_IOIO, INTERCEPT_MSR,
};
use aegishv_hypervisor_core::ids::HostPhysical;
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
static mut RAM: [Page; 8] = [const { Page([0; 512]) }; 8];
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
}

const NX: u64 = 1 << 63;
const EFER: u32 = 0xc000_0080;
const LSTAR: u32 = 0xc000_0082;

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
    for level in 0..3 {
        npt[level].0[0] = npt_pa + (level as u64 + 1) * 4096 | 7;
    }
    for page in 1..8 {
        let permissions = if page == 1 { 5 } else { 7 | NX };
        npt[3].0[page] = ram_pa + page as u64 * 4096 | permissions;
    }
    // Guest page tables use guest addresses; second-stage permissions remain authoritative.
    ram[4].0[0] = 0x5000 | 7;
    ram[5].0[0] = 0x6000 | 7;
    ram[6].0[0] = 0x7000 | 7;
    for page in 1..8 {
        ram[7].0[page] = page as u64 * 4096 | 7;
    }
    let start = addr_of!(aegishv_svm_guest_start);
    let length = addr_of!(aegishv_svm_guest_end) as usize - start as usize;
    assert!(length < 4096);
    unsafe { core::ptr::copy_nonoverlapping(start, addr_of_mut!(ram[1]).cast(), length) };

    guest.control.set_misc_intercepts(
        INTERCEPT_CPUID | INTERCEPT_HLT | INTERCEPT_IOIO | INTERCEPT_MSR | (0x7f << 32),
    );
    guest.control.set_intercept_cr_write(u16::MAX);
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
    guest.state.set_cr3(0x4000);
    guest.state.set_cr4(0x620);
    guest.state.set_efer(0x1d01); // SCE, LME, LMA, NXE, SVME.
    guest.state.set_debug_registers(0xffff_0ff0, 0x400);
    guest.state.set_rflags(2);
    guest.state.set_rip(0x1000);
    guest.state.set_rsp(0x3000);
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
    for sequence in 0..32u64 {
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
            0x72 => {
                regs.rax = 1;
                regs.rbx = u64::from(u32::from_le_bytes(*b"Aegi"));
                regs.rcx = u64::from(u32::from_le_bytes(*b"    "));
                regs.rdx = u64::from(u32::from_le_bytes(*b"sHV "));
                guest.state.set_rip(regs.rip + 2);
                serial_write_line("aegishv:svm:cpuid-exit");
            }
            0x7c if regs.rcx == u64::from(LSTAR) => {
                if guest.control.exit_info1() & 1 != 0 {
                    let target = (regs.rdx & 0xffff_ffff) << 32 | (regs.rax & 0xffff_ffff);
                    events
                        .syscall(regs.rip, LSTAR, target, target >> 12 == 1)
                        .unwrap();
                    if target >> 12 != 1 {
                        fail("aegishv:svm:syscall-target-denied", target);
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
                if !write_blocked && address == 0x1800 && info & 2 != 0 {
                    if ram[1].0[256] != 0 {
                        fail("aegishv:svm:write-escaped", ram[1].0[256]);
                    }
                    events.denied(regs.rip, address, true).unwrap();
                    write_blocked = true;
                    // Start the independent NX probe without executing the denied store.
                    guest.state.set_rip(
                        0x1000 + addr_of!(aegishv_svm_guest_write_exec) as u64 - start as u64,
                    );
                } else if write_blocked && address == 0x3000 && info & (1 << 4) != 0 {
                    if ram[3].0[0] & 0xff != 0xf4 {
                        fail("aegishv:svm:write-not-visible", ram[3].0[0]);
                    }
                    events.denied(regs.rip, address, false).unwrap();
                    serial_write_line("aegishv:svm:guest-run-ok");
                    // QEMU isa-debug-exit. Real hardware falls through to HLT.
                    unsafe {
                        asm!("out dx, eax", in("dx") 0xf4u16, in("eax") 0x10u32, options(nostack))
                    };
                    halt_loop();
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
        if sequence == 31 {
            fail("aegishv:svm:exit-budget", sequence);
        }
    }
    halt_loop()
}
