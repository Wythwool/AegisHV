.section .text.entry, "ax"
.global aegishv_el2_start
aegishv_el2_start:
    msr daifset, #15
    mrs x19, mpidr_el1
    and x19, x19, #0xff
    cmp x19, #4
    b.hs .Lpark
    cbnz x19, 2f
    ldr x0, =__bss_start
    ldr x1, =__bss_end
1:  cmp x0, x1
    b.hs 2f
    stp xzr, xzr, [x0], #16
    b 1b
2:  ldr x0, =__stack_top
    mov x1, #0x1000
    movk x1, #1, lsl #16
    madd x0, x19, x1, x0
    mov sp, x0
    mrs x0, CurrentEL
    cmp x0, #8
    b.ne .Lpark
    mov x0, #0x33ff
    msr cptr_el2, x0
    ldr x0, =aegishv_el2_vectors
    msr vbar_el2, x0
    isb
    cbnz x19, .Lsecondary
    bl aegishv_el2_main
.Lsecondary:
    mov x0, x19
    bl aegishv_el2_secondary
.Lpark:
    wfe
    b .Lpark

.balign 2048
.global aegishv_el2_vectors
aegishv_el2_vectors:
.set vector, 0
.rept 16
    sub sp, sp, #816
    stp x0, x1, [sp]
    mov x0, #vector
    str x0, [sp, #800]
    b aegishv_el2_exception
    .balign 128
    .set vector, vector + 1
.endr

aegishv_el2_exception:
    stp x2, x3, [sp, #16]
    stp x4, x5, [sp, #32]
    stp x6, x7, [sp, #48]
    stp x8, x9, [sp, #64]
    stp x10, x11, [sp, #80]
    stp x12, x13, [sp, #96]
    stp x14, x15, [sp, #112]
    stp x16, x17, [sp, #128]
    stp x18, x19, [sp, #144]
    stp x20, x21, [sp, #160]
    stp x22, x23, [sp, #176]
    stp x24, x25, [sp, #192]
    stp x26, x27, [sp, #208]
    stp x28, x29, [sp, #224]
    str x30, [sp, #240]
    mrs x0, elr_el2
    str x0, [sp, #248]
    mrs x0, spsr_el2
    str x0, [sp, #256]
    mrs x0, sp_el1
    str x0, [sp, #264]
    mrs x0, fpsr
    mrs x1, fpcr
    stp x0, x1, [sp, #272]
    stp q0, q1, [sp, #288]
    stp q2, q3, [sp, #320]
    stp q4, q5, [sp, #352]
    stp q6, q7, [sp, #384]
    stp q8, q9, [sp, #416]
    stp q10, q11, [sp, #448]
    stp q12, q13, [sp, #480]
    stp q14, q15, [sp, #512]
    stp q16, q17, [sp, #544]
    stp q18, q19, [sp, #576]
    stp q20, q21, [sp, #608]
    stp q22, q23, [sp, #640]
    stp q24, q25, [sp, #672]
    stp q26, q27, [sp, #704]
    stp q28, q29, [sp, #736]
    stp q30, q31, [sp, #768]
    mov x0, sp
    ldr x1, [sp, #800]
    bl aegishv_el2_exit
aegishv_el2_restore:
    ldp q0, q1, [sp, #288]
    ldp q2, q3, [sp, #320]
    ldp q4, q5, [sp, #352]
    ldp q6, q7, [sp, #384]
    ldp q8, q9, [sp, #416]
    ldp q10, q11, [sp, #448]
    ldp q12, q13, [sp, #480]
    ldp q14, q15, [sp, #512]
    ldp q16, q17, [sp, #544]
    ldp q18, q19, [sp, #576]
    ldp q20, q21, [sp, #608]
    ldp q22, q23, [sp, #640]
    ldp q24, q25, [sp, #672]
    ldp q26, q27, [sp, #704]
    ldp q28, q29, [sp, #736]
    ldp q30, q31, [sp, #768]
    ldp x0, x1, [sp, #272]
    msr fpsr, x0
    msr fpcr, x1
    ldr x0, [sp, #248]
    msr elr_el2, x0
    ldr x0, [sp, #256]
    msr spsr_el2, x0
    ldr x0, [sp, #264]
    msr sp_el1, x0
    ldp x0, x1, [sp, #0]
    ldp x2, x3, [sp, #16]
    ldp x4, x5, [sp, #32]
    ldp x6, x7, [sp, #48]
    ldp x8, x9, [sp, #64]
    ldp x10, x11, [sp, #80]
    ldp x12, x13, [sp, #96]
    ldp x14, x15, [sp, #112]
    ldp x16, x17, [sp, #128]
    ldp x18, x19, [sp, #144]
    ldp x20, x21, [sp, #160]
    ldp x22, x23, [sp, #176]
    ldp x24, x25, [sp, #192]
    ldp x26, x27, [sp, #208]
    ldp x28, x29, [sp, #224]
    ldr x30, [sp, #240]
    add sp, sp, #816
    eret

.global aegishv_el2_enter
aegishv_el2_enter:
    sub sp, sp, #816
    mov x1, sp
    mov x2, #50
1:  ldp x3, x4, [x0], #16
    stp x3, x4, [x1], #16
    subs x2, x2, #1
    b.ne 1b
    b aegishv_el2_restore

.section .rodata.el1_guest, "a"
.balign 16
.global aegishv_el1_start
.global aegishv_el1_end
.global aegishv_el1_exec_probe
aegishv_el1_start:
    msr daifset, #15
1:  b 1b
.global aegishv_el1_after_timer
aegishv_el1_after_timer:
    mov x19, #0xabcd
    movi v0.16b, #0x5a
    mov x0, #0x40000000
    msr vbar_el1, x0
    hvc #0
    // Check the full cookie and vector register after the EL2 round trip.
    mov x1, #0xabcd
    cmp x19, x1
    b.ne .Lel1_failed
    umov x1, v0.d[0]
    mov x2, #0x5a5a
    movk x2, #0x5a5a, lsl #16
    movk x2, #0x5a5a, lsl #32
    movk x2, #0x5a5a, lsl #48
    cmp x1, x2
    b.ne .Lel1_failed
    mov x0, #0x40000000
    add x0, x0, #0x800
    str wzr, [x0]
.Lel1_failed:
    hvc #255
.global aegishv_el1_exec_probe
aegishv_el1_exec_probe:
    mov x0, #0x40000000
    add x0, x0, #0x1000
    mov w1, #0
    str w1, [x0]
    br x0
aegishv_el1_end:
