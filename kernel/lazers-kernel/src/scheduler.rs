use core::arch::global_asm;
use core::cell::UnsafeCell;

use crate::io::{KernelObject, StdioHandles};
use crate::process::{Process, ProcessId};
use crate::terminal::TerminalEndpoint;
use crate::thread::{Thread, ThreadContext, ThreadEntry, ThreadId, ThreadState};

const MAX_PROCESSES: usize = 4;
const MAX_THREADS: usize = 4;
const THREAD_STACK_SIZE: usize = 16 * 1024;

static SCHEDULER: SchedulerCell = SchedulerCell::new();

global_asm!(
    r#"
    .section .text.context_switch,"ax"
    .global context_switch
context_switch:
    mov [rdi + 0x00], rsp
    mov [rdi + 0x08], rbx
    mov [rdi + 0x10], rbp
    mov [rdi + 0x18], r12
    mov [rdi + 0x20], r13
    mov [rdi + 0x28], r14
    mov [rdi + 0x30], r15

    mov rsp, [rsi + 0x00]
    mov rbx, [rsi + 0x08]
    mov rbp, [rsi + 0x10]
    mov r12, [rsi + 0x18]
    mov r13, [rsi + 0x20]
    mov r14, [rsi + 0x28]
    mov r15, [rsi + 0x30]
    ret
"#
);

unsafe extern "C" {
    fn context_switch(current: *mut ThreadContext, next: *const ThreadContext);
}

#[derive(Clone, Copy)]
pub struct BootstrapProcessConfig {
    pub name: &'static str,
    pub terminal_endpoint: &'static TerminalEndpoint,
}

pub fn init() {
    with_scheduler_mut(|scheduler| scheduler.reset());
}

pub fn create_bootstrap_process(config: BootstrapProcessConfig) -> ProcessId {
    with_scheduler_mut(|scheduler| scheduler.create_bootstrap_process(config))
}

pub fn create_kernel_thread(
    name: &'static str,
    process_id: Option<ProcessId>,
    entry: ThreadEntry,
) -> ThreadId {
    with_scheduler_mut(|scheduler| scheduler.create_thread(name, process_id, entry))
}

pub fn mark_idle_thread(thread_id: ThreadId) {
    with_scheduler_mut(|scheduler| {
        scheduler.idle_thread = Some(thread_id);
    });
}

pub fn start() -> ! {
    let next = with_scheduler_mut(|scheduler| {
        let Some(next_thread) = scheduler.next_runnable_thread(None) else {
            crate::halt_forever();
        };

        scheduler.current_thread = Some(next_thread);
        scheduler.thread_mut(next_thread).set_state(ThreadState::Running);
        next_thread
    });

    unsafe {
        with_scheduler_mut(|scheduler| {
            let next_context = scheduler.thread_context(next) as *const ThreadContext;
            context_switch(&mut scheduler.bootstrap_context as *mut ThreadContext, next_context);
        });
    }

    crate::halt_forever()
}

pub fn yield_now() {
    let switch = with_scheduler_mut(|scheduler| {
        let Some(current) = scheduler.current_thread else {
            return None;
        };

        let Some(next) = scheduler.next_runnable_thread(Some(current)) else {
            return None;
        };

        if next == current {
            return None;
        }

        scheduler.thread_mut(current).set_state(ThreadState::Runnable);
        scheduler.thread_mut(next).set_state(ThreadState::Running);
        scheduler.current_thread = Some(next);

        let current_context = scheduler.thread_context(current) as *mut ThreadContext;
        let next_context = scheduler.thread_context(next) as *const ThreadContext;

        Some((current, next, current_context, next_context))
    });

    let Some((_current, _next, current_context, next_context)) = switch else {
        return;
    };

    unsafe {
        context_switch(current_context, next_context);
    }
}

pub fn current_process_read_stdin_byte() -> Option<u8> {
    with_current_process(|process| process.read_stdin_byte()).flatten()
}

pub fn current_process_write_stdout_byte(byte: u8) -> bool {
    with_current_process(|process| process.write_stdout_byte(byte)).unwrap_or(false)
}

#[allow(dead_code)]
pub fn current_process_write_stderr_byte(byte: u8) -> bool {
    with_current_process(|process| process.write_stderr_byte(byte)).unwrap_or(false)
}

pub fn run_current_thread_entry() -> ! {
    let entry = with_scheduler(|scheduler| {
        let current = scheduler.current_thread.expect("no current thread");
        scheduler.thread(current).entry()
    });

    entry()
}

fn with_current_process<F, T>(operation: F) -> Option<T>
where
    F: FnOnce(&Process) -> T,
{
    with_scheduler(|scheduler| {
        let current = scheduler.current_thread?;
        let process_id = scheduler.thread(current).process_id()?;
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
    run_current_thread_entry()
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
            processes: [None; MAX_PROCESSES],
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

    fn create_bootstrap_process(&mut self, config: BootstrapProcessConfig) -> ProcessId {
        let slot = self
            .processes
            .iter()
            .position(|process| process.is_none())
            .expect("process capacity exhausted");
        let process_id = ProcessId(slot);
        let mut process = Process::new(process_id, config.name);

        let stdin = process
            .install_handle(KernelObject::TerminalEndpoint(config.terminal_endpoint))
            .expect("stdin handle capacity exhausted");
        let stdout = process
            .install_handle(KernelObject::TerminalEndpoint(config.terminal_endpoint))
            .expect("stdout handle capacity exhausted");
        let stderr = process
            .install_handle(KernelObject::TerminalEndpoint(config.terminal_endpoint))
            .expect("stderr handle capacity exhausted");
        process.set_stdio(StdioHandles::new(stdin, stdout, stderr));

        self.processes[slot] = Some(process);
        process_id
    }

    fn create_thread(
        &mut self,
        name: &'static str,
        process_id: Option<ProcessId>,
        entry: ThreadEntry,
    ) -> ThreadId {
        let slot = self
            .threads
            .iter()
            .position(|thread| thread.is_none())
            .expect("thread capacity exhausted");
        let thread_id = ThreadId(slot);
        let context = self.initial_context_for(slot);
        self.threads[slot] = Some(Thread::new(thread_id, name, process_id, entry, context));
        thread_id
    }

    fn next_runnable_thread(&self, current: Option<ThreadId>) -> Option<ThreadId> {
        let non_idle = self.next_non_idle_runnable_thread(current);
        if non_idle.is_some() {
            return non_idle;
        }

        self.idle_thread.filter(|thread_id| {
            self.thread(*thread_id).state() == ThreadState::Runnable
                || self.thread(*thread_id).state() == ThreadState::Running
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

    fn initial_context_for(&mut self, slot: usize) -> ThreadContext {
        let stack = &mut self.stacks[slot];
        let stack_top = stack.bytes.as_mut_ptr_range().end as usize;
        let aligned_top = stack_top & !0xf;
        let initial_rsp = aligned_top - core::mem::size_of::<usize>();
        unsafe {
            (initial_rsp as *mut usize).write(thread_entry_trampoline as *const () as usize);
        }

        ThreadContext {
            rsp: initial_rsp as u64,
            ..ThreadContext::zeroed()
        }
    }

    fn process(&self, process_id: ProcessId) -> &Process {
        self.processes[process_id.0]
            .as_ref()
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
