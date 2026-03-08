use boot_info::{FramebufferInfo, PixelFormat};
use core::cell::UnsafeCell;
use core::fmt;
use core::mem::MaybeUninit;
use core::ptr::write_volatile;
use core::slice;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::font::{glyph_for, GLYPH_HEIGHT, GLYPH_WIDTH};

const DEFAULT_FOREGROUND: u32 = 0xf9fafb;
const DEFAULT_BACKGROUND: u32 = 0x111827;
const PADDING_X: usize = 16;
const PADDING_Y: usize = 16;
const GLYPH_ADVANCE_X: usize = GLYPH_WIDTH + 2;
const GLYPH_ADVANCE_Y: usize = GLYPH_HEIGHT + 3;

static GLOBAL_CONSOLE: ConsoleCell = ConsoleCell::new();

pub struct FramebufferConsole {
    framebuffer: FramebufferInfo,
    foreground: u32,
    background: u32,
    visible_height: usize,
    columns: usize,
    rows: usize,
    cursor_column: usize,
    cursor_row: usize,
}

impl FramebufferConsole {
    pub fn new(framebuffer: FramebufferInfo, foreground: u32, background: u32) -> Self {
        let visible_height = visible_height_pixels(framebuffer);
        let columns = usable_columns(framebuffer.width as usize);
        let rows = usable_rows(visible_height);

        Self {
            framebuffer,
            foreground,
            background,
            visible_height,
            columns,
            rows,
            cursor_column: 0,
            cursor_row: 0,
        }
    }

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
    }

    pub fn write_str(&mut self, text: &str) {
        for byte in text.bytes() {
            match byte {
                b'\n' => self.new_line(),
                _ => {
                    if self.cursor_column >= self.columns {
                        self.new_line();
                    }

                    let origin_x = PADDING_X + (self.cursor_column * GLYPH_ADVANCE_X);
                    let origin_y = PADDING_Y + (self.cursor_row * GLYPH_ADVANCE_Y);
                    self.draw_glyph(origin_x, origin_y, glyph_for(byte));
                    self.cursor_column += 1;
                }
            }
        }
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
