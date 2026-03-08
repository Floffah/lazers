.section .text.load_gdt_tss,"ax"
.global load_gdt_tss
load_gdt_tss:
    lgdt [rdi]
    push {kcode}
    lea rax, [rip + 1f]
    push rax
    retfq
1:
    mov ax, {kdata}
    mov ds, ax
    mov es, ax
    mov ss, ax
    xor eax, eax
    mov fs, ax
    mov gs, ax
    mov ax, {tss}
    ltr ax
    ret

.section .text.trap_invalid_opcode,"ax"
.global trap_invalid_opcode
trap_invalid_opcode:
    push 0
    push {vector_invalid}
    jmp trap_common

.section .text.trap_general_protection,"ax"
.global trap_general_protection
trap_general_protection:
    push {vector_gp}
    jmp trap_common

.section .text.trap_page_fault,"ax"
.global trap_page_fault
trap_page_fault:
    push {vector_pf}
    jmp trap_common

.section .text.trap_syscall,"ax"
.global trap_syscall
trap_syscall:
    push 0
    push {vector_syscall}
    jmp trap_common

.section .text.trap_common,"ax"
.global trap_common
trap_common:
    cld
    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rsi
    push rdi
    push rbp
    push rdx
    push rcx
    push rbx
    push rax
    mov rdi, rsp
    call rust_trap_entry
    pop rax
    pop rbx
    pop rcx
    pop rdx
    pop rbp
    pop rdi
    pop rsi
    pop r8
    pop r9
    pop r10
    pop r11
    pop r12
    pop r13
    pop r14
    pop r15
    add rsp, 16
    iretq
