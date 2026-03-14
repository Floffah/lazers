use core::slice;
use core::str;

#[cfg(not(test))]
use crate::process::exit;

#[cfg(target_arch = "x86_64")]
use core::arch::global_asm;
#[cfg(not(test))]
use core::panic::PanicInfo;

#[cfg(target_arch = "x86_64")]
global_asm!(include_str!("runtime.asm"));

#[derive(Clone, Copy)]
struct StartupArgs {
    argc: usize,
    argv: *const usize,
}

static mut STARTUP_ARGS: StartupArgs = StartupArgs {
    argc: 0,
    argv: core::ptr::null(),
};

/// Returns the current process arguments.
pub fn args() -> Args {
    let startup = unsafe { STARTUP_ARGS };
    Args {
        index: 0,
        argc: startup.argc,
        argv: startup.argv,
    }
}

/// Iterator over the current process arguments.
pub struct Args {
    index: usize,
    argc: usize,
    argv: *const usize,
}

impl Iterator for Args {
    type Item = &'static str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.argc {
            return None;
        }

        let pointer = unsafe { *self.argv.add(self.index) } as *const u8;
        let length = c_string_len(pointer);
        let bytes = unsafe { slice::from_raw_parts(pointer, length) };
        let value = str::from_utf8(bytes).ok()?;
        self.index += 1;
        Some(value)
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(1)
}

#[unsafe(no_mangle)]
extern "Rust" fn __liblazer_initialize(stack_top: usize) {
    let argc = unsafe { *(stack_top as *const usize) };
    let argv = unsafe { (stack_top as *const usize).add(1) };
    unsafe {
        STARTUP_ARGS = StartupArgs { argc, argv };
    }
}

pub(crate) fn c_string_len(pointer: *const u8) -> usize {
    let mut length = 0usize;
    loop {
        let byte = unsafe { *pointer.add(length) };
        if byte == 0 {
            return length;
        }
        length += 1;
    }
}
