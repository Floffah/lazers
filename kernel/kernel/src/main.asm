.section .text._start,"ax"
.global _start
_start:
    cli
    call kernel_main
1:
    hlt
    jmp 1b
