use core::cell::UnsafeCell;

use crate::io::{KernelObject, StdioHandles};
use crate::memory::{AddressSpace, LoadedUserProgram, OwnedPages};
use crate::process::{Process, ProcessExitAction, ProcessId};
use crate::thread::{Thread, ThreadContext, ThreadId, ThreadStart, ThreadState, UserThreadStart};

use super::bootstrap::ProcessConfig;

const MAX_PROCESSES: usize = 8;
const MAX_THREADS: usize = 12;
const THREAD_STACK_SIZE: usize = 64 * 1024;

static SCHEDULER: SchedulerCell = SchedulerCell::new();

pub(super) fn with_scheduler<F, T>(operation: F) -> T
where
    F: FnOnce(&SchedulerState) -> T,
{
    unsafe { operation(SCHEDULER.get()) }
}

pub(super) fn with_scheduler_mut<F, T>(operation: F) -> T
where
    F: FnOnce(&mut SchedulerState) -> T,
{
    unsafe { operation(SCHEDULER.get()) }
}

pub(super) struct SchedulerCell {
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

pub(super) struct SchedulerState {
    processes: [Option<Process>; MAX_PROCESSES],
    threads: [Option<Thread>; MAX_THREADS],
    stacks: [ThreadStack; MAX_THREADS],
    pub(super) current_thread: Option<ThreadId>,
    pub(super) idle_thread: Option<ThreadId>,
    pub(super) bootstrap_context: ThreadContext,
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

    pub(super) fn reset(&mut self) {
        *self = Self::new();
    }

    pub(super) fn try_create_process(&mut self, config: ProcessConfig) -> Option<ProcessId> {
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
            config.exit_action,
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

    pub(super) fn try_create_thread(
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

    pub(super) fn spawn_child_process(
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
        let mut child = Process::new(
            process_id,
            "user-child",
            address_space,
            owned_pages,
            ProcessExitAction::Continue,
        );

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

    pub(super) fn prepare_switch(&mut self, block_current: bool) -> Option<PreparedSwitch> {
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

    pub(super) fn prepare_wait_for_child(
        &mut self,
        child_process: ProcessId,
    ) -> Option<PreparedSwitch> {
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

    pub(super) fn prepare_exit_current_process(&mut self, status: usize) -> Option<ProcessExit> {
        let current_thread = self.current_thread?;
        let process_id = self.thread(current_thread).process_id();

        let (waiting_thread, exit_action) = {
            let process = self.process_mut(process_id);
            process.mark_exited(status);
            (process.take_waiting_thread(), process.exit_action())
        };

        self.threads[current_thread.0] = None;
        let released_pages = {
            let process = self.process_mut(process_id);
            process.take_owned_pages()
        };
        self.processes[process_id.0] = None;

        if matches!(exit_action, ProcessExitAction::ShutdownSystem) {
            self.current_thread = None;
            return Some(ProcessExit::Shutdown(ShutdownExit { released_pages }));
        }

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

        Some(ProcessExit::Switch(ExitedThreadSwitch {
            next_context,
            next_space: activation.address_space,
            next_stack_top: activation.kernel_stack_top,
            released_pages,
        }))
    }

    pub(super) fn next_runnable_thread(&self, current: Option<ThreadId>) -> Option<ThreadId> {
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
            (initial_rsp as *mut usize).write(super::thread_entry_trampoline as *const () as usize);
        }

        (
            ThreadContext {
                rsp: initial_rsp as u64,
                ..ThreadContext::zeroed()
            },
            aligned_top as u64,
        )
    }

    pub(super) fn activation(&self, thread_id: ThreadId) -> ThreadActivation {
        let thread = self.thread(thread_id);
        let process = self.process(thread.process_id());
        ThreadActivation {
            thread_id,
            kernel_stack_top: thread.kernel_stack_top(),
            address_space: process.address_space(),
        }
    }

    pub(super) fn process(&self, process_id: ProcessId) -> &Process {
        self.processes[process_id.0]
            .as_ref()
            .expect("invalid process id")
    }

    pub(super) fn process_mut(&mut self, process_id: ProcessId) -> &mut Process {
        self.processes[process_id.0]
            .as_mut()
            .expect("invalid process id")
    }

    pub(super) fn thread(&self, thread_id: ThreadId) -> &Thread {
        self.threads[thread_id.0]
            .as_ref()
            .expect("invalid thread id")
    }

    pub(super) fn thread_mut(&mut self, thread_id: ThreadId) -> &mut Thread {
        self.threads[thread_id.0]
            .as_mut()
            .expect("invalid thread id")
    }

    pub(super) fn thread_context(&mut self, thread_id: ThreadId) -> &mut ThreadContext {
        self.thread_mut(thread_id).context_mut()
    }
}

pub(super) struct PreparedSwitch {
    pub(super) current_context: *mut ThreadContext,
    pub(super) next_context: *const ThreadContext,
    pub(super) next_space: AddressSpace,
    pub(super) next_stack_top: u64,
}

pub(super) struct ExitedThreadSwitch {
    pub(super) next_context: *const ThreadContext,
    pub(super) next_space: AddressSpace,
    pub(super) next_stack_top: u64,
    pub(super) released_pages: OwnedPages,
}

pub(super) enum ProcessExit {
    Switch(ExitedThreadSwitch),
    Shutdown(ShutdownExit),
}

pub(super) struct ShutdownExit {
    pub(super) released_pages: OwnedPages,
}

#[derive(Clone, Copy)]
pub(super) struct ThreadActivation {
    pub(super) thread_id: ThreadId,
    pub(super) kernel_stack_top: u64,
    pub(super) address_space: AddressSpace,
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
