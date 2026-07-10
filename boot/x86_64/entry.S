.section .text.entry, "ax"
.global aegishv_type1_start
.type aegishv_type1_start, @function

aegishv_type1_start:
    cli
    cld
    call aegishv_install_transition_idt

    lea __aegishv_bss_start(%rip), %rdi
    lea __aegishv_bss_end(%rip), %rcx
    subq %rdi, %rcx
    xorl %eax, %eax
    rep stosb

    lea __aegishv_boot_stack_top(%rip), %rsp
    andq $-16, %rsp
    call aegishv_install_host_tables
    call aegishv_type1_rust_entry

.Lhalt:
    hlt
    jmp .Lhalt

.size aegishv_type1_start, . - aegishv_type1_start
