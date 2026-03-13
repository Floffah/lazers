//! Cooperative scheduler and bootstrap process/thread creation.
//!
//! The scheduler owns the runnable thread set, their kernel stacks, and the
//! activation data needed to switch address spaces before resuming execution.
//! It is intentionally single-core and cooperative for now: threads switch only
//! at explicit yield/block points.

use core::arch::global_asm;
use core::cell::UnsafeCell;

use crate::arch;
use crate::env::EnvironmentError;
use crate::io::{KernelObject, StdioHandles};
use crate::memory::{AddressSpace, LoadedUserProgram, OwnedPages};
use crate::process::{Process, ProcessId};
use crate::storage;
use crate::terminal::TerminalEndpoint;
use crate::thread::{
    KernelThreadEntry, Thread, ThreadContext, ThreadId, ThreadStart, ThreadState, UserThreadStart,
};

const MAX_PROCESSES: usize = 8;
const MAX_THREADS: usize = 12;
const THREAD_STACK_SIZE: usize = 64 * 1024;

static SCHEDULER: SchedulerCell = SchedulerCell::new();

global_asm!(include_str!("scheduler.asm"));

unsafe extern "C" {
    fn context_switch(current: *mut ThreadContext, next: *const ThreadContext);
}

/// Process creation inputs for the bootstrap runtime.
pub struct ProcessConfig {
    pub name: &'static str,
    pub address_space: AddressSpace,
    pub terminal_endpoint: Option<&'static TerminalEndpoint>,
    pub owned_pages: OwnedPages,
}

/// First-step child process launch failures surfaced to userspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnError {
    InvalidPath,
    FileNotFound,
    InvalidExecutable,
    ResourceUnavailable,
}

/// Failures surfaced by current-process environment helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentAccessError {
    InvalidKey,
    NotFound,
    BufferTooSmall,
    KeyTooLong,
    ValueTooLong,
    CapacityExceeded,
    ResourceUnavailable,
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

/// Loads a child executable from the runtime root filesystem, runs it with
/// inherited stdio and cwd, and blocks until it exits.
pub fn spawn_user_process_and_wait(path: &str, argv_tail: &[u8]) -> Result<usize, SpawnError> {
    spawn_user_process_and_wait_with_stdio(path, argv_tail, false)
}

/// Loads a child executable and runs it with inherited stdin and nulled stdout/stderr.
pub fn spawn_user_process_and_wait_silent(
    path: &str,
    argv_tail: &[u8],
) -> Result<usize, SpawnError> {
    spawn_user_process_and_wait_with_stdio(path, argv_tail, true)
}

fn spawn_user_process_and_wait_with_stdio(
    path: &str,
    argv_tail: &[u8],
    silent_stdio: bool,
) -> Result<usize, SpawnError> {
    let current = with_scheduler(|scheduler| scheduler.current_thread)
        .ok_or(SpawnError::ResourceUnavailable)?;
    let parent_process_id = with_scheduler(|scheduler| scheduler.thread(current).process_id());
    let mut normalized_path = [0u8; crate::process::MAX_CWD_LEN];
    let normalized_path_len = with_scheduler(|scheduler| {
        storage::normalize_path(
            scheduler.process(parent_process_id).cwd(),
            path,
            &mut normalized_path,
        )
    })
    .map_err(map_storage_spawn_error)?;
    let normalized_path = core::str::from_utf8(&normalized_path[..normalized_path_len])
        .map_err(|_| SpawnError::InvalidPath)?;

    let file = match storage::read_root_file(normalized_path) {
        Ok(file) => file,
        Err(storage::StorageError::FileNotFound | storage::StorageError::NotAFile) => {
            return Err(SpawnError::FileNotFound);
        }
        Err(
            storage::StorageError::InvalidPath
            | storage::StorageError::PathNotAbsolute
            | storage::StorageError::InvalidShortName,
        ) => {
            return Err(SpawnError::InvalidPath);
        }
        Err(_) => {
            return Err(SpawnError::ResourceUnavailable);
        }
    };
    let startup = crate::memory::ProgramStartup {
        argv0: normalized_path,
        argv_tail,
    };
    let program = match crate::memory::load_user_program(file.as_slice(), &startup) {
        Ok(program) => program,
        Err(_) => {
            file.release();
            return Err(SpawnError::InvalidExecutable);
        }
    };
    file.release();

    let child_process = match with_scheduler_mut(|scheduler| {
        scheduler.spawn_child_process(current, program, silent_stdio)
    }) {
        Ok(process_id) => process_id,
        Err(program) => {
            program.owned_pages.release();
            return Err(SpawnError::ResourceUnavailable);
        }
    };

    wait_for_child(child_process).ok_or(SpawnError::ResourceUnavailable)
}

