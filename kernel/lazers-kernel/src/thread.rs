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
    #[allow(dead_code)]
    Blocked,
}

pub type ThreadEntry = fn() -> !;

#[derive(Clone, Copy)]
pub struct Thread {
    id: ThreadId,
    #[allow(dead_code)]
    name: &'static str,
    process_id: Option<crate::process::ProcessId>,
    entry: ThreadEntry,
    state: ThreadState,
    context: ThreadContext,
}

impl Thread {
    pub const fn new(
        id: ThreadId,
        name: &'static str,
        process_id: Option<crate::process::ProcessId>,
        entry: ThreadEntry,
        context: ThreadContext,
    ) -> Self {
        Self {
            id,
            name,
            process_id,
            entry,
            state: ThreadState::Runnable,
            context,
        }
    }

    pub const fn id(&self) -> ThreadId {
        self.id
    }

    pub const fn process_id(&self) -> Option<crate::process::ProcessId> {
        self.process_id
    }

    pub const fn entry(&self) -> ThreadEntry {
        self.entry
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
}
