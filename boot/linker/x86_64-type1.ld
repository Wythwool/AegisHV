ENTRY(aegishv_type1_start)

KERNEL_PHYS_BASE = 0x00200000;
KERNEL_VIRT_BASE = 0xffffffff80200000;
BOOT_STACK_SIZE = 0x00040000;
HOST_STACK_GUARD_SIZE = 0x00001000;
HOST_PAGE_TABLE_SIZE = 0x00004000;
KERNEL_WINDOW_SIZE = 0x00200000;
HOST_IST_STACK_SIZE = 0x00004000;
HOST_VMX_EXIT_STACK_SIZE = 0x00010000;

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
    __aegishv_text_start = .;
    KEEP(*(.text.entry))
    *(.text .text.*)
    . = ALIGN(4K);
    __aegishv_text_end = .;
  } :text

  .rodata ALIGN(4K) : AT(ADDR(.rodata) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_rodata_start = .;
    *(.rodata .rodata.*)
    *(.srodata .srodata.*)
    *(.got .got.*)
    . = ALIGN(4K);
    __aegishv_rodata_end = .;
  } :rodata

  . = ALIGN(4K);
  .limine_requests : AT(ADDR(.limine_requests) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_writable_start = .;
    KEEP(*(.limine_requests_start))
    KEEP(*(.limine_requests))
    KEEP(*(.limine_requests_end))
    . = ALIGN(4K);
  } :data

  .data ALIGN(4K) : AT(ADDR(.data) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    *(.data .data.*)
    . = ALIGN(4K);
  } :data

  .bss ALIGN(4K) : AT(ADDR(.bss) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_bss_start = .;
    *(.bss .bss.*)
    *(.sbss .sbss.*)
    *(COMMON)
    . = ALIGN(4K);
    __aegishv_host_page_tables_start = .;
    . += HOST_PAGE_TABLE_SIZE;
    __aegishv_host_page_tables_end = .;
    . = ALIGN(4K);
    __aegishv_bss_end = .;
  } :data

  .double_fault_guard ALIGN(4K) (NOLOAD) : AT(ADDR(.double_fault_guard) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_double_fault_guard_bottom = .;
    . += HOST_STACK_GUARD_SIZE;
    __aegishv_double_fault_guard_top = .;
  } :data

  .double_fault_stack ALIGN(4K) (NOLOAD) : AT(ADDR(.double_fault_stack) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_double_fault_stack_bottom = .;
    . += HOST_IST_STACK_SIZE;
    __aegishv_double_fault_stack_top = .;
  } :data

  .nmi_guard ALIGN(4K) (NOLOAD) : AT(ADDR(.nmi_guard) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_nmi_guard_bottom = .;
    . += HOST_STACK_GUARD_SIZE;
    __aegishv_nmi_guard_top = .;
  } :data

  .nmi_stack ALIGN(4K) (NOLOAD) : AT(ADDR(.nmi_stack) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_nmi_stack_bottom = .;
    . += HOST_IST_STACK_SIZE;
    __aegishv_nmi_stack_top = .;
  } :data

  .machine_check_guard ALIGN(4K) (NOLOAD) : AT(ADDR(.machine_check_guard) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_machine_check_guard_bottom = .;
    . += HOST_STACK_GUARD_SIZE;
    __aegishv_machine_check_guard_top = .;
  } :data

  .machine_check_stack ALIGN(4K) (NOLOAD) : AT(ADDR(.machine_check_stack) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_machine_check_stack_bottom = .;
    . += HOST_IST_STACK_SIZE;
    __aegishv_machine_check_stack_top = .;
  } :data

  .vmx_exit_guard ALIGN(4K) (NOLOAD) : AT(ADDR(.vmx_exit_guard) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_vmx_exit_guard_bottom = .;
    . += HOST_STACK_GUARD_SIZE;
    __aegishv_vmx_exit_guard_top = .;
  } :data

  .vmx_exit_stack ALIGN(4K) (NOLOAD) : AT(ADDR(.vmx_exit_stack) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_vmx_exit_stack_bottom = .;
    . += HOST_VMX_EXIT_STACK_SIZE;
    __aegishv_vmx_exit_stack_top = .;
  } :data

  .boot_stack_guard ALIGN(4K) (NOLOAD) : AT(ADDR(.boot_stack_guard) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_boot_stack_guard_bottom = .;
    . += HOST_STACK_GUARD_SIZE;
    __aegishv_boot_stack_guard_top = .;
  } :data

  .boot_stack ALIGN(4K) (NOLOAD) : AT(ADDR(.boot_stack) - KERNEL_VIRT_BASE + KERNEL_PHYS_BASE)
  {
    __aegishv_boot_stack_bottom = .;
    . += BOOT_STACK_SIZE;
    __aegishv_boot_stack_top = .;
  } :data

  __aegishv_writable_end = .;
  __aegishv_kernel_end = .;

  ASSERT((ADDR(.text) & 0xfff) == 0, "text must be page aligned")
  ASSERT((ADDR(.rodata) & 0xfff) == 0, "rodata must be page aligned")
  ASSERT((ADDR(.limine_requests) & 0xfff) == 0, "Limine requests must be page aligned")
  ASSERT((ADDR(.data) & 0xfff) == 0, "data must be page aligned")
  ASSERT((KERNEL_PHYS_BASE & (KERNEL_WINDOW_SIZE - 1)) == 0, "kernel physical base must be 2M aligned")
  ASSERT((KERNEL_VIRT_BASE & (KERNEL_WINDOW_SIZE - 1)) == 0, "kernel virtual base must be 2M aligned")
  ASSERT(__aegishv_kernel_start == KERNEL_VIRT_BASE, "kernel start moved from its paging window")
  ASSERT((__aegishv_kernel_start & 0xfff) == 0, "kernel start must be page aligned")
  ASSERT((__aegishv_kernel_end & 0xfff) == 0, "kernel end must be page aligned")
  ASSERT(__aegishv_kernel_end > __aegishv_kernel_start, "kernel image must not be empty")
  ASSERT(__aegishv_kernel_end - __aegishv_kernel_start <= KERNEL_WINDOW_SIZE, "kernel exceeds its owned paging window")
  ASSERT((__aegishv_text_start & 0xfff) == 0 && (__aegishv_text_end & 0xfff) == 0 && __aegishv_text_end > __aegishv_text_start, "text paging range is invalid")
  ASSERT((__aegishv_rodata_start & 0xfff) == 0 && (__aegishv_rodata_end & 0xfff) == 0 && __aegishv_rodata_end > __aegishv_rodata_start, "rodata paging range is invalid")
  ASSERT((__aegishv_writable_start & 0xfff) == 0 && (__aegishv_writable_end & 0xfff) == 0 && __aegishv_writable_end > __aegishv_writable_start, "writable paging range is invalid")
  ASSERT((__aegishv_host_page_tables_start & 0xfff) == 0 && (__aegishv_host_page_tables_end & 0xfff) == 0, "host page-table pool must be page aligned")
  ASSERT(__aegishv_host_page_tables_end - __aegishv_host_page_tables_start == HOST_PAGE_TABLE_SIZE, "host page-table pool size changed")
  ASSERT(__aegishv_host_page_tables_start >= __aegishv_bss_start && __aegishv_host_page_tables_end <= __aegishv_bss_end, "host page-table pool escaped bss")
  ASSERT(__aegishv_double_fault_guard_top - __aegishv_double_fault_guard_bottom == HOST_STACK_GUARD_SIZE, "double-fault guard size changed")
  ASSERT(__aegishv_nmi_guard_top - __aegishv_nmi_guard_bottom == HOST_STACK_GUARD_SIZE, "NMI guard size changed")
  ASSERT(__aegishv_machine_check_guard_top - __aegishv_machine_check_guard_bottom == HOST_STACK_GUARD_SIZE, "machine-check guard size changed")
  ASSERT(__aegishv_vmx_exit_guard_top - __aegishv_vmx_exit_guard_bottom == HOST_STACK_GUARD_SIZE, "VM-exit guard size changed")
  ASSERT(__aegishv_boot_stack_guard_top - __aegishv_boot_stack_guard_bottom == HOST_STACK_GUARD_SIZE, "boot-stack guard size changed")
  ASSERT(__aegishv_double_fault_guard_top == __aegishv_double_fault_stack_bottom, "double-fault guard must precede its stack")
  ASSERT(__aegishv_nmi_guard_top == __aegishv_nmi_stack_bottom, "NMI guard must precede its stack")
  ASSERT(__aegishv_machine_check_guard_top == __aegishv_machine_check_stack_bottom, "machine-check guard must precede its stack")
  ASSERT(__aegishv_vmx_exit_guard_top == __aegishv_vmx_exit_stack_bottom, "VM-exit guard must precede its stack")
  ASSERT(__aegishv_boot_stack_guard_top == __aegishv_boot_stack_bottom, "boot-stack guard must precede its stack")
  ASSERT(__aegishv_double_fault_stack_top - __aegishv_double_fault_stack_bottom == HOST_IST_STACK_SIZE, "double-fault stack size changed")
  ASSERT(__aegishv_nmi_stack_top - __aegishv_nmi_stack_bottom == HOST_IST_STACK_SIZE, "NMI stack size changed")
  ASSERT(__aegishv_machine_check_stack_top - __aegishv_machine_check_stack_bottom == HOST_IST_STACK_SIZE, "machine-check stack size changed")
  ASSERT(__aegishv_vmx_exit_stack_top - __aegishv_vmx_exit_stack_bottom == HOST_VMX_EXIT_STACK_SIZE, "VM-exit stack size changed")
  ASSERT(BOOT_STACK_SIZE >= 0x00020000, "boot stack is below the audited minimum")
  ASSERT(SIZEOF(.boot_stack) == BOOT_STACK_SIZE, "boot stack size changed")

  /DISCARD/ :
  {
    *(.comment)
    *(.eh_frame)
    *(.note .note.*)
  }
}
