#[repr(C)]
#[derive(Clone, Copy, Default)]
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
pub struct ThreadId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadState {
    Runnable,
    Running,
    Blocked,
}

pub type KernelThreadEntry = fn() -> !;

#[derive(Clone, Copy)]
pub enum ThreadStart {
    Kernel(KernelThreadEntry),
    User(UserThreadStart),
}

#[derive(Clone, Copy)]
pub struct UserThreadStart {
    pub entry_point: u64,
    pub user_stack_top: u64,
}

#[derive(Clone, Copy)]
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

    pub const fn id(&self) -> ThreadId {
        self.id
    }

    pub const fn process_id(&self) -> crate::process::ProcessId {
        self.process_id
    }

    pub const fn start(&self) -> ThreadStart {
        self.start
    }

    pub const fn state(&self) -> ThreadState {
        self.state
    }

    pub fn set_state(&mut self, state: ThreadState) {
        self.state = state;
    }

    pub fn context_mut(&mut self) -> &mut ThreadContext {
        &mut self.context
    }

    pub const fn kernel_stack_top(&self) -> u64 {
        self.kernel_stack_top
    }
}
