#![cfg_attr(not(test), no_std)]

//! Pure `lash` parsing logic.
//!
//! This library layer keeps shell syntax handling testable on the host without
//! pulling in the Lazers runtime or startup glue. The `lash` binary reuses the
//! exact same parser for interactive and batch execution.

#[cfg(test)]
extern crate std;

pub const LINE_CAPACITY: usize = 256;
pub const MAX_COMMAND_ARGS: usize = 16;
pub const MAX_CHAIN_SEGMENTS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    UnmatchedSingleQuote,
    UnmatchedDoubleQuote,
    TrailingBackslash,
    InvalidSyntax,
    ResourceUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentOperator {
    And,
    Or,
    Sequence,
}

#[derive(Clone, Copy)]
enum ParseState {
    Unquoted,
    SingleQuoted,
    DoubleQuoted,
}

#[derive(Debug)]
pub struct SegmentScan {
    starts: [usize; MAX_CHAIN_SEGMENTS],
    ends: [usize; MAX_CHAIN_SEGMENTS],
    operators: [SegmentOperator; MAX_CHAIN_SEGMENTS - 1],
    count: usize,
}

impl SegmentScan {
    pub const fn new() -> Self {
        Self {
            starts: [0; MAX_CHAIN_SEGMENTS],
            ends: [0; MAX_CHAIN_SEGMENTS],
            operators: [SegmentOperator::Sequence; MAX_CHAIN_SEGMENTS - 1],
            count: 0,
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn segment<'a>(&self, line: &'a [u8], index: usize) -> Option<&'a [u8]> {
        if index >= self.count {
            return None;
        }
        Some(&line[self.starts[index]..self.ends[index]])
    }

    pub fn operator_before(&self, index: usize) -> Option<SegmentOperator> {
        if index == 0 || index > self.count.saturating_sub(1) {
            return None;
        }
        Some(self.operators[index - 1])
    }

    fn push(
        &mut self,
        start: usize,
        end: usize,
        operator: Option<SegmentOperator>,
    ) -> Result<(), ParseError> {
        if self.count >= self.starts.len() {
            return Err(ParseError::ResourceUnavailable);
        }

        self.starts[self.count] = start;
        self.ends[self.count] = end;

        if let Some(operator) = operator {
            if self.count >= self.operators.len() {
                return Err(ParseError::ResourceUnavailable);
            }
            self.operators[self.count] = operator;
        }

        self.count += 1;
        Ok(())
    }
}

#[derive(Debug)]
pub struct TokenizedCommand {
    storage: [u8; LINE_CAPACITY],
    offsets: [usize; MAX_COMMAND_ARGS],
    lengths: [usize; MAX_COMMAND_ARGS],
    count: usize,
}

