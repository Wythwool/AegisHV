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
const VMX_TOY_AWAITING_PREEMPTION: u8 = 0;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_AWAITING_DEADLINE_PROBE: u8 = 1;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_AWAITING_IO: u8 = 2;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_AWAITING_IO_BITMAP_B: u8 = 3;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_AWAITING_CPUID: u8 = 4;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_AWAITING_RDMSR: u8 = 5;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_AWAITING_X87_GUARD: u8 = 6;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_AWAITING_SIMD_GUARD: u8 = 7;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_AWAITING_UD_DELIVERY: u8 = 8;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_COMPLETE: u8 = 9;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const VMX_TOY_FAILED: u8 = u8::MAX;

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const X86_CR0_WRITE_PROTECT: u64 = 1 << 16;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const X86_CR4_PAGE_GLOBAL_ENABLE: u64 = 1 << 7;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const X86_CR4_FIVE_LEVEL_PAGING: u64 = 1 << 12;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const X86_EFER_NO_EXECUTE_ENABLE: u64 = 1 << 11;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const X86_CPUID_EXTENDED_NX: u32 = 1 << 20;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
const X86_CPUID_ADDRESS_SIZE_LEAF: u32 = 0x8000_0008;

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
static VMX_TOY_EXIT_STATE: AtomicU8 = AtomicU8::new(VMX_TOY_AWAITING_PREEMPTION);
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
static VMX_TOY_PREEMPTION_RELOAD: AtomicU32 = AtomicU32::new(0);
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
static VMX_EXPECTED_HOST_PAT: AtomicU64 = AtomicU64::new(0);
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
static VMX_EXPECTED_GUEST_CR0: AtomicU64 = AtomicU64::new(0);
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
static VMX_EXPECTED_GUEST_CR4: AtomicU64 = AtomicU64::new(0);
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
    static __aegishv_kernel_start: u8;
    static __aegishv_kernel_end: u8;
    static __aegishv_text_start: u8;
    static __aegishv_text_end: u8;
    static __aegishv_rodata_start: u8;
    static __aegishv_rodata_end: u8;
    static __aegishv_writable_start: u8;
    static __aegishv_writable_end: u8;
    static mut __aegishv_host_page_tables_start: u8;
    static __aegishv_host_page_tables_end: u8;
    static __aegishv_double_fault_guard_bottom: u8;
    static __aegishv_double_fault_guard_top: u8;
    static __aegishv_double_fault_stack_bottom: u8;
    static __aegishv_double_fault_stack_top: u8;
    static __aegishv_nmi_guard_bottom: u8;
    static __aegishv_nmi_guard_top: u8;
    static __aegishv_nmi_stack_bottom: u8;
    static __aegishv_nmi_stack_top: u8;
    static __aegishv_machine_check_guard_bottom: u8;
    static __aegishv_machine_check_guard_top: u8;
    static __aegishv_machine_check_stack_bottom: u8;
    static __aegishv_machine_check_stack_top: u8;
    static __aegishv_vmx_exit_guard_bottom: u8;
    static __aegishv_vmx_exit_guard_top: u8;
    static __aegishv_vmx_exit_stack_bottom: u8;
    static __aegishv_host_gdt: [u8; 72];
    static __aegishv_host_idt: [u8; 4096];
    static __aegishv_host_tss: [u8; 104];
    static __aegishv_vmx_exit_stack_top: u8;
    static __aegishv_boot_stack_guard_bottom: u8;
    static __aegishv_boot_stack_guard_top: u8;
    static __aegishv_boot_stack_bottom: u8;
    static __aegishv_boot_stack_top: u8;
    fn aegishv_vmx_vmexit_entry();
    fn aegishv_vmx_launch(frame: *const aegishv_arch_x86::vmx::exits::GeneralRegisters) -> u64;
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
struct HhdmPageWriter {
    offset: u64,
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
impl HhdmPageWriter {
    fn page_pointer(
        &self,
        page: aegishv_hypervisor_core::ids::HostPhysical,
    ) -> Result<*mut u8, aegishv_hypervisor_core::error::CoreError> {
        let address = hhdm_page_virtual_address(page.get(), self.offset).map_err(|()| {
            aegishv_hypervisor_core::error::CoreError::new(
                aegishv_hypervisor_core::error::CoreErrorKind::InvalidAddress,
                "HHDM page is not an aligned canonical host range",
            )
        })?;
        Ok(address as *mut u8)
    }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
impl aegishv_type1_kernel::Type1PhysicalPageWriter for HhdmPageWriter {
    fn zero_page(
        &mut self,
        page: aegishv_hypervisor_core::ids::HostPhysical,
    ) -> Result<(), aegishv_hypervisor_core::error::CoreError> {
        let pointer = self.page_pointer(page)?;
        for offset in 0..aegishv_type1_kernel::TYPE1_RUNTIME_PAGE_SIZE as usize {
            // SAFETY: Limine's HHDM maps the allocator-owned physical page for
            // the handoff lifetime, and every offset remains inside that page.
            unsafe { pointer.add(offset).write_volatile(0) };
        }
        Ok(())
    }

    fn write_u64(
        &mut self,
        page: aegishv_hypervisor_core::ids::HostPhysical,
        index: u16,
        value: u64,
    ) -> Result<(), aegishv_hypervisor_core::error::CoreError> {
        if usize::from(index) >= 512 {
            return Err(aegishv_hypervisor_core::error::CoreError::new(
                aegishv_hypervisor_core::error::CoreErrorKind::InvalidArgument,
                "page-table write index is outside a 4K page",
            ));
        }
        let pointer = self.page_pointer(page)?.cast::<u64>();
        // SAFETY: the checked index names one aligned u64 slot in the live,
        // zeroed allocator-owned page mapped through Limine's HHDM.
        unsafe { pointer.add(usize::from(index)).write_volatile(value) };
        Ok(())
    }

    fn write_bytes(
        &mut self,
        page: aegishv_hypervisor_core::ids::HostPhysical,
        offset: usize,
        bytes: &[u8],
    ) -> Result<(), aegishv_hypervisor_core::error::CoreError> {
        let end = offset.checked_add(bytes.len()).ok_or(
            aegishv_hypervisor_core::error::CoreError::new(
                aegishv_hypervisor_core::error::CoreErrorKind::InvalidArgument,
                "guest page byte range overflowed",
            ),
        )?;
        if end > aegishv_type1_kernel::TYPE1_RUNTIME_PAGE_SIZE as usize {
            return Err(aegishv_hypervisor_core::error::CoreError::new(
                aegishv_hypervisor_core::error::CoreErrorKind::InvalidArgument,
                "guest page byte range is outside a 4K page",
            ));
        }
        let pointer = self.page_pointer(page)?;
        for (index, byte) in bytes.iter().copied().enumerate() {
            // SAFETY: the checked byte range stays within the allocator-owned
            // HHDM page and volatile writes publish the planned guest bytes.
            unsafe { pointer.add(offset + index).write_volatile(byte) };
        }
        Ok(())
    }

