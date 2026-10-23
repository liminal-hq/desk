// Time: a monotonic tick source, a calibrated delay and the wall clock.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A 486 has no time-stamp counter, so the monotonic clock is the PIT's channel 0 interrupt counted in a 32-bit word (the widest
// atomic the target has). At 100 Hz it wraps after about 497 days and at 1 kHz after about 49 days, so every comparison uses
// wrapping subtraction: an interval is correct as long as it is shorter than the wrap. Sub-tick precision, when a driver needs it,
// comes from reading the PIT's counter (`desk-pit`), not from the CPU.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::irq::IrqService;
use crate::HalError;

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
    /// Ticks per second, as programmed (the PIT's real rate differs slightly; see `desk_pit::actual_millihertz`).
    fn hz(&self) -> u32;

    /// Milliseconds since `earlier`, rounded down. 64-bit arithmetic is fine on the 486 (it is plain code); only 64-bit atomics are
    /// missing.
    fn millis_since(&self, earlier: Ticks) -> u32 {
        match self.hz() {
            0 => 0,
            hz => (u64::from(self.now().since(earlier)) * 1000 / u64::from(hz)) as u32,
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

/// The tick counter as an interrupt service: what a timer driver registers when nothing else is needed per tick.
impl IrqService for &'static TickCounter {
    fn on_irq(&mut self) {
        self.tick();
    }
}

/// A calendar date and time as the machine keeps it (local time: neither the CMOS clock nor DOS knows the time zone).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// The wall clock: the CMOS real-time clock natively, INT 21h AH=2Ah/2Ch under DOS. Not monotonic: the user can set it.
pub trait WallClock {
    fn now(&self) -> Result<DateTime, HalError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(u32, u32);
    impl Clock for Fixed {
        fn now(&self) -> Ticks {
            Ticks(self.0)
        }
        fn hz(&self) -> u32 {
            self.1
        }
    }

    #[test]
    fn intervals_survive_the_wrap() {
        assert_eq!(Ticks(5).since(Ticks(u32::MAX - 4)), 10);
        assert_eq!(Fixed(5, 100).millis_since(Ticks(u32::MAX - 4)), 100);
        assert_eq!(Fixed(1193, 1000).millis_since(Ticks(0)), 1193);
        assert_eq!(Fixed(7, 0).millis_since(Ticks(0)), 0);
    }

    #[test]
    fn the_counter_as_a_service_ticks() {
        static C: TickCounter = TickCounter::new();
        let mut svc: &'static TickCounter = &C;
        svc.on_irq();
        svc.on_irq();
        assert_eq!(C.now(), Ticks(2));
    }
}
