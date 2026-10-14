.section .text
.global _start
_start:
    ldr x0, [sp]
    cmp x0, #2
    b.ne failed
    ldr x0, [sp, #16]
    ldrb w19, [x0]
    sub x19, x19, #48
    cmp x19, #1
    b.lo failed
    cmp x19, #4
    b.hi failed
    sub sp, sp, #32
    mov x0, #0
    mov x1, #4096
    mov x2, #3
    mov x3, #0x21
    mov x4, #-1
    mov x5, #0
    mov x8, #222 // mmap: a shared counter, not fork-private memory.
    svc #0
    tbnz x0, #63, failed
    mov x20, x0
    mov x21, #0
spawn:
    mov x0, #17
    mov x1, #0
    mov x2, #0
    mov x3, #0
    mov x4, #0
    mov x8, #220 // clone(SIGCHLD)
    svc #0
    tbnz x0, #63, failed
    cbz x0, child
    add x21, x21, #1
    cmp x21, x19
    b.lo spawn
wait:
    mov x0, #-1
    mov x1, sp
    mov x2, #0
    mov x3, #0
    mov x8, #260 // wait4
    svc #0
    tbnz x0, #63, failed
    ldr w0, [sp]
    cbnz w0, failed
    subs x21, x21, #1
    b.ne wait
    mov x0, #0x86a0
    movk x0, #1, lsl #16
    mul x0, x0, x19
    ldr x1, [x20]
    cmp x0, x1
    b.ne failed
    mov x0, #1
    adr x1, success
    mov x2, #success_end-success
    mov x8, #64
    svc #0
    mov x0, #0
    b done
child:
    mov x0, #1
    lsl x0, x0, x21
    str x0, [sp]
    mov x0, #0
    mov x1, #8
    mov x2, sp
    mov x8, #122 // sched_setaffinity
    svc #0
    cbnz x0, failed
    mov x0, sp
    mov x1, #0
    mov x2, #0
    mov x8, #168 // getcpu proves that the affinity was applied.
    svc #0
    cbnz x0, failed
    ldr w0, [sp]
    cmp x0, x21
    b.ne failed
    mov x22, #0x86a0
    movk x22, #1, lsl #16
again:
    ldaxr x0, [x20]
    add x0, x0, #1
    stlxr w1, x0, [x20]
    cbnz w1, again
    subs x22, x22, #1
    b.ne again
    mov x0, #0
    b done
failed:
    mov x0, #1
done:
    mov x8, #93
    svc #0
    b .
.section .rodata
success: .ascii "aegishv-linux: atomic-workers-ok\n"
success_end:
