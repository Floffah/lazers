use lash::{TokenizedCommand, MAX_COMMAND_ARGS};
use liblazer::{self, println, SpawnError};

use crate::paths::{PathSearchOutcome, PathSearchStep, ResolutionError};
use crate::shell::{ExecutionMode, Shell};

impl Shell {
    pub(crate) fn run_command(
        &self,
        command: &str,
        tokens: &TokenizedCommand,
        mode: ExecutionMode,
    ) -> usize {
        let mut arguments = [""; MAX_COMMAND_ARGS];
        let mut argument_count = 0usize;
        let mut index = 1usize;
        while index < tokens.count() {
            if argument_count >= arguments.len() {
                println!("lash: unable to parse command: resource unavailable");
                return 1;
            }
            arguments[argument_count] = tokens.token(index).unwrap_or("");
            argument_count += 1;
            index += 1;
        }

        if command.starts_with('/') || command.as_bytes().contains(&b'/') {
            return self.run_command_at_path(command, command, &arguments[..argument_count], mode);
        }

        self.run_path_command(command, &arguments[..argument_count], mode)
    }

    pub(crate) fn command_not_found(&self, command: &str) {
        println!("lash: command not found: {}", command);
    }

    fn report_command_status(&self, status: usize, mode: ExecutionMode) -> usize {
        if status != 0 && matches!(mode, ExecutionMode::Interactive) {
            println!("lash: command exited with status {}", status);
            println!();
        }
        status
    }

    fn print_invalid_path(&self, path: &str) -> usize {
        println!("lash: invalid path: {}", path);
        1
    }

    fn print_resource_unavailable(&self, message: &str) -> usize {
        println!("{}", message);
        1
    }

    pub(crate) fn run_path_command(
        &self,
        command: &str,
        arguments: &[&str],
        mode: ExecutionMode,
    ) -> usize {
        match self.with_path_candidates(command, |candidate| {
            match liblazer::spawn_wait(candidate, arguments) {
                Ok(status) => PathSearchStep::Return(self.report_command_status(status, mode)),
                Err(SpawnError::FileNotFound) => PathSearchStep::Continue,
                Err(SpawnError::InvalidPath) => {
                    PathSearchStep::Return(self.print_invalid_path(candidate))
                }
                Err(SpawnError::InvalidExecutable) => {
                    println!("lash: invalid executable: {}", candidate);
                    PathSearchStep::Return(1)
                }
                Err(SpawnError::ResourceUnavailable) => {
                    PathSearchStep::Return(self.print_resource_unavailable(
                        "lash: unable to run command: resource unavailable",
                    ))
                }
            }
        }) {
            Ok(PathSearchOutcome::Returned(status)) => status,
            Ok(PathSearchOutcome::Matched) | Ok(PathSearchOutcome::NoMatches) => {
                self.command_not_found(command);
                1
            }
            Err(ResolutionError::InvalidPath) => self.print_invalid_path(command),
            Err(ResolutionError::ResourceUnavailable) => {
                self.print_resource_unavailable("lash: unable to run command: resource unavailable")
            }
        }
    }

    pub(crate) fn print_path_matches(&self, command: &str) -> usize {
        match self.with_path_candidates(command, |candidate| match self.probe_path(candidate) {
            Ok(true) => {
                println!("{}", candidate);
                PathSearchStep::Matched
            }
            Ok(false) => PathSearchStep::Continue,
            Err(ResolutionError::InvalidPath) => {
                PathSearchStep::Return(self.print_invalid_path(candidate))
            }
            Err(ResolutionError::ResourceUnavailable) => {
                PathSearchStep::Return(self.print_resource_unavailable(
                    "lash: unable to resolve command: resource unavailable",
                ))
            }
        }) {
            Ok(PathSearchOutcome::Matched) => 0,
            Ok(PathSearchOutcome::Returned(status)) => status,
            Ok(PathSearchOutcome::NoMatches) => {
                self.command_not_found(command);
                1
            }
            Err(ResolutionError::InvalidPath) => self.print_invalid_path(command),
            Err(ResolutionError::ResourceUnavailable) => self.print_resource_unavailable(
                "lash: unable to resolve command: resource unavailable",
            ),
        }
    }

    pub(crate) fn print_explicit_where(&self, display_name: &str, path: &str) -> usize {
        match self.probe_path(path) {
            Ok(true) => {
                println!("{}", path);
                0
            }
            Ok(false) => {
                self.command_not_found(display_name);
                1
            }
            Err(ResolutionError::InvalidPath) => self.print_invalid_path(path),
            Err(ResolutionError::ResourceUnavailable) => self.print_resource_unavailable(
                "lash: unable to resolve command: resource unavailable",
            ),
        }
    }

    fn run_command_at_path(
        &self,
        display_name: &str,
        path: &str,
        arguments: &[&str],
        mode: ExecutionMode,
    ) -> usize {
        match liblazer::spawn_wait(path, arguments) {
            Ok(status) => self.report_command_status(status, mode),
            Err(SpawnError::FileNotFound) => {
                self.command_not_found(display_name);
                1
            }
            Err(SpawnError::InvalidPath) => self.print_invalid_path(path),
            Err(SpawnError::InvalidExecutable) => {
                println!("lash: invalid executable: {}", path);
                1
            }
            Err(SpawnError::ResourceUnavailable) => {
                self.print_resource_unavailable("lash: unable to run command: resource unavailable")
            }
        }
    }
}
