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
    cursor_x: usize,
    cursor_y: usize,
}

impl FramebufferConsole {
    pub fn new(framebuffer: FramebufferInfo, foreground: u32, background: u32) -> Self {
        Self {
            framebuffer,
            foreground,
            background,
            cursor_x: PADDING_X,
            cursor_y: PADDING_Y,
        }
    }

    pub fn clear(&mut self) {
        let pixel = self.encode_color(self.background);
        let max_pixels = self.framebuffer.size / core::mem::size_of::<u32>();
        let pixels = unsafe {
            slice::from_raw_parts_mut(self.framebuffer.base.cast::<u32>(), max_pixels)
        };

        for cell in pixels.iter_mut() {
            unsafe {
                write_volatile(cell, pixel);
            }
        }

        self.cursor_x = PADDING_X;
        self.cursor_y = PADDING_Y;
    }

    pub fn write_str(&mut self, text: &str) {
        for byte in text.bytes() {
            match byte {
                b'\n' => self.new_line(),
                _ => {
                    self.draw_glyph(self.cursor_x, self.cursor_y, glyph_for(byte));
                    self.cursor_x += GLYPH_ADVANCE_X;
                }
            }
        }
    }

    fn new_line(&mut self) {
        self.cursor_x = PADDING_X;
        self.cursor_y += GLYPH_ADVANCE_Y;
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
        if x >= self.framebuffer.width as usize || y >= self.framebuffer.height as usize {
            return;
        }

        let row_start = y * self.framebuffer.stride as usize;
        let index = row_start + x;
        let max_pixels = self.framebuffer.size / core::mem::size_of::<u32>();
        if index >= max_pixels {
            return;
        }

        let pixels = unsafe {
            slice::from_raw_parts_mut(self.framebuffer.base.cast::<u32>(), max_pixels)
        };

        unsafe {
            write_volatile(&mut pixels[index], pixel);
        }
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