/// Transfers control from bootstrap code into the first runnable thread.
///
/// This does not return. The scheduler selects an initial thread, activates its
/// address space and kernel stack, and then jumps into the assembly context
/// switcher using the bootstrap context as the synthetic "current" thread.
pub fn start() -> ! {
    let next = with_scheduler_mut(|scheduler| {
        let Some(thread_id) = scheduler.next_runnable_thread(None) else {
            crate::halt_forever();
        };

        scheduler.current_thread = Some(thread_id);
        scheduler
            .thread_mut(thread_id)
            .set_state(ThreadState::Running);
        scheduler.activation(thread_id)
    });

    arch::activate_address_space(next.address_space, next.kernel_stack_top);
    unsafe {
        with_scheduler_mut(|scheduler| {
            let next_context = scheduler.thread_context(next.thread_id) as *const ThreadContext;
            context_switch(
                &mut scheduler.bootstrap_context as *mut ThreadContext,
                next_context,
            );
        });
    }

    crate::halt_forever()
}

/// Cooperatively yields the CPU to another runnable thread if one exists.
pub fn yield_now() {
    let switch = with_scheduler_mut(|scheduler| scheduler.prepare_switch(false));
    let Some(switch) = switch else {
        return;
    };

    arch::activate_address_space(switch.next_space, switch.next_stack_top);
    unsafe {
        context_switch(switch.current_context, switch.next_context);
    }
}

/// Blocks the current thread until the given child process exits, then returns
/// the child's exit status.
pub fn wait_for_child(child_process: ProcessId) -> Option<usize> {
    let switch = with_scheduler_mut(|scheduler| scheduler.prepare_wait_for_child(child_process))?;

    arch::activate_address_space(switch.next_space, switch.next_stack_top);
    unsafe {
        context_switch(switch.current_context, switch.next_context);
    }

    with_scheduler_mut(|scheduler| {
        let current = scheduler.current_thread?;
        scheduler.thread_mut(current).take_wait_result()
    })
}

/// Terminates the current user process, wakes any waiting parent thread, and
/// never returns.
pub fn exit_current_process(status: usize) -> ! {
    let switch = with_scheduler_mut(|scheduler| scheduler.prepare_exit_current_process(status));
    let Some(switch) = switch else {
        crate::halt_forever();
    };

    arch::activate_address_space(switch.next_space, switch.next_stack_top);
    let _ = switch.released_pages;
    unsafe {
        with_scheduler_mut(|scheduler| {
            context_switch(
                &mut scheduler.bootstrap_context as *mut ThreadContext,
                switch.next_context,
            );
        });
    }

    crate::halt_forever()
}

/// Reads from the current thread's process-owned standard streams.
pub fn current_process_read(fd: usize, buffer: &mut [u8]) -> usize {
    with_current_process(|process| process.read(fd, buffer)).unwrap_or(0)
}

/// Writes to the current thread's process-owned standard streams.
pub fn current_process_write(fd: usize, buffer: &[u8]) -> usize {
    with_current_process(|process| process.write(fd, buffer)).unwrap_or(0)
}

/// Copies the current process cwd into a caller-provided buffer.
pub fn current_process_getcwd(buffer: &mut [u8]) -> Option<usize> {
    with_current_process(|process| process.copy_cwd_into(buffer)).flatten()
}

/// Looks up one environment variable in the current process and copies it into
/// a caller-provided buffer.
pub fn current_process_get_env(
    key: &str,
    buffer: &mut [u8],
) -> Result<usize, EnvironmentAccessError> {
    with_scheduler(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(EnvironmentAccessError::ResourceUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        let process = scheduler.process(process_id);
        let value = process.env(key).map_err(map_environment_error)?;
        let Some(value) = value else {
            return Err(EnvironmentAccessError::NotFound);
        };

        if buffer.len() < value.len() {
            return Err(EnvironmentAccessError::BufferTooSmall);
        }

        buffer[..value.len()].copy_from_slice(value.as_bytes());
        Ok(value.len())
    })
}

