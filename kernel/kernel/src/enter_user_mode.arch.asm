mov ax, {user_data}
mov ds, ax
mov es, ax
push {user_data}
push rsi
push 0x202
push {user_code}
push rdi
iretq
