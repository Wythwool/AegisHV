.global _start
.extern kmain
.section .text
_start:
    /* We rely on GRUB to place us in 64‑bit mode with paging. Set up a tiny stack. */
    lea stack_top(%rip), %rsp
    xor %rbp, %rbp
    call kmain
.hang:
    hlt
    jmp .hang

.bss
.align 16
stack:
    .skip 16384
stack_top:
