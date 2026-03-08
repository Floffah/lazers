use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

use core::arch::asm;

const DATA_PORT: u16 = 0x60;
const STATUS_PORT: u16 = 0x64;
const OUTPUT_BUFFER_FULL: u8 = 1 << 0;
const AUXILIARY_DATA: u8 = 1 << 5;
const EVENT_QUEUE_CAPACITY: usize = 64;

static KEYBOARD_STATE: KeyboardCell = KeyboardCell::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCode {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Space,
    Minus,
    Equals,
    LeftBracket,
    RightBracket,
    Backslash,
    Semicolon,
    Apostrophe,
    Grave,
    Comma,
    Period,
    Slash,
    Enter,
    Backspace,
    LeftShift,
    RightShift,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: KeyCode,
    pub state: KeyState,
    pub shift_active: bool,
}

pub fn poll() {
    with_keyboard(|keyboard| {
        while let Some(byte) = read_controller_byte() {
            keyboard.process_byte(byte);
        }
    });
}

pub fn pop_event() -> Option<KeyEvent> {
    with_keyboard_result(|keyboard| keyboard.queue.pop())
}

pub fn event_to_char(event: KeyEvent) -> Option<char> {
    if event.state != KeyState::Pressed {
        return None;
    }

    let shifted = event.shift_active;
    let byte = match event.key {
        KeyCode::A => shifted_byte(b'a', b'A', shifted),
        KeyCode::B => shifted_byte(b'b', b'B', shifted),
        KeyCode::C => shifted_byte(b'c', b'C', shifted),
        KeyCode::D => shifted_byte(b'd', b'D', shifted),
        KeyCode::E => shifted_byte(b'e', b'E', shifted),
        KeyCode::F => shifted_byte(b'f', b'F', shifted),
        KeyCode::G => shifted_byte(b'g', b'G', shifted),
        KeyCode::H => shifted_byte(b'h', b'H', shifted),
        KeyCode::I => shifted_byte(b'i', b'I', shifted),
        KeyCode::J => shifted_byte(b'j', b'J', shifted),
        KeyCode::K => shifted_byte(b'k', b'K', shifted),
        KeyCode::L => shifted_byte(b'l', b'L', shifted),
        KeyCode::M => shifted_byte(b'm', b'M', shifted),
        KeyCode::N => shifted_byte(b'n', b'N', shifted),
        KeyCode::O => shifted_byte(b'o', b'O', shifted),
        KeyCode::P => shifted_byte(b'p', b'P', shifted),
        KeyCode::Q => shifted_byte(b'q', b'Q', shifted),
        KeyCode::R => shifted_byte(b'r', b'R', shifted),
        KeyCode::S => shifted_byte(b's', b'S', shifted),
        KeyCode::T => shifted_byte(b't', b'T', shifted),
        KeyCode::U => shifted_byte(b'u', b'U', shifted),
        KeyCode::V => shifted_byte(b'v', b'V', shifted),
        KeyCode::W => shifted_byte(b'w', b'W', shifted),
        KeyCode::X => shifted_byte(b'x', b'X', shifted),
        KeyCode::Y => shifted_byte(b'y', b'Y', shifted),
        KeyCode::Z => shifted_byte(b'z', b'Z', shifted),
        KeyCode::Digit0 => shifted_byte(b'0', b')', shifted),
        KeyCode::Digit1 => shifted_byte(b'1', b'!', shifted),
        KeyCode::Digit2 => shifted_byte(b'2', b'@', shifted),
        KeyCode::Digit3 => shifted_byte(b'3', b'#', shifted),
        KeyCode::Digit4 => shifted_byte(b'4', b'$', shifted),
        KeyCode::Digit5 => shifted_byte(b'5', b'%', shifted),
        KeyCode::Digit6 => shifted_byte(b'6', b'^', shifted),
        KeyCode::Digit7 => shifted_byte(b'7', b'&', shifted),
        KeyCode::Digit8 => shifted_byte(b'8', b'*', shifted),
        KeyCode::Digit9 => shifted_byte(b'9', b'(', shifted),
        KeyCode::Space => b' ',
        KeyCode::Minus => shifted_byte(b'-', b'_', shifted),
        KeyCode::Equals => shifted_byte(b'=', b'+', shifted),
        KeyCode::LeftBracket => shifted_byte(b'[', b'{', shifted),
        KeyCode::RightBracket => shifted_byte(b']', b'}', shifted),
        KeyCode::Backslash => shifted_byte(b'\\', b'|', shifted),
        KeyCode::Semicolon => shifted_byte(b';', b':', shifted),
        KeyCode::Apostrophe => shifted_byte(b'\'', b'"', shifted),
        KeyCode::Grave => shifted_byte(b'`', b'~', shifted),
        KeyCode::Comma => shifted_byte(b',', b'<', shifted),
        KeyCode::Period => shifted_byte(b'.', b'>', shifted),
        KeyCode::Slash => shifted_byte(b'/', b'?', shifted),
        _ => return None,
    };

    Some(byte as char)
}