/// Serializes the current process environment into a caller-provided buffer.
pub fn current_process_list_env(buffer: &mut [u8]) -> Result<usize, EnvironmentAccessError> {
    with_scheduler(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(EnvironmentAccessError::ResourceUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        scheduler
            .process(process_id)
            .list_env_into(buffer)
            .map_err(map_environment_error)
    })
}

/// Inserts or updates one environment variable on the current process.
pub fn current_process_set_env(key: &str, value: &str) -> Result<(), EnvironmentAccessError> {
    with_scheduler_mut(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(EnvironmentAccessError::ResourceUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        scheduler
            .process_mut(process_id)
            .set_env(key, value)
            .map_err(map_environment_error)
    })
}

/// Removes one environment variable from the current process.
pub fn current_process_unset_env(key: &str) -> Result<(), EnvironmentAccessError> {
    with_scheduler_mut(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(EnvironmentAccessError::ResourceUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        match scheduler
            .process_mut(process_id)
            .remove_env(key)
            .map_err(map_environment_error)?
        {
            true => Ok(()),
            false => Err(EnvironmentAccessError::NotFound),
        }
    })
}

/// Resolves and installs a new cwd for the current process.
pub fn current_process_chdir(path: &str) -> Result<(), storage::StorageError> {
    with_scheduler_mut(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(storage::StorageError::RootFsUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        let current_cwd = scheduler.process(process_id).cwd();

        let mut normalized = [0u8; crate::process::MAX_CWD_LEN];
        let normalized_len = storage::normalize_path(current_cwd, path, &mut normalized)?;
        let normalized_path = core::str::from_utf8(&normalized[..normalized_len])
            .map_err(|_| storage::StorageError::InvalidPath)?;

        storage::ensure_root_dir(normalized_path)?;
        scheduler
            .process_mut(process_id)
            .set_cwd(normalized_path)
            .ok_or(storage::StorageError::InvalidPath)
    })
}

/// Reads one runtime path file from the current process' cwd context.
pub fn current_process_read_file(
    path: &str,
    buffer: &mut [u8],
) -> Result<usize, storage::StorageError> {
    with_scheduler(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(storage::StorageError::RootFsUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        let current_cwd = scheduler.process(process_id).cwd();

        let mut normalized = [0u8; crate::process::MAX_CWD_LEN];
        let normalized_len = storage::normalize_path(current_cwd, path, &mut normalized)?;
        let normalized_path = core::str::from_utf8(&normalized[..normalized_len])
            .map_err(|_| storage::StorageError::InvalidPath)?;
        storage::read_root_file_into(normalized_path, buffer)
    })
}

/// Lists one runtime path directory from the current process' cwd context.
pub fn current_process_read_dir(
    path: &str,
    buffer: &mut [u8],
) -> Result<usize, storage::StorageError> {
    with_scheduler(|scheduler| {
        let current = scheduler
            .current_thread
            .ok_or(storage::StorageError::RootFsUnavailable)?;
        let process_id = scheduler.thread(current).process_id();
        let current_cwd = scheduler.process(process_id).cwd();

        let mut normalized = [0u8; crate::process::MAX_CWD_LEN];
        let normalized_len = storage::normalize_path(current_cwd, path, &mut normalized)?;
        let normalized_path = core::str::from_utf8(&normalized[..normalized_len])
            .map_err(|_| storage::StorageError::InvalidPath)?;
        storage::read_root_dir(normalized_path, buffer)
    })
}

/// Dispatches the current thread's configured start contract.
///
/// Kernel threads jump to a Rust entrypoint, while user threads transition
/// through the architecture layer into ring 3.
pub fn run_current_thread_start() -> ! {
    let start = with_scheduler(|scheduler| {
        let current = scheduler.current_thread.expect("no current thread");
        scheduler.thread(current).start()
    });

    match start {
        ThreadStart::Kernel(entry) => entry(),
        ThreadStart::User(user) => arch::enter_user_mode(user.entry_point, user.user_stack_top),
    }
}

