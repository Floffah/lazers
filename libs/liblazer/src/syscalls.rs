use kernel_abi::Syscall;

#[cfg(target_arch = "x86_64")]
unsafe extern "C" {
    fn user_syscall0(number: usize) -> usize;
    fn user_syscall1(number: usize, arg0: usize) -> usize;
    fn user_syscall3(number: usize, arg0: usize, arg1: usize, arg2: usize) -> usize;
    fn user_syscall4(number: usize, arg0: usize, arg1: usize, arg2: usize, arg3: usize) -> usize;
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn syscall0(number: Syscall) -> usize {
    unsafe { user_syscall0(number as usize) }
}

#[cfg(not(target_arch = "x86_64"))]
pub(crate) fn syscall0(number: Syscall) -> usize {
    host_stub(number, &[])
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn syscall1(number: Syscall, arg0: usize) -> usize {
    unsafe { user_syscall1(number as usize, arg0) }
}

#[cfg(not(target_arch = "x86_64"))]
pub(crate) fn syscall1(number: Syscall, arg0: usize) -> usize {
    host_stub(number, &[arg0])
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn syscall3(number: Syscall, arg0: usize, arg1: usize, arg2: usize) -> usize {
    unsafe { user_syscall3(number as usize, arg0, arg1, arg2) }
}

#[cfg(not(target_arch = "x86_64"))]
pub(crate) fn syscall3(number: Syscall, arg0: usize, arg1: usize, arg2: usize) -> usize {
    host_stub(number, &[arg0, arg1, arg2])
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn syscall4(
    number: Syscall,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
) -> usize {
    unsafe { user_syscall4(number as usize, arg0, arg1, arg2, arg3) }
}

#[cfg(not(target_arch = "x86_64"))]
pub(crate) fn syscall4(
    number: Syscall,
    arg0: usize,
    arg1: usize,
    arg2: usize,
    arg3: usize,
) -> usize {
    host_stub(number, &[arg0, arg1, arg2, arg3])
}

#[cfg(not(target_arch = "x86_64"))]
fn host_stub(number: Syscall, _args: &[usize]) -> usize {
    panic!(
        "liblazer syscalls are only available on the x86_64 Lazers user target: {:?}",
        number as usize
    );
}
