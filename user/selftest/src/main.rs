#![no_main]
#![no_std]

//! First in-OS self-test runner for Lazers.
//!
//! This binary intentionally stays narrow: it executes a static list of
//! status-based checks using the same `liblazer` surface available to any other
//! user program. It does not rely on shell prompts, output capture, or any
//! kernel-only test mode.

use core::str;

use liblazer::{
    self, println, ChdirError, GetCwdError, GetEnvError, ListEnvError, SetEnvError, SpawnError,
    UnsetEnvError,
};

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
    TestCase {
        name: "env.set-get",
        run: test_env_set_get,
    },
    TestCase {
        name: "env.update",
        run: test_env_update,
    },
    TestCase {
        name: "env.unset",
        run: test_env_unset,
    },
    TestCase {
        name: "env.not-found",
        run: test_env_not_found,
    },
    TestCase {
        name: "env.empty-value",
        run: test_env_empty_value,
    },
    TestCase {
        name: "env.invalid-key",
        run: test_env_invalid_key,
    },
    TestCase {
        name: "env.listing",
        run: test_env_listing,
    },
    TestCase {
        name: "env.listing-after-unset",
        run: test_env_listing_after_unset,
    },
    TestCase {
        name: "path.lookup-via-env",
        run: test_path_lookup_via_env,
    },
    TestCase {
        name: "path.missing-fails",
        run: test_path_missing_fails,
    },
    TestCase {
        name: "path.invalid-entry-ignored",
        run: test_path_invalid_entry_ignored,
    },
    TestCase {
        name: "where.lookup-via-path",
        run: test_where_lookup_via_path,
    },
    TestCase {
        name: "where.path-missing-fails",
        run: test_where_path_missing_fails,
    },
];

const ENV_BUFFER_SIZE: usize = 128;
const ENV_LIST_BUFFER_SIZE: usize = 512;
const SELFTEST_ENV_KEY: &str = "SELFTEST_KEY";

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
    let status = spawn_silent("/bin/echo", &["selftest"])?;
    if status == 0 {
        Ok(())
    } else {
        Err("echo returned nonzero status")
    }
}

fn test_spawn_ls_zero() -> Result<(), &'static str> {
    let status = spawn_silent("/bin/ls", &[])?;
    if status == 0 {
        Ok(())
    } else {
        Err("ls returned nonzero status")
    }
}

fn test_spawn_cat_missing_fails() -> Result<(), &'static str> {
    let status = spawn_silent("/bin/cat", &["/no-such-file"])?;
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
    let status = spawn_silent("./echo", &["selftest"])?;
    if status == 0 {
        Ok(())
    } else {
        Err("relative ./echo returned nonzero status")
    }
}

fn test_env_set_get() -> Result<(), &'static str> {
    clear_env(SELFTEST_ENV_KEY);
    set_env(SELFTEST_ENV_KEY, "alpha")?;
    assert_env(SELFTEST_ENV_KEY, "alpha")?;
    clear_env(SELFTEST_ENV_KEY);
    Ok(())
}

fn test_env_update() -> Result<(), &'static str> {
    clear_env(SELFTEST_ENV_KEY);
    set_env(SELFTEST_ENV_KEY, "alpha")?;
    set_env(SELFTEST_ENV_KEY, "beta")?;
    assert_env(SELFTEST_ENV_KEY, "beta")?;
    clear_env(SELFTEST_ENV_KEY);
    Ok(())
}

fn test_env_unset() -> Result<(), &'static str> {
    clear_env(SELFTEST_ENV_KEY);
    set_env(SELFTEST_ENV_KEY, "alpha")?;
    unset_env(SELFTEST_ENV_KEY)?;
    match liblazer::get_env(SELFTEST_ENV_KEY, &mut [0u8; ENV_BUFFER_SIZE]) {
        Err(GetEnvError::NotFound) => Ok(()),
        Ok(_) => Err("unset variable still existed"),
        Err(GetEnvError::InvalidKey) => Err("unset variable lookup reported invalid key"),
        Err(GetEnvError::BufferTooSmall) => Err("unset variable lookup reported buffer too small"),
        Err(GetEnvError::ResourceUnavailable) => Err("unset variable lookup reported resource unavailable"),
    }
}

