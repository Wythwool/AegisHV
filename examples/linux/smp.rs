#![no_std]
#![no_main]
use core::arch::global_asm;
global_asm!(include_str!("smp.S"));
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! { loop { core::hint::spin_loop(); } }
