#![no_main]
#![no_std]

//! Kernel entry and bootstrap orchestration.
//!
//! This module ties together the early boot handoff, kernel-owned runtime
//! initialization, root filesystem mounting, and creation of the first kernel
//! and user processes. It intentionally stays small: subsystems own their own
//! policy, while `kernel_main` wires them together into the first runnable
//! system state.

#[macro_use]
mod macros;
mod arch;
mod console;
mod font;
mod io;
mod keyboard;
mod memory;
mod pci;
mod process;
mod scheduler;
mod storage;
mod syscall;
mod terminal;
mod thread;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use boot_info::{BootInfo, PixelFormat};
use memory::LoadedUserProgram;

global_asm!(include_str!("main.asm"));

#[no_mangle]
/// First Rust entrypoint after the assembly `_start` shim.
///
/// The loader hands over a validated [`BootInfo`] pointer in `rdi`. From there
/// the kernel is responsible for taking ownership of paging, console output,
/// storage discovery, and scheduler bring-up before transferring execution to
/// the first real threads.
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
    kprintln!("Welcome to lazers !!");
    kprintln!(
        "Using screen of {}x{} {}",
        boot_info.framebuffer.width,
        boot_info.framebuffer.height,
        pixel_format_name(boot_info.framebuffer.format)
    );

    let endpoint = terminal::primary_endpoint();
    let surface = terminal::TerminalSurface::new(endpoint);
    surface.begin_session();

    storage::init_root_fs()
        .unwrap_or_else(|error| panic!("failed to mount root filesystem: {}", error.as_str()));
    let user_program = load_user_program_from_disk("/bin/lash");

    scheduler::init();
    let kernel_process = scheduler::create_process(scheduler::ProcessConfig {
        name: "kernel-system",
        address_space: memory::kernel_address_space(),
        terminal_endpoint: None,
        owned_pages: memory::OwnedPages::empty(),
    });
    let user_process = scheduler::create_process(scheduler::ProcessConfig {
        name: "user-lash",
        address_space: user_program.address_space,
        terminal_endpoint: Some(endpoint),
        owned_pages: user_program.owned_pages,
    });
    let _terminal_thread = scheduler::create_kernel_thread("terminal", kernel_process, terminal_thread_entry);
    let _user_thread = scheduler::create_user_thread(
        "user-lash-main",
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
            asm!(
                include_str!("halt_forever.main.asm"),
                options(nomem, nostack, preserves_flags)
            );
        }
    }
}

/// Loads the default disk-backed user program from the runtime root filesystem.
///
/// The path is deliberately hard-coded at this stage so the kernel proves the
/// full `root fs -> ELF loader -> user process` path without yet introducing
/// session policy or shell selection.
fn load_user_program_from_disk(path: &str) -> LoadedUserProgram {
    let program_bytes = storage::read_root_file(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {}", path, error.as_str()));
    let startup = memory::ProgramStartup {
        argv0: path,
        argv_tail: &[],
    };
    let program = memory::load_user_program(program_bytes.as_slice(), &startup)
        .unwrap_or_else(|error| panic!("failed to load {}: {}", path, error.as_str()));
    program_bytes.release();
    program
}

/// Runs the fullscreen terminal service loop for the primary terminal session.
///
/// This thread stays in kernel mode because it owns hardware polling and
/// framebuffer flushing. Programs talk to it indirectly through the terminal
/// endpoint and stdio handles rather than writing to the screen directly.
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

/// Cooperative idle thread run when no other work is runnable.
///
/// It does not own any policy beyond providing a stable fallback thread for the
/// scheduler while the kernel remains preemption-free.
fn idle_thread_entry() -> ! {
    loop {
        unsafe {
            asm!(
                include_str!("idle_thread_entry.main.asm"),
                options(nomem, nostack, preserves_flags)
            );
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
