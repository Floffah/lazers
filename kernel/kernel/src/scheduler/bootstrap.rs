use crate::memory::{AddressSpace, OwnedPages};
use crate::process::{ProcessExitAction, ProcessId};
use crate::terminal::TerminalEndpoint;
use crate::thread::{KernelThreadEntry, ThreadId, ThreadStart, UserThreadStart};

use super::services::{map_environment_error, EnvironmentAccessError};
use super::state::with_scheduler_mut;

/// Process creation inputs for the bootstrap runtime.
pub struct ProcessConfig {
    pub name: &'static str,
    pub address_space: AddressSpace,
    pub terminal_endpoint: Option<&'static TerminalEndpoint>,
    pub owned_pages: OwnedPages,
    pub exit_action: ProcessExitAction,
}

/// Resets the global scheduler state to an empty runtime.
pub fn init() {
    with_scheduler_mut(|scheduler| scheduler.reset());
}

/// Creates a process and installs its initial stdio bindings if a terminal
/// endpoint was supplied.
pub fn create_process(config: ProcessConfig) -> ProcessId {
    with_scheduler_mut(|scheduler| {
        scheduler
            .try_create_process(config)
            .expect("process capacity exhausted")
    })
}

/// Inserts or updates one environment variable on a specific process.
///
/// This is used during bootstrap to seed the initial user process before the
/// shell or other user programs begin mutating their own environment.
pub fn set_process_env(
    process_id: ProcessId,
    key: &str,
    value: &str,
) -> Result<(), EnvironmentAccessError> {
    with_scheduler_mut(|scheduler| {
        scheduler
            .process_mut(process_id)
            .set_env(key, value)
            .map_err(map_environment_error)
    })
}

/// Creates a kernel-mode thread owned by an existing process.
pub fn create_kernel_thread(
    name: &'static str,
    process_id: ProcessId,
    entry: KernelThreadEntry,
) -> ThreadId {
    with_scheduler_mut(|scheduler| {
        scheduler
            .try_create_thread(name, process_id, ThreadStart::Kernel(entry))
            .expect("thread capacity exhausted")
    })
}

/// Creates a user-mode thread owned by an existing process.
pub fn create_user_thread(
    name: &'static str,
    process_id: ProcessId,
    entry_point: u64,
    user_stack_top: u64,
) -> ThreadId {
    with_scheduler_mut(|scheduler| {
        scheduler
            .try_create_thread(
                name,
                process_id,
                ThreadStart::User(UserThreadStart {
                    entry_point,
                    user_stack_top,
                }),
            )
            .expect("thread capacity exhausted")
    })
}

/// Marks a previously created thread as the scheduler's idle fallback.
pub fn mark_idle_thread(thread_id: ThreadId) {
    with_scheduler_mut(|scheduler| {
        scheduler.idle_thread = Some(thread_id);
    });
}
