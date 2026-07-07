ENTRY(aegishv_type1_start)

KERNEL_PHYS_BASE = 0x00200000;
KERNEL_VIRT_BASE = 0xffffffff80200000;
BOOT_STACK_SIZE = 0x00010000;

PHDRS
{
  text PT_LOAD FLAGS(5);
  rodata PT_LOAD FLAGS(4);
  data PT_LOAD FLAGS(6);
}

SECTIONS
{
  . = KERNEL_VIRT_BASE;
  __aegishv_kernel_start = .;

  .text ALIGN(4K) : AT(ADDR(.text) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    KEEP(*(.text.entry))
    *(.text .text.*)
  } :text

  .rodata ALIGN(4K) : AT(ADDR(.rodata) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    *(.rodata .rodata.*)
  } :rodata

  .data ALIGN(4K) : AT(ADDR(.data) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    *(.data .data.*)
  } :data

  .bss ALIGN(4K) : AT(ADDR(.bss) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_bss_start = .;
    *(.bss .bss.* COMMON)
    __aegishv_bss_end = .;
  } :data

  .boot_stack ALIGN(4K) : AT(ADDR(.boot_stack) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_boot_stack_bottom = .;
    . += BOOT_STACK_SIZE;
    __aegishv_boot_stack_top = .;
  } :data

  __aegishv_kernel_end = .;

  /DISCARD/ :
  {
    *(.comment)
    *(.eh_frame)
    *(.note .note.*)
  }
}
