// COM1 serial port: 16550 at 115200 baud, 8N1, polled.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::port::{inb, outb};
use core::fmt;

const COM1: u16 = 0x3F8;

/// Programs the UART: interrupts off, divisor 1 (115200 baud), 8 data bits, no parity, one stop bit, FIFOs on.
pub fn init() {
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x80); // DLAB on
    outb(COM1, 0x01); // divisor low
    outb(COM1 + 1, 0x00); // divisor high
    outb(COM1 + 3, 0x03); // 8N1, DLAB off
    outb(COM1 + 2, 0xC7); // enable and clear FIFOs, 14-byte trigger
    outb(COM1 + 4, 0x0B); // DTR, RTS, OUT2
}

pub fn write_byte(b: u8) {
    // Bounded wait so a missing UART cannot stall the kernel forever.
    for _ in 0..100_000 {
        if inb(COM1 + 5) & 0x20 != 0 {
            break;
        }
    }
    outb(COM1, b);
}

pub struct Serial;

impl fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        s.bytes().for_each(write_byte);
        Ok(())
    }
}