impl TokenizedCommand {
    pub fn parse(line: &[u8]) -> Result<Self, ParseError> {
        let mut parsed = Self {
            storage: [0; LINE_CAPACITY],
            offsets: [0; MAX_COMMAND_ARGS],
            lengths: [0; MAX_COMMAND_ARGS],
            count: 0,
        };

        let mut state = ParseState::Unquoted;
        let mut source_index = 0usize;
        let mut storage_len = 0usize;
        let mut current_len = 0usize;
        let mut token_active = false;

        while source_index < line.len() {
            let byte = line[source_index];
            match state {
                ParseState::Unquoted => match byte {
                    b' ' => {
                        if token_active {
                            parsed.finish_token(current_len)?;
                            current_len = 0;
                            token_active = false;
                        }
                    }
                    b'\'' => {
                        if !token_active {
                            parsed.start_token(storage_len)?;
                            token_active = true;
                        }
                        state = ParseState::SingleQuoted;
                    }
                    b'"' => {
                        if !token_active {
                            parsed.start_token(storage_len)?;
                            token_active = true;
                        }
                        state = ParseState::DoubleQuoted;
                    }
                    b'\\' => {
                        if source_index + 1 >= line.len() {
                            return Err(ParseError::TrailingBackslash);
                        }
                        if !token_active {
                            parsed.start_token(storage_len)?;
                            token_active = true;
                        }
                        source_index += 1;
                        parsed.push_token_byte(storage_len, line[source_index])?;
                        storage_len += 1;
                        current_len += 1;
                    }
                    _ => {
                        if !token_active {
                            parsed.start_token(storage_len)?;
                            token_active = true;
                        }
                        parsed.push_token_byte(storage_len, byte)?;
                        storage_len += 1;
                        current_len += 1;
                    }
                },
                ParseState::SingleQuoted => {
                    if byte == b'\'' {
                        state = ParseState::Unquoted;
                    } else {
                        parsed.push_token_byte(storage_len, byte)?;
                        storage_len += 1;
                        current_len += 1;
                    }
                }
                ParseState::DoubleQuoted => match byte {
                    b'"' => state = ParseState::Unquoted,
                    b'\\' => {
                        if source_index + 1 >= line.len() {
                            return Err(ParseError::TrailingBackslash);
                        }
                        source_index += 1;
                        parsed.push_token_byte(storage_len, line[source_index])?;
                        storage_len += 1;
                        current_len += 1;
                    }
                    _ => {
                        parsed.push_token_byte(storage_len, byte)?;
                        storage_len += 1;
                        current_len += 1;
                    }
                },
            }

            source_index += 1;
        }

        match state {
            ParseState::SingleQuoted => return Err(ParseError::UnmatchedSingleQuote),
            ParseState::DoubleQuoted => return Err(ParseError::UnmatchedDoubleQuote),
            ParseState::Unquoted => {}
        }

        if token_active {
            parsed.finish_token(current_len)?;
        }

        Ok(parsed)
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn token(&self, index: usize) -> Option<&str> {
        let start = *self.offsets.get(index)?;
        let len = *self.lengths.get(index)?;
        core::str::from_utf8(&self.storage[start..start + len]).ok()
    }

    fn start_token(&mut self, storage_offset: usize) -> Result<(), ParseError> {
        if self.count >= self.offsets.len() {
            return Err(ParseError::ResourceUnavailable);
        }
        self.offsets[self.count] = storage_offset;
        self.lengths[self.count] = 0;
        Ok(())
    }

    fn finish_token(&mut self, token_len: usize) -> Result<(), ParseError> {
        if self.count >= self.lengths.len() {
            return Err(ParseError::ResourceUnavailable);
        }
        self.lengths[self.count] = token_len;
        self.count += 1;
        Ok(())
    }

    fn push_token_byte(&mut self, storage_index: usize, byte: u8) -> Result<(), ParseError> {
        if storage_index >= self.storage.len() {
            return Err(ParseError::ResourceUnavailable);
        }
        self.storage[storage_index] = byte;
        Ok(())
    }
}

pub fn scan_segments(line: &[u8]) -> Result<SegmentScan, ParseError> {
    let mut segments = SegmentScan::new();
    let mut segment_start = 0usize;
    let mut index = 0usize;
    let mut state = ParseState::Unquoted;

    while index < line.len() {
        let byte = line[index];
        match state {
            ParseState::Unquoted => match byte {
                b'\'' => state = ParseState::SingleQuoted,
                b'"' => state = ParseState::DoubleQuoted,
                b'\\' => {
                    if index + 1 >= line.len() {
                        return Err(ParseError::TrailingBackslash);
                    }
                    index += 2;
                    continue;
                }
                b'&' if index + 1 < line.len() && line[index + 1] == b'&' => {
                    let (start, end) = trim_spaces_range(line, segment_start, index);
                    if start == end {
                        return Err(ParseError::InvalidSyntax);
                    }
                    segments.push(start, end, Some(SegmentOperator::And))?;
                    index += 2;
                    segment_start = index;
                    continue;
                }
                b'|' if index + 1 < line.len() && line[index + 1] == b'|' => {
                    let (start, end) = trim_spaces_range(line, segment_start, index);
                    if start == end {
                        return Err(ParseError::InvalidSyntax);
                    }
                    segments.push(start, end, Some(SegmentOperator::Or))?;
                    index += 2;
                    segment_start = index;
                    continue;
                }
                b';' => {
                    let (start, end) = trim_spaces_range(line, segment_start, index);
                    if start == end {
                        return Err(ParseError::InvalidSyntax);
                    }
                    segments.push(start, end, Some(SegmentOperator::Sequence))?;
                    index += 1;
                    segment_start = index;
                    continue;
                }
                _ => {}
            },
            ParseState::SingleQuoted => {
                if byte == b'\'' {
                    state = ParseState::Unquoted;
                }
            }
            ParseState::DoubleQuoted => match byte {
                b'"' => state = ParseState::Unquoted,
                b'\\' => {
                    if index + 1 >= line.len() {
                        return Err(ParseError::TrailingBackslash);
                    }
                    index += 2;
                    continue;
                }
                _ => {}
            },
        }

        index += 1;
    }

    match state {
        ParseState::SingleQuoted => return Err(ParseError::UnmatchedSingleQuote),
        ParseState::DoubleQuoted => return Err(ParseError::UnmatchedDoubleQuote),
        ParseState::Unquoted => {}
    }

    let (start, end) = trim_spaces_range(line, segment_start, line.len());
    if start == end {
        if segments.count == 0 {
            return Ok(segments);
        }
        return Err(ParseError::InvalidSyntax);
    }

    segments.push(start, end, None)?;
    Ok(segments)
}

fn trim_spaces_range(bytes: &[u8], start: usize, end: usize) -> (usize, usize) {
    let mut trimmed_start = start;
    let mut trimmed_end = end;

    while trimmed_start < trimmed_end && bytes[trimmed_start] == b' ' {
        trimmed_start += 1;
    }
    while trimmed_end > trimmed_start && bytes[trimmed_end - 1] == b' ' {
        trimmed_end -= 1;
    }

    (trimmed_start, trimmed_end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::String;
    use std::vec::Vec;

    fn segment_strings(line: &str) -> Vec<String> {
        let scan = scan_segments(line.as_bytes()).unwrap();
        (0..scan.count())
            .map(|index| String::from_utf8(scan.segment(line.as_bytes(), index).unwrap().to_vec()).unwrap())
            .collect()
    }

    fn operators(line: &str) -> Vec<SegmentOperator> {
        let scan = scan_segments(line.as_bytes()).unwrap();
        (1..scan.count())
            .map(|index| scan.operator_before(index).unwrap())
            .collect()
    }

    fn tokens(line: &str) -> Vec<String> {
        let parsed = TokenizedCommand::parse(line.as_bytes()).unwrap();
        (0..parsed.count())
            .map(|index| parsed.token(index).unwrap().to_string())
            .collect()
    }

    #[test]
    fn scan_segments_handles_empty_line() {
        assert_eq!(scan_segments(b"").unwrap().count(), 0);
        assert_eq!(scan_segments(b"   ").unwrap().count(), 0);
    }

    #[test]
    fn scan_segments_tracks_operator_order() {
        assert_eq!(segment_strings("echo a && echo b || echo c ; echo d"), vec!["echo a", "echo b", "echo c", "echo d"]);
        assert_eq!(
            operators("echo a && echo b || echo c ; echo d"),
            vec![SegmentOperator::And, SegmentOperator::Or, SegmentOperator::Sequence]
        );
    }

    #[test]
    fn scan_segments_respects_quotes_and_escapes() {
        assert_eq!(segment_strings("echo \"a && b\""), vec!["echo \"a && b\""]);
        assert_eq!(segment_strings("echo \"a ; b\""), vec!["echo \"a ; b\""]);
        assert_eq!(segment_strings("echo a\\&\\&b"), vec!["echo a\\&\\&b"]);
        assert_eq!(segment_strings("echo a\\;b"), vec!["echo a\\;b"]);
    }

    #[test]
    fn scan_segments_rejects_invalid_syntax() {
        assert_eq!(scan_segments(b"|| echo").unwrap_err(), ParseError::InvalidSyntax);
        assert_eq!(scan_segments(b"echo ||").unwrap_err(), ParseError::InvalidSyntax);
        assert_eq!(scan_segments(b"echo ; ; ls").unwrap_err(), ParseError::InvalidSyntax);
        assert_eq!(scan_segments(b"echo || && ls").unwrap_err(), ParseError::InvalidSyntax);
    }

    #[test]
    fn scan_segments_reports_quote_and_escape_errors() {
        assert_eq!(scan_segments(b"echo 'oops").unwrap_err(), ParseError::UnmatchedSingleQuote);
        assert_eq!(scan_segments(b"echo \"oops").unwrap_err(), ParseError::UnmatchedDoubleQuote);
        assert_eq!(scan_segments(b"echo hello\\").unwrap_err(), ParseError::TrailingBackslash);
    }

    #[test]
    fn scan_segments_reports_resource_exhaustion() {
        let mut line = String::new();
        for index in 0..=MAX_CHAIN_SEGMENTS {
            if index != 0 {
                line.push_str(" ; ");
            }
            line.push('a');
        }
        assert_eq!(scan_segments(line.as_bytes()).unwrap_err(), ParseError::ResourceUnavailable);
    }

    #[test]
    fn token_parser_splits_and_collapses_spaces() {
        assert_eq!(tokens("echo hello world"), vec!["echo", "hello", "world"]);
        assert_eq!(tokens("echo   hello   world"), vec!["echo", "hello", "world"]);
    }

    #[test]
    fn token_parser_preserves_quoted_arguments() {
        assert_eq!(tokens("echo \"hello world\""), vec!["echo", "hello world"]);
        assert_eq!(tokens("echo 'hello world'"), vec!["echo", "hello world"]);
        assert_eq!(tokens("echo foo\"bar\""), vec!["echo", "foobar"]);
    }

    #[test]
    fn token_parser_preserves_empty_quoted_arguments() {
        assert_eq!(tokens("echo \"\""), vec!["echo", ""]);
        assert_eq!(tokens("echo ''"), vec!["echo", ""]);
    }

    #[test]
    fn token_parser_handles_escapes() {
        assert_eq!(tokens("echo a\\ b"), vec!["echo", "a b"]);
        assert_eq!(tokens("echo \\\"x\\\""), vec!["echo", "\"x\""]);
    }

    #[test]
    fn token_parser_reports_quote_and_escape_errors() {
        assert_eq!(TokenizedCommand::parse(b"echo 'oops").unwrap_err(), ParseError::UnmatchedSingleQuote);
        assert_eq!(TokenizedCommand::parse(b"echo \"oops").unwrap_err(), ParseError::UnmatchedDoubleQuote);
        assert_eq!(TokenizedCommand::parse(b"echo hello\\").unwrap_err(), ParseError::TrailingBackslash);
    }

    #[test]
    fn token_parser_reports_argument_capacity_exhaustion() {
        let mut line = String::from("cmd");
        for _ in 0..MAX_COMMAND_ARGS {
            line.push_str(" arg");
        }
        assert_eq!(
            TokenizedCommand::parse(line.as_bytes()).unwrap_err(),
            ParseError::ResourceUnavailable
        );
    }

    #[test]
    fn token_parser_reports_storage_exhaustion() {
        let line = "a".repeat(LINE_CAPACITY + 1);
        assert_eq!(
            TokenizedCommand::parse(line.as_bytes()).unwrap_err(),
            ParseError::ResourceUnavailable
        );
    }
}
