ENTRY(aegishv_type1_start)

KERNEL_PHYS_BASE = 0x00200000;
KERNEL_VIRT_BASE = 0xffffffff80200000;
BOOT_STACK_SIZE = 0x00040000;

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
    *(.got .got.*)
  } :rodata

  . = ALIGN(4K);
  .limine_requests : AT(ADDR(.limine_requests) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    KEEP(*(.limine_requests_start))
    KEEP(*(.limine_requests))
    KEEP(*(.limine_requests_end))
  } :data

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

  .boot_stack ALIGN(4K) (NOLOAD) : AT(ADDR(.boot_stack) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_boot_stack_bottom = .;
    . += BOOT_STACK_SIZE;
    __aegishv_boot_stack_top = .;
  } :data

  __aegishv_kernel_end = .;

  ASSERT((ADDR(.text) & 0xfff) == 0, "text must be page aligned")
  ASSERT((ADDR(.rodata) & 0xfff) == 0, "rodata must be page aligned")
  ASSERT((ADDR(.limine_requests) & 0xfff) == 0, "Limine requests must be page aligned")
  ASSERT((ADDR(.data) & 0xfff) == 0, "data must be page aligned")
  ASSERT(BOOT_STACK_SIZE >= 0x00020000, "boot stack is below the audited minimum")
  ASSERT(SIZEOF(.boot_stack) == BOOT_STACK_SIZE, "boot stack size changed")

  /DISCARD/ :
  {
    *(.comment)
    *(.eh_frame)
    *(.note .note.*)
  }
}
