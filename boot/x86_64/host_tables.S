.section .text.host_tables, "ax"
.p2align 4

.global aegishv_install_transition_idt
.type aegishv_install_transition_idt, @function
aegishv_install_transition_idt:
    lea rdi, [rip + __aegishv_transition_idt + 2 * 16]
    lea rax, [rip + aegishv_host_fatal]
    xor edx, edx
    call .Lset_gate

    lea rdi, [rip + __aegishv_transition_idt + 6 * 16]
    lea rax, [rip + aegishv_host_ud]
    call .Lset_gate

    lea rdi, [rip + __aegishv_transition_idt + 8 * 16]
    lea rax, [rip + aegishv_host_df]
    call .Lset_gate

    lea rdi, [rip + __aegishv_transition_idt + 13 * 16]
    lea rax, [rip + aegishv_host_gp]
    call .Lset_gate

    lea rdi, [rip + __aegishv_transition_idt + 14 * 16]
    lea rax, [rip + aegishv_host_pf]
    call .Lset_gate

    lea rdi, [rip + __aegishv_transition_idt + 18 * 16]
    lea rax, [rip + aegishv_host_mc]
    call .Lset_gate

    lidt [rip + __aegishv_transition_idtr]
    ret
.size aegishv_install_transition_idt, . - aegishv_install_transition_idt

.global aegishv_install_host_tables
.type aegishv_install_host_tables, @function
aegishv_install_host_tables:
    lea rdi, [rip + __aegishv_host_idt]
    lea rax, [rip + aegishv_host_fatal]
    xor edx, edx
    mov ecx, 256

.Lfill_idt:
    call .Lset_gate
    add rdi, 16
    dec ecx
    jnz .Lfill_idt

    lea rdi, [rip + __aegishv_host_idt + 2 * 16]
    lea rax, [rip + aegishv_host_fatal]
    mov dl, 2
    call .Lset_gate

    lea rdi, [rip + __aegishv_host_idt + 6 * 16]
    lea rax, [rip + aegishv_host_ud]
    xor edx, edx
    call .Lset_gate

    lea rdi, [rip + __aegishv_host_idt + 8 * 16]
    lea rax, [rip + aegishv_host_df]
    mov dl, 1
    call .Lset_gate

    lea rdi, [rip + __aegishv_host_idt + 13 * 16]
    lea rax, [rip + aegishv_host_gp]
    xor edx, edx
    call .Lset_gate

    lea rdi, [rip + __aegishv_host_idt + 14 * 16]
    lea rax, [rip + aegishv_host_pf]
    xor edx, edx
    call .Lset_gate

    lea rdi, [rip + __aegishv_host_idt + 18 * 16]
    lea rax, [rip + aegishv_host_mc]
    mov dl, 3
    call .Lset_gate

    lea rax, [rip + __aegishv_boot_stack_top]
    mov [rip + __aegishv_host_tss + 4], rax
    lea rax, [rip + __aegishv_double_fault_stack_top]
    mov [rip + __aegishv_host_tss + 36], rax
    lea rax, [rip + __aegishv_nmi_stack_top]
    mov [rip + __aegishv_host_tss + 44], rax
    lea rax, [rip + __aegishv_machine_check_stack_top]
    mov [rip + __aegishv_host_tss + 52], rax
    mov word ptr [rip + __aegishv_host_tss + 102], 104

    lea rax, [rip + __aegishv_host_tss]
    mov rdx, 0x0000890000000067
    mov r8, rax
    and r8, 0x00ffffff
    shl r8, 16
    or rdx, r8
    mov r8, rax
    shr r8, 24
    and r8, 0xff
    shl r8, 56
    or rdx, r8
    mov [rip + __aegishv_host_gdt + 56], rdx
    shr rax, 32
    mov dword ptr [rip + __aegishv_host_gdt + 64], eax
    mov dword ptr [rip + __aegishv_host_gdt + 68], 0

    lidt [rip + __aegishv_transition_idtr]
    lgdt [rip + __aegishv_host_gdtr]
    push 0x28
    lea rax, [rip + .Lowned_cs]
    push rax
    retfq

.Lowned_cs:
    mov ax, 0x30
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ax, 0x38
    ltr ax
    lidt [rip + __aegishv_host_idtr]
    cld
    ret
.size aegishv_install_host_tables, . - aegishv_install_host_tables

.Lset_gate:
    mov word ptr [rdi], ax
    mov word ptr [rdi + 2], 0x28
    mov byte ptr [rdi + 4], dl
    mov byte ptr [rdi + 5], 0x8e
    mov r8, rax
    shr r8, 16
    mov word ptr [rdi + 6], r8w
    shr r8, 16
    mov dword ptr [rdi + 8], r8d
    mov dword ptr [rdi + 12], 0
    ret

.global aegishv_host_ud
.type aegishv_host_ud, @function
aegishv_host_ud:
    push 0
    push 6
    jmp .Lhost_exception_common
.size aegishv_host_ud, . - aegishv_host_ud

.global aegishv_host_df
.type aegishv_host_df, @function
aegishv_host_df:
    push 8
    jmp .Lhost_exception_common
.size aegishv_host_df, . - aegishv_host_df

.global aegishv_host_gp
.type aegishv_host_gp, @function
aegishv_host_gp:
    push 13
    jmp .Lhost_exception_common
.size aegishv_host_gp, . - aegishv_host_gp

.global aegishv_host_pf
.type aegishv_host_pf, @function
aegishv_host_pf:
    push 14
    jmp .Lhost_exception_common
.size aegishv_host_pf, . - aegishv_host_pf

.global aegishv_host_mc
.type aegishv_host_mc, @function
aegishv_host_mc:
    push 0
    push 18
    jmp .Lhost_exception_common
.size aegishv_host_mc, . - aegishv_host_mc

.Lhost_exception_common:
    cli
    cld
    mov rdi, rsp
    mov rsi, cr2
    and rsp, -16
    call aegishv_type1_host_exception
    jmp .Lhost_halt

.global aegishv_host_fatal
.type aegishv_host_fatal, @function
aegishv_host_fatal:
    cli
    cld
    and rsp, -16
    call aegishv_type1_host_fatal
.Lhost_halt:
    cli
    hlt
    jmp .Lhost_halt
.size aegishv_host_fatal, . - aegishv_host_fatal

.section .data.host_tables, "aw"
.p2align 4
.global __aegishv_host_gdt
__aegishv_host_gdt:
    .quad 0x0000000000000000
    .quad 0x00009b000000ffff
    .quad 0x000093000000ffff
    .quad 0x00cf9b000000ffff
    .quad 0x00cf93000000ffff
    .quad 0x00af9b000000ffff
    .quad 0x00cf93000000ffff
    .quad 0
    .quad 0
.global __aegishv_host_gdt_end
__aegishv_host_gdt_end:

.global __aegishv_host_gdtr
__aegishv_host_gdtr:
    .word __aegishv_host_gdt_end - __aegishv_host_gdt - 1
    .quad __aegishv_host_gdt

.global __aegishv_transition_idtr
__aegishv_transition_idtr:
    .word 4095
    .quad __aegishv_transition_idt

.global __aegishv_host_idtr
__aegishv_host_idtr:
    .word 4095
    .quad __aegishv_host_idt

.p2align 4
.global __aegishv_transition_idt
__aegishv_transition_idt:
    .skip 4096

.section .bss.host_tables, "aw", @nobits
.p2align 4
.global __aegishv_host_tss
__aegishv_host_tss:
    .skip 104

.p2align 4
.global __aegishv_host_idt
__aegishv_host_idt:
.skip 4096
