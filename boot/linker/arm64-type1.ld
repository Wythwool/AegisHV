ENTRY(aegishv_el2_start)
SECTIONS
{
  . = 0x40080000;
  .text : { KEEP(*(.text.entry)) *(.text .text.*) }
  .rodata ALIGN(4K) : { *(.rodata .rodata.*) }
  .data ALIGN(4K) : { *(.data .data.*) }
  .bss ALIGN(4K) (NOLOAD) : {
    __bss_start = .;
    *(.bss .bss.*) *(COMMON)
    . = ALIGN(16);
    __bss_end = .;
    . += 64K;
    __stack_top = .;
  }
  /DISCARD/ : { *(.eh_frame*) *(.comment*) }
}
