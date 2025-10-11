OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)
SECTIONS
{
  . = 0x100000; /* load at 1MB */
  .text : {
    *(.multiboot2)
    *(.text*)
  }
  .rodata : { *(.rodata*) }
  .data : { *(.data*) }
  .bss : {
    *(.bss*)
    *(COMMON)
  }
}
