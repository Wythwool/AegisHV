ENTRY(aegishv_el2_start)
SECTIONS
{
  . = 0x40080000;
  __image_start = .;
  .text : { KEEP(*(.text.entry)) *(.text .text.*) }
  . = ALIGN(4K);
  __text_end = .;
  .rodata ALIGN(4K) : { *(.rodata .rodata.*) }
  . = ALIGN(4K);
  __rodata_end = .;
  .data ALIGN(4K) : { *(.data .data.*) }
  .bss ALIGN(4K) (NOLOAD) : {
    __bss_start = .;
    *(.bss .bss.*) *(COMMON)
    . = ALIGN(4K);
    __bss_end = .;
    __stack_guard = .;
    . += 4K;
    . += 64K;
    __stack_top = .;
    . += 3 * (4K + 64K);
  }
  __image_end = .;
  ASSERT(__image_end <= 0x40200000, "EL2 image exceeds its mapped window")
  /DISCARD/ : { *(.eh_frame*) *(.comment*) }
}
