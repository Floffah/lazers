pub const GLYPH_WIDTH: usize = 5;
pub const GLYPH_HEIGHT: usize = 7;

const SPACE: [u8; GLYPH_HEIGHT] = [
    0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000,
];
const EXCLAMATION: [u8; GLYPH_HEIGHT] = [
    0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
];
const DOUBLE_QUOTE: [u8; GLYPH_HEIGHT] = [
    0b01010, 0b01010, 0b01010, 0b00000, 0b00000, 0b00000, 0b00000,
];
const HASH: [u8; GLYPH_HEIGHT] = [
    0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010, 0b00000,
];
const DOLLAR: [u8; GLYPH_HEIGHT] = [
    0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100,
];
const PERCENT: [u8; GLYPH_HEIGHT] = [
    0b11001, 0b11010, 0b00100, 0b01000, 0b10110, 0b00110, 0b00000,
];
const AMPERSAND: [u8; GLYPH_HEIGHT] = [
    0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101,
];
const APOSTROPHE: [u8; GLYPH_HEIGHT] = [
    0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000,
];
const LEFT_PAREN: [u8; GLYPH_HEIGHT] = [
    0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010,
];
const RIGHT_PAREN: [u8; GLYPH_HEIGHT] = [
    0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000,
];
const ASTERISK: [u8; GLYPH_HEIGHT] = [
    0b00000, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0b00000,
];
const PLUS: [u8; GLYPH_HEIGHT] = [
    0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
];
const COMMA: [u8; GLYPH_HEIGHT] = [
    0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000,
];
const MINUS: [u8; GLYPH_HEIGHT] = [
    0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
];
const PERIOD: [u8; GLYPH_HEIGHT] = [
    0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
];
const SLASH: [u8; GLYPH_HEIGHT] = [
    0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000,
];
const ZERO: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
];
const ONE: [u8; GLYPH_HEIGHT] = [
    0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
];
const TWO: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
];
const THREE: [u8; GLYPH_HEIGHT] = [
    0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
];
const FOUR: [u8; GLYPH_HEIGHT] = [
    0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
];
const FIVE: [u8; GLYPH_HEIGHT] = [
    0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
];
const SIX: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
];
const SEVEN: [u8; GLYPH_HEIGHT] = [
    0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
];
const EIGHT: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
];
const NINE: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
];
const COLON: [u8; GLYPH_HEIGHT] = [
    0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
];
const SEMICOLON: [u8; GLYPH_HEIGHT] = [
    0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b01000,
];
const LESS_THAN: [u8; GLYPH_HEIGHT] = [
    0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010,
];
const EQUAL: [u8; GLYPH_HEIGHT] = [
    0b00000, 0b11111, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
];
const GREATER_THAN: [u8; GLYPH_HEIGHT] = [
    0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000,
];
const QUESTION: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100,
];
const AT: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110,
];

const A: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
];
const B: [u8; GLYPH_HEIGHT] = [
    0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
];
const C: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
];
const D: [u8; GLYPH_HEIGHT] = [
    0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
];
const E: [u8; GLYPH_HEIGHT] = [
    0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
];
const F: [u8; GLYPH_HEIGHT] = [
    0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
];
const G: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
];
const H: [u8; GLYPH_HEIGHT] = [
    0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
];
const I: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
];
const J: [u8; GLYPH_HEIGHT] = [
    0b00001, 0b00001, 0b00001, 0b00001, 0b10001, 0b10001, 0b01110,
];
const K: [u8; GLYPH_HEIGHT] = [
    0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
];
const L: [u8; GLYPH_HEIGHT] = [
    0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
];
const M: [u8; GLYPH_HEIGHT] = [
    0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
];
const N: [u8; GLYPH_HEIGHT] = [
    0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
];
const O: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
];
const P: [u8; GLYPH_HEIGHT] = [
    0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
];
const Q: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
];
const R: [u8; GLYPH_HEIGHT] = [
    0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
];
const S: [u8; GLYPH_HEIGHT] = [
    0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
];
const T: [u8; GLYPH_HEIGHT] = [
    0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
];
const U: [u8; GLYPH_HEIGHT] = [
    0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
];
const V: [u8; GLYPH_HEIGHT] = [
    0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
];
const W: [u8; GLYPH_HEIGHT] = [
    0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
];
const X: [u8; GLYPH_HEIGHT] = [
    0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
];
const Y: [u8; GLYPH_HEIGHT] = [
    0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
];
const Z: [u8; GLYPH_HEIGHT] = [
    0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
];

