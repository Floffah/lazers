//! Terminal endpoint queues and fullscreen terminal surface glue.
//!
//! The endpoint is the byte-stream object that processes talk to through stdio.
//! The surface is the hardware-facing side that turns keyboard events into input
//! bytes and drains output bytes into the framebuffer console.

use core::cell::UnsafeCell;

use crate::console;
use crate::keyboard::{KeyCode, KeyEvent, KeyState};

const TERMINAL_INPUT_CAPACITY: usize = 256;
const TERMINAL_OUTPUT_CAPACITY: usize = 1024;

static PRIMARY_TERMINAL_ENDPOINT: TerminalEndpoint = TerminalEndpoint::new();

/// Bidirectional byte queues for one interactive text session.
///
/// Kernel services and user processes share this object indirectly through
/// process handles rather than talking to the framebuffer or keyboard directly.
pub struct TerminalEndpoint {
    state: UnsafeCell<TerminalEndpointState>,
}

impl TerminalEndpoint {
    /// Creates an empty endpoint with fixed-size input and output queues.
    pub const fn new() -> Self {
        Self {
            state: UnsafeCell::new(TerminalEndpointState::new()),
        }
    }

    /// Pushes one translated input byte for the foreground program.
    pub fn push_input_byte(&self, byte: u8) -> bool {
        unsafe { (*self.state.get()).input.push(byte) }
    }

    /// Pops one pending input byte for a reader attached through stdio.
    pub fn pop_input_byte(&self) -> Option<u8> {
        unsafe { (*self.state.get()).input.pop() }
    }

    /// Pushes one output byte emitted by a program.
    pub fn push_output_byte(&self, byte: u8) -> bool {
        unsafe { (*self.state.get()).output.push(byte) }
    }

    /// Pops one pending output byte for presentation on the framebuffer.
    pub fn pop_output_byte(&self) -> Option<u8> {
        unsafe { (*self.state.get()).output.pop() }
    }
}

unsafe impl Sync for TerminalEndpoint {}

/// Fullscreen presentation layer for the primary terminal session.
pub struct TerminalSurface {
    endpoint: &'static TerminalEndpoint,
}

impl TerminalSurface {
    /// Binds the surface to the endpoint it should render and feed.
    pub const fn new(endpoint: &'static TerminalEndpoint) -> Self {
        Self { endpoint }
    }

    /// Marks the current console cursor position as the start of interactive
    /// terminal output.
    pub fn begin_session(&self) {
        console::begin_terminal_session();
    }

    /// Converts one keyboard event into terminal input bytes when appropriate.
    pub fn handle_key_event(&self, event: KeyEvent) {
        if let Some(byte) = key_event_to_input_byte(event) {
            let _ = self.endpoint.push_input_byte(byte);
        }
    }

    /// Drains any pending terminal output bytes into the framebuffer console.
    pub fn flush_output(&self) {
        while let Some(byte) = self.endpoint.pop_output_byte() {
            console::write_terminal_byte(byte);
        }
    }
}

/// Returns the single bootstrap terminal endpoint used by the early system.
pub fn primary_endpoint() -> &'static TerminalEndpoint {
    &PRIMARY_TERMINAL_ENDPOINT
}