fn test_env_not_found() -> Result<(), &'static str> {
    clear_env(SELFTEST_ENV_KEY);
    match liblazer::get_env(SELFTEST_ENV_KEY, &mut [0u8; ENV_BUFFER_SIZE]) {
        Err(GetEnvError::NotFound) => Ok(()),
        Ok(_) => Err("missing variable unexpectedly existed"),
        Err(GetEnvError::InvalidKey) => Err("missing variable reported invalid key"),
        Err(GetEnvError::BufferTooSmall) => Err("missing variable reported buffer too small"),
        Err(GetEnvError::ResourceUnavailable) => Err("missing variable reported resource unavailable"),
    }
}

fn test_env_empty_value() -> Result<(), &'static str> {
    clear_env(SELFTEST_ENV_KEY);
    set_env(SELFTEST_ENV_KEY, "")?;
    let mut buffer = [0u8; ENV_BUFFER_SIZE];
    match liblazer::get_env(SELFTEST_ENV_KEY, &mut buffer) {
        Ok(0) => {
            clear_env(SELFTEST_ENV_KEY);
            Ok(())
        }
        Ok(_) => {
            clear_env(SELFTEST_ENV_KEY);
            Err("empty value did not round-trip as empty")
        }
        Err(GetEnvError::InvalidKey) => {
            clear_env(SELFTEST_ENV_KEY);
            Err("empty value lookup reported invalid key")
        }
        Err(GetEnvError::NotFound) => {
            clear_env(SELFTEST_ENV_KEY);
            Err("empty value variable was missing")
        }
        Err(GetEnvError::BufferTooSmall) => {
            clear_env(SELFTEST_ENV_KEY);
            Err("empty value lookup reported buffer too small")
        }
        Err(GetEnvError::ResourceUnavailable) => {
            clear_env(SELFTEST_ENV_KEY);
            Err("empty value lookup reported resource unavailable")
        }
    }
}

fn test_env_invalid_key() -> Result<(), &'static str> {
    match liblazer::set_env("BAD=KEY", "value") {
        Err(SetEnvError::InvalidKey) => Ok(()),
        Ok(()) => Err("invalid env key unexpectedly succeeded"),
        Err(SetEnvError::KeyTooLong) => Err("invalid env key reported key too long"),
        Err(SetEnvError::ValueTooLong) => Err("invalid env key reported value too long"),
        Err(SetEnvError::CapacityExceeded) => Err("invalid env key reported capacity exceeded"),
        Err(SetEnvError::ResourceUnavailable) => Err("invalid env key reported resource unavailable"),
    }
}

fn test_env_listing() -> Result<(), &'static str> {
    clear_env(SELFTEST_ENV_KEY);
    set_env(SELFTEST_ENV_KEY, "alpha")?;
    let contains = env_listing_contains("SELFTEST_KEY=alpha\n")?;
    clear_env(SELFTEST_ENV_KEY);
    if contains {
        Ok(())
    } else {
        Err("env listing did not include the expected key/value")
    }
}

fn test_env_listing_after_unset() -> Result<(), &'static str> {
    clear_env(SELFTEST_ENV_KEY);
    set_env(SELFTEST_ENV_KEY, "alpha")?;
    unset_env(SELFTEST_ENV_KEY)?;
    let contains = env_listing_contains("SELFTEST_KEY=")?;
    if !contains {
        Ok(())
    } else {
        Err("env listing still contained the removed key")
    }
}

fn test_path_lookup_via_env() -> Result<(), &'static str> {
    set_env("PATH", "/bin")?;
    let status = spawn_silent("/bin/lash", &["-c", "echo"])?;
    if status == 0 {
        set_env("PATH", "/bin")?;
        Ok(())
    } else {
        set_env("PATH", "/bin")?;
        Err("lash failed to resolve echo through PATH")
    }
}

fn test_path_missing_fails() -> Result<(), &'static str> {
    clear_env("PATH");
    let status = spawn_silent("/bin/lash", &["-c", "echo"])?;
    if status != 0 {
        set_env("PATH", "/bin")?;
        Ok(())
    } else {
        set_env("PATH", "/bin")?;
        Err("lash unexpectedly resolved bare command with PATH unset")
    }
}

