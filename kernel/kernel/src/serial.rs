//! Best-effort COM1 serial output used for host-visible runtime logs.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::port_io::{inb, outb};

const COM1_BASE: u16 = 0x3f8;
const DATA_REGISTER: u16 = COM1_BASE;
const INTERRUPT_ENABLE_REGISTER: u16 = COM1_BASE + 1;
const FIFO_CONTROL_REGISTER: u16 = COM1_BASE + 2;
const LINE_CONTROL_REGISTER: u16 = COM1_BASE + 3;
const MODEM_CONTROL_REGISTER: u16 = COM1_BASE + 4;
const LINE_STATUS_REGISTER: u16 = COM1_BASE + 5;
const SCRATCH_REGISTER: u16 = COM1_BASE + 7;

const LINE_STATUS_TRANSMIT_HOLDING_EMPTY: u8 = 1 << 5;

static SERIAL_READY: AtomicBool = AtomicBool::new(false);

/// Initializes the bootstrap serial sink if a COM1-compatible port exists.
pub fn init() {
    if !probe_port() {
        SERIAL_READY.store(false, Ordering::Release);
        return;
    }

    unsafe {
        outb(INTERRUPT_ENABLE_REGISTER, 0x00);
        outb(LINE_CONTROL_REGISTER, 0x80);
        outb(DATA_REGISTER, 0x03);
        outb(INTERRUPT_ENABLE_REGISTER, 0x00);
        outb(LINE_CONTROL_REGISTER, 0x03);
        outb(FIFO_CONTROL_REGISTER, 0xc7);
        outb(MODEM_CONTROL_REGISTER, 0x0b);
    }

    SERIAL_READY.store(true, Ordering::Release);
}

/// Writes one byte to serial output, translating newlines for host terminals.
pub fn write_byte(byte: u8) {
    if !SERIAL_READY.load(Ordering::Acquire) {
        return;
    }

    if byte == b'\n' {
        write_raw_byte(b'\r');
    }
    write_raw_byte(byte);
}

fn probe_port() -> bool {
    unsafe {
        outb(SCRATCH_REGISTER, 0x5a);
        inb(SCRATCH_REGISTER) == 0x5a
    }
}

fn write_raw_byte(byte: u8) {
    unsafe {
        while (inb(LINE_STATUS_REGISTER) & LINE_STATUS_TRANSMIT_HOLDING_EMPTY) == 0 {
            core::hint::spin_loop();
        }
        outb(DATA_REGISTER, byte);
    }
}
