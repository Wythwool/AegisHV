BITS 64
SECTION .text
global gdt64_ptr

SECTION .rodata
align 8
gdt64:
    dq 0x0000000000000000
    dq 0x00AF9A000000FFFF
    dq 0x00AF92000000FFFF

gdt64_ptr:
    dw gdt64_end - gdt64 - 1
    dq gdt64
gdt64_end:
