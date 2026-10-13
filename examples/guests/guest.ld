ENTRY(_start)
PHDRS {
    text PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}
SECTIONS {
    . = GUEST_BASE;
    .text : { *(.text.entry) *(.text .text.*) *(.rodata .rodata.*) } :text
    . = ALIGN(4096);
    .data : { KEEP(*(.data .data.*)) } :data
    .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } :data
    /DISCARD/ : { *(.eh_frame*) *(.comment*) }
}
