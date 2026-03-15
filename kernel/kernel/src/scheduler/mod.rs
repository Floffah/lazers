//! Cooperative scheduler and process-facing runtime services.
//!
//! The public `crate::scheduler` facade still owns the kernel's bootstrap
//! runtime API, but its implementation is split into focused modules:
//! bootstrap creation helpers, ABI-sensitive scheduling core, current-process
//! services, and the private global scheduler state.

use ::core::arch::global_asm;

mod bootstrap;
mod core;
mod services;
mod state;

global_asm!(include_str!("mod.asm"));

unsafe extern "C" {
    pub(super) fn context_switch(
        current: *mut crate::thread::ThreadContext,
        next: *const crate::thread::ThreadContext,
    );
}

pub use crate::process::ProcessExitAction;
pub use bootstrap::{
    create_kernel_thread, create_process, create_user_thread, init, mark_idle_thread,
    set_process_env, ProcessConfig,
};
pub use core::{exit_current_process, run_current_thread_start, start, wait_for_child, yield_now};
pub use services::{
    current_process_chdir, current_process_get_env, current_process_getcwd,
    current_process_list_env, current_process_read, current_process_read_dir,
    current_process_read_file, current_process_set_env, current_process_unset_env,
    current_process_write, spawn_user_process_and_wait, spawn_user_process_and_wait_silent,
    EnvironmentAccessError, SpawnError,
};

pub(super) extern "C" fn thread_entry_trampoline() -> ! {
    core::run_current_thread_start()
}
