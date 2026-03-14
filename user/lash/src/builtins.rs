use lash::{TokenizedCommand, LINE_CAPACITY};
use liblazer::{self, println, ChdirError, ListEnvError, SetEnvError, UnsetEnvError};

use crate::paths::ResolutionError;
use crate::shell::Shell;

const ENV_LIST_BUFFER_CAPACITY: usize = 1024;

impl Shell {
    pub(crate) fn run_cd(&mut self, tokens: &TokenizedCommand) -> usize {
        let path = if tokens.count() > 1 {
            tokens.token(1).unwrap_or("/")
        } else {
            "/"
        };

        match liblazer::chdir(path) {
            Ok(()) => 0,
            Err(ChdirError::InvalidPath | ChdirError::NotFound) => {
                println!("lash: directory not found: {}", path);
                1
            }
            Err(ChdirError::ResourceUnavailable) => {
                println!("lash: unable to change directory: resource unavailable");
                1
            }
        }
    }

    pub(crate) fn run_env(&self, tokens: &TokenizedCommand) -> usize {
        if tokens.count() != 1 {
            println!("lash: usage: env");
            return 1;
        }

        let mut buffer = [0u8; ENV_LIST_BUFFER_CAPACITY];
        match liblazer::list_env(&mut buffer) {
            Ok(len) => {
                if len > 0 {
                    let _ = liblazer::stdout_write(&buffer[..len]);
                }
                0
            }
            Err(ListEnvError::BufferTooSmall) => {
                println!("lash: unable to list environment: buffer too small");
                1
            }
            Err(ListEnvError::ResourceUnavailable) => {
                println!("lash: unable to list environment: resource unavailable");
                1
            }
        }
    }

    pub(crate) fn run_set(&self, tokens: &TokenizedCommand) -> usize {
        if tokens.count() < 3 {
            println!("lash: usage: set KEY VALUE...");
            return 1;
        }

        let key = tokens.token(1).unwrap_or("");
        let mut value_bytes = [0u8; LINE_CAPACITY];
        let mut value_len = 0usize;
        let mut index = 2usize;
        while index < tokens.count() {
            let token = tokens.token(index).unwrap_or("");
            let token_bytes = token.as_bytes();
            let separator_len = if index == 2 { 0 } else { 1 };
            if value_len + separator_len + token_bytes.len() > value_bytes.len() {
                println!("lash: unable to set environment: value too long");
                return 1;
            }
            if separator_len == 1 {
                value_bytes[value_len] = b' ';
                value_len += 1;
            }
            value_bytes[value_len..value_len + token_bytes.len()].copy_from_slice(token_bytes);
            value_len += token_bytes.len();
            index += 1;
        }

        let value = core::str::from_utf8(&value_bytes[..value_len]).unwrap_or("");
        match liblazer::set_env(key, value) {
            Ok(()) => 0,
            Err(SetEnvError::InvalidKey) => {
                println!("lash: invalid environment key: {}", key);
                1
            }
            Err(SetEnvError::KeyTooLong) => {
                println!("lash: environment key too long: {}", key);
                1
            }
            Err(SetEnvError::ValueTooLong) => {
                println!("lash: environment value too long for key: {}", key);
                1
            }
            Err(SetEnvError::CapacityExceeded) => {
                println!("lash: unable to set environment: capacity exceeded");
                1
            }
            Err(SetEnvError::ResourceUnavailable) => {
                println!("lash: unable to set environment: resource unavailable");
                1
            }
        }
    }

    pub(crate) fn run_unset(&self, tokens: &TokenizedCommand) -> usize {
        if tokens.count() != 2 {
            println!("lash: usage: unset KEY");
            return 1;
        }

        let key = tokens.token(1).unwrap_or("");
        match liblazer::unset_env(key) {
            Ok(()) => 0,
            Err(UnsetEnvError::InvalidKey) => {
                println!("lash: invalid environment key: {}", key);
                1
            }
            Err(UnsetEnvError::NotFound) => {
                println!("lash: environment variable not found: {}", key);
                1
            }
            Err(UnsetEnvError::ResourceUnavailable) => {
                println!("lash: unable to unset environment: resource unavailable");
                1
            }
        }
    }

    pub(crate) fn run_where(&mut self, tokens: &TokenizedCommand) -> usize {
        if tokens.count() != 2 {
            println!("lash: usage: where NAME");
            return 1;
        }

        let name = tokens.token(1).unwrap_or("");
        if name.starts_with('/') {
            return self.print_explicit_where(name, name);
        }

        if name.as_bytes().contains(&b'/') {
            let resolved_path = match self.normalize_command_path(name) {
                Ok(path) => path,
                Err(ResolutionError::InvalidPath) => {
                    println!("lash: invalid path: {}", name);
                    return 1;
                }
                Err(ResolutionError::ResourceUnavailable) => {
                    println!("lash: unable to resolve command: resource unavailable");
                    return 1;
                }
            };
            return self.print_explicit_where(name, resolved_path.as_str().unwrap_or(name));
        }

        self.print_path_matches(name)
    }
}
