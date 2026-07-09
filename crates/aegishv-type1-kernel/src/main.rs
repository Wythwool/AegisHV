#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use core::arch::x86_64::__cpuid_count;
#[cfg(target_os = "none")]
use core::arch::{asm, global_asm};
#[cfg(target_os = "none")]
use core::panic::PanicInfo;

#[cfg(target_os = "none")]
global_asm!(
    include_str!("../../../boot/x86_64/entry.S"),
    options(att_syntax)
);

#[cfg(target_os = "none")]
#[used]
#[link_section = ".limine_requests_start"]
static LIMINE_REQUESTS_START: [u64; 4] = aegishv_type1_kernel::LIMINE_REQUESTS_START_MARKER;

#[cfg(target_os = "none")]
#[used]
#[link_section = ".limine_requests"]
static mut LIMINE_BASE_REVISION_TAG: [u64; 3] = aegishv_type1_kernel::limine_base_revision_tag();

#[cfg(target_os = "none")]
#[used]
#[link_section = ".limine_requests"]
static mut LIMINE_BOOTLOADER_INFO_REQUEST: aegishv_type1_kernel::LimineRequest =
    aegishv_type1_kernel::LimineRequest::new(
        aegishv_type1_kernel::LIMINE_BOOTLOADER_INFO_REQUEST_ID,
    );

#[cfg(target_os = "none")]
#[used]
#[link_section = ".limine_requests"]
static mut LIMINE_EXECUTABLE_CMDLINE_REQUEST: aegishv_type1_kernel::LimineRequest =
    aegishv_type1_kernel::LimineRequest::new(
        aegishv_type1_kernel::LIMINE_EXECUTABLE_CMDLINE_REQUEST_ID,
    );

#[cfg(target_os = "none")]
#[used]
#[link_section = ".limine_requests"]
static mut LIMINE_HHDM_REQUEST: aegishv_type1_kernel::LimineRequest =
    aegishv_type1_kernel::LimineRequest::new(aegishv_type1_kernel::LIMINE_HHDM_REQUEST_ID);

#[cfg(target_os = "none")]
#[used]
#[link_section = ".limine_requests"]
static mut LIMINE_MEMMAP_REQUEST: aegishv_type1_kernel::LimineRequest =
    aegishv_type1_kernel::LimineRequest::new(aegishv_type1_kernel::LIMINE_MEMMAP_REQUEST_ID);

#[cfg(target_os = "none")]
#[used]
#[link_section = ".limine_requests"]
static mut LIMINE_RSDP_REQUEST: aegishv_type1_kernel::LimineRequest =
    aegishv_type1_kernel::LimineRequest::new(aegishv_type1_kernel::LIMINE_RSDP_REQUEST_ID);

#[cfg(target_os = "none")]
#[used]
#[link_section = ".limine_requests"]
static mut LIMINE_EXECUTABLE_ADDRESS_REQUEST: aegishv_type1_kernel::LimineRequest =
    aegishv_type1_kernel::LimineRequest::new(
        aegishv_type1_kernel::LIMINE_EXECUTABLE_ADDRESS_REQUEST_ID,
    );

#[cfg(target_os = "none")]
#[used]
#[link_section = ".limine_requests_end"]
static LIMINE_REQUESTS_END: [u64; 2] = aegishv_type1_kernel::LIMINE_REQUESTS_END_MARKER;

#[cfg(target_os = "none")]
const COM1: u16 = aegishv_type1_boot::layout::SERIAL_COM1_PORT;

#[cfg(target_os = "none")]
#[no_mangle]
pub extern "C" fn aegishv_type1_rust_entry() -> ! {
    unsafe {
        serial_init(COM1);
    }
    let handoff = unsafe { read_limine_minimal_handoff() };
    let status = aegishv_type1_kernel::limine_minimal_handoff_status(handoff);
    if status.is_ready() {
        serial_write_line(status.serial_marker());
        let (backend_marker, preflight_marker, enable_marker, regions_marker, vmxon_marker) =
            runtime_markers(handoff);
        serial_write_line(backend_marker);
        serial_write_line(preflight_marker);
        serial_write_line(enable_marker);
        serial_write_line(regions_marker);
        serial_write_line(vmxon_marker);
    } else {
        serial_write_line(aegishv_type1_kernel::SERIAL_LIMINE_MISSING_MARKER);
        serial_write_line(status.serial_marker());
    }
    halt_loop()
}

