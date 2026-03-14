# User and Kernel Interface

This page describes how user programs talk to the kernel in Lazers today.

The short version is:

- user programs run in ring 3
- the kernel runs in ring 0
- user programs call into the kernel through `int 0x80`
- `liblazer` is the shared runtime layer that exposes those kernel services to user code

This is the core boundary that keeps Lazers from turning into “userspace is just a thin wrapper around kernel internals.”

## The Boundary

User programs do not call kernel Rust functions directly.

Instead, a user program:

1. calls a `liblazer` API such as `spawn_wait`, `read_dir`, `getcwd`, `read_file`, or the env helpers
2. `liblazer` marshals the syscall number and arguments into registers
3. `liblazer` executes `int 0x80`
4. the CPU traps into the kernel through the syscall entry in the IDT
5. the kernel dispatches the syscall and writes a return value back to the trap frame
6. execution returns to user mode
7. `liblazer` converts the raw status code into a small Rust result or error type

That means the kernel owns mechanism, while `liblazer` owns the first user-friendly API layer.

## Calling Convention Today

The current syscall ABI is intentionally small.

At the trap boundary:

- `rax` holds the syscall number
- `rdi`, `rsi`, `rdx`, and `rcx` hold syscall arguments
- `rax` is also used for the return value

This is not exposed directly to most user programs. It is hidden behind `liblazer`, which provides Rust wrappers around the low-level assembly stubs.

## What `liblazer` Provides

`liblazer` is the early shared userland runtime.

Today it provides:

- process startup glue
- syscall-entry assembly shims
- a private runtime layer for startup argv state and panic handling
- private raw syscall wrappers used by the typed APIs
- panic-to-exit behavior
- stdio helpers
- argv access through `args()`
- cwd helpers
- environment-variable helpers
- environment-variable listing
- directory listing helpers
- whole-file read helpers
- synchronous child-process spawn

User binaries still run with `no_std` and `no_main`, but `liblazer` already gives them a shared OS-specific runtime surface instead of forcing each binary to invent its own startup and syscall path.

## Process Startup Data

When the kernel launches a user program, it prepares a startup block on the child user stack.

Today that startup state includes:

- the program entry point
- the initial user stack
- argv data copied into user memory

`liblazer` reads that startup argv layout during process startup and exposes it through `args()`.

This is why user programs can use a simple `main()`-style entry shape while still receiving process arguments.

## What The Kernel Owns vs What Userland Owns

The kernel owns:

- privilege transitions
- syscall dispatch
- process and thread creation
- address spaces
- cwd and stdio inheritance
- environment-variable storage and inheritance
- filesystem resolution and executable loading

Userland owns:

- shell parsing and command policy
- prompt behavior
- built-ins like `cd` and `exit`
- command-line interpretation
- normal program behavior on top of the syscall surface

That split is deliberate. The kernel should execute explicit requests. The shell and other user programs should decide what requests to make.

## Spawn Model Today

The current child-process model is synchronous and explicit.

When a user program spawns a child:

- the child is selected by explicit path
- the child inherits stdio
- the child inherits cwd
- the child inherits environment variables
- the child receives argv startup data
- the parent waits for the child to exit
- the child exit status is returned to the parent

This model is intentionally small, but it is already enough for the current shell and command set.

## Error Model Today

At the kernel boundary, syscalls currently return small integer status codes.

Those values are not intended to be the final user-facing API. `liblazer` maps them into narrow Rust result types such as:

- `SpawnError`
- `ReadDirError`
- `ChdirError`
- `GetCwdError`
- `ReadFileError`
- `GetEnvError`
- `SetEnvError`
- `UnsetEnvError`
- `ListEnvError`

This keeps the kernel ABI simple while still letting userland write clear Rust code.

## Current Syscall Reference

The current syscall surface is intentionally small and process-oriented.

### `0` — `read`

- Purpose: read bytes from a standard stream
- Arguments:
  - `fd`
  - user buffer pointer
  - buffer length
- Success: number of bytes read
- Failure at `liblazer` level: returned as a raw size value; current helpers use it directly for stdio reads

### `1` — `write`

- Purpose: write bytes to a standard stream
- Arguments:
  - `fd`
  - user buffer pointer
  - buffer length
- Success: number of bytes written
- Failure at `liblazer` level: returned as a raw size value; current helpers use it directly for stdio writes

### `2` — `yield`

- Purpose: cooperatively yield the current thread
- Arguments: none
- Success: returns to userland after yielding
- Failure at `liblazer` level: no structured failure is currently exposed

### `3` — `exit`

- Purpose: terminate the current process with an exit code
- Arguments:
  - exit status
- Success: does not return
- Failure at `liblazer` level: not exposed; `exit` is treated as final

### `4` — `spawn_wait`

- Purpose: launch a child program and wait for it synchronously
- Arguments:
  - executable path pointer
  - executable path length
  - argv payload pointer
  - argv payload length
- Success: child exit status
- Failure at `liblazer` level: `SpawnError`

### `5` — `read_dir`

- Purpose: list one directory into a caller-provided buffer
- Arguments:
  - path pointer
  - path length
  - output buffer pointer
  - output buffer length
- Success: number of bytes written to the output buffer
- Failure at `liblazer` level: `ReadDirError`

### `6` — `chdir`

- Purpose: change the current process working directory
- Arguments:
  - path pointer
  - path length
- Success: zero-like success mapped to `Ok(())`
- Failure at `liblazer` level: `ChdirError`

### `7` — `getcwd`

- Purpose: copy the current working directory into a caller-provided buffer
- Arguments:
  - output buffer pointer
  - output buffer length
- Success: number of bytes written
- Failure at `liblazer` level: `GetCwdError`

### `8` — `read_file`

- Purpose: read one file into a caller-provided buffer
- Arguments:
  - path pointer
  - path length
  - output buffer pointer
  - output buffer length
- Success: number of bytes written
- Failure at `liblazer` level: `ReadFileError`

### `9` — `get_env`

- Purpose: copy one process-owned environment variable into a caller-provided buffer
- Arguments:
  - key pointer
  - key length
  - output buffer pointer
  - output buffer length
- Success: number of bytes written
- Failure at `liblazer` level: `GetEnvError`

### `10` — `set_env`

- Purpose: insert or update one process-owned environment variable
- Arguments:
  - key pointer
  - key length
  - value pointer
  - value length
- Success: zero-like success mapped to `Ok(())`
- Failure at `liblazer` level: `SetEnvError`

### `11` — `unset_env`

- Purpose: remove one process-owned environment variable
- Arguments:
  - key pointer
  - key length
- Success: zero-like success mapped to `Ok(())`
- Failure at `liblazer` level: `UnsetEnvError`

### `12` — `list_env`

- Purpose: serialize the current process environment into a caller-provided buffer
- Arguments:
  - output buffer pointer
  - output buffer length
- Success: number of bytes written
- Failure at `liblazer` level: `ListEnvError`

## What This Interface Does Not Provide Yet

The current boundary is useful, but intentionally incomplete.

It does not yet provide:

- file descriptors for arbitrary files
- open/read/close style file APIs
- pipes or output capture
- asynchronous child control
- signals
- networking
- timer APIs

Those can be added later, but this page documents the real interface Lazers has today.
