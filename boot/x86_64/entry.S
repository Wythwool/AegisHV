.section .text.entry, "ax"
.global aegishv_type1_start
.type aegishv_type1_start, @function

aegishv_type1_start:
    cli
    lea __aegishv_boot_stack_top(%rip), %rsp
    andq $-16, %rsp
    call aegishv_type1_rust_entry

.Lhalt:
    hlt
    jmp .Lhalt

.size aegishv_type1_start, . - aegishv_type1_start
