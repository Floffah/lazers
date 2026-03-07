#![no_main]
#![no_std]

mod console;
mod font;

use boot_info::{BootInfo, FramebufferInfo, PixelFormat};
use console::FramebufferConsole;
use core::arch::{asm, global_asm};
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

const BACKGROUND_COLOR: u32 = 0x111827;
const FOREGROUND_COLOR: u32 = 0xf9fafb;

static PANIC_FRAMEBUFFER: PanicFramebufferCell = PanicFramebufferCell::new();

global_asm!(
    r#"
    .section .text._start,"ax"
    .global _start
_start:
    cli
    call kernel_main
1:
    hlt
    jmp 1b
"#
);

#[no_mangle]
pub extern "sysv64" fn kernel_main(boot_info: *const BootInfo) -> ! {
    let Some(boot_info) = (unsafe { boot_info.as_ref() }) else {
        halt_forever();
    };

    if !boot_info.has_valid_header() || !boot_info.framebuffer.is_usable() {
        halt_forever();
    }

    PANIC_FRAMEBUFFER.initialize(boot_info.framebuffer);

    let mut console =
        FramebufferConsole::new(boot_info.framebuffer, FOREGROUND_COLOR, BACKGROUND_COLOR);
    console.clear();
    console.write_line("lazers v0.2");
    console.write_line("kernel entered");
    console.write_line("framebuffer ready");
    write_framebuffer_mode_line(&mut console, &boot_info.framebuffer);

    halt_forever();
}

fn write_framebuffer_mode_line(console: &mut FramebufferConsole, framebuffer: &FramebufferInfo) {
    let mut bytes = [0u8; 32];
    let mut length = 0;

    length += write_u32_decimal(&mut bytes[length..], framebuffer.width);
    bytes[length] = b'x';
    length += 1;
    length += write_u32_decimal(&mut bytes[length..], framebuffer.height);
    bytes[length] = b' ';
    length += 1;

    let format_name = match framebuffer.format {
        PixelFormat::Rgb => b"rgb".as_slice(),
        PixelFormat::Bgr => b"bgr".as_slice(),
        PixelFormat::Unknown => b"unknown".as_slice(),
    };
    bytes[length..length + format_name.len()].copy_from_slice(format_name);
    length += format_name.len();

    let text = unsafe { core::str::from_utf8_unchecked(&bytes[..length]) };
    console.write_line(text);
}

fn write_u32_decimal(output: &mut [u8], value: u32) -> usize {
    if value == 0 {
        output[0] = b'0';
        return 1;
    }

    let mut digits = [0u8; 10];
    let mut remaining = value;
    let mut count = 0;

    while remaining != 0 {
        digits[count] = b'0' + (remaining % 10) as u8;
        remaining /= 10;
        count += 1;
    }

    for index in 0..count {
        output[index] = digits[count - 1 - index];
    }

    count
}

fn render_panic_screen() {
    let Some(framebuffer) = PANIC_FRAMEBUFFER.get() else {
        return;
    };

    let mut console = FramebufferConsole::new(framebuffer, FOREGROUND_COLOR, BACKGROUND_COLOR);
    console.clear();
    console.write_line("lazers panic");
    console.write_line("kernel halted");
}

struct PanicFramebufferCell {
    initialized: AtomicBool,
    framebuffer: UnsafeCell<MaybeUninit<FramebufferInfo>>,
}

impl PanicFramebufferCell {
    const fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            framebuffer: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    fn initialize(&self, framebuffer: FramebufferInfo) {
        unsafe {
            (*self.framebuffer.get()).write(framebuffer);
        }
        self.initialized.store(true, Ordering::Release);
    }

    fn get(&self) -> Option<FramebufferInfo> {
        if !self.initialized.load(Ordering::Acquire) {
            return None;
        }

        Some(unsafe { (*self.framebuffer.get()).assume_init_read() })
    }
}

unsafe impl Sync for PanicFramebufferCell {}

fn halt_forever() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    render_panic_screen();
    halt_forever()
}
