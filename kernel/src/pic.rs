// The 8259 programmable interrupt controllers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::port::{inb, io_wait, outb};

const PIC1: u16 = 0x20;
const PIC2: u16 = 0xA0;

pub const IRQ_BASE: u8 = 0x20;

/// Remaps IRQ 0-7 to vectors 0x20-0x27 and IRQ 8-15 to 0x28-0x2F, away from the CPU exception vectors the BIOS default
/// overlaps (IRQ 0-7 on 0x08-0x0F), then masks every line except the timer.
pub fn init() {
    outb(PIC1, 0x11); // ICW1: edge-triggered, cascade, ICW4 follows
    io_wait();
    outb(PIC2, 0x11);
    io_wait();
    outb(PIC1 + 1, IRQ_BASE); // ICW2: vector offsets
    io_wait();
    outb(PIC2 + 1, IRQ_BASE + 8);
    io_wait();
    outb(PIC1 + 1, 0x04); // ICW3: slave on IRQ2
    io_wait();
    outb(PIC2 + 1, 0x02); // ICW3: slave identity
    io_wait();
    outb(PIC1 + 1, 0x01); // ICW4: 8086 mode
    io_wait();
    outb(PIC2 + 1, 0x01);
    io_wait();
    outb(PIC1 + 1, 0xFE); // only IRQ0
    outb(PIC2 + 1, 0xFF);
}

/// Masks IRQ0 again, for the negative test that expects the missing timer to be caught.
pub fn mask_timer() {
    outb(PIC1 + 1, inb(PIC1 + 1) | 1);
}

/// End of interrupt for a line on the master controller.
pub fn eoi_master() {
    outb(PIC1, 0x20);
}
