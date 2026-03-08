#![no_main]
#![no_std]

use core::arch::global_asm;
use core::panic::PanicInfo;

const SYS_READ: usize = 0;
const SYS_WRITE: usize = 1;
const SYS_YIELD: usize = 2;
const SYS_EXIT: usize = 3;

global_asm!(
    r#"
    .section .text._start,"ax"
    .global _start
_start:
    call user_main
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
"#
);

unsafe extern "C" {
    fn user_syscall0(number: usize) -> usize;
    fn user_syscall1(number: usize, arg0: usize) -> usize;
    fn user_syscall3(number: usize, arg0: usize, arg1: usize, arg2: usize) -> usize;
}

#[no_mangle]
pub extern "C" fn user_main() -> ! {
    let mut byte = [0u8; 1];

    loop {
        let bytes_read = syscall3(SYS_READ, 0, byte.as_mut_ptr() as usize, byte.len());
        if bytes_read == 0 {
            let _ = syscall0(SYS_YIELD);
            continue;
        }

        match byte[0] {
            b'\n' | 0x7f | 0x20..=0x7e => {
                let _ = syscall3(SYS_WRITE, 1, byte.as_ptr() as usize, byte.len());
            }
            _ => {}
        }

        let _ = syscall0(SYS_YIELD);
    }
}

fn syscall0(number: usize) -> usize {
    unsafe { user_syscall0(number) }
}

fn syscall1(number: usize, arg0: usize) -> usize {
    unsafe { user_syscall1(number, arg0) }
}

fn syscall3(number: usize, arg0: usize, arg1: usize, arg2: usize) -> usize {
    unsafe { user_syscall3(number, arg0, arg1, arg2) }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    let _ = syscall1(SYS_EXIT, 1);
    loop {
        core::hint::spin_loop();
    }
}
