#![no_std]
#![no_main]
core::arch::global_asm!(include_str!("mmio.S"));
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
