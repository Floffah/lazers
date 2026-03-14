use lash::LINE_CAPACITY;
use liblazer::{self, GetEnvError, ReadFileError};

use crate::shell::Shell;

const COMMAND_PATH_CAPACITY: usize = LINE_CAPACITY + 1 + 128;
const PATH_BUFFER_CAPACITY: usize = 128;

pub(crate) struct NormalizedPath {
    bytes: [u8; LINE_CAPACITY],
    len: usize,
}

impl NormalizedPath {
    pub(crate) fn as_str(&self) -> Result<&str, ResolutionError> {
        core::str::from_utf8(&self.bytes[..self.len]).map_err(|_| ResolutionError::InvalidPath)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ResolutionError {
    InvalidPath,
    ResourceUnavailable,
}

pub(crate) enum PathSearchStep<T> {
    Continue,
    Matched,
    Return(T),
}

pub(crate) enum PathSearchOutcome<T> {
    NoMatches,
    Matched,
    Returned(T),
}

impl Shell {
    pub(crate) fn normalize_command_path(
        &mut self,
        input: &str,
    ) -> Result<NormalizedPath, ResolutionError> {
        let cwd = match liblazer::getcwd(&mut self.cwd) {
            Ok(len) => core::str::from_utf8(&self.cwd[..len]).unwrap_or("/"),
            Err(_) => return Err(ResolutionError::ResourceUnavailable),
        };

        let mut output = [0u8; LINE_CAPACITY];
        let mut len = if input.starts_with('/') {
            output[0] = b'/';
            1
        } else {
            let cwd_bytes = cwd.as_bytes();
            if cwd_bytes.len() > output.len() {
                return Err(ResolutionError::ResourceUnavailable);
            }
            output[..cwd_bytes.len()].copy_from_slice(cwd_bytes);
            cwd_bytes.len()
        };

        let bytes = input.as_bytes();
        let mut index = 0usize;
        while index <= bytes.len() {
            while index < bytes.len() && bytes[index] == b'/' {
                index += 1;
            }
            if index >= bytes.len() {
                break;
            }

            let start = index;
            while index < bytes.len() && bytes[index] != b'/' {
                index += 1;
            }
            let segment = &input[start..index];

            if segment == "." {
                continue;
            }

            if segment == ".." {
                if len > 1 {
                    len -= 1;
                    while len > 0 && output[len] != b'/' {
                        len -= 1;
                    }
                    if len == 0 {
                        output[0] = b'/';
                        len = 1;
                    }
                }
                continue;
            }

            if len == 0 {
                output[0] = b'/';
                len = 1;
            }
            if len > 1 && output[len - 1] != b'/' {
                if len >= output.len() {
                    return Err(ResolutionError::ResourceUnavailable);
                }
                output[len] = b'/';
                len += 1;
            }

            let segment_bytes = segment.as_bytes();
            if len + segment_bytes.len() > output.len() {
                return Err(ResolutionError::ResourceUnavailable);
            }
            output[len..len + segment_bytes.len()].copy_from_slice(segment_bytes);
            len += segment_bytes.len();
        }

        if len == 0 {
            output[0] = b'/';
            len = 1;
        }

        Ok(NormalizedPath { bytes: output, len })
    }

    pub(crate) fn probe_path(&self, path: &str) -> Result<bool, ResolutionError> {
        let mut probe = [0u8; 1];
        match liblazer::read_file(path, &mut probe) {
            Ok(_) => Ok(true),
            Err(ReadFileError::BufferTooSmall) => Ok(true),
            Err(ReadFileError::NotFound | ReadFileError::NotAFile) => Ok(false),
            Err(ReadFileError::InvalidPath) => Err(ResolutionError::InvalidPath),
            Err(ReadFileError::ResourceUnavailable) => Err(ResolutionError::ResourceUnavailable),
        }
    }

    fn load_path<'a>(&self, path_buffer: &'a mut [u8; PATH_BUFFER_CAPACITY]) -> Option<&'a str> {
        let path_len = match liblazer::get_env("PATH", path_buffer) {
            Ok(len) => len,
            Err(GetEnvError::InvalidKey | GetEnvError::NotFound | GetEnvError::BufferTooSmall)
            | Err(GetEnvError::ResourceUnavailable) => return None,
        };

        core::str::from_utf8(&path_buffer[..path_len]).ok()
    }

    fn build_path_candidate<'a>(
        &self,
        entry: &str,
        command: &str,
        candidate_buffer: &'a mut [u8; COMMAND_PATH_CAPACITY],
    ) -> Result<&'a str, ResolutionError> {
        let needed = entry.len() + 1 + command.len();
        if needed > candidate_buffer.len() {
            return Err(ResolutionError::ResourceUnavailable);
        }

        candidate_buffer[..entry.len()].copy_from_slice(entry.as_bytes());
        candidate_buffer[entry.len()] = b'/';
        candidate_buffer[entry.len() + 1..needed].copy_from_slice(command.as_bytes());
        core::str::from_utf8(&candidate_buffer[..needed]).map_err(|_| ResolutionError::InvalidPath)
    }

    pub(crate) fn with_path_candidates<T, F>(
        &self,
        command: &str,
        mut visitor: F,
    ) -> Result<PathSearchOutcome<T>, ResolutionError>
    where
        F: FnMut(&str) -> PathSearchStep<T>,
    {
        let mut path_buffer = [0u8; PATH_BUFFER_CAPACITY];
        let mut candidate_buffer = [0u8; COMMAND_PATH_CAPACITY];
        let Some(path) = self.load_path(&mut path_buffer) else {
            return Ok(PathSearchOutcome::NoMatches);
        };

        let mut matched = false;
        let mut segment_start = 0usize;
        while segment_start <= path.len() {
            let remainder = &path[segment_start..];
            let segment_len = remainder.find(':').unwrap_or(remainder.len());
            let entry = &remainder[..segment_len];

            if !entry.is_empty() && entry.starts_with('/') {
                let candidate = self.build_path_candidate(entry, command, &mut candidate_buffer)?;
                match visitor(candidate) {
                    PathSearchStep::Continue => {}
                    PathSearchStep::Matched => matched = true,
                    PathSearchStep::Return(value) => return Ok(PathSearchOutcome::Returned(value)),
                }
            }

            if segment_start + segment_len >= path.len() {
                break;
            }
            segment_start += segment_len + 1;
        }

        if matched {
            Ok(PathSearchOutcome::Matched)
        } else {
            Ok(PathSearchOutcome::NoMatches)
        }
    }
}
