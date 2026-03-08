//! Thread metadata and saved execution context.
//!
//! A thread is the schedulable execution unit in the kernel runtime. Processes
//! own resources; threads carry the CPU context and start mode needed to run
//! within those resources.

#[repr(C)]
#[derive(Clone, Copy, Default)]
/// Callee-saved register set captured by the cooperative context switcher.
pub struct ThreadContext {
    pub rsp: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

impl ThreadContext {
    /// Returns a zeroed context suitable for bootstrap initialization.
    pub const fn zeroed() -> Self {
        Self {
            rsp: 0,
            rbx: 0,
            rbp: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Opaque scheduler-assigned identifier for a thread slot.
pub struct ThreadId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Scheduler-visible lifecycle states for a thread.
pub enum ThreadState {
    Runnable,
    Running,
    Blocked,
}

/// Function signature for kernel-mode thread entrypoints.
pub type KernelThreadEntry = fn() -> !;

#[derive(Clone, Copy)]
/// Bootstrap start mode for a newly created thread.
pub enum ThreadStart {
    Kernel(KernelThreadEntry),
    User(UserThreadStart),
}

#[derive(Clone, Copy)]
/// User-mode entry metadata prepared before the first ring transition.
pub struct UserThreadStart {
    pub entry_point: u64,
    pub user_stack_top: u64,
}

#[derive(Clone, Copy)]
/// Scheduler-owned thread record.
pub struct Thread {
    id: ThreadId,
    #[allow(dead_code)]
    name: &'static str,
    process_id: crate::process::ProcessId,
    start: ThreadStart,
    state: ThreadState,
    context: ThreadContext,
    kernel_stack_top: u64,
}

impl Thread {
    /// Constructs a runnable thread record with an already prepared kernel
    /// stack and saved context.
    pub const fn new(
        id: ThreadId,
        name: &'static str,
        process_id: crate::process::ProcessId,
        start: ThreadStart,
        context: ThreadContext,
        kernel_stack_top: u64,
    ) -> Self {
        Self {
            id,
            name,
            process_id,
            start,
            state: ThreadState::Runnable,
            context,
            kernel_stack_top,
        }
    }

    /// Returns the stable thread id for this slot.
    pub const fn id(&self) -> ThreadId {
        self.id
    }

    /// Returns the owning process of this thread.
    pub const fn process_id(&self) -> crate::process::ProcessId {
        self.process_id
    }

    /// Returns the thread's initial start contract.
    pub const fn start(&self) -> ThreadStart {
        self.start
    }

    /// Returns the thread's current scheduler state.
    pub const fn state(&self) -> ThreadState {
        self.state
    }

    /// Updates the thread's scheduler state.
    pub fn set_state(&mut self, state: ThreadState) {
        self.state = state;
    }

    /// Exposes the mutable saved context used by the assembly context switcher.
    pub fn context_mut(&mut self) -> &mut ThreadContext {
        &mut self.context
    }

    /// Returns the kernel stack top that should be loaded into the TSS before
    /// this thread runs.
    pub const fn kernel_stack_top(&self) -> u64 {
        self.kernel_stack_top
    }
}
