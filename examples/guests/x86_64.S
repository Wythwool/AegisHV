.section .text.entry, "ax"
.global _start
_start:
.if CASE == 1
    cli
1:  jmp 1b
.elseif CASE == 2
    mov byte ptr [rip + _start], 0x90
.elseif CASE == 3
    lea rax, [rip + scratch]
    mov byte ptr [rax], 0xf4
    jmp rax
.elseif CASE == 4
    mov eax, 0x40000000
    mov rax, [rax]
.else
    cmp qword ptr [rip + scratch], 0
    jne failed
    mov ecx, [rip + count]
    xor eax, eax
    mov edx, 1
1:  mov rbx, rax
    add rbx, rdx
    mov rax, rdx
    mov rdx, rbx
    dec ecx
    jnz 1b
    mov [rip + scratch], rax
    cmp qword ptr [rip + scratch], 55
    jne failed
    hlt
.endif
failed:
    ud2

.section .data, "aw"
count: .long 10
.section .bss, "aw", @nobits
.balign 8
scratch: .skip 8
