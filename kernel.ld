ENTRY(start)

SECTIONS
{
  . = 1M;
  .text : ALIGN(4096) {
    *(.multiboot2)
    *(.text*)
  }
  .rodata : ALIGN(4096) { *(.rodata*) }
  .data : ALIGN(4096) { *(.data*) }
  .bss : ALIGN(4096) {
    *(COMMON)
    *(.bss*)
  }
}
