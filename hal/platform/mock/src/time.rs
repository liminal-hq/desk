// A clock a test moves by hand, and a delay that records instead of waiting.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::cell::Cell;

use desk_hal::time::{Clock, Delay, Ticks};

#[derive(Debug)]
pub struct MockClock {
    pub ticks: Cell<u32>,
    pub millihertz: u32,
}

impl MockClock {
    pub fn new(millihertz: u32) -> MockClock {
        MockClock {
            ticks: Cell::new(0),
            millihertz,
        }
    }

    pub fn advance(&self, ticks: u32) {
        self.ticks.set(self.ticks.get().wrapping_add(ticks));
    }
}

impl Clock for MockClock {
    fn now(&self) -> Ticks {
        Ticks(self.ticks.get())
    }
    fn millihertz(&self) -> u32 {
        self.millihertz
    }
}

/// Adds up the time asked for, so a test can check a driver waited long enough without waiting.
#[derive(Debug, Default)]
pub struct MockDelay {
    pub total_us: Cell<u64>,
    pub calls: Cell<u32>,
}

impl Delay for MockDelay {
    fn delay_us(&self, us: u32) {
        self.total_us.set(self.total_us.get() + u64::from(us));
        self.calls.set(self.calls.get() + 1);
    }
}
