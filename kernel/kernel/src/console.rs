//! Framebuffer-backed text console used for boot output, panic screens, and the
//! first terminal surface.
//!
//! This console is intentionally simple: fixed-width glyphs, fixed colors,
//! wrapping and scrolling, and a small amount of terminal-session state so
//! backspace cannot erase the boot banner above the active input region.

use boot_info::{FramebufferInfo, PixelFormat};
use core::cell::UnsafeCell;
use core::fmt;
use core::mem::MaybeUninit;
use core::ptr::write_volatile;
use core::slice;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::font::{glyph_for, GLYPH_HEIGHT, GLYPH_WIDTH};
use crate::serial;

const DEFAULT_FOREGROUND: u32 = 0xf9fafb;
const DEFAULT_BACKGROUND: u32 = 0x111827;
const PADDING_X: usize = 16;
const PADDING_Y: usize = 16;
const GLYPH_ADVANCE_X: usize = GLYPH_WIDTH + 2;
const GLYPH_ADVANCE_Y: usize = GLYPH_HEIGHT + 3;
const MAX_TRACKED_ROWS: usize = 512;

static GLOBAL_CONSOLE: ConsoleCell = ConsoleCell::new();

#[derive(Clone, Copy, Default)]
struct ConsoleCursor {
    column: usize,
    row: usize,
}

pub struct FramebufferConsole {
    framebuffer: FramebufferInfo,
    foreground: u32,
    background: u32,
    visible_height: usize,
    columns: usize,
    rows: usize,
    cursor_column: usize,
    cursor_row: usize,
    row_lengths: [usize; MAX_TRACKED_ROWS],
    input_floor: Option<ConsoleCursor>,
}

impl FramebufferConsole {
    /// Creates a console over a validated direct-color framebuffer.
    pub fn new(framebuffer: FramebufferInfo, foreground: u32, background: u32) -> Self {
        let visible_height = visible_height_pixels(framebuffer);
        let columns = usable_columns(framebuffer.width as usize);
        let rows = usable_rows(visible_height).min(MAX_TRACKED_ROWS);

        Self {
            framebuffer,
            foreground,
            background,
            visible_height,
            columns,
            rows,
            cursor_column: 0,
            cursor_row: 0,
            row_lengths: [0; MAX_TRACKED_ROWS],
            input_floor: None,
        }
    }

    /// Clears the visible framebuffer and resets cursor/session state.
    pub fn clear(&mut self) {
        let pixel = self.encode_color(self.background);
        let pixels = self.pixels_mut();

        for cell in pixels.iter_mut() {
            unsafe {
                write_volatile(cell, pixel);
            }
        }

        self.cursor_column = 0;
        self.cursor_row = 0;
        self.row_lengths.fill(0);
        self.input_floor = None;
    }

    /// Writes ordinary UTF-8 text by routing each byte through the terminal
    /// byte path.
    ///
    /// Unsupported bytes are handled by the glyph layer rather than here.
    pub fn write_str(&mut self, text: &str) {
        for byte in text.bytes() {
            serial::write_byte(byte);
            self.write_terminal_byte(byte);
        }
    }

    /// Marks the current cursor location as the lowest row/column a terminal
    /// session may backspace into.
    pub fn begin_terminal_session(&mut self) {
        self.input_floor = Some(self.cursor());
    }

