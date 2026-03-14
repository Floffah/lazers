use lash::{scan_segments, ParseError, SegmentOperator, TokenizedCommand, LINE_CAPACITY};
use liblazer::{self, println};

pub(crate) struct Shell {
    pub(crate) line: [u8; LINE_CAPACITY],
    pub(crate) len: usize,
    pub(crate) byte: [u8; 1],
    pub(crate) cwd: [u8; LINE_CAPACITY],
}

#[derive(Clone, Copy)]
pub(crate) enum ExecutionMode {
    Interactive,
    Batch,
}

impl Shell {
    pub(crate) const fn new() -> Self {
        Self {
            line: [0; LINE_CAPACITY],
            len: 0,
            byte: [0; 1],
            cwd: [0; LINE_CAPACITY],
        }
    }

    pub(crate) fn start(&mut self) -> ! {
        let mut args = liblazer::args();
        let _ = args.next();
        if let Some(option) = args.next() {
            if option == "-c" {
                let Some(command_line) = args.next() else {
                    println!("lash: missing command string");
                    liblazer::exit(1);
                };
                let status = self.execute_line(command_line.as_bytes(), ExecutionMode::Batch);
                liblazer::exit(status);
            }
        }

        self.run_interactive()
    }

    pub(crate) fn execute_line(&mut self, line: &[u8], mode: ExecutionMode) -> usize {
        let segments = match scan_segments(line) {
            Ok(segments) => segments,
            Err(ParseError::UnmatchedSingleQuote)
            | Err(ParseError::UnmatchedDoubleQuote)
            | Err(ParseError::TrailingBackslash)
            | Err(ParseError::InvalidSyntax) => {
                self.print_parse_error(mode);
                return 1;
            }
            Err(ParseError::ResourceUnavailable) => {
                println!("lash: unable to parse command: resource unavailable");
                return 1;
            }
        };

        if segments.count() == 0 {
            return 0;
        }

        let mut last_status = 0usize;
        let mut index = 0usize;
        while index < segments.count() {
            let should_run = if index == 0 {
                true
            } else {
                match segments.operator_before(index).unwrap() {
                    SegmentOperator::And => last_status == 0,
                    SegmentOperator::Or => last_status != 0,
                    SegmentOperator::Sequence => true,
                }
            };

            if should_run {
                last_status = self.execute_segment(segments.segment(line, index).unwrap(), mode);
            }

            index += 1;
        }

        last_status
    }

    fn execute_segment(&mut self, line: &[u8], mode: ExecutionMode) -> usize {
        match TokenizedCommand::parse(line) {
            Ok(tokens) => {
                if tokens.count() == 0 {
                    return 0;
                }
                let command = tokens.token(0).unwrap_or("");
                if command == "cd" {
                    self.run_cd(&tokens)
                } else if command == "env" {
                    self.run_env(&tokens)
                } else if command == "set" {
                    self.run_set(&tokens)
                } else if command == "unset" {
                    self.run_unset(&tokens)
                } else if command == "where" {
                    self.run_where(&tokens)
                } else if command == "exit" {
                    liblazer::exit(0);
                } else if !command.is_empty() {
                    self.run_command(command, &tokens, mode)
                } else {
                    self.command_not_found(command);
                    1
                }
            }
            Err(ParseError::UnmatchedSingleQuote)
            | Err(ParseError::UnmatchedDoubleQuote)
            | Err(ParseError::TrailingBackslash)
            | Err(ParseError::InvalidSyntax) => {
                self.print_parse_error(mode);
                1
            }
            Err(ParseError::ResourceUnavailable) => {
                println!("lash: unable to parse command: resource unavailable");
                1
            }
        }
    }

    pub(crate) fn print_parse_error(&self, mode: ExecutionMode) {
        println!("lash: parse error");
        if matches!(mode, ExecutionMode::Interactive) {
            println!();
        }
    }
}