fn key_event_to_input_byte(event: KeyEvent) -> Option<u8> {
    if event.state != KeyState::Pressed {
        return None;
    }

    let shifted = event.shift_active;
    match event.key {
        KeyCode::Enter => Some(b'\n'),
        KeyCode::Backspace => Some(0x7f),
        KeyCode::A => Some(shifted_byte(b'a', b'A', shifted)),
        KeyCode::B => Some(shifted_byte(b'b', b'B', shifted)),
        KeyCode::C => Some(shifted_byte(b'c', b'C', shifted)),
        KeyCode::D => Some(shifted_byte(b'd', b'D', shifted)),
        KeyCode::E => Some(shifted_byte(b'e', b'E', shifted)),
        KeyCode::F => Some(shifted_byte(b'f', b'F', shifted)),
        KeyCode::G => Some(shifted_byte(b'g', b'G', shifted)),
        KeyCode::H => Some(shifted_byte(b'h', b'H', shifted)),
        KeyCode::I => Some(shifted_byte(b'i', b'I', shifted)),
        KeyCode::J => Some(shifted_byte(b'j', b'J', shifted)),
        KeyCode::K => Some(shifted_byte(b'k', b'K', shifted)),
        KeyCode::L => Some(shifted_byte(b'l', b'L', shifted)),
        KeyCode::M => Some(shifted_byte(b'm', b'M', shifted)),
        KeyCode::N => Some(shifted_byte(b'n', b'N', shifted)),
        KeyCode::O => Some(shifted_byte(b'o', b'O', shifted)),
        KeyCode::P => Some(shifted_byte(b'p', b'P', shifted)),
        KeyCode::Q => Some(shifted_byte(b'q', b'Q', shifted)),
        KeyCode::R => Some(shifted_byte(b'r', b'R', shifted)),
        KeyCode::S => Some(shifted_byte(b's', b'S', shifted)),
        KeyCode::T => Some(shifted_byte(b't', b'T', shifted)),
        KeyCode::U => Some(shifted_byte(b'u', b'U', shifted)),
        KeyCode::V => Some(shifted_byte(b'v', b'V', shifted)),
        KeyCode::W => Some(shifted_byte(b'w', b'W', shifted)),
        KeyCode::X => Some(shifted_byte(b'x', b'X', shifted)),
        KeyCode::Y => Some(shifted_byte(b'y', b'Y', shifted)),
        KeyCode::Z => Some(shifted_byte(b'z', b'Z', shifted)),
        KeyCode::Digit0 => Some(shifted_byte(b'0', b')', shifted)),
        KeyCode::Digit1 => Some(shifted_byte(b'1', b'!', shifted)),
        KeyCode::Digit2 => Some(shifted_byte(b'2', b'@', shifted)),
        KeyCode::Digit3 => Some(shifted_byte(b'3', b'#', shifted)),
        KeyCode::Digit4 => Some(shifted_byte(b'4', b'$', shifted)),
        KeyCode::Digit5 => Some(shifted_byte(b'5', b'%', shifted)),
        KeyCode::Digit6 => Some(shifted_byte(b'6', b'^', shifted)),
        KeyCode::Digit7 => Some(shifted_byte(b'7', b'&', shifted)),
        KeyCode::Digit8 => Some(shifted_byte(b'8', b'*', shifted)),
        KeyCode::Digit9 => Some(shifted_byte(b'9', b'(', shifted)),
        KeyCode::Space => Some(b' '),
        KeyCode::Minus => Some(shifted_byte(b'-', b'_', shifted)),
        KeyCode::Equals => Some(shifted_byte(b'=', b'+', shifted)),
        KeyCode::LeftBracket => Some(shifted_byte(b'[', b'{', shifted)),
        KeyCode::RightBracket => Some(shifted_byte(b']', b'}', shifted)),
        KeyCode::Backslash => Some(shifted_byte(b'\\', b'|', shifted)),
        KeyCode::Semicolon => Some(shifted_byte(b';', b':', shifted)),
        KeyCode::Apostrophe => Some(shifted_byte(b'\'', b'"', shifted)),
        KeyCode::Grave => Some(shifted_byte(b'`', b'~', shifted)),
        KeyCode::Comma => Some(shifted_byte(b',', b'<', shifted)),
        KeyCode::Period => Some(shifted_byte(b'.', b'>', shifted)),
        KeyCode::Slash => Some(shifted_byte(b'/', b'?', shifted)),
        KeyCode::LeftShift | KeyCode::RightShift | KeyCode::Unknown => None,
    }
}

fn shifted_byte(normal: u8, shifted: u8, shift_active: bool) -> u8 {
    if shift_active {
        shifted
    } else {
        normal
    }
}

struct TerminalEndpointState {
    input: ByteQueue<TERMINAL_INPUT_CAPACITY>,
    output: ByteQueue<TERMINAL_OUTPUT_CAPACITY>,
}

impl TerminalEndpointState {
    const fn new() -> Self {
        Self {
            input: ByteQueue::new(),
            output: ByteQueue::new(),
        }
    }
}

struct ByteQueue<const CAPACITY: usize> {
    bytes: [u8; CAPACITY],
    read_index: usize,
    write_index: usize,
    len: usize,
}

impl<const CAPACITY: usize> ByteQueue<CAPACITY> {
    const fn new() -> Self {
        Self {
            bytes: [0; CAPACITY],
            read_index: 0,
            write_index: 0,
            len: 0,
        }
    }

    fn push(&mut self, byte: u8) -> bool {
        if self.len >= CAPACITY {
            return false;
        }

        self.bytes[self.write_index] = byte;
        self.write_index = (self.write_index + 1) % CAPACITY;
        self.len += 1;
        true
    }

    fn pop(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }

        let byte = self.bytes[self.read_index];
        self.read_index = (self.read_index + 1) % CAPACITY;
        self.len -= 1;
        Some(byte)
    }
}
