use core::fmt::{self, Write};

use crate::io::{stderr_write, stdout_write};

/// Writes one formatted string to standard output.
pub fn print(args: fmt::Arguments<'_>) {
    let mut stdout = Stdout;
    let _ = stdout.write_fmt(args);
}

/// Writes one formatted string to standard error.
pub fn eprint(args: fmt::Arguments<'_>) {
    let mut stderr = Stderr;
    let _ = stderr.write_fmt(args);
}

#[doc(hidden)]
pub struct Stdout;

impl fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let _ = stdout_write(s.as_bytes());
        Ok(())
    }
}

#[doc(hidden)]
pub struct Stderr;

impl fmt::Write for Stderr {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let _ = stderr_write(s.as_bytes());
        Ok(())
    }
}