    /// Interprets one terminal byte, including the minimal control set currently
    /// supported by the text runtime.
    pub fn write_terminal_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            0x7f => {
                let _ = self.backspace_from_session();
            }
            _ => self.write_byte(byte),
        }
    }

    fn backspace_from_session(&mut self) -> bool {
        let Some(floor) = self.input_floor else {
            return false;
        };

        let cursor = self.cursor();
        if cursor.row == floor.row && cursor.column == floor.column {
            return false;
        }

        let Some(target) = self.previous_cursor_position() else {
            return false;
        };
        if target.row < floor.row || (target.row == floor.row && target.column < floor.column) {
            return false;
        }

        self.clear_cell(target.column, target.row);
        self.cursor_column = target.column;
        self.cursor_row = target.row;
        self.row_lengths[target.row] = target.column;
        true
    }

    fn new_line(&mut self) {
        self.cursor_column = 0;

        if self.cursor_row + 1 >= self.rows {
            self.scroll_up();
            self.cursor_row = self.rows.saturating_sub(1);
        } else {
            self.cursor_row += 1;
        }
    }

    fn scroll_up(&mut self) {
        let top = PADDING_Y.min(self.visible_height);
        let bottom = self.visible_height.saturating_sub(PADDING_Y);
        if bottom <= top {
            self.clear();
            return;
        }

        if bottom - top <= GLYPH_ADVANCE_Y {
            self.clear_band(top, bottom);
            return;
        }

        let stride = self.framebuffer.stride as usize;
        let shift = GLYPH_ADVANCE_Y;
        let source_start = (top + shift) * stride;
        let source_end = bottom * stride;
        let destination_start = top * stride;

        {
            let pixels = self.pixels_mut();
            pixels.copy_within(source_start..source_end, destination_start);
        }

        self.row_lengths.copy_within(1..self.rows, 0);
        self.row_lengths[self.rows.saturating_sub(1)] = 0;
        if let Some(floor) = self.input_floor.as_mut() {
            floor.row = floor.row.saturating_sub(1);
        }

        self.clear_band(bottom - shift, bottom);
    }

    fn clear_band(&mut self, start_y: usize, end_y: usize) {
        let pixel = self.encode_color(self.background);
        let stride = self.framebuffer.stride as usize;
        let pixels = self.pixels_mut();

        for y in start_y..end_y {
            let row_start = y * stride;
            let row_end = row_start + stride;

            for cell in &mut pixels[row_start..row_end] {
                unsafe {
                    write_volatile(cell, pixel);
                }
            }
        }
    }

    fn draw_glyph(&mut self, origin_x: usize, origin_y: usize, glyph: &[u8; GLYPH_HEIGHT]) {
        let pixel = self.encode_color(self.foreground);

        for (row_index, row) in glyph.iter().enumerate() {
            for column_index in 0..GLYPH_WIDTH {
                let bit = 1 << (GLYPH_WIDTH - 1 - column_index);
                if row & bit == 0 {
                    continue;
                }

                self.put_pixel(origin_x + column_index, origin_y + row_index, pixel);
            }
        }
    }

    fn put_pixel(&mut self, x: usize, y: usize, pixel: u32) {
        if x >= self.framebuffer.width as usize || y >= self.visible_height {
            return;
        }

        let stride = self.framebuffer.stride as usize;
        let index = (y * stride) + x;
        let pixels = self.pixels_mut();

        if index >= pixels.len() {
            return;
        }

        unsafe {
            write_volatile(&mut pixels[index], pixel);
        }
    }

    fn pixels_mut(&mut self) -> &mut [u32] {
        let visible_pixels = self.visible_height * self.framebuffer.stride as usize;
        unsafe { slice::from_raw_parts_mut(self.framebuffer.base.cast::<u32>(), visible_pixels) }
    }

    fn encode_color(&self, rgb: u32) -> u32 {
        let red = (rgb >> 16) & 0xff;
        let green = (rgb >> 8) & 0xff;
        let blue = rgb & 0xff;

        match self.framebuffer.format {
            // These writes target little-endian x86_64, so the least-significant byte lands
            // at the lowest framebuffer address.
            PixelFormat::Rgb => red | (green << 8) | (blue << 16),
            PixelFormat::Bgr => blue | (green << 8) | (red << 16),
            PixelFormat::Unknown => rgb,
        }
    }

    fn write_byte(&mut self, byte: u8) {
        if self.cursor_column >= self.columns {
            self.new_line();
        }

        let origin_x = PADDING_X + (self.cursor_column * GLYPH_ADVANCE_X);
        let origin_y = PADDING_Y + (self.cursor_row * GLYPH_ADVANCE_Y);
        self.clear_glyph_area(origin_x, origin_y);
        self.draw_glyph(origin_x, origin_y, glyph_for(byte));
        self.cursor_column += 1;
        self.row_lengths[self.cursor_row] =
            self.row_lengths[self.cursor_row].max(self.cursor_column);
    }

    fn clear_glyph_area(&mut self, origin_x: usize, origin_y: usize) {
        let background = self.encode_color(self.background);

        for row in 0..GLYPH_HEIGHT {
            for column in 0..GLYPH_ADVANCE_X {
                self.put_pixel(origin_x + column, origin_y + row, background);
            }
        }
    }

    fn clear_cell(&mut self, column: usize, row: usize) {
        let origin_x = PADDING_X + (column * GLYPH_ADVANCE_X);
        let origin_y = PADDING_Y + (row * GLYPH_ADVANCE_Y);
        self.clear_glyph_area(origin_x, origin_y);
    }

    fn previous_cursor_position(&self) -> Option<ConsoleCursor> {
        if self.cursor_column > 0 {
            return Some(ConsoleCursor {
                column: self.cursor_column - 1,
                row: self.cursor_row,
            });
        }

        let mut row = self.cursor_row;
        while row > 0 {
            row -= 1;
            let length = self.row_lengths[row];
            if length > 0 {
                return Some(ConsoleCursor {
                    column: length - 1,
                    row,
                });
            }
        }

        None
    }

    fn cursor(&self) -> ConsoleCursor {
        ConsoleCursor {
            column: self.cursor_column,
            row: self.cursor_row,
        }
    }
}