fn shifted_byte(normal: u8, shifted: u8, shift_active: bool) -> u8 {
    if shift_active { shifted } else { normal }
}

fn read_controller_byte() -> Option<u8> {
    let status = unsafe { inb(STATUS_PORT) };
    if status & OUTPUT_BUFFER_FULL == 0 {
        return None;
    }

    let byte = unsafe { inb(DATA_PORT) };
    if status & AUXILIARY_DATA != 0 {
        return None;
    }

    Some(byte)
}

fn with_keyboard<F>(operation: F)
where
    F: FnOnce(&mut KeyboardState),
{
    unsafe {
        operation(KEYBOARD_STATE.get());
    }
}

fn with_keyboard_result<F, T>(operation: F) -> T
where
    F: FnOnce(&mut KeyboardState) -> T,
{
    unsafe { operation(KEYBOARD_STATE.get()) }
}

struct KeyboardCell {
    state: UnsafeCell<MaybeUninit<KeyboardState>>,
}

impl KeyboardCell {
    const fn new() -> Self {
        Self {
            state: UnsafeCell::new(MaybeUninit::new(KeyboardState::new())),
        }
    }

    unsafe fn get(&self) -> &mut KeyboardState {
        (*self.state.get()).assume_init_mut()
    }
}

unsafe impl Sync for KeyboardCell {}

struct KeyboardState {
    left_shift_active: bool,
    right_shift_active: bool,
    extended_prefix: bool,
    queue: EventQueue,
}

impl KeyboardState {
    const fn new() -> Self {
        Self {
            left_shift_active: false,
            right_shift_active: false,
            extended_prefix: false,
            queue: EventQueue::new(),
        }
    }

    fn process_byte(&mut self, byte: u8) {
        if byte == 0xe0 {
            self.extended_prefix = true;
            return;
        }

        let event = if self.extended_prefix {
            self.extended_prefix = false;
            self.decode_extended(byte)
        } else {
            self.decode_set1(byte)
        };

        let Some(mut event) = event else {
            return;
        };

        match event.key {
            KeyCode::LeftShift => self.left_shift_active = event.state == KeyState::Pressed,
            KeyCode::RightShift => self.right_shift_active = event.state == KeyState::Pressed,
            _ => {}
        }

        event.shift_active = self.shift_active();
        let _ = self.queue.push(event);
    }

    fn decode_set1(&self, byte: u8) -> Option<KeyEvent> {
        let released = byte & 0x80 != 0;
        let scan_code = byte & 0x7f;
        let key = decode_key(scan_code);

        if key == KeyCode::Unknown {
            return None;
        }

        Some(KeyEvent {
            key,
            state: if released {
                KeyState::Released
            } else {
                KeyState::Pressed
            },
            shift_active: self.shift_active(),
        })
    }

