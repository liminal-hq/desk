// Time: a monotonic tick source and a calibrated delay.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A 486 has no time-stamp counter, so the monotonic clock is the PIT's channel 0 interrupt counted in a 32-bit word (the widest
// atomic the target has). At 100 Hz it wraps after about 497 days and at 1 kHz after about 49 days, so every comparison uses
// wrapping subtraction: an interval is correct as long as it is shorter than the wrap. Sub-tick precision, when a driver needs it,
// comes from reading the PIT's counter (`desk-pit`), not from the CPU.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::irq::SharedIrqService;

/// A reading of a monotonic tick counter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticks(pub u32);

impl Ticks {
    /// Ticks from `earlier` to `self`, correct across one wrap of the counter.
    pub const fn since(self, earlier: Ticks) -> u32 {
        self.0.wrapping_sub(earlier.0)
    }
}

/// A monotonic clock that advances by whole ticks at a known rate.
pub trait Clock {
    fn now(&self) -> Ticks;
    /// The tick rate in millihertz, so the BIOS's 18.2 Hz (18,207 mHz) and the PIT's real rates (99,998 mHz when 100 Hz is asked
    /// for) are exact enough. Zero if the clock is not running.
    fn millihertz(&self) -> u32;

    /// Milliseconds since `earlier`, rounded down, saturating at `u32::MAX`. 64-bit arithmetic is fine on the 486 (it is plain
    /// code); only 64-bit atomics are missing.
    fn millis_since(&self, earlier: Ticks) -> u32 {
        match self.millihertz() {
            0 => 0,
            mhz => (u64::from(self.now().since(earlier)) * 1_000_000 / u64::from(mhz)).min(u64::from(u32::MAX)) as u32,
        }
    }
}

/// A busy wait of at least the given time, calibrated against a `Clock` at start-up (or by reading the PIT's counter). For device
/// timing (the OPL's register delays, a reset pulse), not for scheduling: the core waits for events, it does not spin.
pub trait Delay {
    fn delay_us(&self, us: u32);
}

/// A counter of interrupts, shared between an interrupt handler (the only writer) and the core. The handler's half holds a
/// `&'static TickCounter` and calls `tick`; the core reads it through `Clock`.
#[derive(Debug, Default)]
pub struct TickCounter {
    ticks: AtomicU32,
}

impl TickCounter {
    pub const fn new() -> TickCounter {
        TickCounter {
            ticks: AtomicU32::new(0),
        }
    }

    /// One tick. `fetch_add` is `lock xadd`, which a 486 has and DOSBox's `cputype = 486` accepts.
    pub fn tick(&self) {
        self.ticks.fetch_add(1, Ordering::Release);
    }

    pub fn now(&self) -> Ticks {
        Ticks(self.ticks.load(Ordering::Acquire))
    }
}

/// The tick counter as an interrupt service: what a timer driver registers (with `IrqHandler::from_shared`) when nothing else is
/// needed per tick.
impl SharedIrqService for TickCounter {
    fn on_irq(&self) {
        self.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(u32, u32);
    impl Clock for Fixed {
        fn now(&self) -> Ticks {
            Ticks(self.0)
        }
        fn millihertz(&self) -> u32 {
            self.1
        }
    }

    #[test]
    fn intervals_survive_the_wrap() {
        assert_eq!(Ticks(5).since(Ticks(u32::MAX - 4)), 10);
        assert_eq!(Fixed(5, 100_000).millis_since(Ticks(u32::MAX - 4)), 100);
        assert_eq!(Fixed(1193, 1_000_000).millis_since(Ticks(0)), 1193);
        assert_eq!(Fixed(182, 18_207).millis_since(Ticks(0)), 9_996, "ten seconds of BIOS ticks");
        assert_eq!(Fixed(u32::MAX, 18_207).millis_since(Ticks(0)), u32::MAX, "saturates");
        assert_eq!(Fixed(7, 0).millis_since(Ticks(0)), 0);
    }

    #[test]
    fn the_counter_as_a_service_ticks() {
        static C: TickCounter = TickCounter::new();
        let h = crate::IrqHandler::from_shared(&C);
        // SAFETY: the test is the only caller.
        unsafe {
            h.call();
            h.call();
        }
        assert_eq!(C.now(), Ticks(2));
    }
}
