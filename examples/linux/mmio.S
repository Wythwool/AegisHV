.section .text
.global _start
_start:
    mov x0, #-100
    adr x1, path
    mov x2, #2
    mov x3, #0
    mov x8, #56
    svc #0
    tbnz x0, #63, fail
    mov x4, x0
    mov x0, #0
    mov x1, #4096
    mov x2, #3
    mov x3, #1
    mov x5, #0x09040000
    mov x8, #222
    svc #0
    tbnz x0, #63, fail
    mov w1, #65
    str w1, [x0]
fail:
    mov x0, #1
    mov x8, #93
    svc #0
.section .rodata
path:
    .asciz "/dev/mem"