    fn decode_extended(&self, byte: u8) -> Option<KeyEvent> {
        let released = byte & 0x80 != 0;
        let scan_code = byte & 0x7f;

        let key = match scan_code {
            _ => KeyCode::Unknown,
        };

        if key == KeyCode::Unknown {
            return None;
        }

        Some(KeyEvent {
            key,
            state: if released {
                KeyState::Released
            } else {
                KeyState::Pressed
            },
            shift_active: self.shift_active(),
        })
    }

    fn shift_active(&self) -> bool {
        self.left_shift_active || self.right_shift_active
    }
}

fn decode_key(scan_code: u8) -> KeyCode {
    match scan_code {
        0x02 => KeyCode::Digit1,
        0x03 => KeyCode::Digit2,
        0x04 => KeyCode::Digit3,
        0x05 => KeyCode::Digit4,
        0x06 => KeyCode::Digit5,
        0x07 => KeyCode::Digit6,
        0x08 => KeyCode::Digit7,
        0x09 => KeyCode::Digit8,
        0x0a => KeyCode::Digit9,
        0x0b => KeyCode::Digit0,
        0x0c => KeyCode::Minus,
        0x0d => KeyCode::Equals,
        0x0e => KeyCode::Backspace,
        0x10 => KeyCode::Q,
        0x11 => KeyCode::W,
        0x12 => KeyCode::E,
        0x13 => KeyCode::R,
        0x14 => KeyCode::T,
        0x15 => KeyCode::Y,
        0x16 => KeyCode::U,
        0x17 => KeyCode::I,
        0x18 => KeyCode::O,
        0x19 => KeyCode::P,
        0x1a => KeyCode::LeftBracket,
        0x1b => KeyCode::RightBracket,
        0x1c => KeyCode::Enter,
        0x1e => KeyCode::A,
        0x1f => KeyCode::S,
        0x20 => KeyCode::D,
        0x21 => KeyCode::F,
        0x22 => KeyCode::G,
        0x23 => KeyCode::H,
        0x24 => KeyCode::J,
        0x25 => KeyCode::K,
        0x26 => KeyCode::L,
        0x27 => KeyCode::Semicolon,
        0x28 => KeyCode::Apostrophe,
        0x29 => KeyCode::Grave,
        0x2a => KeyCode::LeftShift,
        0x2b => KeyCode::Backslash,
        0x2c => KeyCode::Z,
        0x2d => KeyCode::X,
        0x2e => KeyCode::C,
        0x2f => KeyCode::V,
        0x30 => KeyCode::B,
        0x31 => KeyCode::N,
        0x32 => KeyCode::M,
        0x33 => KeyCode::Comma,
        0x34 => KeyCode::Period,
        0x35 => KeyCode::Slash,
        0x36 => KeyCode::RightShift,
        0x39 => KeyCode::Space,
        _ => KeyCode::Unknown,
    }
}

struct EventQueue {
    entries: [Option<KeyEvent>; EVENT_QUEUE_CAPACITY],
    read_index: usize,
    write_index: usize,
    len: usize,
}

impl EventQueue {
    const fn new() -> Self {
        Self {
            entries: [None; EVENT_QUEUE_CAPACITY],
            read_index: 0,
            write_index: 0,
            len: 0,
        }
    }

    fn push(&mut self, event: KeyEvent) -> Result<(), ()> {
        if self.len >= self.entries.len() {
            return Err(());
        }

        self.entries[self.write_index] = Some(event);
        self.write_index = (self.write_index + 1) % self.entries.len();
        self.len += 1;
        Ok(())
    }

    fn pop(&mut self) -> Option<KeyEvent> {
        if self.len == 0 {
            return None;
        }

        let event = self.entries[self.read_index].take();
        self.read_index = (self.read_index + 1) % self.entries.len();
        self.len -= 1;
        event
    }
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!(
        "in al, dx",
        in("dx") port,
        out("al") value,
        options(nomem, nostack, preserves_flags)
    );
    value
}
