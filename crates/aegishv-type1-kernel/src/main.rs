#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use core::arch::x86_64::__cpuid_count;
#[cfg(target_os = "none")]
use core::arch::{asm, global_asm};
#[cfg(target_os = "none")]
use core::panic::PanicInfo;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

#[cfg(target_os = "none")]
global_asm!(
    include_str!("../../../boot/x86_64/entry.S"),
    options(att_syntax)
);

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
global_asm!(include_str!("../../../boot/x86_64/vmx_entry.S"));
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
global_asm!(include_str!("../../../boot/x86_64/host_tables.S"));

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_AWAITING_CPUID: u8 = 0;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_AWAITING_HLT: u8 = 1;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_COMPLETE: u8 = 2;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_FAILED: u8 = u8::MAX;

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
static VMX_TOY_EXIT_STATE: AtomicU8 = AtomicU8::new(VMX_TOY_AWAITING_CPUID);
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
static VMX_LAST_INSTRUCTION_ERROR: AtomicU32 = AtomicU32::new(0);
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
static HOST_LAST_EXCEPTION_VECTOR: AtomicU8 = AtomicU8::new(u8::MAX);
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
static HOST_LAST_EXCEPTION_CR2: AtomicU64 = AtomicU64::new(0);

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[repr(C)]
struct HostExceptionFrame {
    vector: u64,
    error_code: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
extern "C" {
    static __aegishv_host_gdt: u8;
    static __aegishv_host_idt: u8;
}

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
    if unsafe { host_tables_are_owned() } {
        serial_write_line(aegishv_type1_kernel::SERIAL_HOST_TABLES_OK_MARKER);
    } else {
        serial_write_line(aegishv_type1_kernel::SERIAL_HOST_TABLES_ERROR_MARKER);
        halt_loop();
    }
    let handoff = unsafe { read_limine_minimal_handoff() };
    let status = aegishv_type1_kernel::limine_minimal_handoff_status(handoff);
    if status.is_ready() {
        serial_write_line(status.serial_marker());
        let (
            backend_marker,
            preflight_marker,
            enable_marker,
            regions_marker,
            vmxon_marker,
            vmcs_marker,
        ) = runtime_markers(handoff);
        serial_write_line(backend_marker);
        serial_write_line(preflight_marker);
        serial_write_line(enable_marker);
        serial_write_line(regions_marker);
        serial_write_line(vmxon_marker);
        serial_write_line(vmcs_marker);
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
    &'static str,
) {
    let capability_report = aegishv_type1_kernel::type1_capabilities_from_snapshot(unsafe {
        read_type1_cpu_snapshot()
    });
    let backend = match aegishv_type1_kernel::select_type1_runtime_backend(
        aegishv_type1_kernel::Type1BackendRequest::Auto,
        capability_report.capabilities,
    ) {
        Ok(backend) => backend,
        Err(_) => return runtime_plan_error_markers(),
    };
    let (memory_entries, memory_entry_count) = match copy_limine_memory_entries(handoff) {
        Ok(entries) => entries,
        Err(()) => return runtime_plan_error_markers(),
    };
    let allocation = match aegishv_type1_kernel::allocate_type1_runtime_memory::<
        { aegishv_type1_kernel::TYPE1_MAX_MEMORY_MAP_ENTRIES },
    >(&memory_entries[..memory_entry_count], backend)
    {
        Ok(allocation) => allocation,
        Err(_) => return runtime_plan_error_markers(),
    };
    match aegishv_type1_kernel::plan_type1_runtime_with_memory(
        handoff,
        aegishv_type1_kernel::Type1BackendRequest::Auto,
        capability_report.capabilities,
        allocation.plan(),
    ) {
        Ok(plan) => {
            let backend_marker = plan.backend.serial_marker();
            let controls = unsafe { read_type1_control_snapshot(plan.backend) };
            match aegishv_type1_kernel::plan_type1_runtime_preflight(plan, controls) {
                Ok(preflight) => {
                    let enable_plan = aegishv_type1_kernel::plan_type1_runtime_enable(preflight);
                    match unsafe { apply_type1_enable_plan(enable_plan) } {
                        Ok(()) => {
                            let (regions_marker, vmxon_marker, vmcs_marker) =
                                match aegishv_type1_kernel::plan_type1_runtime_regions(
                                    plan,
                                    unsafe { read_type1_vmx_basic(plan.backend) },
                                ) {
                                    Ok(regions) => match unsafe {
                                        materialize_type1_runtime_regions(handoff, regions)
                                    } {
                                        Ok(()) => {
                                            let (vmxon_marker, vmcs_marker) =
                                                unsafe { run_type1_vmcs_load_cycle(regions) };
                                            (
                                                aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_OK_MARKER,
                                                vmxon_marker,
                                                vmcs_marker,
                                            )
                                        }
                                        Err(()) => {
                                            (
                                                aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_ERROR_MARKER,
                                                aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
                                                aegishv_type1_kernel::SERIAL_RUNTIME_VMCS_LOAD_ERROR_MARKER,
                                            )
                                        }
                                    },
                                    Err(_) => {
                                        (
                                            aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_ERROR_MARKER,
                                            aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
                                            aegishv_type1_kernel::SERIAL_RUNTIME_VMCS_LOAD_ERROR_MARKER,
                                        )
                                    }
                                };
                            (
                                backend_marker,
                                aegishv_type1_kernel::SERIAL_RUNTIME_PREFLIGHT_OK_MARKER,
                                aegishv_type1_kernel::SERIAL_RUNTIME_ENABLE_OK_MARKER,
                                regions_marker,
                                vmxon_marker,
                                vmcs_marker,
                            )
                        }
                        Err(()) => (
                            backend_marker,
                            aegishv_type1_kernel::SERIAL_RUNTIME_PREFLIGHT_OK_MARKER,
                            aegishv_type1_kernel::SERIAL_RUNTIME_ENABLE_ERROR_MARKER,
                            aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_ERROR_MARKER,
                            aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
                            aegishv_type1_kernel::SERIAL_RUNTIME_VMCS_LOAD_ERROR_MARKER,
                        ),
                    }
                }
                Err(_) => (
                    backend_marker,
                    aegishv_type1_kernel::SERIAL_RUNTIME_PREFLIGHT_ERROR_MARKER,
                    aegishv_type1_kernel::SERIAL_RUNTIME_ENABLE_ERROR_MARKER,
                    aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_ERROR_MARKER,
                    aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
                    aegishv_type1_kernel::SERIAL_RUNTIME_VMCS_LOAD_ERROR_MARKER,
                ),
            }
        }
        Err(_) => (
            aegishv_type1_kernel::SERIAL_RUNTIME_PLAN_ERROR_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_PREFLIGHT_ERROR_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_ENABLE_ERROR_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_ERROR_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_VMCS_LOAD_ERROR_MARKER,
        ),
    }
}

#[cfg(target_os = "none")]
const fn runtime_plan_error_markers() -> (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    (
        aegishv_type1_kernel::SERIAL_RUNTIME_PLAN_ERROR_MARKER,
        aegishv_type1_kernel::SERIAL_RUNTIME_PREFLIGHT_ERROR_MARKER,
        aegishv_type1_kernel::SERIAL_RUNTIME_ENABLE_ERROR_MARKER,
        aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_ERROR_MARKER,
        aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
        aegishv_type1_kernel::SERIAL_RUNTIME_VMCS_LOAD_ERROR_MARKER,
    )
}

#[cfg(target_os = "none")]
fn copy_limine_memory_entries(
    handoff: aegishv_type1_kernel::LimineMinimalHandoff,
) -> Result<
    (
        [aegishv_type1_boot::LimineMemmapEntry; aegishv_type1_kernel::TYPE1_MAX_MEMORY_MAP_ENTRIES],
        usize,
    ),
    (),
> {
    let count = usize::try_from(handoff.memmap_entry_count).map_err(|_| ())?;
    if count == 0 || count > aegishv_type1_kernel::TYPE1_MAX_MEMORY_MAP_ENTRIES {
        return Err(());
    }
    let table_size = count
        .checked_mul(core::mem::size_of::<
            *const aegishv_type1_boot::LimineMemmapEntry,
        >())
        .ok_or(())?;
    validate_limine_object_range(
        handoff.memmap_entries,
        table_size,
        core::mem::align_of::<*const aegishv_type1_boot::LimineMemmapEntry>(),
    )?;

    let table =
        handoff.memmap_entries as usize as *const *const aegishv_type1_boot::LimineMemmapEntry;
    let mut copied = [aegishv_type1_boot::LimineMemmapEntry::empty();
        aegishv_type1_kernel::TYPE1_MAX_MEMORY_MAP_ENTRIES];
    for (index, slot) in copied.iter_mut().take(count).enumerate() {
        // The accepted Limine response guarantees that entry_count is the
        // accessible extent of this live pointer array. The range check above
        // rules out arithmetic wrap and noncanonical endpoints; Limine owns and
        // keeps the mapped array alive until bootloader memory is reclaimed.
        let entry = unsafe { table.add(index).read_volatile() };
        if entry.is_null() {
            return Err(());
        }
        validate_limine_object_range(
            entry as usize as u64,
            core::mem::size_of::<aegishv_type1_boot::LimineMemmapEntry>(),
            core::mem::align_of::<aegishv_type1_boot::LimineMemmapEntry>(),
        )?;
        // Limine guarantees that each validated pointer addresses one complete
        // 24-byte protocol object which remains mapped and alive until the
        // bootloader-reclaimable ranges are explicitly reclaimed.
        *slot = unsafe { entry.read_volatile() };
    }
    Ok((copied, count))
}

#[cfg(any(target_os = "none", test))]
fn validate_limine_object_range(start: u64, size: usize, alignment: usize) -> Result<(), ()> {
    if size == 0 || alignment == 0 || start % alignment as u64 != 0 {
        return Err(());
    }
    let size = u64::try_from(size).map_err(|_| ())?;
    let last = start
        .checked_add(size)
        .and_then(|end| end.checked_sub(1))
        .ok_or(())?;
    if start > usize::MAX as u64
        || last > usize::MAX as u64
        || !aegishv_arch_x86::vmx::features::is_canonical_u64(start)
        || !aegishv_arch_x86::vmx::features::is_canonical_u64(last)
    {
        return Err(());
    }
    Ok(())
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

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn host_tables_are_owned() -> bool {
    let mut gdtr = DescriptorTablePointer { limit: 0, base: 0 };
    let mut idtr = DescriptorTablePointer { limit: 0, base: 0 };
    let (cs, ss, ds, es, fs, gs, tr): (u64, u64, u64, u64, u64, u64, u64);
    // SAFETY: both destinations are live ten-byte packed objects, and these
    // read-only instructions only snapshot the current CPU's descriptor state.
    unsafe {
        asm!(
            "sgdt [{}]",
            in(reg) core::ptr::addr_of_mut!(gdtr),
            options(nostack, preserves_flags)
        );
        asm!(
            "sidt [{}]",
            in(reg) core::ptr::addr_of_mut!(idtr),
            options(nostack, preserves_flags)
        );
        asm!("mov {value:x}, cs", value = out(reg) cs, options(nomem, nostack, preserves_flags));
        asm!("mov {value:x}, ss", value = out(reg) ss, options(nomem, nostack, preserves_flags));
        asm!("mov {value:x}, ds", value = out(reg) ds, options(nomem, nostack, preserves_flags));
        asm!("mov {value:x}, es", value = out(reg) es, options(nomem, nostack, preserves_flags));
        asm!("mov {value:x}, fs", value = out(reg) fs, options(nomem, nostack, preserves_flags));
        asm!("mov {value:x}, gs", value = out(reg) gs, options(nomem, nostack, preserves_flags));
        asm!("str {value:x}", value = out(reg) tr, options(nomem, nostack, preserves_flags));
    }
    // SAFETY: SGDT and SIDT initialized the complete packed fields above;
    // unaligned reads avoid creating references to packed u64 members.
    let (gdtr_base, idtr_base) = unsafe {
        (
            core::ptr::addr_of!(gdtr.base).read_unaligned(),
            core::ptr::addr_of!(idtr.base).read_unaligned(),
        )
    };
    // SAFETY: the linker-owned GDT is mapped and writable for the kernel
    // lifetime. LTR has changed the installed TSS descriptor to busy type 11.
    let tss_access = unsafe {
        core::ptr::addr_of!(__aegishv_host_gdt)
            .add(61)
            .read_volatile()
    };

    gdtr.limit == 0x47
        && idtr.limit == 0x0fff
        && gdtr_base == core::ptr::addr_of!(__aegishv_host_gdt) as u64
        && idtr_base == core::ptr::addr_of!(__aegishv_host_idt) as u64
        && cs as u16 == 0x28
        && ss as u16 == 0x30
        && ds as u16 == 0x30
        && es as u16 == 0x30
        && fs as u16 == 0x30
        && gs as u16 == 0x30
        && tr as u16 == 0x38
        && tss_access & 0x8f == 0x8b
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
unsafe fn run_type1_vmcs_load_cycle(
    regions: aegishv_type1_kernel::Type1RuntimeRegionMaterialization,
) -> (&'static str, &'static str) {
    let mut executor = aegishv_arch_x86::vmx::hardware::HardwareVmxInstructions::new();
    match unsafe { aegishv_type1_kernel::run_type1_vmcs_load_cycle_with(regions, &mut executor) } {
        Ok(aegishv_type1_kernel::Type1VmcsLoadCycleStatus::LoadedAndLeft) => (
            aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_OK_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_VMCS_LOAD_OK_MARKER,
        ),
        Ok(aegishv_type1_kernel::Type1VmcsLoadCycleStatus::Skipped) => (
            aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_SKIPPED_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_VMCS_LOAD_SKIPPED_MARKER,
        ),
        Err(aegishv_type1_kernel::Type1VmcsLoadCycleError::Vmclear(_))
        | Err(aegishv_type1_kernel::Type1VmcsLoadCycleError::Vmptrld(_)) => (
            aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_OK_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_VMCS_LOAD_ERROR_MARKER,
        ),
        Err(_) => (
            aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
            aegishv_type1_kernel::SERIAL_RUNTIME_VMCS_LOAD_ERROR_MARKER,
        ),
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

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[no_mangle]
extern "C" fn aegishv_type1_host_exception(frame: *const HostExceptionFrame, cr2: u64) -> ! {
    if !frame.is_null() {
        // SAFETY: the dedicated assembly stubs normalize vector and error code
        // before passing a live exception frame. The handler never returns.
        let vector = unsafe { (*frame).vector };
        HOST_LAST_EXCEPTION_VECTOR.store(vector as u8, Ordering::Release);
    }
    HOST_LAST_EXCEPTION_CR2.store(cr2, Ordering::Release);
    // SAFETY: direct COM1 initialization is lock-free and remains available on
    // the dedicated exception stacks without relying on interrupted state.
    unsafe { serial_init(COM1) };
    serial_write_line(aegishv_type1_kernel::SERIAL_HOST_EXCEPTION_MARKER);
    halt_loop()
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[no_mangle]
extern "C" fn aegishv_type1_host_fatal() -> ! {
    // SAFETY: the catch-all IDT entry deliberately reinitializes COM1 and does
    // not touch the unnormalized hardware exception frame.
    unsafe { serial_init(COM1) };
    serial_write_line(aegishv_type1_kernel::SERIAL_HOST_FATAL_MARKER);
    halt_loop()
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[no_mangle]
extern "C" fn aegishv_vmx_exit_dispatch(
    frame: *mut aegishv_arch_x86::vmx::exits::GeneralRegisters,
) -> u64 {
    if frame.is_null()
        || (frame as usize)
            % core::mem::align_of::<aegishv_arch_x86::vmx::exits::GeneralRegisters>()
            != 0
    {
        VMX_TOY_EXIT_STATE.store(VMX_TOY_FAILED, Ordering::Release);
        serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_EXIT_ERROR_MARKER);
        return 1;
    }
    let mut sequence = match VMX_TOY_EXIT_STATE.load(Ordering::Acquire) {
        VMX_TOY_AWAITING_CPUID => {
            aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingCpuid
        }
        VMX_TOY_AWAITING_HLT => aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingHlt,
        VMX_TOY_COMPLETE => aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::Complete,
        _ => {
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_EXIT_ERROR_MARKER);
            return 1;
        }
    };
    let contract = aegishv_arch_x86::vmx::toy_exit::ToyVmxExitContract {
        cpuid_rip: aegishv_type1_kernel::TYPE1_TOY_CPUID_RIP,
        hlt_rip: aegishv_type1_kernel::TYPE1_TOY_HLT_RIP,
    };
    let mut executor = aegishv_arch_x86::vmx::hardware::HardwareVmxInstructions::new();
    // SAFETY: this function is reached only at the VMCS HOST_RIP. The current
    // CPU therefore still owns the launched VMCS until the stop path runs VMXOFF.
    let mut access =
        unsafe { aegishv_arch_x86::vmx::toy_exit::InstructionVmcsAccess::new(&mut executor) };
    // SAFETY: the VM-exit assembly allocates one aligned GeneralRegisters
    // object on its private stack and passes its live address to this function.
    let registers = unsafe { &mut *frame };

    match aegishv_arch_x86::vmx::toy_exit::dispatch_toy_vmx_exit(
        &mut access,
        registers,
        &mut sequence,
        contract,
    ) {
        Ok(aegishv_arch_x86::vmx::toy_exit::ToyVmxExitAction::Resume)
            if sequence == aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingHlt =>
        {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_AWAITING_HLT, Ordering::Release);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_CPUID_EXIT_OK_MARKER);
            0
        }
        Ok(aegishv_arch_x86::vmx::toy_exit::ToyVmxExitAction::Stop)
            if sequence == aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::Complete =>
        {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_COMPLETE, Ordering::Release);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_HLT_EXIT_OK_MARKER);
            1
        }
        _ => {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_FAILED, Ordering::Release);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_EXIT_ERROR_MARKER);
            1
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[no_mangle]
extern "C" fn aegishv_vmx_exit_stop(
    _frame: *mut aegishv_arch_x86::vmx::exits::GeneralRegisters,
) -> ! {
    let complete = VMX_TOY_EXIT_STATE.load(Ordering::Acquire) == VMX_TOY_COMPLETE;
    let mut executor = aegishv_arch_x86::vmx::hardware::HardwareVmxInstructions::new();
    // SAFETY: HOST_RIP reaches this handler only while this CPU remains in VMX
    // operation with the toy VMCS current. No later VMX instruction is issued.
    let left_vmx = unsafe {
        aegishv_arch_x86::vmx::instructions::VmxInstructionExecutor::vmxoff(&mut executor)
    }
    .is_ok();
    if complete && left_vmx {
        serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_RUN_OK_MARKER);
    } else {
        serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_EXIT_ERROR_MARKER);
    }
    halt_loop()
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[no_mangle]
extern "C" fn aegishv_vmx_resume_failed(
    _frame: *mut aegishv_arch_x86::vmx::exits::GeneralRegisters,
    flags: u64,
) -> ! {
    VMX_TOY_EXIT_STATE.store(VMX_TOY_FAILED, Ordering::Release);
    serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_RESUME_ERROR_MARKER);
    let mut executor = aegishv_arch_x86::vmx::hardware::HardwareVmxInstructions::new();
    if flags & 1 == 0 && flags & (1 << 6) != 0 {
        // SAFETY: VMfailValid leaves the VMCS current, and the instruction
        // error field is architecturally readable until VMXOFF below.
        if let Ok(error) = unsafe {
            aegishv_arch_x86::vmx::instructions::VmxInstructionExecutor::vmread(
                &mut executor,
                aegishv_arch_x86::vmx::vmcs::VmcsField::VM_INSTRUCTION_ERROR.raw(),
            )
        } {
            VMX_LAST_INSTRUCTION_ERROR.store(error as u32, Ordering::Release);
        }
    }
    // SAFETY: a failed VMRESUME leaves the current VMCS owned by this CPU and
    // the processor in VMX operation, so VMXOFF is the terminal cleanup step.
    let _ = unsafe {
        aegishv_arch_x86::vmx::instructions::VmxInstructionExecutor::vmxoff(&mut executor)
    };
    halt_loop()
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

#[cfg(test)]
mod tests {
    use super::validate_limine_object_range;

    #[test]
    fn limine_object_range_rejects_wrap_and_noncanonical_endpoints() {
        assert!(validate_limine_object_range(0x1000, 24, 8).is_ok());
        assert!(validate_limine_object_range(u64::MAX - 7, 16, 8).is_err());
        assert!(validate_limine_object_range(0x0000_7fff_ffff_fff8, 16, 8).is_err());
        assert!(validate_limine_object_range(0x1001, 24, 8).is_err());
    }
}
