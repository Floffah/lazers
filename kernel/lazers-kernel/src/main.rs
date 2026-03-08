#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod console;
mod font;
mod io;
mod keyboard;
mod process;
mod scheduler;
mod terminal;
mod thread;

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

    let endpoint = terminal::primary_endpoint();
    let surface = terminal::TerminalSurface::new(endpoint);
    surface.begin_session();
    scheduler::init();
    let terminal_process = scheduler::create_bootstrap_process(scheduler::BootstrapProcessConfig {
        name: "bootstrap-terminal",
        terminal_endpoint: endpoint,
    });
    let _terminal_thread =
        scheduler::create_kernel_thread("terminal", Some(terminal_process), terminal_thread_entry);
    let idle_thread = scheduler::create_kernel_thread("idle", None, idle_thread_entry);
    scheduler::mark_idle_thread(idle_thread);
    scheduler::start();
}

fn pixel_format_name(format: PixelFormat) -> &'static str {
    match format {
        PixelFormat::Rgb => "rgb",
        PixelFormat::Bgr => "bgr",
        PixelFormat::Unknown => "unknown",
    }
}

pub(crate) fn halt_forever() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

fn terminal_thread_entry() -> ! {
    let endpoint = terminal::primary_endpoint();
    let surface = terminal::TerminalSurface::new(endpoint);

    loop {
        keyboard::poll();

        while let Some(event) = keyboard::pop_event() {
            surface.handle_key_event(event);
        }

        echo_program_step();
        surface.flush_output();
        scheduler::yield_now();
    }
}

fn idle_thread_entry() -> ! {
    loop {
        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
        scheduler::yield_now();
    }
}

fn echo_program_step() {
    while let Some(byte) = scheduler::current_process_read_stdin_byte() {
        match byte {
            b'\n' => {
                let _ = scheduler::current_process_write_stdout_byte(b'\n');
            }
            0x7f => {
                let _ = scheduler::current_process_write_stdout_byte(0x7f);
            }
            0x20..=0x7e => {
                let _ = scheduler::current_process_write_stdout_byte(byte);
            }
            _ => {}
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
