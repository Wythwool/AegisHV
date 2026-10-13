.section .text.entry, "ax"
.global _start
_start:
.if CASE == 1
    msr daifset, #15
1:  b 1b
.elseif CASE == 2
    adr x0, _start
    str wzr, [x0]
.elseif CASE == 3
    adrp x0, scratch
    add x0, x0, :lo12:scratch
    str wzr, [x0]
    br x0
.elseif CASE == 4
    mov x0, #0x80000000
    ldr x0, [x0]
.elseif CASE == 5
    mov x0, #0x40000000
    add x0, x0, #0x1000
    msr vbar_el1, x0
    hvc #0
.else
    adrp x4, scratch
    add x4, x4, :lo12:scratch
    ldr x0, [x4]
    cbnz x0, failed
    adrp x3, count
    ldr w3, [x3, :lo12:count]
    mov x1, #1
1:  add x2, x0, x1
    mov x0, x1
    mov x1, x2
    subs w3, w3, #1
    b.ne 1b
    str x0, [x4]
    ldr x0, [x4]
    cmp x0, #55
    b.ne failed
    hvc #1
.endif
failed:
    hvc #255

.section .data, "aw"
count: .long 10
.section .bss, "aw", @nobits
.balign 8
scratch: .skip 8