fn test_path_invalid_entry_ignored() -> Result<(), &'static str> {
    set_env("PATH", "bin:/bin")?;
    let status = spawn_silent("/bin/lash", &["-c", "echo"])?;
    if status == 0 {
        set_env("PATH", "/bin")?;
        Ok(())
    } else {
        set_env("PATH", "/bin")?;
        Err("lash did not ignore invalid PATH entry before valid /bin")
    }
}

fn test_where_lookup_via_path() -> Result<(), &'static str> {
    set_env("PATH", "/bin")?;
    let status = spawn_silent("/bin/lash", &["-c", "where echo"])?;
    if status == 0 {
        Ok(())
    } else {
        Err("where failed to resolve echo through PATH")
    }
}

fn test_where_path_missing_fails() -> Result<(), &'static str> {
    clear_env("PATH");
    let status = spawn_silent("/bin/lash", &["-c", "where echo"])?;
    if status != 0 {
        set_env("PATH", "/bin")?;
        Ok(())
    } else {
        set_env("PATH", "/bin")?;
        Err("where unexpectedly resolved bare command with PATH unset")
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

fn spawn_silent(path: &str, args: &[&str]) -> Result<usize, &'static str> {
    match liblazer::spawn_wait_silent(path, args) {
        Ok(status) => Ok(status),
        Err(SpawnError::InvalidPath) => Err("spawn reported invalid path"),
        Err(SpawnError::FileNotFound) => Err("spawn reported file not found"),
        Err(SpawnError::InvalidExecutable) => Err("spawn reported invalid executable"),
        Err(SpawnError::ResourceUnavailable) => Err("spawn reported resource unavailable"),
    }
}

fn set_env(key: &str, value: &str) -> Result<(), &'static str> {
    match liblazer::set_env(key, value) {
        Ok(()) => Ok(()),
        Err(SetEnvError::InvalidKey) => Err("set_env reported invalid key"),
        Err(SetEnvError::KeyTooLong) => Err("set_env reported key too long"),
        Err(SetEnvError::ValueTooLong) => Err("set_env reported value too long"),
        Err(SetEnvError::CapacityExceeded) => Err("set_env reported capacity exceeded"),
        Err(SetEnvError::ResourceUnavailable) => Err("set_env reported resource unavailable"),
    }
}

fn unset_env(key: &str) -> Result<(), &'static str> {
    match liblazer::unset_env(key) {
        Ok(()) => Ok(()),
        Err(UnsetEnvError::InvalidKey) => Err("unset_env reported invalid key"),
        Err(UnsetEnvError::NotFound) => Err("unset_env reported missing variable"),
        Err(UnsetEnvError::ResourceUnavailable) => Err("unset_env reported resource unavailable"),
    }
}

fn clear_env(key: &str) {
    let _ = liblazer::unset_env(key);
}

fn env_listing_contains(needle: &str) -> Result<bool, &'static str> {
    let mut buffer = [0u8; ENV_LIST_BUFFER_SIZE];
    let len = match liblazer::list_env(&mut buffer) {
        Ok(len) => len,
        Err(ListEnvError::BufferTooSmall) => return Err("list_env buffer too small"),
        Err(ListEnvError::ResourceUnavailable) => return Err("list_env reported resource unavailable"),
    };
    let text = str::from_utf8(&buffer[..len]).map_err(|_| "env listing was not valid utf-8")?;
    Ok(text.contains(needle))
}

fn assert_env(key: &str, expected: &str) -> Result<(), &'static str> {
    let mut buffer = [0u8; ENV_BUFFER_SIZE];
    let len = match liblazer::get_env(key, &mut buffer) {
        Ok(len) => len,
        Err(GetEnvError::InvalidKey) => return Err("get_env reported invalid key"),
        Err(GetEnvError::NotFound) => return Err("expected variable was missing"),
        Err(GetEnvError::BufferTooSmall) => return Err("get_env buffer too small"),
        Err(GetEnvError::ResourceUnavailable) => return Err("get_env reported resource unavailable"),
    };
    let value = str::from_utf8(&buffer[..len]).map_err(|_| "env value was not valid utf-8")?;
    if value == expected {
        Ok(())
    } else {
        Err("env value did not match expected value")
    }
}