fn with_current_process<F, T>(operation: F) -> Option<T>
where
    F: FnOnce(&Process) -> T,
{
    with_scheduler(|scheduler| {
        let current = scheduler.current_thread?;
        let process_id = scheduler.thread(current).process_id();
        Some(operation(scheduler.process(process_id)))
    })
}

fn with_scheduler<F, T>(operation: F) -> T
where
    F: FnOnce(&SchedulerState) -> T,
{
    unsafe { operation(SCHEDULER.get()) }
}

fn with_scheduler_mut<F, T>(operation: F) -> T
where
    F: FnOnce(&mut SchedulerState) -> T,
{
    unsafe { operation(SCHEDULER.get()) }
}

pub extern "C" fn thread_entry_trampoline() -> ! {
    run_current_thread_start()
}

struct SchedulerCell {
    state: UnsafeCell<SchedulerState>,
}

impl SchedulerCell {
    const fn new() -> Self {
        Self {
            state: UnsafeCell::new(SchedulerState::new()),
        }
    }

    unsafe fn get(&self) -> &mut SchedulerState {
        &mut *self.state.get()
    }
}

unsafe impl Sync for SchedulerCell {}

struct SchedulerState {
    processes: [Option<Process>; MAX_PROCESSES],
    threads: [Option<Thread>; MAX_THREADS],
    stacks: [ThreadStack; MAX_THREADS],
    current_thread: Option<ThreadId>,
    idle_thread: Option<ThreadId>,
    bootstrap_context: ThreadContext,
}

