#![no_main]
#![no_std]

//! First in-OS self-test runner for Lazers.
//!
//! This binary intentionally stays narrow: it executes a static list of
//! status-based checks using the same `liblazer` surface available to any other
//! user program. It does not rely on shell prompts, output capture, or any
//! kernel-only test mode.

use core::str;

use liblazer::{self, println, ChdirError, GetCwdError, SpawnError};

const CWD_BUFFER_SIZE: usize = 256;

liblazer::entry!(main);

struct TestCase {
    name: &'static str,
    run: fn() -> Result<(), &'static str>,
}

const TESTS: &[TestCase] = &[
    TestCase {
        name: "cwd.initial-root",
        run: test_cwd_initial_root,
    },
    TestCase {
        name: "cwd.chdir-bin",
        run: test_cwd_chdir_bin,
    },
    TestCase {
        name: "cwd.chdir-parent",
        run: test_cwd_chdir_parent,
    },
    TestCase {
        name: "spawn.echo-zero",
        run: test_spawn_echo_zero,
    },
    TestCase {
        name: "spawn.ls-zero",
        run: test_spawn_ls_zero,
    },
    TestCase {
        name: "spawn.cat-missing-fails",
        run: test_spawn_cat_missing_fails,
    },
    TestCase {
        name: "spawn.invalid-path-fails",
        run: test_spawn_invalid_path_fails,
    },
    TestCase {
        name: "spawn.relative-cwd",
        run: test_spawn_relative_cwd,
    },
];

fn main() -> ! {
    let mut passed = 0usize;
    let mut failed = 0usize;

    println!("selftest: running {} tests", TESTS.len());

    for test in TESTS {
        if ensure_root().is_err() {
            println!("FAIL {}: unable to reset cwd before test", test.name);
            failed += 1;
            continue;
        }

        match (test.run)() {
            Ok(()) => {
                println!("PASS {}", test.name);
                passed += 1;
            }
            Err(message) => {
                println!("FAIL {}: {}", test.name, message);
                failed += 1;
            }
        }

        if ensure_root().is_err() {
            println!("FAIL {}: unable to reset cwd after test", test.name);
            failed += 1;
        }
    }

    println!();
    println!("selftest: {} passed, {} failed", passed, failed);

    if failed == 0 {
        liblazer::exit(0);
    }

    liblazer::exit(1);
}

fn test_cwd_initial_root() -> Result<(), &'static str> {
    assert_cwd("/")
}

fn test_cwd_chdir_bin() -> Result<(), &'static str> {
    change_dir("/bin")?;
    assert_cwd("/bin")
}

fn test_cwd_chdir_parent() -> Result<(), &'static str> {
    change_dir("/bin")?;
    change_dir("..")?;
    assert_cwd("/")
}

fn test_spawn_echo_zero() -> Result<(), &'static str> {
    let status = spawn("/bin/echo", &["selftest"])?;
    if status == 0 {
        Ok(())
    } else {
        Err("echo returned nonzero status")
    }
}

fn test_spawn_ls_zero() -> Result<(), &'static str> {
    let status = spawn("/bin/ls", &[])?;
    if status == 0 {
        Ok(())
    } else {
        Err("ls returned nonzero status")
    }
}

fn test_spawn_cat_missing_fails() -> Result<(), &'static str> {
    let status = spawn("/bin/cat", &["/no-such-file"])?;
    if status != 0 {
        Ok(())
    } else {
        Err("cat unexpectedly succeeded for missing file")
    }
}

fn test_spawn_invalid_path_fails() -> Result<(), &'static str> {
    match liblazer::spawn_wait("/bin/nope", &[]) {
        Err(SpawnError::FileNotFound) => Ok(()),
        Err(SpawnError::InvalidPath) => Err("missing executable reported invalid path"),
        Err(SpawnError::InvalidExecutable) => Err("missing executable reported invalid executable"),
        Err(SpawnError::ResourceUnavailable) => Err("missing executable reported resource unavailable"),
        Ok(_) => Err("missing executable unexpectedly launched"),
    }
}

fn test_spawn_relative_cwd() -> Result<(), &'static str> {
    change_dir("/bin")?;
    let status = spawn("./echo", &["selftest"])?;
    if status == 0 {
        Ok(())
    } else {
        Err("relative ./echo returned nonzero status")
    }
}

fn ensure_root() -> Result<(), &'static str> {
    change_dir("/")
}

fn change_dir(path: &str) -> Result<(), &'static str> {
    match liblazer::chdir(path) {
        Ok(()) => Ok(()),
        Err(ChdirError::InvalidPath) => Err("invalid path"),
        Err(ChdirError::NotFound) => Err("directory not found"),
        Err(ChdirError::ResourceUnavailable) => Err("unable to update cwd"),
    }
}

fn assert_cwd(expected: &str) -> Result<(), &'static str> {
    let mut buffer = [0u8; CWD_BUFFER_SIZE];
    let len = match liblazer::getcwd(&mut buffer) {
        Ok(len) => len,
        Err(GetCwdError::BufferTooSmall) => return Err("cwd buffer too small"),
        Err(GetCwdError::ResourceUnavailable) => return Err("unable to read cwd"),
    };

    let cwd = str::from_utf8(&buffer[..len]).map_err(|_| "cwd was not valid utf-8")?;
    if cwd == expected {
        Ok(())
    } else {
        Err("cwd did not match expected value")
    }
}

fn spawn(path: &str, args: &[&str]) -> Result<usize, &'static str> {
    match liblazer::spawn_wait(path, args) {
        Ok(status) => Ok(status),
        Err(SpawnError::InvalidPath) => Err("spawn reported invalid path"),
        Err(SpawnError::FileNotFound) => Err("spawn reported file not found"),
        Err(SpawnError::InvalidExecutable) => Err("spawn reported invalid executable"),
        Err(SpawnError::ResourceUnavailable) => Err("spawn reported resource unavailable"),
    }
}
