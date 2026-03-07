#![no_main]
#![no_std]

use boot_info::{BootInfo, FramebufferInfo};
use core::arch::{asm, global_asm};
use core::panic::PanicInfo;
use core::ptr::write_volatile;
use core::slice;

global_asm!(
    r#"
    .section .text._start,"ax"
    .global _start
_start:
    cli
    call kernel_main
1:
    hlt
    jmp 1b
"#
);

#[no_mangle]
pub extern "sysv64" fn kernel_main(boot_info: *const BootInfo) -> ! {
    let Some(boot_info) = (unsafe { boot_info.as_ref() }) else {
        halt_forever();
    };

    if !boot_info.has_valid_header() || !boot_info.framebuffer.is_usable() {
        halt_forever();
    }

    paint_framebuffer(&boot_info.framebuffer, 0x0000_ff00);
    halt_forever();
}

fn paint_framebuffer(framebuffer: &FramebufferInfo, pixel: u32) {
    let pixel_count = framebuffer.stride as usize * framebuffer.height as usize;
    let max_pixels = framebuffer.size / core::mem::size_of::<u32>();
    let visible_pixels = core::cmp::min(pixel_count, max_pixels);
    let pixels = unsafe { slice::from_raw_parts_mut(framebuffer.base.cast::<u32>(), visible_pixels) };

    for y in 0..framebuffer.height as usize {
        let row_start = y * framebuffer.stride as usize;
        let row_end = row_start + framebuffer.width as usize;
        for cell in &mut pixels[row_start..row_end] {
            unsafe {
                write_volatile(cell, pixel);
            }
        }
    }
}

fn halt_forever() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    halt_forever()
}
