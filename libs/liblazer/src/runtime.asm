.section .text._start,"ax"
.global _start
_start:
    mov rdi, rsp
    call __liblazer_initialize
    call __liblazer_main
1:
    jmp 1b

.section .text.user_syscall0,"ax"
.global user_syscall0
user_syscall0:
    mov rax, rdi
    int 0x80
    ret

.section .text.user_syscall1,"ax"
.global user_syscall1
user_syscall1:
    mov rax, rdi
    mov rdi, rsi
    int 0x80
    ret

.section .text.user_syscall3,"ax"
.global user_syscall3
user_syscall3:
    mov rax, rdi
    mov rdi, rsi
    mov rsi, rdx
    mov rdx, rcx
    int 0x80
    ret

.section .text.user_syscall4,"ax"
.global user_syscall4
user_syscall4:
    mov rax, rdi
    mov rdi, rsi
    mov rsi, rdx
    mov rdx, rcx
    mov rcx, r8
    int 0x80
    ret
