// The 8254 timer's periodic tick on channel 0 (IRQ0), through the shared driver.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::hal::{CLOCK, IRQS, PORTS};
use desk_hal::irq::{Chain, Irq, IrqHandler, IrqRegistry};
use desk_hal::Clock;
use desk_pit::Pit;

pub const HZ: u32 = 100;

/// Programs channel 0 as a rate generator dividing the 1.193182 MHz input clock down to `hz`, and registers the tick counter as
/// the IRQ 0 handler.
pub fn init(hz: u32) {
    Pit::new(PORTS).program_clock(&CLOCK, hz).expect("a rate the PIT can produce");
    IRQS.register(Irq::TIMER, IrqHandler::from_shared(CLOCK.counter()), Chain::None).expect("IRQ 0 is free");
}

pub fn ticks() -> u32 {
    CLOCK.now().0
}
