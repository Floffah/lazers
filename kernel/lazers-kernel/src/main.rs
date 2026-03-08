#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod console;
mod font;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use boot_info::{BootInfo, PixelFormat};

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

    console::init(boot_info.framebuffer);
    console::clear();
    kprintln!("lazers v0.3");
    kprintln!("kernel entered");
    kprintln!("framebuffer ready");
    kprintln!(
        "{}x{} {}",
        boot_info.framebuffer.width,
        boot_info.framebuffer.height,
        pixel_format_name(boot_info.framebuffer.format)
    );

    halt_forever();
}

fn pixel_format_name(format: PixelFormat) -> &'static str {
    match format {
        PixelFormat::Rgb => "rgb",
        PixelFormat::Bgr => "bgr",
        PixelFormat::Unknown => "unknown",
    }
}

fn halt_forever() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    console::clear();
    kprintln!("lazers panic");
    kprintln!("{}", info.message());
    kprintln!("kernel halted");
    halt_forever()
}
