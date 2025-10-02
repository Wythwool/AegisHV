BITS 64
SECTION .text
extern puts

; --- VMX helpers ---
%define IA32_FEATURE_CONTROL   0x3A
%define IA32_VMX_BASIC         0x480
%define IA32_VMX_CR0_FIXED0    0x486
%define IA32_VMX_CR0_FIXED1    0x487
%define IA32_VMX_CR4_FIXED0    0x488
%define IA32_VMX_CR4_FIXED1    0x489

global vmx_init

SECTION .bss
align 4096
vmxon_region:  resb 4096

SECTION .text
vmx_init:
    ; CPUID check for VMX
    mov eax, 1
    cpuid
    bt ecx, 5
    jc .vmx_supported
    lea rdi, [rel msg_no_vmx]
    call puts
    jmp .fail

.vmx_supported:
    ; Enable VMX in CR4 (after applying fixed bits)
    ; Fix CR0
    mov ecx, IA32_VMX_CR0_FIXED0
    rdmsr
    mov r8, rax
    mov ecx, IA32_VMX_CR0_FIXED1
    rdmsr
    mov r9, rax
    mov rax, cr0
    or  rax, r8
    and rax, r9
    mov cr0, rax

    ; Fix CR4 and set VMXE
    mov ecx, IA32_VMX_CR4_FIXED0
    rdmsr
    mov r8, rax
    mov ecx, IA32_VMX_CR4_FIXED1
    rdmsr
    mov r9, rax
    mov rax, cr4
    or  rax, r8
    and rax, r9
    bts rax, 13         ; VMXE
    mov cr4, rax

    ; FEATURE_CONTROL: lock + enable VMXON outside SMX
    mov ecx, IA32_FEATURE_CONTROL
    rdmsr
    mov r8, rax
    test eax, 1
    jnz .locked
    or eax, 1 | (1<<2)
    wrmsr
.locked:
    test r8d, (1<<2)
    jnz .feature_ok
    lea rdi, [rel msg_fc]
    call puts
    jmp .fail

.feature_ok:
    ; VMX BASIC -> revision id
    mov ecx, IA32_VMX_BASIC
    rdmsr
    mov dword [vmxon_region], eax
    lea rdi, [rel msg_basic]
    call puts

    ; execute VMXON
    lea rax, [rel vmxon_region]
    vmxon [rax]
    jz .vmxon_ok
    lea rdi, [rel msg_vmxon_fail]
    call puts
    jmp .fail

.vmxon_ok:
    lea rdi, [rel msg_vmxon_ok]
    call puts

    ; We are in VMX root now. For this PoC we just VMXOFF cleanly.
    vmxoff
    lea rdi, [rel msg_vmxoff]
    call puts
    ret

.fail:
    ; hang
.halt:
    hlt
    jmp .halt

SECTION .rodata
msg_no_vmx:      db "AegisHV: VMX not supported by CPU", 13,10,0
msg_fc:          db "AegisHV: FEATURE_CONTROL not enabling VMX outside SMX", 13,10,0
msg_basic:       db "AegisHV: VMX basic rev set for VMXON region", 13,10,0
msg_vmxon_ok:    db "AegisHV: VMXON OK", 13,10,0
msg_vmxon_fail:  db "AegisHV: VMXON failed", 13,10,0
msg_vmxoff:      db "AegisHV: VMXOFF. Bye.", 13,10,0
