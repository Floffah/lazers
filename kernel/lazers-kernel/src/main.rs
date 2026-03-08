#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod console;
mod font;
mod keyboard;

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
    kprintln!("Running lazers-kernel in suite v0.5");
    kprintln!(
        "Using screen of {}x{} {}",
        boot_info.framebuffer.width,
        boot_info.framebuffer.height,
        pixel_format_name(boot_info.framebuffer.format)
    );
    console::begin_input_region();

    run_keyboard_echo_loop();
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

fn run_keyboard_echo_loop() -> ! {
    loop {
        keyboard::poll();

        while let Some(event) = keyboard::pop_event() {
            match event.key {
                keyboard::KeyCode::Enter if event.state == keyboard::KeyState::Pressed => {
                    kprintln!();
                    console::begin_input_region();
                }
                keyboard::KeyCode::Backspace if event.state == keyboard::KeyState::Pressed => {
                    let _ = console::backspace_input();
                }
                _ => {
                    if let Some(character) = keyboard::event_to_char(event) {
                        kprint!("{}", character);
                    }
                }
            }
        }

        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
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
