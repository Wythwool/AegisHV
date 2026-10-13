.section .text.svm_entry, "ax"
.global aegishv_svm_enter
.type aegishv_svm_enter, @function
// rdi = guest VMCB PA, rsi = host VMCB PA, rdx = GPRs,
// rcx = host FXSAVE area, r8 = guest FXSAVE area. BSP only.
aegishv_svm_enter:
    clgi
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15
    push rdi
    push rsi
    push rdx
    push rcx
    push r8
    fxsave64 [rcx]
    fxrstor64 [r8]
    mov rax, rsi
    vmsave
    mov rax, rdi
    vmload
    mov rbx, [rdx + 8]
    mov rcx, [rdx + 16]
    mov rbp, [rdx + 40]
    mov rsi, [rdx + 48]
    mov rdi, [rdx + 56]
    mov r8, [rdx + 64]
    mov r9, [rdx + 72]
    mov r10, [rdx + 80]
    mov r11, [rdx + 88]
    mov r12, [rdx + 96]
    mov r13, [rdx + 104]
    mov r14, [rdx + 112]
    mov r15, [rdx + 120]
    mov rdx, [rdx + 24]
    sti
    vmrun
    cli
    // VMRUN restores RSP and RAX. Save guest GPRs before using scratch registers.
    push rdx
    mov rdx, [rsp + 24]
    mov [rdx + 8], rbx
    mov [rdx + 16], rcx
    mov [rdx + 40], rbp
    mov [rdx + 48], rsi
    mov [rdx + 56], rdi
    mov [rdx + 64], r8
    mov [rdx + 72], r9
    mov [rdx + 80], r10
    mov [rdx + 88], r11
    mov [rdx + 96], r12
    mov [rdx + 104], r13
    mov [rdx + 112], r14
    mov [rdx + 120], r15
    pop rcx
    mov [rdx + 24], rcx
    vmsave
    mov rax, [rsp + 24]
    vmload
    vmsave
    mov rax, [rsp]
    fxsave64 [rax]
    mov rax, [rsp + 8]
    fxrstor64 [rax]
    add rsp, 40
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    stgi
    ret
.size aegishv_svm_enter, . - aegishv_svm_enter

.section .rodata.svm_guest, "a"
.global aegishv_svm_guest_start
.global aegishv_svm_guest_end
aegishv_svm_guest_start:
    cli
1:  jmp 1b
.global aegishv_svm_guest_after_timer
aegishv_svm_guest_after_timer:
    mov r12, 0x123456789abcdef0
    pcmpeqd xmm0, xmm0
    xor eax, eax
    cpuid
    mov eax, 1
    cpuid
    test ecx, 0x14000000
    jnz .Lguest_failed
    mov eax, 0x80000001
    cpuid
    test ecx, 4
    jnz .Lguest_failed
    // The register and SIMD values must survive a host exit.
    mov rax, 0x123456789abcdef0
    cmp r12, rax
    jne .Lguest_failed
    movq rax, xmm0
    cmp rax, -1
    jne .Lguest_failed
    mov ecx, 0xc0000082
    mov eax, 0x1100
    xor edx, edx
    wrmsr
    rdmsr
    cmp eax, 0x1100
    jne .Lguest_failed
    syscall
    cmp dword ptr [0x2800], 0x13579bdf
    jne .Lguest_failed
    // A code-page write must fault before any byte changes.
    mov byte ptr [0x1800], 0x90
    hlt
.Lguest_failed:
    ud2
.global aegishv_svm_guest_write_exec
aegishv_svm_guest_write_exec:
    mov byte ptr [0x3000], 0xf4
    mov eax, 0x3000
    jmp rax
    .fill 0x100 - (. - aegishv_svm_guest_start), 1, 0x90
    mov dword ptr [0x2800], 0x13579bdf
    jmp rcx
aegishv_svm_guest_end:
