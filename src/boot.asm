BITS 32
SECTION .multiboot2 align=8
align 8
multiboot2_header:
    dd 0xE85250D6
    dd 0 ; architecture (i386)
    dd multiboot2_end - multiboot2_header ; header length
    dd 0

align 8
    dw 0
    dw 0
    dd 8
multiboot2_end:

SECTION .text
global start
extern vmx_init

; Simple I/O helpers
%define COM1 0x3F8

start:
    ; fix header checksum (lazy, but fine here)
    mov eax, dword [multiboot2_header]
    mov edx, dword [multiboot2_header+4]
    mov ecx, dword [multiboot2_header+8]
    add eax, edx
    add eax, ecx
    neg eax
    mov dword [multiboot2_header+12], eax

    cli
    ; set up a basic GDT and enter protected mode is already set by GRUB
    ; switch to long mode:

    ; enable PAE
    mov eax, cr4
    or eax, 1<<5
    mov cr4, eax

    ; enable LME via EFER
    mov ecx, 0xC0000080
    rdmsr
    or eax, (1<<8)
    wrmsr

    ; build a trivial 1:1 PML4/PDPT/PD for long mode (first 1GiB via 2MiB pages)
    ; We keep it extremely small: just enough to run our code.
SECTION .bss
align 4096
pml4:    resq 512
pdpt:    resq 512
pd:      resq 512
SECTION .text

    ; zero tables
    lea edi, [pml4]
    mov ecx, (512*3*8)/4
    xor eax, eax
    rep stosd

    ; PD entries: 2MiB pages covering first 1GiB
    ; P = 1, RW =1, PS=1, NX=0
    mov ecx, 512
    mov rbx, 0
.make_pd:
    mov rax, rbx
    or rax, (1<<7) | 0x3       ; PS + P/RW
    mov [pd + (rcx-512)*8], rax
    add rbx, (2*1024*1024)
    loop .make_pd

    ; PDPT[0] -> PD
    lea rax, [pd]
    or rax, 0x3
    mov [pdpt], rax

    ; PML4[0] -> PDPT
    lea rax, [pdpt]
    or rax, 0x3
    mov [pml4], rax

    ; load CR3
    lea eax, [pml4]
    mov cr3, eax

    ; enable PG + PE if not already
    mov eax, cr0
    or eax, (1<<31) | 1
    mov cr0, eax

    ; set up GDT for 64-bit
    extern gdt64_ptr
    lgdt [gdt64_ptr]

    ; long jump into 64-bit code segment
    jmp 0x08:long_mode_entry

BITS 64
long_mode_entry:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax

    ; set up a stack
    mov rsp, 0x90000

    ; init COM1
    call com1_init

    ; print banner
    lea rdi, [rel msg1]
    call puts

    ; jump to VMX init
    call vmx_init

    lea rdi, [rel msg_ok]
    call puts

.halt:
    hlt
    jmp .halt

com1_init:
    mov dx, COM1+1
    xor al, al
    out dx, al
    mov dx, COM1+3
    mov al, 0x80
    out dx, al
    mov dx, COM1
    mov al, 0x01
    out dx, al
    mov dx, COM1+1
    xor al, al
    out dx, al
    mov dx, COM1+3
    mov al, 0x03
    out dx, al
    mov dx, COM1+2
    mov al, 0xC7
    out dx, al
    mov dx, COM1+4
    mov al, 0x0B
    out dx, al
    lea rdi, [rel msg_serial]
    call puts
    ret

; write char in AL
putc:
    push rdx
.wait:
    mov dx, COM1+5
    in al, dx
    test al, 0x20
    jz .wait
    pop rdx
    mov dx, COM1
    xchg al, dil
    out dx, al
    ret

; write zero-terminated string at RDI
puts:
    push rax
    .next:
        mov al, [rdi]
        test al, al
        jz .done
        call putc
        inc rdi
        jmp .next
    .done:
    pop rax
    ret

SECTION .rodata
msg1:      db "AegisHV: entering long mode...", 13,10,0
msg_serial:db "AegisHV: COM1 ready", 13,10,0
msg_ok:    db "AegisHV: done. Halting.", 13,10,0