#[cfg(target_os = "none")]
fn runtime_markers(
    handoff: aegishv_type1_kernel::LimineMinimalHandoff,
) -> (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    let capability_report = aegishv_type1_kernel::type1_capabilities_from_snapshot(unsafe {
        read_type1_cpu_snapshot()
    });
    match aegishv_type1_kernel::plan_type1_runtime(
        handoff,
        aegishv_type1_kernel::Type1BackendRequest::Auto,
        capability_report.capabilities,
    ) {
        Ok(plan) => {
            let backend_marker = plan.backend.serial_marker();
            let controls = unsafe { read_type1_control_snapshot(plan.backend) };
            match aegishv_type1_kernel::plan_type1_runtime_preflight(plan, controls) {
                Ok(preflight) => {
                    let enable_plan = aegishv_type1_kernel::plan_type1_runtime_enable(preflight);
                    match unsafe { apply_type1_enable_plan(enable_plan) } {
                        Ok(()) => {
                            let (regions_marker, vmxon_marker) =
                                match aegishv_type1_kernel::plan_type1_runtime_regions(
                                    plan,
                                    unsafe { read_type1_vmx_basic(plan.backend) },
                                ) {
                                    Ok(regions) => match unsafe {
                                        materialize_type1_runtime_regions(handoff, regions)
                                    } {
                                        Ok(()) => (
                                            aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_OK_MARKER,
                                            unsafe { run_type1_vmxon_cycle(regions) },
                                        ),
                                        Err(()) => {
                                            (
                                                aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_ERROR_MARKER,
                                                aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
                                            )
                                        }
                                    },
                                    Err(_) => {
                                        (
                                            aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_ERROR_MARKER,
                                            aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
                                        )
                                    }
                                };
                            (
                                backend_marker,
                                aegishv_type1_kernel::SERIAL_RUNTIME_PREFLIGHT_OK_MARKER,
                                aegishv_type1_kernel::SERIAL_RUNTIME_ENABLE_OK_MARKER,
                                regions_marker,
                                vmxon_marker,
                            )
                        }
                        Err(()) => (
                            backend_marker,
                            aegishv_type1_kernel::SERIAL_RUNTIME_PREFLIGHT_OK_MARKER,
                            aegishv_type1_kernel::SERIAL_RUNTIME_ENABLE_ERROR_MARKER,
                            aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_ERROR_MARKER,
                            aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
                        ),
                    }
                }
                Err(_) => (
                    backend_marker,
                    aegishv_type1_kernel::SERIAL_RUNTIME_PREFLIGHT_ERROR_MARKER,
                    aegishv_type1_kernel::SERIAL_RUNTIME_ENABLE_ERROR_MARKER,
                    aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_ERROR_MARKER,
                    aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
                ),
            }
        }
        Err(_) => (
            aegishv_type1_kernel::SERIAL_RUNTIME_PLAN_ERROR_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_PREFLIGHT_ERROR_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_ENABLE_ERROR_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_ERROR_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
        ),
    }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn read_type1_cpu_snapshot() -> aegishv_type1_kernel::Type1CpuSnapshot {
    let vendor_leaf = __cpuid_count(aegishv_type1_kernel::CPUID_VENDOR_LEAF, 0);
    let vendor = aegishv_type1_kernel::Type1CpuVendor::from_cpuid0(
        vendor_leaf.ebx,
        vendor_leaf.ecx,
        vendor_leaf.edx,
    );
    let feature_leaf = __cpuid_count(aegishv_type1_kernel::CPUID_FEATURE_LEAF, 0);
    let feature_control_msr = if vendor == aegishv_type1_kernel::Type1CpuVendor::Intel
        && feature_leaf.ecx & aegishv_arch_x86::vmx::features::CPUID_LEAF1_ECX_VMX != 0
    {
        read_msr(aegishv_type1_kernel::IA32_FEATURE_CONTROL_MSR)
    } else {
        0
    };

    let extended_limit = __cpuid_count(aegishv_type1_kernel::CPUID_EXTENDED_LIMIT_LEAF, 0);
    let extended_feature =
        if extended_limit.eax >= aegishv_type1_kernel::CPUID_EXTENDED_FEATURE_LEAF {
            __cpuid_count(aegishv_type1_kernel::CPUID_EXTENDED_FEATURE_LEAF, 0)
        } else {
            __cpuid_count(0, 0)
        };
    let svm_leaf = if extended_limit.eax >= aegishv_type1_kernel::CPUID_SVM_FEATURE_LEAF {
        __cpuid_count(aegishv_type1_kernel::CPUID_SVM_FEATURE_LEAF, 0)
    } else {
        __cpuid_count(0, 0)
    };

    aegishv_type1_kernel::Type1CpuSnapshot::from_raw(
        vendor,
        feature_leaf.ecx,
        feature_control_msr,
        extended_limit.eax,
        extended_feature.ecx,
        svm_leaf.ebx,
        svm_leaf.edx,
    )
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    asm!(
        "rdmsr",
        in("ecx") msr,
        out("eax") low,
        out("edx") high,
        options(nomem, nostack, preserves_flags)
    );
    ((high as u64) << 32) | low as u64
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn read_type1_vmx_basic(
    backend: aegishv_type1_kernel::Type1RuntimeBackend,
) -> Option<aegishv_type1_kernel::Type1VmxBasic> {
    if backend == aegishv_type1_kernel::Type1RuntimeBackend::IntelVmx {
        Some(aegishv_type1_kernel::Type1VmxBasic::new(read_msr(
            aegishv_type1_kernel::IA32_VMX_BASIC_MSR,
        )))
    } else {
        None
    }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn read_type1_control_snapshot(
    backend: aegishv_type1_kernel::Type1RuntimeBackend,
) -> aegishv_type1_kernel::Type1ControlSnapshot {
    let (vmx_cr0_fixed0, vmx_cr0_fixed1, vmx_cr4_fixed0, vmx_cr4_fixed1) =
        if backend == aegishv_type1_kernel::Type1RuntimeBackend::IntelVmx {
            (
                read_msr(aegishv_type1_kernel::IA32_VMX_CR0_FIXED0_MSR),
                read_msr(aegishv_type1_kernel::IA32_VMX_CR0_FIXED1_MSR),
                read_msr(aegishv_type1_kernel::IA32_VMX_CR4_FIXED0_MSR),
                read_msr(aegishv_type1_kernel::IA32_VMX_CR4_FIXED1_MSR),
            )
        } else {
            (0, u64::MAX, 0, u64::MAX)
        };
    aegishv_type1_kernel::Type1ControlSnapshot {
        cr0: read_cr0(),
        cr4: read_cr4(),
        efer: read_msr(aegishv_type1_kernel::IA32_EFER_MSR),
        vmx_cr0_fixed0,
        vmx_cr0_fixed1,
        vmx_cr4_fixed0,
        vmx_cr4_fixed1,
    }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn read_cr0() -> u64 {
    let value: u64;
    asm!(
        "mov {}, cr0",
        out(reg) value,
        options(nomem, nostack, preserves_flags)
    );
    value
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn read_cr4() -> u64 {
    let value: u64;
    asm!(
        "mov {}, cr4",
        out(reg) value,
        options(nomem, nostack, preserves_flags)
    );
    value
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn apply_type1_enable_plan(
    plan: aegishv_type1_kernel::Type1RuntimeEnablePlan,
) -> Result<(), ()> {
    if let Some(value) = plan.cr0 {
        write_cr0(value);
    }
    if let Some(value) = plan.cr4 {
        write_cr4(value);
    }
    if let Some(value) = plan.efer {
        write_msr(aegishv_type1_kernel::IA32_EFER_MSR, value);
    }
    Ok(())
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn write_cr0(value: u64) {
    asm!(
        "mov cr0, {}",
        in(reg) value,
        options(nostack, preserves_flags)
    );
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn write_cr4(value: u64) {
    asm!(
        "mov cr4, {}",
        in(reg) value,
        options(nostack, preserves_flags)
    );
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    asm!(
        "wrmsr",
        in("ecx") msr,
        in("eax") low,
        in("edx") high,
        options(nostack, preserves_flags)
    );
}

#[cfg(target_os = "none")]
unsafe fn materialize_type1_runtime_regions(
    handoff: aegishv_type1_kernel::LimineMinimalHandoff,
    regions: aegishv_type1_kernel::Type1RuntimeRegionMaterialization,
) -> Result<(), ()> {
    match regions.backend {
        aegishv_type1_kernel::Type1RuntimeBackend::IntelVmx => {
            let vmxon_revision = match regions.vmxon_revision {
                Some(value) => value,
                None => return Err(()),
            };
            let vmcs_revision = match regions.vmcs_revision {
                Some(value) => value,
                None => return Err(()),
            };
            let vmxon = physical_to_hhdm(regions.vmxon_physical, handoff.hhdm_offset)?;
            let vmcs = physical_to_hhdm(regions.vmcs_physical, handoff.hhdm_offset)?;
            write_runtime_page(vmxon);
            write_runtime_revision(vmxon, vmxon_revision);
            write_runtime_page(vmcs);
            write_runtime_revision(vmcs, vmcs_revision);
            Ok(())
        }
        aegishv_type1_kernel::Type1RuntimeBackend::AmdSvm => {
            let vmcb = physical_to_hhdm(regions.svm_vmcb_physical, handoff.hhdm_offset)?;
            write_runtime_page(vmcb);
            Ok(())
        }
        aegishv_type1_kernel::Type1RuntimeBackend::None => Ok(()),
    }
}

#[cfg(target_os = "none")]
unsafe fn run_type1_vmxon_cycle(
    regions: aegishv_type1_kernel::Type1RuntimeRegionMaterialization,
) -> &'static str {
    let mut executor = aegishv_arch_x86::vmx::hardware::HardwareVmxInstructions::new();
    match unsafe { aegishv_type1_kernel::run_type1_vmxon_cycle_with(regions, &mut executor) } {
        Ok(aegishv_type1_kernel::Type1VmxonCycleStatus::EnteredAndLeft) => {
            aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_OK_MARKER
        }
        Ok(aegishv_type1_kernel::Type1VmxonCycleStatus::Skipped) => {
            aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_SKIPPED_MARKER
        }
        Err(_) => aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
    }
}

#[cfg(target_os = "none")]
fn physical_to_hhdm(physical: u64, hhdm_offset: u64) -> Result<usize, ()> {
    let virtual_address = match physical.checked_add(hhdm_offset) {
        Some(value) => value,
        None => return Err(()),
    };
    if virtual_address > usize::MAX as u64 {
        return Err(());
    }
    Ok(virtual_address as usize)
}

#[cfg(target_os = "none")]
unsafe fn write_runtime_page(address: usize) {
    let page = address as *mut u8;
    let mut offset = 0usize;
    while offset < aegishv_type1_kernel::TYPE1_RUNTIME_PAGE_SIZE as usize {
        page.add(offset).write_volatile(0);
        offset += 1;
    }
}

#[cfg(target_os = "none")]
unsafe fn write_runtime_revision(address: usize, revision: u32) {
    let page = address as *mut u8;
    let bytes = revision.to_le_bytes();
    let mut offset = 0usize;
    while offset < bytes.len() {
        page.add(offset).write_volatile(bytes[offset]);
        offset += 1;
    }
}

#[cfg(all(target_os = "none", not(target_arch = "x86_64")))]
unsafe fn read_type1_cpu_snapshot() -> aegishv_type1_kernel::Type1CpuSnapshot {
    aegishv_type1_kernel::Type1CpuSnapshot::empty()
}

#[cfg(all(target_os = "none", not(target_arch = "x86_64")))]
unsafe fn read_type1_vmx_basic(
    _backend: aegishv_type1_kernel::Type1RuntimeBackend,
) -> Option<aegishv_type1_kernel::Type1VmxBasic> {
    None
}

#[cfg(all(target_os = "none", not(target_arch = "x86_64")))]
unsafe fn read_type1_control_snapshot(
    _backend: aegishv_type1_kernel::Type1RuntimeBackend,
) -> aegishv_type1_kernel::Type1ControlSnapshot {
    aegishv_type1_kernel::Type1ControlSnapshot::empty()
}

#[cfg(all(target_os = "none", not(target_arch = "x86_64")))]
unsafe fn apply_type1_enable_plan(
    plan: aegishv_type1_kernel::Type1RuntimeEnablePlan,
) -> Result<(), ()> {
    if plan.has_writes() {
        Err(())
    } else {
        Ok(())
    }
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    unsafe {
        serial_init(COM1);
    }
    serial_write_line(aegishv_type1_kernel::SERIAL_PANIC_MARKER);
    halt_loop()
}

#[cfg(target_os = "none")]
fn serial_write_line(text: &str) {
    let mut line = [0u8; 64];
    if let Some(len) = aegishv_type1_kernel::marker_line(text, &mut line) {
        for byte in &line[..len] {
            unsafe {
                serial_write_byte(COM1, *byte);
            }
        }
    }
}

#[cfg(target_os = "none")]
unsafe fn read_limine_minimal_handoff() -> aegishv_type1_kernel::LimineMinimalHandoff {
    let base_revision = core::ptr::addr_of!(LIMINE_BASE_REVISION_TAG)
        .cast::<u64>()
        .add(2)
        .read_volatile();
    let hhdm_response = core::ptr::addr_of!(LIMINE_HHDM_REQUEST.response).read_volatile();
    let memmap_response = core::ptr::addr_of!(LIMINE_MEMMAP_REQUEST.response).read_volatile();
    let executable_address_response =
        core::ptr::addr_of!(LIMINE_EXECUTABLE_ADDRESS_REQUEST.response).read_volatile();

    let hhdm_revision = if hhdm_response == 0 {
        0
    } else {
        read_limine_response_u64(
            hhdm_response,
            aegishv_type1_kernel::LIMINE_RESPONSE_REVISION_OFFSET,
        )
    };
    let hhdm_offset = if hhdm_response == 0 {
        0
    } else {
        read_limine_response_u64(
            hhdm_response,
            aegishv_type1_kernel::LIMINE_HHDM_OFFSET_OFFSET,
        )
    };
    let memmap_revision = if memmap_response == 0 {
        0
    } else {
        read_limine_response_u64(
            memmap_response,
            aegishv_type1_kernel::LIMINE_RESPONSE_REVISION_OFFSET,
        )
    };
    let memmap_entry_count = if memmap_response == 0 {
        0
    } else {
        read_limine_response_u64(
            memmap_response,
            aegishv_type1_kernel::LIMINE_MEMMAP_ENTRY_COUNT_OFFSET,
        )
    };
    let memmap_entries = if memmap_response == 0 {
        0
    } else {
        read_limine_response_u64(
            memmap_response,
            aegishv_type1_kernel::LIMINE_MEMMAP_ENTRIES_OFFSET,
        )
    };
    let executable_address_revision = if executable_address_response == 0 {
        0
    } else {
        read_limine_response_u64(
            executable_address_response,
            aegishv_type1_kernel::LIMINE_RESPONSE_REVISION_OFFSET,
        )
    };
    let executable_physical_base = if executable_address_response == 0 {
        0
    } else {
        read_limine_response_u64(
            executable_address_response,
            aegishv_type1_kernel::LIMINE_EXECUTABLE_PHYSICAL_BASE_OFFSET,
        )
    };
    let executable_virtual_base = if executable_address_response == 0 {
        0
    } else {
        read_limine_response_u64(
            executable_address_response,
            aegishv_type1_kernel::LIMINE_EXECUTABLE_VIRTUAL_BASE_OFFSET,
        )
    };

    aegishv_type1_kernel::LimineMinimalHandoff {
        base_revision_value: base_revision,
        hhdm_response,
        hhdm_revision,
        hhdm_offset,
        memmap_response,
        memmap_revision,
        memmap_entry_count,
        memmap_entries,
        executable_address_response,
        executable_address_revision,
        executable_physical_base,
        executable_virtual_base,
    }
}

#[cfg(target_os = "none")]
unsafe fn read_limine_response_u64(response: u64, offset: usize) -> u64 {
    (response as usize as *const u8)
        .add(offset)
        .cast::<u64>()
        .read_volatile()
}

#[cfg(target_os = "none")]
unsafe fn serial_init(port: u16) {
    outb(port + 1, 0x00);
    outb(port + 3, 0x80);
    outb(port, 0x03);
    outb(port + 1, 0x00);
    outb(port + 3, 0x03);
    outb(port + 2, 0xc7);
    outb(port + 4, 0x0b);
}

#[cfg(target_os = "none")]
unsafe fn serial_write_byte(port: u16, byte: u8) {
    while inb(port + 5) & 0x20 == 0 {}
    outb(port, byte);
}

#[cfg(target_os = "none")]
unsafe fn outb(port: u16, value: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}

#[cfg(target_os = "none")]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!(
        "in al, dx",
        in("dx") port,
        out("al") value,
        options(nomem, nostack, preserves_flags)
    );
    value
}

#[cfg(target_os = "none")]
fn halt_loop() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("{}", aegishv_type1_kernel::SERIAL_READY_MARKER);
}