const LEFT_BRACKET: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110,
];
const BACKSLASH: [u8; GLYPH_HEIGHT] = [
    0b10000, 0b01000, 0b00100, 0b00010, 0b00001, 0b00000, 0b00000,
];
const RIGHT_BRACKET: [u8; GLYPH_HEIGHT] = [
    0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110,
];
const CARET: [u8; GLYPH_HEIGHT] = [
    0b00100, 0b01010, 0b10001, 0b00000, 0b00000, 0b00000, 0b00000,
];
const UNDERSCORE: [u8; GLYPH_HEIGHT] = [
    0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111,
];
const BACKTICK: [u8; GLYPH_HEIGHT] = [
    0b01000, 0b00100, 0b00010, 0b00000, 0b00000, 0b00000, 0b00000,
];
const LEFT_BRACE: [u8; GLYPH_HEIGHT] = [
    0b00010, 0b00100, 0b00100, 0b01000, 0b00100, 0b00100, 0b00010,
];
const VERTICAL_BAR: [u8; GLYPH_HEIGHT] = [
    0b00100, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00100,
];
const RIGHT_BRACE: [u8; GLYPH_HEIGHT] = [
    0b01000, 0b00100, 0b00100, 0b00010, 0b00100, 0b00100, 0b01000,
];
const TILDE: [u8; GLYPH_HEIGHT] = [
    0b00000, 0b00000, 0b01001, 0b10110, 0b00000, 0b00000, 0b00000,
];

pub fn glyph_for(byte: u8) -> &'static [u8; GLYPH_HEIGHT] {
    match byte {
        b' ' => &SPACE,
        b'!' => &EXCLAMATION,
        b'"' => &DOUBLE_QUOTE,
        b'#' => &HASH,
        b'$' => &DOLLAR,
        b'%' => &PERCENT,
        b'&' => &AMPERSAND,
        b'\'' => &APOSTROPHE,
        b'(' => &LEFT_PAREN,
        b')' => &RIGHT_PAREN,
        b'*' => &ASTERISK,
        b'+' => &PLUS,
        b',' => &COMMA,
        b'-' => &MINUS,
        b'.' => &PERIOD,
        b'/' => &SLASH,
        b'0' => &ZERO,
        b'1' => &ONE,
        b'2' => &TWO,
        b'3' => &THREE,
        b'4' => &FOUR,
        b'5' => &FIVE,
        b'6' => &SIX,
        b'7' => &SEVEN,
        b'8' => &EIGHT,
        b'9' => &NINE,
        b':' => &COLON,
        b';' => &SEMICOLON,
        b'<' => &LESS_THAN,
        b'=' => &EQUAL,
        b'>' => &GREATER_THAN,
        b'?' => &QUESTION,
        b'@' => &AT,
        b'A' | b'a' => &A,
        b'B' | b'b' => &B,
        b'C' | b'c' => &C,
        b'D' | b'd' => &D,
        b'E' | b'e' => &E,
        b'F' | b'f' => &F,
        b'G' | b'g' => &G,
        b'H' | b'h' => &H,
        b'I' | b'i' => &I,
        b'J' | b'j' => &J,
        b'K' | b'k' => &K,
        b'L' | b'l' => &L,
        b'M' | b'm' => &M,
        b'N' | b'n' => &N,
        b'O' | b'o' => &O,
        b'P' | b'p' => &P,
        b'Q' | b'q' => &Q,
        b'R' | b'r' => &R,
        b'S' | b's' => &S,
        b'T' | b't' => &T,
        b'U' | b'u' => &U,
        b'V' | b'v' => &V,
        b'W' | b'w' => &W,
        b'X' | b'x' => &X,
        b'Y' | b'y' => &Y,
        b'Z' | b'z' => &Z,
        b'[' => &LEFT_BRACKET,
        b'\\' => &BACKSLASH,
        b']' => &RIGHT_BRACKET,
        b'^' => &CARET,
        b'_' => &UNDERSCORE,
        b'`' => &BACKTICK,
        b'{' => &LEFT_BRACE,
        b'|' => &VERTICAL_BAR,
        b'}' => &RIGHT_BRACE,
        b'~' => &TILDE,
        _ => &QUESTION,
    }
}
