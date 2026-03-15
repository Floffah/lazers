//! Minimal x86 port-I/O helpers shared by hardware-facing bootstrap subsystems.

use core::arch::asm;

/// Reads one byte from an I/O port.
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!(
        include_str!("inb.port_io.asm"),
        in("dx") port,
        out("al") value,
        options(nomem, nostack, preserves_flags)
    );
    value
}

/// Writes one byte to an I/O port.
pub unsafe fn outb(port: u16, value: u8) {
    asm!(
        include_str!("outb.port_io.asm"),
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}

/// Writes one word to an I/O port.
pub unsafe fn outw(port: u16, value: u16) {
    asm!(
        include_str!("outw.port_io.asm"),
        in("dx") port,
        in("ax") value,
        options(nomem, nostack, preserves_flags)
    );
}