    fn read_u8(
        &mut self,
        page: aegishv_hypervisor_core::ids::HostPhysical,
        offset: usize,
    ) -> Result<u8, aegishv_hypervisor_core::error::CoreError> {
        if offset >= aegishv_type1_kernel::TYPE1_RUNTIME_PAGE_SIZE as usize {
            return Err(aegishv_hypervisor_core::error::CoreError::new(
                aegishv_hypervisor_core::error::CoreErrorKind::InvalidArgument,
                "guest page byte offset is outside a 4K page",
            ));
        }
        let pointer = self.page_pointer(page)?;
        // SAFETY: the checked offset stays within the allocator-owned HHDM
        // page. A volatile read verifies the guest image or bitmap byte.
        Ok(unsafe { pointer.add(offset).read_volatile() })
    }
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
    // SAFETY: entry assembly disabled interrupts, installed a valid stack, and
    // COM1 is the fixed early console owned by this BSP.
    unsafe {
        serial_init(COM1);
    }
    // SAFETY: entry assembly has completed the owned table installation; this
    // read-only snapshot verifies the exact linker-owned objects and selectors.
    if unsafe { host_tables_are_owned() } {
        serial_write_line(aegishv_type1_kernel::SERIAL_HOST_TABLES_OK_MARKER);
    } else {
        serial_write_line(aegishv_type1_kernel::SERIAL_HOST_TABLES_ERROR_MARKER);
        halt_loop();
    }
    // SAFETY: the Limine request block remains mapped and response pointers are
    // range-checked before their protocol prefixes are read.
    let handoff = unsafe { read_limine_minimal_handoff() };
    let status = aegishv_type1_kernel::limine_minimal_handoff_status(handoff);
    if status.is_ready() {
        serial_write_line(status.serial_marker());
        let (
            (
                backend_marker,
                preflight_marker,
                enable_marker,
                regions_marker,
                vmxon_marker,
                vmcs_marker,
            ),
            mut runtime_memory,
        ) = runtime_markers(handoff);
        serial_write_line(backend_marker);
        serial_write_line(preflight_marker);
        serial_write_line(enable_marker);
        serial_write_line(regions_marker);
        serial_write_line(vmxon_marker);
        serial_write_line(vmcs_marker);
        if backend_marker == aegishv_type1_kernel::SERIAL_RUNTIME_BACKEND_VMX_MARKER
            && preflight_marker == aegishv_type1_kernel::SERIAL_RUNTIME_PREFLIGHT_OK_MARKER
            && enable_marker == aegishv_type1_kernel::SERIAL_RUNTIME_ENABLE_OK_MARKER
            && regions_marker == aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_OK_MARKER
            && vmxon_marker == aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_OK_MARKER
            && vmcs_marker == aegishv_type1_kernel::SERIAL_RUNTIME_VMCS_LOAD_OK_MARKER
        {
            let runtime_memory = match runtime_memory.as_mut() {
                Some(allocation) => allocation,
                None => halt_loop(),
            };
            // SAFETY: every preceding marker corresponds to successful host
            // table, capability, VMXON, and VMCS-load validation on this BSP.
            unsafe { run_type1_vmx_toy_guest(handoff, runtime_memory) };
        }
    } else {
        serial_write_line(aegishv_type1_kernel::SERIAL_LIMINE_MISSING_MARKER);
        serial_write_line(status.serial_marker());
    }
    halt_loop()
}

#[cfg(target_os = "none")]
type RuntimeMarkerSet = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

#[cfg(target_os = "none")]
struct PreparedRuntimeMemory {
    allocation: aegishv_type1_kernel::Type1RuntimeMemoryAllocation,
    inherited_cr3: u64,
    inherited_cr3_root: aegishv_type1_kernel::Type1PhysicalReservation,
}

#[cfg(target_os = "none")]
fn runtime_markers(
    handoff: aegishv_type1_kernel::LimineMinimalHandoff,
) -> (RuntimeMarkerSet, Option<PreparedRuntimeMemory>) {
    // SAFETY: CPUID is available in x86-64 mode and RDMSR is reached only for
    // the vendor capability advertised by CPUID.
    let capability_report = aegishv_type1_kernel::type1_capabilities_from_snapshot(unsafe {
        read_type1_cpu_snapshot()
    });
    let backend = match aegishv_type1_kernel::select_type1_runtime_backend(
        aegishv_type1_kernel::Type1BackendRequest::Auto,
        capability_report.capabilities,
    ) {
        Ok(backend) => backend,
        Err(_) => return (runtime_plan_error_markers(), None),
    };
    let (memory_entries, memory_entry_count) = match copy_limine_memory_entries(handoff) {
        Ok(entries) => entries,
        Err(()) => return (runtime_plan_error_markers(), None),
    };
    let kernel_reservation = match type1_kernel_physical_reservation(handoff) {
        Ok(reservation) => reservation,
        Err(()) => return (runtime_plan_error_markers(), None),
    };
    // SAFETY: this is a read-only snapshot of the paging root currently
    // keeping the bootloader-provided address space live on the BSP.
    let inherited_cr3 = unsafe { read_cr3() };
    let active_cr3_root =
        match aegishv_type1_kernel::inherited_x86_cr3_root_reservation(inherited_cr3) {
            Ok(reservation) => reservation,
            Err(_) => return (runtime_plan_error_markers(), None),
        };
    let allocation = match aegishv_type1_kernel::allocate_type1_runtime_memory_with_reservations::<
        { aegishv_type1_kernel::TYPE1_MAX_MEMORY_MAP_ENTRIES },
    >(
        &memory_entries[..memory_entry_count],
        backend,
        &[kernel_reservation, active_cr3_root],
    ) {
        Ok(allocation) => allocation,
        Err(_) => return (runtime_plan_error_markers(), None),
    };
    let markers = match aegishv_type1_kernel::plan_type1_runtime_with_memory(
        handoff,
        aegishv_type1_kernel::Type1BackendRequest::Auto,
        capability_report.capabilities,
        allocation.plan(),
    ) {
        Ok(plan) => {
            let backend_marker = plan.backend.serial_marker();
            // SAFETY: backend selection established which architectural MSRs
            // are legal to read on this CPU.
            let controls = unsafe { read_type1_control_snapshot(plan.backend) };
            match aegishv_type1_kernel::plan_type1_runtime_preflight(plan, controls) {
                Ok(preflight) => {
                    let enable_plan = aegishv_type1_kernel::plan_type1_runtime_enable(preflight);
                    // SAFETY: preflight applied the VMX/SVM fixed-bit rules and
                    // this BSP owns the control-register transition.
                    match unsafe { apply_type1_enable_plan(enable_plan) } {
                        Ok(()) => {
                            let (regions_marker, vmxon_marker, vmcs_marker) =
                                match aegishv_type1_kernel::plan_type1_runtime_regions(
                                    plan,
                                    // SAFETY: the planner reads IA32_VMX_BASIC
                                    // only for the selected Intel VMX backend.
                                    unsafe { read_type1_vmx_basic(plan.backend) },
                                ) {
                                    // SAFETY: region addresses came from the
                                    // bounded USABLE-memory allocator and HHDM.
                                    Ok(regions) => match unsafe {
                                        materialize_type1_runtime_regions(handoff, regions)
                                    } {
                                        Ok(()) => {
                                            let (vmxon_marker, vmcs_marker) =
                                                // SAFETY: initialized regions
                                                // are exclusively owned here.
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
    };
    (
        markers,
        Some(PreparedRuntimeMemory {
            allocation,
            inherited_cr3,
            inherited_cr3_root: active_cr3_root,
        }),
    )
}

#[cfg(target_os = "none")]
const fn runtime_plan_error_markers() -> RuntimeMarkerSet {
    (
        aegishv_type1_kernel::SERIAL_RUNTIME_PLAN_ERROR_MARKER,
        aegishv_type1_kernel::SERIAL_RUNTIME_PREFLIGHT_ERROR_MARKER,
        aegishv_type1_kernel::SERIAL_RUNTIME_ENABLE_ERROR_MARKER,
        aegishv_type1_kernel::SERIAL_RUNTIME_REGIONS_ERROR_MARKER,
        aegishv_type1_kernel::SERIAL_RUNTIME_VMXON_ERROR_MARKER,
        aegishv_type1_kernel::SERIAL_RUNTIME_VMCS_LOAD_ERROR_MARKER,
    )
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn type1_kernel_physical_reservation(
    handoff: aegishv_type1_kernel::LimineMinimalHandoff,
) -> Result<aegishv_type1_kernel::Type1PhysicalReservation, ()> {
    // SAFETY: these linker symbols bound the one loaded kernel image and only
    // their addresses are observed; no memory is read through either symbol.
    let (virtual_start, virtual_end) = unsafe {
        (
            core::ptr::addr_of!(__aegishv_kernel_start) as u64,
            core::ptr::addr_of!(__aegishv_kernel_end) as u64,
        )
    };
    if virtual_start != handoff.executable_virtual_base {
        return Err(());
    }
    aegishv_type1_kernel::linked_kernel_reservation(
        handoff.executable_physical_base,
        handoff.executable_virtual_base,
        virtual_start,
        virtual_end,
    )
    .map_err(|_| ())
}

#[cfg(all(target_os = "none", not(target_arch = "x86_64")))]
fn type1_kernel_physical_reservation(
    _handoff: aegishv_type1_kernel::LimineMinimalHandoff,
) -> Result<aegishv_type1_kernel::Type1PhysicalReservation, ()> {
    Err(())
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn linked_host_physical_address(
    handoff: aegishv_type1_kernel::LimineMinimalHandoff,
    virtual_address: u64,
) -> Result<u64, ()> {
    let offset = virtual_address
        .checked_sub(handoff.executable_virtual_base)
        .ok_or(())?;
    handoff
        .executable_physical_base
        .checked_add(offset)
        .ok_or(())
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn linked_host_mapping(
    handoff: aegishv_type1_kernel::LimineMinimalHandoff,
    virtual_start: u64,
    virtual_end: u64,
    permissions: aegishv_type1_kernel::host_paging::HostPagePermissions,
) -> Result<aegishv_type1_kernel::host_paging::HostPageMapping, ()> {
    let length = virtual_end
        .checked_sub(virtual_start)
        .filter(|length| *length != 0)
        .ok_or(())?;
    let physical_start = linked_host_physical_address(handoff, virtual_start)?;
    Ok(aegishv_type1_kernel::host_paging::HostPageMapping::new(
        virtual_start,
        physical_start,
        length,
        permissions,
    ))
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn prepare_owned_host_page_tables(
    handoff: aegishv_type1_kernel::LimineMinimalHandoff,
) -> Result<aegishv_type1_kernel::host_paging::HostPageTableImage, ()> {
    use aegishv_type1_kernel::host_paging::{
        build_host_page_table_image, HostPageMapping, HostPagePermissions, HostPageTableLayout,
        HostPagingCapabilities, HostUnmappedPage, HOST_KERNEL_WINDOW_SIZE, HOST_PAGE_SIZE_4K,
        HOST_PAGE_TABLE_PAGE_COUNT,
    };

    // SAFETY: the extended CPUID leaves are read-only and the limit leaf is
    // architectural on every x86-64 CPU.
    let extended_limit =
        unsafe { __cpuid_count(aegishv_type1_kernel::CPUID_EXTENDED_LIMIT_LEAF, 0).eax };
    if extended_limit < X86_CPUID_ADDRESS_SIZE_LEAF {
        return Err(());
    }
    // SAFETY: the limit above proves both extended leaves are available.
    let (extended_features, address_sizes) = unsafe {
        (
            __cpuid_count(aegishv_type1_kernel::CPUID_EXTENDED_FEATURE_LEAF, 0),
            __cpuid_count(X86_CPUID_ADDRESS_SIZE_LEAF, 0),
        )
    };
    if extended_features.edx & X86_CPUID_EXTENDED_NX == 0 {
        return Err(());
    }
    let physical_address_bits = (address_sizes.eax & 0xff) as u8;
    if !(36..=52).contains(&physical_address_bits) {
        return Err(());
    }

    // SAFETY: this BSP owns CR0/CR4 and EFER at CPL0. NX is advertised by
    // CPUID, and setting WP/NXE before the switch makes every planned leaf's
    // permission meaningful as soon as the new root becomes active.
    let cr4 = unsafe { read_cr4() };
    if cr4 & X86_CR4_FIVE_LEVEL_PAGING != 0 {
        return Err(());
    }
    let efer = unsafe { read_msr(aegishv_type1_kernel::IA32_EFER_MSR) };
    unsafe {
        write_msr(
            aegishv_type1_kernel::IA32_EFER_MSR,
            efer | X86_EFER_NO_EXECUTE_ENABLE,
        )
    };
    let cr0 = unsafe { read_cr0() };
    unsafe { write_cr0(cr0 | X86_CR0_WRITE_PROTECT) };
    if unsafe { read_msr(aegishv_type1_kernel::IA32_EFER_MSR) } & X86_EFER_NO_EXECUTE_ENABLE == 0
        || unsafe { read_cr0() } & X86_CR0_WRITE_PROTECT == 0
    {
        return Err(());
    }

    // SAFETY: these linker symbols are only observed as addresses. Linker
    // assertions make every boundary page-aligned, ordered, and contained in
    // the one 2 MiB higher-half kernel window.
    let (
        kernel_start,
        kernel_end,
        text_start,
        text_end,
        rodata_start,
        rodata_end,
        writable_start,
        writable_end,
        table_start,
        table_end,
        double_fault_guard_bottom,
        double_fault_guard_top,
        nmi_guard_bottom,
        nmi_guard_top,
        machine_check_guard_bottom,
        machine_check_guard_top,
        vmx_exit_guard_bottom,
        vmx_exit_guard_top,
        boot_stack_guard_bottom,
        boot_stack_guard_top,
    ) = unsafe {
        (
            core::ptr::addr_of!(__aegishv_kernel_start) as u64,
            core::ptr::addr_of!(__aegishv_kernel_end) as u64,
            core::ptr::addr_of!(__aegishv_text_start) as u64,
            core::ptr::addr_of!(__aegishv_text_end) as u64,
            core::ptr::addr_of!(__aegishv_rodata_start) as u64,
            core::ptr::addr_of!(__aegishv_rodata_end) as u64,
            core::ptr::addr_of!(__aegishv_writable_start) as u64,
            core::ptr::addr_of!(__aegishv_writable_end) as u64,
            core::ptr::addr_of_mut!(__aegishv_host_page_tables_start) as u64,
            core::ptr::addr_of!(__aegishv_host_page_tables_end) as u64,
            core::ptr::addr_of!(__aegishv_double_fault_guard_bottom) as u64,
            core::ptr::addr_of!(__aegishv_double_fault_guard_top) as u64,
            core::ptr::addr_of!(__aegishv_nmi_guard_bottom) as u64,
            core::ptr::addr_of!(__aegishv_nmi_guard_top) as u64,
            core::ptr::addr_of!(__aegishv_machine_check_guard_bottom) as u64,
            core::ptr::addr_of!(__aegishv_machine_check_guard_top) as u64,
            core::ptr::addr_of!(__aegishv_vmx_exit_guard_bottom) as u64,
            core::ptr::addr_of!(__aegishv_vmx_exit_guard_top) as u64,
            core::ptr::addr_of!(__aegishv_boot_stack_guard_bottom) as u64,
            core::ptr::addr_of!(__aegishv_boot_stack_guard_top) as u64,
        )
    };
    if kernel_start != handoff.executable_virtual_base
        || kernel_end
            .checked_sub(kernel_start)
            .filter(|length| *length <= HOST_KERNEL_WINDOW_SIZE)
            .is_none()
        || table_end.checked_sub(table_start)
            != Some(HOST_PAGE_SIZE_4K * HOST_PAGE_TABLE_PAGE_COUNT as u64)
    {
        return Err(());
    }

    let table_physical = linked_host_physical_address(handoff, table_start)?;
    let table_layout =
        HostPageTableLayout::contiguous(table_physical, table_start).map_err(|_| ())?;
    let mappings: [HostPageMapping; 8] = [
        linked_host_mapping(
            handoff,
            text_start,
            text_end,
            HostPagePermissions::READ_EXECUTE,
        )?,
        linked_host_mapping(
            handoff,
            rodata_start,
            rodata_end,
            HostPagePermissions::READ_ONLY,
        )?,
        linked_host_mapping(
            handoff,
            writable_start,
            double_fault_guard_bottom,
            HostPagePermissions::READ_WRITE,
        )?,
        linked_host_mapping(
            handoff,
            double_fault_guard_top,
            nmi_guard_bottom,
            HostPagePermissions::READ_WRITE,
        )?,
        linked_host_mapping(
            handoff,
            nmi_guard_top,
            machine_check_guard_bottom,
            HostPagePermissions::READ_WRITE,
        )?,
        linked_host_mapping(
            handoff,
            machine_check_guard_top,
            vmx_exit_guard_bottom,
            HostPagePermissions::READ_WRITE,
        )?,
        linked_host_mapping(
            handoff,
            vmx_exit_guard_top,
            boot_stack_guard_bottom,
            HostPagePermissions::READ_WRITE,
        )?,
        linked_host_mapping(
            handoff,
            boot_stack_guard_top,
            writable_end,
            HostPagePermissions::READ_WRITE,
        )?,
    ];
    let unmapped_pages = [
        HostUnmappedPage::NULL,
        HostUnmappedPage::new(double_fault_guard_bottom),
        HostUnmappedPage::new(nmi_guard_bottom),
        HostUnmappedPage::new(machine_check_guard_bottom),
        HostUnmappedPage::new(vmx_exit_guard_bottom),
        HostUnmappedPage::new(boot_stack_guard_bottom),
    ];
    let image = build_host_page_table_image(
        HostPagingCapabilities::new(true, false, physical_address_bits),
        table_layout,
        kernel_start,
        &mappings,
        &unmapped_pages,
    )
    .map_err(|_| ())?;
    // SAFETY: the linker reserves exactly four aligned writable pages inside
    // the still-live inherited mapping; the image contains exactly four
    // validated hardware page-table pages.
    unsafe { materialize_owned_host_page_tables(&image)? };
    Ok(image)
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn materialize_owned_host_page_tables(
    image: &aegishv_type1_kernel::host_paging::HostPageTableImage,
) -> Result<(), ()> {
    use aegishv_type1_kernel::host_paging::{
        HOST_PAGE_TABLE_ENTRY_COUNT, HOST_PAGE_TABLE_PAGE_COUNT,
    };

    let start = core::ptr::addr_of_mut!(__aegishv_host_page_tables_start) as usize;
    let end = core::ptr::addr_of!(__aegishv_host_page_tables_end) as usize;
    let expected_bytes = HOST_PAGE_TABLE_ENTRY_COUNT
        .checked_mul(HOST_PAGE_TABLE_PAGE_COUNT)
        .and_then(|entries| entries.checked_mul(core::mem::size_of::<u64>()))
        .ok_or(())?;
    if end.checked_sub(start) != Some(expected_bytes) || start % 4096 != 0 {
        return Err(());
    }
    let destination = start as *mut u64;
    let mut table_index = 0;
    while table_index < HOST_PAGE_TABLE_PAGE_COUNT {
        let entries = image.tables()[table_index].entries();
        let mut entry_index = 0;
        while entry_index < HOST_PAGE_TABLE_ENTRY_COUNT {
            let flat_index = table_index * HOST_PAGE_TABLE_ENTRY_COUNT + entry_index;
            // SAFETY: the checked pool size covers every computed u64 slot and
            // the pool start is page-aligned, hence u64-aligned.
            unsafe {
                destination
                    .add(flat_index)
                    .write_volatile(entries[entry_index])
            };
            entry_index += 1;
        }
        table_index += 1;
    }
    core::sync::atomic::compiler_fence(Ordering::SeqCst);

    let mut table_index = 0;
    while table_index < HOST_PAGE_TABLE_PAGE_COUNT {
        let entries = image.tables()[table_index].entries();
        let mut entry_index = 0;
        while entry_index < HOST_PAGE_TABLE_ENTRY_COUNT {
            let flat_index = table_index * HOST_PAGE_TABLE_ENTRY_COUNT + entry_index;
            // SAFETY: this is the read-back of the same checked initialized
            // slot written above before the table root is made active.
            if unsafe { destination.add(flat_index).read_volatile() } != entries[entry_index] {
                return Err(());
            }
            entry_index += 1;
        }
        table_index += 1;
    }
    Ok(())
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn snapshot_materialized_host_page_tables(
    planned: &aegishv_type1_kernel::host_paging::HostPageTableImage,
) -> Result<
    [aegishv_type1_kernel::host_paging::HostPageTable;
        aegishv_type1_kernel::host_paging::HOST_PAGE_TABLE_PAGE_COUNT],
    (),
> {
    use aegishv_type1_kernel::host_paging::{
        HostPageTable, HOST_PAGE_TABLE_ENTRY_COUNT, HOST_PAGE_TABLE_PAGE_COUNT,
    };

    let start = core::ptr::addr_of_mut!(__aegishv_host_page_tables_start) as usize;
    let end = core::ptr::addr_of!(__aegishv_host_page_tables_end) as usize;
    let expected_bytes = HOST_PAGE_TABLE_ENTRY_COUNT
        .checked_mul(HOST_PAGE_TABLE_PAGE_COUNT)
        .and_then(|entries| entries.checked_mul(core::mem::size_of::<u64>()))
        .ok_or(())?;
    if end.checked_sub(start) != Some(expected_bytes) || start % 4096 != 0 {
        return Err(());
    }
    let mut snapshot = *planned.tables();
    let source = start as *const u64;
    let mut table_index = 0;
    while table_index < HOST_PAGE_TABLE_PAGE_COUNT {
        let mut entries = [0_u64; HOST_PAGE_TABLE_ENTRY_COUNT];
        let mut entry_index = 0;
        while entry_index < HOST_PAGE_TABLE_ENTRY_COUNT {
            let flat_index = table_index * HOST_PAGE_TABLE_ENTRY_COUNT + entry_index;
            // SAFETY: materialization already checked this exact four-page
            // pool, and the active root maps it once as supervisor RW/NX.
            entries[entry_index] = unsafe { source.add(flat_index).read_volatile() };
            entry_index += 1;
        }
        snapshot[table_index] = HostPageTable::from_entries(entries);
        table_index += 1;
    }
    Ok(snapshot)
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn activate_owned_host_paging(
    image: &aegishv_type1_kernel::host_paging::HostPageTableImage,
    inherited_cr3: u64,
) -> Result<(), ()> {
    let root = image.root_physical_address();
    let original_cr4 = unsafe { read_cr4() };
    if root == 0
        || root % aegishv_type1_kernel::host_paging::HOST_PAGE_SIZE_4K != 0
        || original_cr4 & X86_CR4_FIVE_LEVEL_PAGING != 0
    {
        return Err(());
    }

    let cr4_without_global = original_cr4 & !X86_CR4_PAGE_GLOBAL_ENABLE;
    if cr4_without_global != original_cr4 {
        // SAFETY: clearing PGE at CPL0 invalidates inherited global TLB
        // entries before the new root removes every Limine alias.
        unsafe { write_cr4(cr4_without_global) };
        if unsafe { read_cr4() } != cr4_without_global {
            unsafe { write_cr4(original_cr4) };
            return Err(());
        }
    }

    // SAFETY: all four hierarchy pages were materialized and read back, and
    // the current RIP/RSP plus host tables are covered by the new root.
    unsafe { write_cr3(root) };
    if unsafe { read_cr3() } != root {
        unsafe {
            write_cr3(inherited_cr3);
            write_cr4(original_cr4);
        }
        return Err(());
    }
    if cr4_without_global != original_cr4 {
        // SAFETY: inherited global translations were flushed while PGE was
        // clear; restoring the original CR4 does not recreate those entries.
        unsafe { write_cr4(original_cr4) };
        if unsafe { read_cr4() } != original_cr4 {
            unsafe {
                write_cr3(inherited_cr3);
                write_cr4(original_cr4);
            }
            return Err(());
        }
    }
    core::sync::atomic::compiler_fence(Ordering::SeqCst);

    // SAFETY: the new root maps the table pool RW/NX and hardware may only
    // have added architectural accessed/dirty bits since the switch.
    let materialized = match unsafe { snapshot_materialized_host_page_tables(image) } {
        Ok(tables) => tables,
        Err(()) => {
            // SAFETY: the reserved inherited root is still available and
            // restores the bootloader mapping before the terminal error path.
            unsafe { write_cr3(inherited_cr3) };
            return Err(());
        }
    };
    let live_state_is_valid = image.validate_materialized_tables(&materialized).is_ok()
        && unsafe { read_cr3() } == root
        && unsafe { read_cr0() } & X86_CR0_WRITE_PROTECT != 0
        && unsafe { read_cr4() } & X86_CR4_FIVE_LEVEL_PAGING == 0
        && unsafe { read_msr(aegishv_type1_kernel::IA32_EFER_MSR) } & X86_EFER_NO_EXECUTE_ENABLE
            != 0
        && unsafe { host_tables_are_owned() };
    if !live_state_is_valid {
        // SAFETY: the inherited root was reserved and remained untouched; it
        // is restored before reporting a post-switch validation failure.
        unsafe { write_cr3(inherited_cr3) };
        return Err(());
    }
    Ok(())
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
        // SAFETY: entry_count is the accessible extent of this live pointer
        // array. The checked range cannot wrap or become noncanonical, and
        // Limine keeps the mapped array alive through early boot.
        let entry = unsafe { table.add(index).read_volatile() };
        if entry.is_null() {
            return Err(());
        }
        validate_limine_object_range(
            entry as usize as u64,
            core::mem::size_of::<aegishv_type1_boot::LimineMemmapEntry>(),
            core::mem::align_of::<aegishv_type1_boot::LimineMemmapEntry>(),
        )?;
        // SAFETY: Limine guarantees that each validated pointer addresses one complete
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
unsafe fn read_vmx_capability_snapshot() -> Result<
    aegishv_arch_x86::vmx::capabilities::VmxCapabilitySnapshot,
    aegishv_arch_x86::vmx::VmxError,
> {
    use aegishv_arch_x86::vmx::capabilities::*;
    use aegishv_arch_x86::vmx::controls::{
        PRIMARY_ACTIVATE_SECONDARY_CONTROLS, SECONDARY_ENABLE_EPT,
    };

    // SAFETY: CPUID has already selected the Intel VMX backend, making the
    // architectural IA32_VMX_BASIC capability MSR available on this CPU.
    let basic = unsafe { read_msr(IA32_VMX_BASIC_MSR) };
    // SAFETY: CPUID leaf 1 is architectural on every x86-64 processor. Its
    // signature rejects broken VMX timers and EDX proves x87/SSE support.
    let cpuid_leaf1 = unsafe { __cpuid_count(1, 0) };
    if !VmxCapabilitySnapshot::uses_true_controls(basic) {
        return Err(aegishv_arch_x86::vmx::VmxError::new(
            aegishv_arch_x86::vmx::VmxErrorKind::UnsupportedCapability,
            "live VMX entry requires true control MSRs",
        ));
    }
    // SAFETY: IA32_VMX_BASIC reported true-control support, so the true
    // primary-control capability MSR is architecturally defined.
    let primary = unsafe { read_msr(IA32_VMX_TRUE_PROCBASED_CTLS_MSR) };
    if !VmxCapabilitySnapshot::control_allows(primary, PRIMARY_ACTIVATE_SECONDARY_CONTROLS) {
        return Err(aegishv_arch_x86::vmx::VmxError::new(
            aegishv_arch_x86::vmx::VmxErrorKind::UnsupportedCapability,
            "CPU does not expose secondary VM-execution controls",
        ));
    }
    // SAFETY: the true primary controls advertise activation of secondary
    // controls, so IA32_VMX_PROCBASED_CTLS2 is available to read.
    let secondary = unsafe { read_msr(IA32_VMX_PROCBASED_CTLS2_MSR) };
    if !VmxCapabilitySnapshot::control_allows(secondary, SECONDARY_ENABLE_EPT) {
        return Err(aegishv_arch_x86::vmx::VmxError::new(
            aegishv_arch_x86::vmx::VmxErrorKind::UnsupportedCapability,
            "CPU does not allow EPT",
        ));
    }
    // SAFETY: the VMX backend and true/secondary-control checks above make all
    // remaining Intel VMX capability and fixed-bit MSRs architectural here.
    let (misc, pin_based, exit, entry, ept_vpid, cr0_fixed0, cr0_fixed1, cr4_fixed0, cr4_fixed1) = unsafe {
        (
            read_msr(IA32_VMX_MISC_MSR),
            read_msr(IA32_VMX_TRUE_PINBASED_CTLS_MSR),
            read_msr(IA32_VMX_TRUE_EXIT_CTLS_MSR),
            read_msr(IA32_VMX_TRUE_ENTRY_CTLS_MSR),
            read_msr(IA32_VMX_EPT_VPID_CAP_MSR),
            read_msr(IA32_VMX_CR0_FIXED0_MSR),
            read_msr(IA32_VMX_CR0_FIXED1_MSR),
            read_msr(IA32_VMX_CR4_FIXED0_MSR),
            read_msr(IA32_VMX_CR4_FIXED1_MSR),
        )
    };
    Ok(VmxCapabilitySnapshot {
        processor_signature: cpuid_leaf1.eax,
        cpuid_leaf1_edx: cpuid_leaf1.edx,
        basic,
        misc,
        pin_based,
        primary,
        secondary,
        exit,
        entry,
        ept_vpid,
        cr0_fixed: aegishv_arch_x86::vmx::features::CrFixedBits::new(cr0_fixed0, cr0_fixed1),
        cr4_fixed: aegishv_arch_x86::vmx::features::CrFixedBits::new(cr4_fixed0, cr4_fixed1),
    })
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
unsafe fn read_cr3() -> u64 {
    let value: u64;
    asm!(
        "mov {}, cr3",
        out(reg) value,
        options(nomem, nostack, preserves_flags)
    );
    value
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn capture_vmx_host_state(
    cr0_fixed: aegishv_arch_x86::vmx::features::CrFixedBits,
    cr4_fixed: aegishv_arch_x86::vmx::features::CrFixedBits,
) -> Result<aegishv_arch_x86::vmx::vmcs_config::VmcsHostState64, aegishv_arch_x86::vmx::VmxError> {
    use aegishv_arch_x86::vmx::vmcs_config::{
        VmcsHostSelectors, VmcsHostState64, VmxPat, VMX_TOY_GUEST_PAT_RAW,
    };
    // SAFETY: VMX preflight runs at CPL0 on the bootstrap CPU; these
    // read-only snapshots capture the control registers used by HOST state.
    let (raw_cr3, cr0, cr4) = unsafe { (read_cr3(), read_cr0(), read_cr4()) };
    let cr3 = aegishv_hypervisor_core::ids::HostPhysical::new(raw_cr3).map_err(|_| {
        aegishv_arch_x86::vmx::VmxError::new(
            aegishv_arch_x86::vmx::VmxErrorKind::InvalidGuestState,
            "host CR3 cannot be represented as a physical address",
        )
    })?;
    aegishv_arch_x86::vmx::features::validate_control_register(
        cr0,
        cr0_fixed,
        "host CR0 violates the CPU's VMX fixed bits",
    )?;
    aegishv_arch_x86::vmx::features::validate_control_register(
        cr4,
        cr4_fixed,
        "host CR4 violates the CPU's VMX fixed bits",
    )?;
    // SAFETY: long mode defines the FS/GS base, SYSENTER, PAT, and EFER MSRs;
    // this CPL0 BSP only reads them to reproduce the host return context.
    let (fs_base, gs_base, sysenter_cs, sysenter_esp, sysenter_eip, pat, efer) = unsafe {
        (
            read_msr(0xc000_0100),
            read_msr(0xc000_0101),
            read_msr(0x174) as u32,
            read_msr(0x175),
            read_msr(0x176),
            read_msr(aegishv_type1_kernel::IA32_PAT_MSR),
            read_msr(aegishv_type1_kernel::IA32_EFER_MSR),
        )
    };
    let pat = VmxPat::new(pat)?.validate_owned_host_mappings()?;
    if pat.raw() == VMX_TOY_GUEST_PAT_RAW {
        return Err(aegishv_arch_x86::vmx::VmxError::new(
            aegishv_arch_x86::vmx::VmxErrorKind::InvalidGuestState,
            "host PAT must differ from the toy guest isolation pattern",
        ));
    }
    let state = VmcsHostState64 {
        cr0,
        cr3,
        cr4,
        selectors: VmcsHostSelectors {
            es: 0x30,
            cs: 0x28,
            ss: 0x30,
            ds: 0x30,
            fs: 0x30,
            gs: 0x30,
            tr: 0x38,
        },
        fs_base,
        gs_base,
        tr_base: core::ptr::addr_of!(__aegishv_host_tss) as u64,
        gdtr_base: core::ptr::addr_of!(__aegishv_host_gdt) as u64,
        idtr_base: core::ptr::addr_of!(__aegishv_host_idt) as u64,
        sysenter_cs,
        sysenter_esp,
        sysenter_eip,
        pat,
        efer,
        rsp: core::ptr::addr_of!(__aegishv_vmx_exit_stack_top) as u64,
        rip: aegishv_vmx_vmexit_entry as usize as u64,
    };
    state.validate()
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
unsafe fn write_cr3(value: u64) {
    asm!(
        "mov cr3, {}",
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
            .cast::<u8>()
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
        && tss_access == 0x8b
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
    // SAFETY: the caller completed VMX preflight and materialized exclusive,
    // revision-initialized VMXON and VMCS pages for this bootstrap CPU.
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

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn run_type1_vmx_toy_guest(
    handoff: aegishv_type1_kernel::LimineMinimalHandoff,
    runtime_memory: &mut PreparedRuntimeMemory,
) -> ! {
    use aegishv_arch_x86::vmx::instructions::VmxInstructionExecutor;
    use aegishv_arch_x86::vmx::vmcs_config::{
        MinimalVmcsConfiguration, VmcsExecutionState, VmcsGuestState64, VmcsInterceptionBitmaps,
        VmxPat,
    };

    // SAFETY: runtime dispatch selected Intel VMX from CPUID and the bootstrap
    // CPU remains at CPL0 while its capability MSRs are snapshotted.
    let capability_snapshot = match unsafe { read_vmx_capability_snapshot() } {
        Ok(snapshot) => snapshot,
        Err(_) => vmx_guest_entry_error(),
    };
    let capabilities = match capability_snapshot.prepare_toy_guest() {
        Ok(capabilities) => capabilities,
        Err(_) => vmx_guest_entry_error(),
    };
    serial_write_hex_u32(
        aegishv_type1_kernel::SERIAL_VMX_CPU_SIGNATURE_PREFIX,
        capability_snapshot.processor_signature,
    );
    serial_write_hex_u32(
        aegishv_type1_kernel::SERIAL_VMX_TIMER_RATE_PREFIX,
        u32::from(capabilities.preemption_timer.rate_shift),
    );
    serial_write_hex_u32(
        aegishv_type1_kernel::SERIAL_VMX_TIMER_RELOAD_PREFIX,
        capabilities.preemption_timer.reload_value,
    );
    serial_write_hex_u64(
        aegishv_type1_kernel::SERIAL_VMX_TIMER_EFFECTIVE_PREFIX,
        capabilities.preemption_timer.effective_budget_tsc_ticks,
    );
    // SAFETY: this only snapshots the current paging root. The lower PCID or
    // cache-control bits are deliberately ignored for physical ownership.
    let current_cr3_root =
        match aegishv_type1_kernel::inherited_x86_cr3_root_reservation(unsafe { read_cr3() }) {
            Ok(reservation) => reservation,
            Err(_) => vmx_guest_entry_error(),
        };
    if current_cr3_root != runtime_memory.inherited_cr3_root {
        vmx_guest_entry_error();
    }
    let guest_pages = match runtime_memory.allocation.allocate_intel_toy_guest() {
        Ok(pages) => pages,
        Err(_) => vmx_guest_entry_error(),
    };
    // SAFETY: CPUID is available in 64-bit mode and the selected Intel backend
    // makes the VMX feature-control MSR used by this snapshot architectural.
    let cpu_snapshot = unsafe { read_type1_cpu_snapshot() };
    let capability_report = aegishv_type1_kernel::type1_capabilities_from_snapshot(cpu_snapshot);
    let runtime_plan = match aegishv_type1_kernel::plan_type1_runtime_with_memory(
        handoff,
        aegishv_type1_kernel::Type1BackendRequest::IntelVmx,
        capability_report.capabilities,
        runtime_memory.allocation.plan(),
    ) {
        Ok(plan) if plan.backend == aegishv_type1_kernel::Type1RuntimeBackend::IntelVmx => plan,
        _ => vmx_guest_entry_error(),
    };
    // SAFETY: backend selection established that the VMX fixed-bit MSRs are
    // available; this read-only snapshot precedes the owned CR transition.
    let control_snapshot =
        unsafe { read_type1_control_snapshot(aegishv_type1_kernel::Type1RuntimeBackend::IntelVmx) };
    let preflight =
        match aegishv_type1_kernel::plan_type1_runtime_preflight(runtime_plan, control_snapshot) {
            Ok(preflight) => preflight,
            Err(_) => vmx_guest_entry_error(),
        };
    // SAFETY: preflight constructed fixed-bit-compliant CR0/CR4 values and the
    // bootstrap CPU exclusively owns this transition before entering VMX.
    let enable_result = unsafe {
        apply_type1_enable_plan(aegishv_type1_kernel::plan_type1_runtime_enable(preflight))
    };
    // SAFETY: CPL0 readback is side-effect-free and verifies the exact values
    // written by the immediately preceding enable transition.
    let (cr0_after, cr4_after) = unsafe { (read_cr0(), read_cr4()) };
    if enable_result.is_err()
        || cr0_after != preflight.cr0_after
        || cr4_after != preflight.cr4_after
    {
        vmx_guest_entry_error();
    }

    let regions = match aegishv_type1_kernel::plan_type1_runtime_regions(
        runtime_plan,
        Some(aegishv_type1_kernel::Type1VmxBasic::new(
            capability_snapshot.basic,
        )),
    ) {
        Ok(regions) => regions,
        Err(_) => vmx_guest_entry_error(),
    };
    // SAFETY: every runtime physical page was allocated from distinct USABLE
    // memory and maps through the validated Limine HHDM for one full page.
    if unsafe { materialize_type1_runtime_regions(handoff, regions) }.is_err() {
        vmx_guest_entry_error();
    }
    let guest_plan =
        match aegishv_type1_kernel::Type1ToyGuestBuildPlan::new(guest_pages, capabilities.ept) {
            Ok(plan) => plan,
            Err(_) => vmx_guest_entry_error(),
        };
    let mut writer = HhdmPageWriter {
        offset: handoff.hhdm_offset,
    };
    if aegishv_type1_kernel::materialize_type1_toy_guest(&guest_plan, &mut writer).is_err() {
        vmx_guest_entry_error();
    }
    core::sync::atomic::compiler_fence(Ordering::SeqCst);

    let guest = match VmcsGuestState64::toy_long_mode(
        capabilities.cr0_fixed,
        guest_plan.guest_cr3,
        capabilities.cr4_fixed,
        guest_plan.rsp,
        guest_plan.rip,
    )
    .and_then(|guest| guest.with_descriptor_tables(guest_plan.gdtr, guest_plan.idtr))
    {
        Ok(guest) => guest,
        Err(_) => vmx_guest_entry_error(),
    };
    if guest.segments.cs.selector != aegishv_type1_kernel::TYPE1_TOY_GUEST_CS
        || guest.segments.ss.selector != aegishv_type1_kernel::TYPE1_TOY_GUEST_SS
        || guest.rflags != aegishv_type1_kernel::TYPE1_TOY_GUEST_RFLAGS
        || guest.gdtr != guest_plan.gdtr
        || guest.idtr != guest_plan.idtr
    {
        vmx_guest_entry_error();
    }
    let interception_bitmaps = match VmcsInterceptionBitmaps::new(
        guest_pages.io_bitmap_a,
        guest_pages.io_bitmap_b,
        guest_pages.msr_bitmap,
    ) {
        Ok(bitmaps) => bitmaps,
        Err(_) => vmx_guest_entry_error(),
    };
    let execution = match VmcsExecutionState::toy_isolated(
        capabilities.controls,
        guest_plan.ept_pointer,
        interception_bitmaps,
        guest,
        capabilities.cr0_fixed,
        capabilities.cr4_fixed,
    ) {
        Ok(execution) => execution,
        Err(_) => vmx_guest_entry_error(),
    };
    // SAFETY: all Limine response reads and HHDM-backed runtime/guest writes
    // are complete. The next root maps only the linked higher-half kernel,
    // with RX/R/RW permissions and five explicit non-present guard pages.
    let owned_host_paging = match unsafe { prepare_owned_host_page_tables(handoff) } {
        Ok(image) => image,
        Err(()) => host_paging_error(),
    };
    // SAFETY: CPUID reported PAT support and this BSP reads its current PAT
    // before switching to leaves whose cache selector is fixed at index zero.
    let host_pat_before_owned_paging = unsafe { read_msr(aegishv_type1_kernel::IA32_PAT_MSR) };
    if VmxPat::new(host_pat_before_owned_paging)
        .and_then(VmxPat::validate_owned_host_mappings)
        .is_err()
    {
        host_paging_error();
    }
    // SAFETY: the inherited CR3 root was reserved before any allocation and
    // remains available for rollback until the new root passes live readback.
    if unsafe { activate_owned_host_paging(&owned_host_paging, runtime_memory.inherited_cr3) }
        .is_err()
    {
        host_paging_error();
    }
    serial_write_line(aegishv_type1_kernel::SERIAL_HOST_PAGING_OK_MARKER);
    // SAFETY: owned descriptor tables are active and preflight validated the
    // current CR fixed bits; the snapshot now records the owned host CR3.
    let host =
        match unsafe { capture_vmx_host_state(capabilities.cr0_fixed, capabilities.cr4_fixed) } {
            Ok(host) => host,
            Err(_) => vmx_guest_entry_error(),
        };
    let configuration = MinimalVmcsConfiguration {
        execution,
        host,
        guest,
    };
    if configuration.validate().is_err() {
        vmx_guest_entry_error();
    }

    let vmxon = match aegishv_hypervisor_core::ids::HostPhysical::new(regions.vmxon_physical) {
        Ok(address) => address,
        Err(_) => vmx_guest_entry_error(),
    };
    let vmcs = match aegishv_hypervisor_core::ids::HostPhysical::new(regions.vmcs_physical) {
        Ok(address) => address,
        Err(_) => vmx_guest_entry_error(),
    };
    let mut executor = aegishv_arch_x86::vmx::hardware::HardwareVmxInstructions::new();
    // SAFETY: preflight enabled VMXE and both allocator-owned VMX pages were
    // zeroed, revision-initialized, and validated against IA32_VMX_BASIC.
    if unsafe { executor.vmxon(vmxon) }.is_err() {
        vmx_guest_entry_error();
    }
    // SAFETY: this CPU exclusively owns the initialized VMCS page until the
    // terminal VMXOFF path, and no other CPU is started by this BSP prototype.
    if unsafe { executor.vmclear(vmcs) }.is_err() {
        // SAFETY: VMXON succeeded and this terminal path abandons all VMX state.
        unsafe { vmx_guest_cleanup_after_error(&mut executor) };
    }
    // SAFETY: VMCLEAR initialized the exclusively owned VMCS and the processor
    // remains in VMX operation on the same bootstrap CPU.
    if unsafe { executor.vmptrld(vmcs) }.is_err() {
        // SAFETY: VMXON succeeded and this terminal path abandons all VMX state.
        unsafe { vmx_guest_cleanup_after_error(&mut executor) };
    }
    // SAFETY: VMPTRLD made the owned VMCS current; configuration validation
    // covered every field and all referenced host/guest pages remain live.
    if unsafe { configuration.write_to(&mut executor) }.is_err() {
        // SAFETY: VMXON succeeded and this terminal path abandons all VMX state.
        unsafe { vmx_guest_cleanup_after_error(&mut executor) };
    }
    // SAFETY: the same owned VMCS remains current after configuration writes;
    // exact isolation, PAT, control, and address readback must pass first.
    if unsafe { configuration.verify_isolation_fields(&mut executor) }.is_err() {
        // SAFETY: VMXON succeeded and this terminal path abandons all VMX state.
        unsafe { vmx_guest_cleanup_after_error(&mut executor) };
    }

    VMX_EXPECTED_HOST_PAT.store(host.pat.raw(), Ordering::Release);
    VMX_EXPECTED_GUEST_CR0.store(guest.cr0, Ordering::Release);
    VMX_EXPECTED_GUEST_CR4.store(guest.cr4, Ordering::Release);
    serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_CONFIG_OK_MARKER);
    VMX_TOY_PREEMPTION_RELOAD.store(
        capabilities.preemption_timer.reload_value,
        Ordering::Release,
    );
    VMX_TOY_EXIT_STATE.store(VMX_TOY_AWAITING_PREEMPTION, Ordering::Release);
    VMX_LAST_INSTRUCTION_ERROR.store(0, Ordering::Release);
    let registers = aegishv_arch_x86::vmx::exits::GeneralRegisters::default();
    // SAFETY: the current VMCS contains the complete validated host, guest,
    // control, and EPT state. Success transfers to non-root mode and never
    // returns here; only VMfail returns its RFLAGS classification.
    let flags = unsafe { aegishv_vmx_launch(core::ptr::addr_of!(registers)) };
    if flags & 1 == 0 && flags & (1 << 6) != 0 {
        // SAFETY: VMfailValid leaves this VMCS current until cleanup, so the
        // architectural VM-instruction error field can still be inspected.
        if let Ok(error) = unsafe {
            executor.vmread(aegishv_arch_x86::vmx::vmcs::VmcsField::VM_INSTRUCTION_ERROR.raw())
        } {
            VMX_LAST_INSTRUCTION_ERROR.store(error as u32, Ordering::Release);
            serial_write_vmx_instruction_error(error as u32);
        }
    }
    // SAFETY: reaching this point means VMLAUNCH failed while the CPU remains
    // in VMX operation with the same current VMCS; cleanup is terminal.
    unsafe { vmx_guest_cleanup_after_error(&mut executor) }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn vmx_guest_entry_error() -> ! {
    serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_ENTRY_ERROR_MARKER);
    halt_loop()
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn host_paging_error() -> ! {
    serial_write_line(aegishv_type1_kernel::SERIAL_HOST_PAGING_ERROR_MARKER);
    halt_loop()
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
unsafe fn vmx_guest_cleanup_after_error(
    executor: &mut aegishv_arch_x86::vmx::hardware::HardwareVmxInstructions,
) -> ! {
    serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_ENTRY_ERROR_MARKER);
    // SAFETY: this helper is called only after VMXON succeeded on this CPU;
    // VMXOFF is the final best-effort cleanup and no VMX state is reused.
    let _ =
        unsafe { aegishv_arch_x86::vmx::instructions::VmxInstructionExecutor::vmxoff(executor) };
    halt_loop()
}

#[cfg(target_os = "none")]
fn physical_to_hhdm(physical: u64, hhdm_offset: u64) -> Result<usize, ()> {
    hhdm_page_virtual_address(physical, hhdm_offset)
}

#[cfg(any(target_os = "none", test))]
fn hhdm_page_virtual_address(physical: u64, hhdm_offset: u64) -> Result<usize, ()> {
    if physical % aegishv_type1_kernel::TYPE1_RUNTIME_PAGE_SIZE != 0 {
        return Err(());
    }
    let virtual_address = match physical.checked_add(hhdm_offset) {
        Some(value) => value,
        None => return Err(()),
    };
    let last = match virtual_address.checked_add(aegishv_type1_kernel::TYPE1_RUNTIME_PAGE_SIZE - 1)
    {
        Some(value) => value,
        None => return Err(()),
    };
    if last > usize::MAX as u64
        || !aegishv_arch_x86::vmx::features::is_canonical_u64(virtual_address)
        || !aegishv_arch_x86::vmx::features::is_canonical_u64(last)
    {
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

#[cfg(any(all(target_os = "none", target_arch = "x86_64"), test))]
fn vmx_toy_exit_error_marker(
    error: aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError,
    sequence: aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence,
) -> &'static str {
    match error {
        aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError::ExecutionDeadlineExpired => {
            aegishv_type1_kernel::SERIAL_VMX_GUEST_TIMEOUT_MARKER
        }
        aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError::GuestPatMismatch
        | aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError::InvalidGuestPat => {
            aegishv_type1_kernel::SERIAL_VMX_GUEST_PAT_STATE_ERROR_MARKER
        }
        aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError::VmcsRead { field, .. }
        | aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError::VmcsWrite { field, .. }
            if matches!(
                field,
                aegishv_arch_x86::vmx::vmcs::VmcsField::VM_ENTRY_INTERRUPTION_INFO
                    | aegishv_arch_x86::vmx::vmcs::VmcsField::VM_ENTRY_EXCEPTION_ERROR_CODE
                    | aegishv_arch_x86::vmx::vmcs::VmcsField::VM_ENTRY_INSTRUCTION_LENGTH
            ) =>
        {
            aegishv_type1_kernel::SERIAL_VMX_GUEST_UD_INJECT_ERROR_MARKER
        }
        aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError::InvalidExceptionInjection
        | aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError::ExceptionDeliveryFailed(_)
        | aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError::InvalidIdtVectoringState
        | aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError::InvalidGuestStack
        | aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError::InvalidGuestCookie
        | aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError::InvalidGuestReturnState
        | aegishv_arch_x86::vmx::toy_exit::ToyVmxExitError::InvalidDescriptorTableState => {
            aegishv_type1_kernel::SERIAL_VMX_GUEST_UD_INJECT_ERROR_MARKER
        }
        _ => match sequence {
            aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingX87Guard => {
                aegishv_type1_kernel::SERIAL_VMX_GUEST_NM_X87_EXIT_ERROR_MARKER
            }
            aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingSimdGuard => {
                aegishv_type1_kernel::SERIAL_VMX_GUEST_NM_SIMD_EXIT_ERROR_MARKER
            }
            aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingUdDelivery => {
                aegishv_type1_kernel::SERIAL_VMX_GUEST_UD_INJECT_ERROR_MARKER
            }
            _ => aegishv_type1_kernel::SERIAL_VMX_GUEST_EXIT_ERROR_MARKER,
        },
    }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
#[no_mangle]
extern "C" fn aegishv_vmx_exit_dispatch(
    frame: *mut aegishv_arch_x86::vmx::exits::GeneralRegisters,
) -> u64 {
    // SAFETY: IA32_PAT is architectural on the selected Intel 64 VMX backend.
    // Every normal exit checks it before inspecting or resuming guest state.
    let live_host_pat = unsafe { read_msr(aegishv_type1_kernel::IA32_PAT_MSR) };
    if live_host_pat != VMX_EXPECTED_HOST_PAT.load(Ordering::Acquire) {
        VMX_TOY_EXIT_STATE.store(VMX_TOY_FAILED, Ordering::Release);
        serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_PAT_STATE_ERROR_MARKER);
        return 1;
    }
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
        VMX_TOY_AWAITING_PREEMPTION => {
            aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingPreemption
        }
        VMX_TOY_AWAITING_DEADLINE_PROBE => {
            aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingDeadlineProbe
        }
        VMX_TOY_AWAITING_IO => aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingIo,
        VMX_TOY_AWAITING_IO_BITMAP_B => {
            aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingIoBitmapB
        }
        VMX_TOY_AWAITING_CPUID => {
            aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingCpuid
        }
        VMX_TOY_AWAITING_RDMSR => {
            aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingRdmsr
        }
        VMX_TOY_AWAITING_X87_GUARD => {
            aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingX87Guard
        }
        VMX_TOY_AWAITING_SIMD_GUARD => {
            aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingSimdGuard
        }
        VMX_TOY_AWAITING_UD_DELIVERY => {
            aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingUdDelivery
        }
        VMX_TOY_COMPLETE => aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::Complete,
        _ => {
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_EXIT_ERROR_MARKER);
            return 1;
        }
    };
    let contract = aegishv_arch_x86::vmx::toy_exit::ToyVmxExitContract {
        initial_rip: aegishv_type1_kernel::TYPE1_TOY_GUEST_RIP,
        deadline_probe_rips: aegishv_type1_kernel::TYPE1_TOY_DEADLINE_PROBE_RIPS,
        deadline_fallback_rip: aegishv_type1_kernel::TYPE1_TOY_DEADLINE_FALLBACK_RIP,
        continuation_rip: aegishv_type1_kernel::TYPE1_TOY_CONTINUATION_RIP,
        io_rip: aegishv_type1_kernel::TYPE1_TOY_IO_RIP,
        io_bitmap_b_rip: aegishv_type1_kernel::TYPE1_TOY_IO_BITMAP_B_RIP,
        cpuid_rip: aegishv_type1_kernel::TYPE1_TOY_CPUID_RIP,
        rdmsr_rip: aegishv_type1_kernel::TYPE1_TOY_RDMSR_RIP,
        pat_rdmsr_rip: aegishv_type1_kernel::TYPE1_TOY_PAT_RDMSR_RIP,
        x87_guard_rip: aegishv_type1_kernel::TYPE1_TOY_X87_GUARD_RIP,
        simd_guard_rip: aegishv_type1_kernel::TYPE1_TOY_SIMD_GUARD_RIP,
        ud2_rip: aegishv_type1_kernel::TYPE1_TOY_UD2_RIP,
        hlt_rip: aegishv_type1_kernel::TYPE1_TOY_HLT_RIP,
        pat_mismatch_hlt_rip: aegishv_type1_kernel::TYPE1_TOY_PAT_MISMATCH_HLT_RIP,
        hlt_rsp: aegishv_type1_kernel::TYPE1_TOY_GUEST_RSP,
        hlt_cs: aegishv_type1_kernel::TYPE1_TOY_GUEST_CS,
        hlt_ss: aegishv_type1_kernel::TYPE1_TOY_GUEST_SS,
        hlt_rflags: aegishv_type1_kernel::TYPE1_TOY_HLT_EXIT_RFLAGS,
        guest_gdtr: aegishv_arch_x86::vmx::vmcs_config::VmcsDescriptorTableState::new(
            aegishv_type1_kernel::TYPE1_TOY_GDT_BASE,
            aegishv_type1_kernel::TYPE1_TOY_GDT_LIMIT,
        ),
        guest_idtr: aegishv_arch_x86::vmx::vmcs_config::VmcsDescriptorTableState::new(
            aegishv_type1_kernel::TYPE1_TOY_IDT_BASE,
            aegishv_type1_kernel::TYPE1_TOY_IDT_LIMIT,
        ),
        ud_handler_cookie: aegishv_type1_kernel::TYPE1_TOY_UD_HANDLER_COOKIE,
        io_port: 0xe9,
        io_bitmap_b_port: 0x8000,
        io_value: b'A',
        preemption_timer_reload: VMX_TOY_PREEMPTION_RELOAD.load(Ordering::Acquire),
        guest_pat: aegishv_arch_x86::vmx::vmcs_config::VmxPat::toy_guest(),
        guest_cr0: VMX_EXPECTED_GUEST_CR0.load(Ordering::Acquire),
        guest_cr4: VMX_EXPECTED_GUEST_CR4.load(Ordering::Acquire),
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
            if sequence
                == aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingDeadlineProbe =>
        {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_AWAITING_DEADLINE_PROBE, Ordering::Release);
            0
        }
        Ok(aegishv_arch_x86::vmx::toy_exit::ToyVmxExitAction::Resume)
            if sequence == aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingIo =>
        {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_AWAITING_IO, Ordering::Release);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_PREEMPT_EXIT_OK_MARKER);
            0
        }
        Ok(aegishv_arch_x86::vmx::toy_exit::ToyVmxExitAction::Resume)
            if sequence
                == aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingIoBitmapB =>
        {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_AWAITING_IO_BITMAP_B, Ordering::Release);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_IO_EXIT_OK_MARKER);
            0
        }
        Ok(aegishv_arch_x86::vmx::toy_exit::ToyVmxExitAction::Resume)
            if sequence == aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingCpuid =>
        {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_AWAITING_CPUID, Ordering::Release);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_IO_B_EXIT_OK_MARKER);
            0
        }
        Ok(aegishv_arch_x86::vmx::toy_exit::ToyVmxExitAction::Resume)
            if sequence == aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingRdmsr =>
        {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_AWAITING_RDMSR, Ordering::Release);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_CPUID_EXIT_OK_MARKER);
            0
        }
        Ok(aegishv_arch_x86::vmx::toy_exit::ToyVmxExitAction::Resume)
            if sequence
                == aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingX87Guard =>
        {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_AWAITING_X87_GUARD, Ordering::Release);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_RDMSR_EXIT_OK_MARKER);
            0
        }
        Ok(aegishv_arch_x86::vmx::toy_exit::ToyVmxExitAction::Resume)
            if sequence
                == aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingSimdGuard =>
        {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_AWAITING_SIMD_GUARD, Ordering::Release);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_PAT_STATE_OK_MARKER);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_NM_X87_EXIT_OK_MARKER);
            0
        }
        Ok(aegishv_arch_x86::vmx::toy_exit::ToyVmxExitAction::Resume)
            if sequence
                == aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::AwaitingUdDelivery =>
        {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_AWAITING_UD_DELIVERY, Ordering::Release);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_NM_SIMD_EXIT_OK_MARKER);
            0
        }
        Ok(aegishv_arch_x86::vmx::toy_exit::ToyVmxExitAction::Stop)
            if sequence == aegishv_arch_x86::vmx::toy_exit::ToyVmxExitSequence::Complete =>
        {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_COMPLETE, Ordering::Release);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_UD_INJECT_OK_MARKER);
            serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_HLT_EXIT_OK_MARKER);
            1
        }
        Err(error) => {
            VMX_TOY_EXIT_STATE.store(VMX_TOY_FAILED, Ordering::Release);
            serial_write_line(vmx_toy_exit_error_marker(error, sequence));
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
    let failed_state = VMX_TOY_EXIT_STATE.load(Ordering::Acquire);
    VMX_TOY_EXIT_STATE.store(VMX_TOY_FAILED, Ordering::Release);
    if failed_state == VMX_TOY_AWAITING_UD_DELIVERY {
        serial_write_line(aegishv_type1_kernel::SERIAL_VMX_GUEST_UD_INJECT_ERROR_MARKER);
    }
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
            serial_write_vmx_instruction_error(error as u32);
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
    // SAFETY: the panic path is terminal, runs at CPL0, and reinitializes only
    // the fixed legacy COM1 device before emitting a bounded marker.
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
            // SAFETY: early boot owns COM1 and the byte writer polls the UART
            // transmit-ready bit before performing the single port write.
            unsafe {
                serial_write_byte(COM1, *byte);
            }
        }
    }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn serial_write_vmx_instruction_error(error: u32) {
    serial_write_hex_u32(
        aegishv_type1_kernel::SERIAL_VMX_INSTRUCTION_ERROR_PREFIX,
        error,
    );
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn serial_write_hex_u32(prefix: &str, value: u32) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    // SAFETY: early boot owns COM1. Every emitted byte is from a fixed prefix,
    // an in-bounds hexadecimal lookup, or the final newline.
    unsafe {
        for byte in prefix.as_bytes() {
            serial_write_byte(COM1, *byte);
        }
        for shift in (0..8).rev() {
            let nibble = ((value >> (shift * 4)) & 0xf) as usize;
            serial_write_byte(COM1, HEX[nibble]);
        }
        serial_write_byte(COM1, b'\n');
    }
}

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
fn serial_write_hex_u64(prefix: &str, value: u64) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    // SAFETY: early boot owns COM1. Every emitted byte is from a fixed prefix,
    // an in-bounds hexadecimal lookup, or the final newline.
    unsafe {
        for byte in prefix.as_bytes() {
            serial_write_byte(COM1, *byte);
        }
        for shift in (0..16).rev() {
            let nibble = ((value >> (shift * 4)) & 0xf) as usize;
            serial_write_byte(COM1, HEX[nibble]);
        }
        serial_write_byte(COM1, b'\n');
    }
}

#[cfg(target_os = "none")]
unsafe fn read_limine_minimal_handoff() -> aegishv_type1_kernel::LimineMinimalHandoff {
    // SAFETY: this linker-owned three-word tag is mapped for the kernel
    // lifetime, and Limine is permitted to update only its final word.
    let base_revision = unsafe {
        core::ptr::addr_of!(LIMINE_BASE_REVISION_TAG)
            .cast::<u64>()
            .add(2)
            .read_volatile()
    };
    if base_revision != 0 {
        return aegishv_type1_kernel::LimineMinimalHandoff {
            base_revision_value: base_revision,
            hhdm_response: 0,
            hhdm_revision: 0,
            hhdm_offset: 0,
            memmap_response: 0,
            memmap_revision: 0,
            memmap_entry_count: 0,
            memmap_entries: 0,
            executable_address_response: 0,
            executable_address_revision: 0,
            executable_physical_base: 0,
            executable_virtual_base: 0,
        };
    }
    // SAFETY: the request records are linker-owned writable protocol objects;
    // reading their response words does not dereference bootloader pointers.
    let (hhdm_response, memmap_response, executable_address_response) = unsafe {
        (
            core::ptr::addr_of!(LIMINE_HHDM_REQUEST.response).read_volatile(),
            core::ptr::addr_of!(LIMINE_MEMMAP_REQUEST.response).read_volatile(),
            core::ptr::addr_of!(LIMINE_EXECUTABLE_ADDRESS_REQUEST.response).read_volatile(),
        )
    };
    let hhdm_response = validated_limine_response(
        hhdm_response,
        core::mem::size_of::<aegishv_type1_kernel::LimineHhdmResponse>(),
    );
    let memmap_response = validated_limine_response(
        memmap_response,
        core::mem::size_of::<aegishv_type1_kernel::LimineMemmapResponse>(),
    );
    let executable_address_response = validated_limine_response(
        executable_address_response,
        core::mem::size_of::<aegishv_type1_kernel::LimineExecutableAddressResponse>(),
    );

    let (hhdm_revision, hhdm_offset) = if hhdm_response == 0 {
        (0, 0)
    } else {
        // SAFETY: the complete aligned response prefix was range-validated
        // above and remains owned by Limine throughout early boot.
        unsafe {
            (
                read_limine_response_u64(
                    hhdm_response,
                    aegishv_type1_kernel::LIMINE_RESPONSE_REVISION_OFFSET,
                ),
                read_limine_response_u64(
                    hhdm_response,
                    aegishv_type1_kernel::LIMINE_HHDM_OFFSET_OFFSET,
                ),
            )
        }
    };
    let (memmap_revision, memmap_entry_count, memmap_entries) = if memmap_response == 0 {
        (0, 0, 0)
    } else {
        // SAFETY: the complete aligned response prefix was range-validated
        // above and remains owned by Limine throughout early boot.
        unsafe {
            (
                read_limine_response_u64(
                    memmap_response,
                    aegishv_type1_kernel::LIMINE_RESPONSE_REVISION_OFFSET,
                ),
                read_limine_response_u64(
                    memmap_response,
                    aegishv_type1_kernel::LIMINE_MEMMAP_ENTRY_COUNT_OFFSET,
                ),
                read_limine_response_u64(
                    memmap_response,
                    aegishv_type1_kernel::LIMINE_MEMMAP_ENTRIES_OFFSET,
                ),
            )
        }
    };
    let (executable_address_revision, executable_physical_base, executable_virtual_base) =
        if executable_address_response == 0 {
            (0, 0, 0)
        } else {
            // SAFETY: the complete aligned response prefix was range-validated
            // above and remains owned by Limine throughout early boot.
            unsafe {
                (
                    read_limine_response_u64(
                        executable_address_response,
                        aegishv_type1_kernel::LIMINE_RESPONSE_REVISION_OFFSET,
                    ),
                    read_limine_response_u64(
                        executable_address_response,
                        aegishv_type1_kernel::LIMINE_EXECUTABLE_PHYSICAL_BASE_OFFSET,
                    ),
                    read_limine_response_u64(
                        executable_address_response,
                        aegishv_type1_kernel::LIMINE_EXECUTABLE_VIRTUAL_BASE_OFFSET,
                    ),
                )
            }
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
fn validated_limine_response(response: u64, size: usize) -> u64 {
    if response != 0 && validate_limine_object_range(response, size, 8).is_ok() {
        response
    } else {
        0
    }
}

#[cfg(target_os = "none")]
unsafe fn read_limine_response_u64(response: u64, offset: usize) -> u64 {
    // SAFETY: callers validate the full response prefix and use only aligned
    // u64 offsets within that prefix before invoking this helper.
    unsafe {
        (response as usize as *const u8)
            .add(offset)
            .cast::<u64>()
            .read_volatile()
    }
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
        // SAFETY: all callers are terminal CPL0 paths with interrupts disabled;
        // HLT intentionally leaves the CPU quiescent until an external event.
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
    use super::{
        hhdm_page_virtual_address, validate_limine_object_range, vmx_toy_exit_error_marker,
    };

    #[test]
    fn vmx_exit_errors_have_explicit_fail_closed_markers() {
        use aegishv_arch_x86::vmx::toy_exit::{ToyVmxExitError, ToyVmxExitSequence};

        assert_eq!(
            vmx_toy_exit_error_marker(
                ToyVmxExitError::ExecutionDeadlineExpired,
                ToyVmxExitSequence::AwaitingPreemption,
            ),
            aegishv_type1_kernel::SERIAL_VMX_GUEST_TIMEOUT_MARKER
        );
        assert_eq!(
            vmx_toy_exit_error_marker(
                ToyVmxExitError::GuestPatMismatch,
                ToyVmxExitSequence::AwaitingX87Guard,
            ),
            aegishv_type1_kernel::SERIAL_VMX_GUEST_PAT_STATE_ERROR_MARKER
        );
        assert_eq!(
            vmx_toy_exit_error_marker(
                ToyVmxExitError::InvalidGuestPat,
                ToyVmxExitSequence::AwaitingSimdGuard,
            ),
            aegishv_type1_kernel::SERIAL_VMX_GUEST_PAT_STATE_ERROR_MARKER
        );
        assert_eq!(
            vmx_toy_exit_error_marker(
                ToyVmxExitError::InvalidFpuGuardState,
                ToyVmxExitSequence::AwaitingX87Guard,
            ),
            aegishv_type1_kernel::SERIAL_VMX_GUEST_NM_X87_EXIT_ERROR_MARKER
        );
        assert_eq!(
            vmx_toy_exit_error_marker(
                ToyVmxExitError::InvalidFpuGuardState,
                ToyVmxExitSequence::AwaitingSimdGuard,
            ),
            aegishv_type1_kernel::SERIAL_VMX_GUEST_NM_SIMD_EXIT_ERROR_MARKER
        );
        assert_eq!(
            vmx_toy_exit_error_marker(
                ToyVmxExitError::InvalidFpuGuardState,
                ToyVmxExitSequence::AwaitingUdDelivery,
            ),
            aegishv_type1_kernel::SERIAL_VMX_GUEST_UD_INJECT_ERROR_MARKER
        );
        assert_eq!(
            vmx_toy_exit_error_marker(
                ToyVmxExitError::InvalidExceptionInjection,
                ToyVmxExitSequence::AwaitingSimdGuard,
            ),
            aegishv_type1_kernel::SERIAL_VMX_GUEST_UD_INJECT_ERROR_MARKER
        );
        assert_eq!(
            vmx_toy_exit_error_marker(
                ToyVmxExitError::InvalidGuestCookie,
                ToyVmxExitSequence::AwaitingUdDelivery,
            ),
            aegishv_type1_kernel::SERIAL_VMX_GUEST_UD_INJECT_ERROR_MARKER
        );
    }

    #[test]
    fn vm_entry_injection_vmcs_failures_have_the_ud_marker() {
        use aegishv_arch_x86::vmx::features::VmxErrorKind;
        use aegishv_arch_x86::vmx::toy_exit::{ToyVmxExitError, ToyVmxExitSequence};
        use aegishv_arch_x86::vmx::vmcs::VmcsField;

        for field in [
            VmcsField::VM_ENTRY_INTERRUPTION_INFO,
            VmcsField::VM_ENTRY_EXCEPTION_ERROR_CODE,
            VmcsField::VM_ENTRY_INSTRUCTION_LENGTH,
        ] {
            for error in [
                ToyVmxExitError::VmcsRead {
                    field,
                    kind: VmxErrorKind::InstructionFailed,
                },
                ToyVmxExitError::VmcsWrite {
                    field,
                    kind: VmxErrorKind::InstructionFailed,
                },
            ] {
                assert_eq!(
                    vmx_toy_exit_error_marker(error, ToyVmxExitSequence::AwaitingSimdGuard),
                    aegishv_type1_kernel::SERIAL_VMX_GUEST_UD_INJECT_ERROR_MARKER
                );
            }
        }

        assert_eq!(
            vmx_toy_exit_error_marker(
                ToyVmxExitError::VmcsWrite {
                    field: VmcsField::GUEST_RIP,
                    kind: VmxErrorKind::InstructionFailed,
                },
                ToyVmxExitSequence::AwaitingSimdGuard,
            ),
            aegishv_type1_kernel::SERIAL_VMX_GUEST_NM_SIMD_EXIT_ERROR_MARKER
        );
    }

    #[test]
    fn limine_object_range_rejects_wrap_and_noncanonical_endpoints() {
        assert!(validate_limine_object_range(0x1000, 24, 8).is_ok());
        assert!(validate_limine_object_range(u64::MAX - 7, 16, 8).is_err());
        assert!(validate_limine_object_range(0x0000_7fff_ffff_fff8, 16, 8).is_err());
        assert!(validate_limine_object_range(0x1001, 24, 8).is_err());
    }

    #[test]
    fn hhdm_page_range_requires_alignment_and_canonical_endpoints() {
        assert_eq!(
            hhdm_page_virtual_address(0x2000, 0xffff_8000_0000_0000),
            Ok(0xffff_8000_0000_2000usize)
        );
        assert!(hhdm_page_virtual_address(0x2001, 0xffff_8000_0000_0000).is_err());
        assert!(hhdm_page_virtual_address(0x1000, 0x0000_8000_0000_0000).is_err());
        assert!(hhdm_page_virtual_address(0x2000, u64::MAX - 0x1000).is_err());
    }
}
