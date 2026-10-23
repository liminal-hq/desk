// The 8254 programmable interval timer: a periodic tick on channel 0 (IRQ0).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{pic, port::outb};
use core::sync::atomic::{AtomicU32, Ordering};

const CHANNEL0: u16 = 0x40;
const COMMAND: u16 = 0x43;
const INPUT_HZ: u32 = 1_193_182;

pub const HZ: u32 = 100;

static TICKS: AtomicU32 = AtomicU32::new(0);

/// Programs channel 0 as a rate generator (mode 2) dividing the 1.193182 MHz input clock down to `hz`.
pub fn init(hz: u32) {
    let divisor = (INPUT_HZ + hz / 2) / hz;
    outb(COMMAND, 0x34); // channel 0, low then high byte, mode 2, binary
    outb(CHANNEL0, divisor as u8);
    outb(CHANNEL0, (divisor >> 8) as u8);
}

/// Called from the IRQ0 handler.
pub fn tick() {
    TICKS.fetch_add(1, Ordering::SeqCst);
    pic::eoi_master();
}

pub fn ticks() -> u32 {
    TICKS.load(Ordering::SeqCst)
}