impl fmt::Write for FramebufferConsole {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        FramebufferConsole::write_str(self, text);
        Ok(())
    }
}

pub fn init(framebuffer: FramebufferInfo) {
    GLOBAL_CONSOLE.initialize(FramebufferConsole::new(
        framebuffer,
        DEFAULT_FOREGROUND,
        DEFAULT_BACKGROUND,
    ));
}

pub fn clear() {
    with_console(|console| {
        console.clear();
    });
}

pub fn begin_terminal_session() {
    with_console(|console| {
        console.begin_terminal_session();
    });
}

pub fn write_terminal_byte(byte: u8) {
    with_console(|console| {
        console.write_terminal_byte(byte);
    });
}

pub fn write_fmt(args: fmt::Arguments<'_>) {
    with_console(|console| {
        let _ = fmt::Write::write_fmt(console, args);
    });
}

fn with_console<F>(operation: F)
where
    F: FnOnce(&mut FramebufferConsole),
{
    let Some(mut guard) = GLOBAL_CONSOLE.try_lock() else {
        return;
    };

    operation(guard.get());
}

fn visible_height_pixels(framebuffer: FramebufferInfo) -> usize {
    let stride = framebuffer.stride as usize;
    let max_pixels = framebuffer.size / core::mem::size_of::<u32>();
    let rows_from_size = if stride == 0 { 0 } else { max_pixels / stride };
    (framebuffer.height as usize).min(rows_from_size)
}

fn usable_columns(width: usize) -> usize {
    let usable_width = width.saturating_sub(PADDING_X * 2);
    if usable_width < GLYPH_WIDTH {
        1
    } else {
        1 + ((usable_width - GLYPH_WIDTH) / GLYPH_ADVANCE_X)
    }
}

fn usable_rows(visible_height: usize) -> usize {
    let usable_height = visible_height.saturating_sub(PADDING_Y * 2);
    if usable_height < GLYPH_HEIGHT {
        1
    } else {
        1 + ((usable_height - GLYPH_HEIGHT) / GLYPH_ADVANCE_Y)
    }
}

struct ConsoleCell {
    initialized: AtomicBool,
    locked: AtomicBool,
    console: UnsafeCell<MaybeUninit<FramebufferConsole>>,
}

impl ConsoleCell {
    const fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            locked: AtomicBool::new(false),
            console: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    fn initialize(&self, console: FramebufferConsole) {
        unsafe {
            (*self.console.get()).write(console);
        }
        self.initialized.store(true, Ordering::Release);
    }

    fn try_lock(&self) -> Option<ConsoleGuard<'_>> {
        if !self.initialized.load(Ordering::Acquire) {
            return None;
        }

        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return None;
        }

        Some(ConsoleGuard { cell: self })
    }
}

unsafe impl Sync for ConsoleCell {}

struct ConsoleGuard<'a> {
    cell: &'a ConsoleCell,
}

impl<'a> ConsoleGuard<'a> {
    fn get(&mut self) -> &mut FramebufferConsole {
        unsafe { (*self.cell.console.get()).assume_init_mut() }
    }
}

impl Drop for ConsoleGuard<'_> {
    fn drop(&mut self) {
        self.cell.locked.store(false, Ordering::Release);
    }
}
