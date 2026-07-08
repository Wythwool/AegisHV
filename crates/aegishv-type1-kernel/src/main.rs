#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

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
        serial_write_line(runtime_backend_marker(handoff));
    } else {
        serial_write_line(aegishv_type1_kernel::SERIAL_LIMINE_MISSING_MARKER);
        serial_write_line(status.serial_marker());
    }
    halt_loop()
}

#[cfg(target_os = "none")]
fn runtime_backend_marker(handoff: aegishv_type1_kernel::LimineMinimalHandoff) -> &'static str {
    match aegishv_type1_kernel::plan_type1_runtime(
        handoff,
        aegishv_type1_kernel::Type1BackendRequest::Auto,
        aegishv_type1_kernel::Type1ArchCapabilities::none(),
    ) {
        Ok(plan) => plan.backend.serial_marker(),
        Err(_) => aegishv_type1_kernel::SERIAL_RUNTIME_PLAN_ERROR_MARKER,
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
