use crate::arch;
use crate::process::ProcessId;
use crate::thread::{ThreadContext, ThreadStart, ThreadState};

use super::state::{with_scheduler, with_scheduler_mut};

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
            super::context_switch(
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
        super::context_switch(switch.current_context, switch.next_context);
    }
}

/// Blocks the current thread until the given child process exits, then returns
/// the child's exit status.
pub fn wait_for_child(child_process: ProcessId) -> Option<usize> {
    let switch = with_scheduler_mut(|scheduler| scheduler.prepare_wait_for_child(child_process))?;

    arch::activate_address_space(switch.next_space, switch.next_stack_top);
    unsafe {
        super::context_switch(switch.current_context, switch.next_context);
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
            super::context_switch(
                &mut scheduler.bootstrap_context as *mut ThreadContext,
                switch.next_context,
            );
        });
    }

    crate::halt_forever()
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