impl SchedulerState {
    const fn new() -> Self {
        Self {
            processes: [const { None }; MAX_PROCESSES],
            threads: [None; MAX_THREADS],
            stacks: [ThreadStack::new(); MAX_THREADS],
            current_thread: None,
            idle_thread: None,
            bootstrap_context: ThreadContext::zeroed(),
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn try_create_process(&mut self, config: ProcessConfig) -> Option<ProcessId> {
        let slot = self
            .processes
            .iter()
            .position(|process| process.is_none())?;
        let process_id = ProcessId(slot);
        let mut process = Process::new(
            process_id,
            config.name,
            config.address_space,
            config.owned_pages,
        );

        if let Some(endpoint) = config.terminal_endpoint {
            let stdin = process.install_handle(KernelObject::TerminalEndpoint(endpoint))?;
            let stdout = process.install_handle(KernelObject::TerminalEndpoint(endpoint))?;
            let stderr = process.install_handle(KernelObject::TerminalEndpoint(endpoint))?;
            process.set_stdio(StdioHandles::new(stdin, stdout, stderr));
        }

        self.processes[slot] = Some(process);
        Some(process_id)
    }

    fn try_create_thread(
        &mut self,
        name: &'static str,
        process_id: ProcessId,
        start: ThreadStart,
    ) -> Option<ThreadId> {
        let slot = self.threads.iter().position(|thread| thread.is_none())?;
        let thread_id = ThreadId(slot);
        let (context, kernel_stack_top) = self.initial_context_for(slot);
        self.threads[slot] = Some(Thread::new(
            thread_id,
            name,
            process_id,
            start,
            context,
            kernel_stack_top,
        ));
        Some(thread_id)
    }

    fn spawn_child_process(
        &mut self,
        parent_thread: ThreadId,
        program: LoadedUserProgram,
        silent_stdio: bool,
    ) -> Result<ProcessId, LoadedUserProgram> {
        let LoadedUserProgram {
            address_space,
            entry_point,
            user_stack_top,
            owned_pages,
        } = program;
        let parent_process_id = self.thread(parent_thread).process_id();
        let Some(slot) = self.processes.iter().position(|process| process.is_none()) else {
            return Err(LoadedUserProgram {
                address_space,
                entry_point,
                user_stack_top,
                owned_pages,
            });
        };
        let process_id = ProcessId(slot);
        let mut child = Process::new(process_id, "user-child", address_space, owned_pages);

        let inherited_stdio = if silent_stdio {
            self.process(parent_process_id)
                .inherit_stdio_silent_into(&mut child)
        } else {
            self.process(parent_process_id)
                .inherit_stdio_into(&mut child)
        };

        if inherited_stdio.is_none() {
            return Err(LoadedUserProgram {
                address_space: child.address_space(),
                entry_point,
                user_stack_top,
                owned_pages: child.take_owned_pages(),
            });
        }

        if self
            .process(parent_process_id)
            .inherit_cwd_into(&mut child)
            .is_none()
        {
            return Err(LoadedUserProgram {
                address_space: child.address_space(),
                entry_point,
                user_stack_top,
                owned_pages: child.take_owned_pages(),
            });
        }

        if self
            .process(parent_process_id)
            .inherit_env_into(&mut child)
            .is_err()
        {
            return Err(LoadedUserProgram {
                address_space: child.address_space(),
                entry_point,
                user_stack_top,
                owned_pages: child.take_owned_pages(),
            });
        }

        child.set_waiting_thread(parent_thread);
        self.processes[slot] = Some(child);
        if self
            .try_create_thread(
                "user-child-main",
                process_id,
                ThreadStart::User(UserThreadStart {
                    entry_point,
                    user_stack_top,
                }),
            )
            .is_none()
        {
            let address_space = self.process(process_id).address_space();
            let owned_pages = {
                let child = self.process_mut(process_id);
                child.take_owned_pages()
            };
            self.processes[slot] = None;
            return Err(LoadedUserProgram {
                address_space,
                entry_point,
                user_stack_top,
                owned_pages,
            });
        }

        Ok(process_id)
    }

    fn prepare_switch(&mut self, block_current: bool) -> Option<PreparedSwitch> {
        let current = self.current_thread?;
        let next = self.next_runnable_thread(Some(current))?;

        if next == current && !block_current {
            return None;
        }

        let current_state = if block_current {
            ThreadState::Blocked
        } else {
            ThreadState::Runnable
        };
        self.thread_mut(current).set_state(current_state);
        self.thread_mut(next).set_state(ThreadState::Running);
        self.current_thread = Some(next);

        let current_context = self.thread_context(current) as *mut ThreadContext;
        let next_context = self.thread_context(next) as *const ThreadContext;
        let activation = self.activation(next);

        Some(PreparedSwitch {
            current_context,
            next_context,
            next_space: activation.address_space,
            next_stack_top: activation.kernel_stack_top,
        })
    }

    fn prepare_wait_for_child(&mut self, child_process: ProcessId) -> Option<PreparedSwitch> {
        let current = self.current_thread?;
        self.thread_mut(current).block_for_child(child_process);

        let next = self.next_runnable_thread(Some(current))?;
        self.thread_mut(next).set_state(ThreadState::Running);
        self.current_thread = Some(next);

        let current_context = self.thread_context(current) as *mut ThreadContext;
        let next_context = self.thread_context(next) as *const ThreadContext;
        let activation = self.activation(next);

        Some(PreparedSwitch {
            current_context,
            next_context,
            next_space: activation.address_space,
            next_stack_top: activation.kernel_stack_top,
        })
    }

    fn prepare_exit_current_process(&mut self, status: usize) -> Option<ExitedThreadSwitch> {
        let current_thread = self.current_thread?;
        let process_id = self.thread(current_thread).process_id();

        let waiting_thread = {
            let process = self.process_mut(process_id);
            process.mark_exited(status);
            process.take_waiting_thread()
        };

        self.threads[current_thread.0] = None;
        let released_pages = {
            let process = self.process_mut(process_id);
            process.take_owned_pages()
        };
        self.processes[process_id.0] = None;

        let next = if let Some(waiting_thread) = waiting_thread {
            let thread = self.thread_mut(waiting_thread);
            thread.set_wait_result(status);
            thread.wake();
            waiting_thread
        } else {
            self.next_runnable_thread(Some(current_thread))?
        };

        self.current_thread = Some(next);
        self.thread_mut(next).set_state(ThreadState::Running);
        let activation = self.activation(next);
        let next_context = self.thread_context(next) as *const ThreadContext;

        Some(ExitedThreadSwitch {
            next_context,
            next_space: activation.address_space,
            next_stack_top: activation.kernel_stack_top,
            released_pages,
        })
    }

    fn next_runnable_thread(&self, current: Option<ThreadId>) -> Option<ThreadId> {
        let non_idle = self.next_non_idle_runnable_thread(current);
        if non_idle.is_some() {
            return non_idle;
        }

        self.idle_thread.filter(|thread_id| {
            matches!(
                self.thread(*thread_id).state(),
                ThreadState::Runnable | ThreadState::Running
            )
        })
    }

    fn next_non_idle_runnable_thread(&self, current: Option<ThreadId>) -> Option<ThreadId> {
        let start = current.map_or(0, |thread_id| (thread_id.0 + 1) % self.threads.len());
        let mut offset = 0;
        while offset < self.threads.len() {
            let index = (start + offset) % self.threads.len();
            let Some(thread) = self.threads[index] else {
                offset += 1;
                continue;
            };

            if Some(thread.id()) == self.idle_thread {
                offset += 1;
                continue;
            }

            if matches!(thread.state(), ThreadState::Runnable | ThreadState::Running) {
                return Some(thread.id());
            }

            offset += 1;
        }

        None
    }

    fn initial_context_for(&mut self, slot: usize) -> (ThreadContext, u64) {
        let stack = &mut self.stacks[slot];
        let stack_top = stack.bytes.as_mut_ptr_range().end as usize;
        let aligned_top = stack_top & !0xf;
        let initial_rsp = aligned_top - core::mem::size_of::<usize>();
        unsafe {
            (initial_rsp as *mut usize).write(thread_entry_trampoline as *const () as usize);
        }

        (
            ThreadContext {
                rsp: initial_rsp as u64,
                ..ThreadContext::zeroed()
            },
            aligned_top as u64,
        )
    }

    fn activation(&self, thread_id: ThreadId) -> ThreadActivation {
        let thread = self.thread(thread_id);
        let process = self.process(thread.process_id());
        ThreadActivation {
            thread_id,
            kernel_stack_top: thread.kernel_stack_top(),
            address_space: process.address_space(),
        }
    }

    fn process(&self, process_id: ProcessId) -> &Process {
        self.processes[process_id.0]
            .as_ref()
            .expect("invalid process id")
    }

    fn process_mut(&mut self, process_id: ProcessId) -> &mut Process {
        self.processes[process_id.0]
            .as_mut()
            .expect("invalid process id")
    }

    fn thread(&self, thread_id: ThreadId) -> &Thread {
        self.threads[thread_id.0]
            .as_ref()
            .expect("invalid thread id")
    }

    fn thread_mut(&mut self, thread_id: ThreadId) -> &mut Thread {
        self.threads[thread_id.0]
            .as_mut()
            .expect("invalid thread id")
    }

    fn thread_context(&mut self, thread_id: ThreadId) -> &mut ThreadContext {
        self.thread_mut(thread_id).context_mut()
    }
}

struct PreparedSwitch {
    current_context: *mut ThreadContext,
    next_context: *const ThreadContext,
    next_space: AddressSpace,
    next_stack_top: u64,
}

struct ExitedThreadSwitch {
    next_context: *const ThreadContext,
    next_space: AddressSpace,
    next_stack_top: u64,
    released_pages: OwnedPages,
}

#[derive(Clone, Copy)]
struct ThreadActivation {
    thread_id: ThreadId,
    kernel_stack_top: u64,
    address_space: AddressSpace,
}

#[derive(Clone, Copy)]
#[repr(C, align(16))]
struct ThreadStack {
    bytes: [u8; THREAD_STACK_SIZE],
}

impl ThreadStack {
    const fn new() -> Self {
        Self {
            bytes: [0; THREAD_STACK_SIZE],
        }
    }
}

fn map_storage_spawn_error(error: storage::StorageError) -> SpawnError {
    match error {
        storage::StorageError::InvalidPath
        | storage::StorageError::PathNotAbsolute
        | storage::StorageError::InvalidShortName => SpawnError::InvalidPath,
        storage::StorageError::FileNotFound | storage::StorageError::NotAFile => {
            SpawnError::FileNotFound
        }
        _ => SpawnError::ResourceUnavailable,
    }
}

fn map_environment_error(error: EnvironmentError) -> EnvironmentAccessError {
    match error {
        EnvironmentError::InvalidKey => EnvironmentAccessError::InvalidKey,
        EnvironmentError::KeyTooLong => EnvironmentAccessError::KeyTooLong,
        EnvironmentError::ValueTooLong => EnvironmentAccessError::ValueTooLong,
        EnvironmentError::CapacityExceeded => EnvironmentAccessError::CapacityExceeded,
    }
}
