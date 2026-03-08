#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod console;
mod font;
mod io;
mod keyboard;
mod task;
mod terminal;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use boot_info::{BootInfo, PixelFormat};
use io::{IoHandle, StdioHandles};

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

    let endpoint = terminal::primary_endpoint();
    let surface = terminal::TerminalSurface::new(endpoint);
    let text_task = task::TextTask::new(
        task::echo_task_entry,
        StdioHandles::new(
            IoHandle::terminal_input(endpoint),
            IoHandle::terminal_output(endpoint),
            IoHandle::terminal_output(endpoint),
        ),
    );
    surface.begin_session();

    run_terminal_loop(surface, text_task);
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

fn run_terminal_loop(surface: terminal::TerminalSurface, text_task: task::TextTask) -> ! {
    loop {
        keyboard::poll();

        while let Some(event) = keyboard::pop_event() {
            surface.handle_key_event(event);
        }

        text_task.run_step();
        surface.flush_output();

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
