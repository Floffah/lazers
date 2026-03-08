cli
mov rsp, {stack_top}
xor rbp, rbp
mov rdi, {boot_info}
jmp {entry_point}
