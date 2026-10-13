#![no_std]
#![no_main]

#[cfg(target_arch = "x86_64")]
core::arch::global_asm!(concat!(".equ CASE, ", env!("AEGISHV_GUEST_CASE")), include_str!("x86_64.S"));
#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(concat!(".equ CASE, ", env!("AEGISHV_GUEST_CASE")), include_str!("arm64.S"));

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop { core::hint::spin_loop(); }
}
