BITS 64
SECTION .text
global gdt64_ptr

SECTION .rodata
align 8
gdt64:
    dq 0x0000000000000000        ; null
    dq 0x00AF9A000000FFFF        ; code: base=0, limit=FFFFF, G=1, 64-bit
    dq 0x00AF92000000FFFF        ; data: base=0, limit=FFFFF, G=1

gdt64_ptr:
    dw gdt64_end - gdt64 - 1
    dq gdt64
gdt64_end:
