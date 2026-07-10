.section .text.vmx_entry, "ax"
.p2align 4

.global aegishv_vmx_launch
.type aegishv_vmx_launch, @function
aegishv_vmx_launch:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    mov rax, [rdi + 0]
    mov rbx, [rdi + 8]
    mov rcx, [rdi + 16]
    mov rdx, [rdi + 24]
    mov rbp, [rdi + 40]
    mov rsi, [rdi + 48]
    mov r8,  [rdi + 64]
    mov r9,  [rdi + 72]
    mov r10, [rdi + 80]
    mov r11, [rdi + 88]
    mov r12, [rdi + 96]
    mov r13, [rdi + 104]
    mov r14, [rdi + 112]
    mov r15, [rdi + 120]
    mov rdi, [rdi + 56]

    vmlaunch
    pushfq
    pop rax

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.size aegishv_vmx_launch, . - aegishv_vmx_launch

.p2align 4
.global aegishv_vmx_vmexit_entry
.type aegishv_vmx_vmexit_entry, @function
aegishv_vmx_vmexit_entry:
    sub rsp, 144

    mov [rsp + 0], rax
    mov [rsp + 8], rbx
    mov [rsp + 16], rcx
    mov [rsp + 24], rdx
    mov qword ptr [rsp + 32], 0
    mov [rsp + 40], rbp
    mov [rsp + 48], rsi
    mov [rsp + 56], rdi
    mov [rsp + 64], r8
    mov [rsp + 72], r9
    mov [rsp + 80], r10
    mov [rsp + 88], r11
    mov [rsp + 96], r12
    mov [rsp + 104], r13
    mov [rsp + 112], r14
    mov [rsp + 120], r15
    mov qword ptr [rsp + 128], 0

    cld
    mov rdi, rsp
    call aegishv_vmx_exit_dispatch
    test rax, rax
    jz .Lvmresume

    mov rdi, rsp
    call aegishv_vmx_exit_stop
    jmp .Lvmx_halt

.Lvmresume:
    mov rax, [rsp + 0]
    mov rbx, [rsp + 8]
    mov rcx, [rsp + 16]
    mov rdx, [rsp + 24]
    mov rbp, [rsp + 40]
    mov rsi, [rsp + 48]
    mov rdi, [rsp + 56]
    mov r8,  [rsp + 64]
    mov r9,  [rsp + 72]
    mov r10, [rsp + 80]
    mov r11, [rsp + 88]
    mov r12, [rsp + 96]
    mov r13, [rsp + 104]
    mov r14, [rsp + 112]
    mov r15, [rsp + 120]

    vmresume
    pushfq
    pop rsi
    mov rdi, rsp
    cld
    call aegishv_vmx_resume_failed
    jmp .Lvmx_halt

.Lvmx_halt:
    cli
    hlt
    jmp .Lvmx_halt
.size aegishv_vmx_vmexit_entry, . - aegishv_vmx_vmexit_entry
