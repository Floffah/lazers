#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod arch;
mod console;
mod font;
mod io;
mod keyboard;
mod memory;
mod process;
mod scheduler;
mod syscall;
mod terminal;
mod thread;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use boot_info::{BootInfo, PixelFormat};
use memory::LoadedUserProgram;

const EMBEDDED_USER_ECHO_ELF: &[u8] = include_bytes!(env!("LAZERS_USER_ECHO_ELF"));

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

    memory::init(boot_info)
        .unwrap_or_else(|error| panic!("memory init failed: {}", error.as_str()));
    arch::init();
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

    let user_program = load_embedded_user_program();

    scheduler::init();
    let kernel_process = scheduler::create_process(scheduler::ProcessConfig {
        name: "kernel-system",
        address_space: memory::kernel_address_space(),
        terminal_endpoint: None,
    });
    let user_process = scheduler::create_process(scheduler::ProcessConfig {
        name: "user-echo",
        address_space: user_program.address_space,
        terminal_endpoint: Some(endpoint),
    });
    let _terminal_thread = scheduler::create_kernel_thread("terminal", kernel_process, terminal_thread_entry);
    let _user_thread = scheduler::create_user_thread(
        "user-echo-main",
        user_process,
        user_program.entry_point,
        user_program.user_stack_top,
    );
    let idle_thread = scheduler::create_kernel_thread("idle", kernel_process, idle_thread_entry);
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

fn load_embedded_user_program() -> LoadedUserProgram {
    memory::load_user_program(EMBEDDED_USER_ECHO_ELF)
        .unwrap_or_else(|error| panic!("failed to load embedded user program: {}", error.as_str()))
}

fn terminal_thread_entry() -> ! {
    let endpoint = terminal::primary_endpoint();
    let surface = terminal::TerminalSurface::new(endpoint);

    loop {
        keyboard::poll();

        while let Some(event) = keyboard::pop_event() {
            surface.handle_key_event(event);
        }

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

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    console::clear();
    kprintln!("lazers panic");
    kprintln!("{}", info.message());
    kprintln!("kernel halted");
    halt_forever()
}
