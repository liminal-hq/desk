// The 8259 interrupt controllers, through the shared driver.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::hal::PIC;
use crate::sync;
use desk_hal::irq::{InterruptController, Irq};
use desk_pic::REMAPPED_BASES;

/// Remaps IRQ 0-7 to vectors 0x20-0x27 and IRQ 8-15 to 0x28-0x2F, away from the CPU exception vectors the BIOS default
/// overlaps (IRQ 0-7 on 0x08-0x0F), then masks every line except the timer.
pub fn init() {
    PIC.init(REMAPPED_BASES, 0xFFFE);
}

/// Masks IRQ0 again, for the negative test that expects the missing timer to be caught.
pub fn mask_timer() {
    sync::without_interrupts(|| PIC.mask(Irq::TIMER));
}

/// Unmasks a line (and, for IRQ 8-15, the cascade).
pub fn unmask(irq: Irq) {
    sync::without_interrupts(|| PIC.unmask(irq));
}
