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

/// Turns a tick source into whole steps of a fixed length: the frame or update pacing the core wants from a clock that is faster
/// (or slower) than its steps. Integer only, one 32-bit division per call, and a function of the tick readings it is given alone.
///
/// The first call to `due` after `new` returns 0 and fixes the origin. After that `due` returns how many whole steps have elapsed
/// since the last call, and the leftover part of a step carries to the next call, so the long-run rate is exact. If more than
/// `max_catch_up` steps are owed (a stall: a long disk read, a debugger), it returns `max_catch_up`, drops the rest and
/// re-synchronises to `now`, so the stall is not followed by a burst. Readings are compared with wrapping subtraction, so the
/// counter's wrap is harmless; a reading that goes backwards looks like a very long stall. Needed by the fast-timer work (roadmap
/// R139 and R134), which has not been built on it yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pacer {
    ticks_per_step: u32,
    max_catch_up: u32,
    origin: Ticks,
    started: bool,
}

impl Pacer {
    /// A pacer of steps `ticks_per_step` ticks long (0 is taken as 1) that reports at most `max_catch_up` steps per call (0 means
    /// every elapsed step is dropped and `due` always returns 0).
    pub const fn new(ticks_per_step: u32, max_catch_up: u32) -> Pacer {
        Pacer {
            ticks_per_step: if ticks_per_step == 0 { 1 } else { ticks_per_step },
            max_catch_up,
            origin: Ticks(0),
            started: false,
        }
    }

    /// The whole steps elapsed since the last call, at most `max_catch_up`. Returns 0 on the first call, which sets the origin.
    pub fn due(&mut self, now: Ticks) -> u32 {
        if !self.started {
            self.started = true;
            self.origin = now;
            return 0;
        }
        let owed = now.since(self.origin) / self.ticks_per_step;
        if owed > self.max_catch_up {
            self.origin = now;
            return self.max_catch_up;
        }
        // `owed * ticks_per_step` is at most the elapsed ticks, so it cannot overflow.
        self.origin = Ticks(self.origin.0.wrapping_add(owed * self.ticks_per_step));
        owed
    }

    /// Starts afresh at `now`: steps are counted from this reading, so the next `due` returns the whole steps since `now`, not 0.
    pub fn reset(&mut self, now: Ticks) {
        self.started = true;
        self.origin = now;
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
    fn the_first_call_sets_the_origin() {
        let mut p = Pacer::new(10, 5);
        assert_eq!(p.due(Ticks(1_000)), 0);
        assert_eq!(p.due(Ticks(1_009)), 0);
        assert_eq!(p.due(Ticks(1_010)), 1, "exactly on the boundary");
        assert_eq!(p.due(Ticks(1_010)), 0);
    }

    #[test]
    fn whole_steps_only_and_the_remainder_carries() {
        let mut p = Pacer::new(10, 100);
        p.due(Ticks(0));
        assert_eq!(p.due(Ticks(25)), 2);
        assert_eq!(p.due(Ticks(29)), 0);
        assert_eq!(p.due(Ticks(30)), 1, "the 5 ticks left over were kept");
        let total: u32 = (31..=1_000).map(|t| p.due(Ticks(t))).sum::<u32>() + 3;
        assert_eq!(total, 100, "no step is lost or gained over a thousand ticks");
    }

    #[test]
    fn a_stall_is_capped_and_resynchronises() {
        let mut p = Pacer::new(10, 3);
        p.due(Ticks(0));
        assert_eq!(p.due(Ticks(30)), 3, "exactly the cap is not a stall");
        assert_eq!(p.due(Ticks(30 + 40)), 3, "four owed: three reported, one dropped");
        assert_eq!(p.due(Ticks(70 + 9)), 0, "no burst afterwards: counted from the stall's end");
        assert_eq!(p.due(Ticks(70 + 10)), 1);
        assert_eq!(p.due(Ticks(1_000_000)), 3, "a stall of many thousand steps");
        assert_eq!(p.due(Ticks(1_000_010)), 1);
        let mut none = Pacer::new(10, 0);
        none.due(Ticks(0));
        assert_eq!(none.due(Ticks(500)), 0);
    }

    #[test]
    fn the_counter_wrap_is_harmless() {
        let mut p = Pacer::new(10, 5);
        p.due(Ticks(u32::MAX - 15));
        assert_eq!(p.due(Ticks(u32::MAX)), 1, "15 ticks since the origin: one step, 5 left over");
        assert_eq!(p.due(Ticks(3)), 0, "9 ticks into the next step, across the wrap");
        assert_eq!(p.due(Ticks(4)), 1, "the step began at u32::MAX - 5, so it ends at 4");
        assert_eq!(p.due(Ticks(4 + 50)), 5);
        let mut q = Pacer::new(10, 3);
        q.due(Ticks(u32::MAX - 1));
        assert_eq!(q.due(Ticks(100)), 3, "a stall across the wrap is capped too");
        assert_eq!(q.due(Ticks(110)), 1);
    }

    #[test]
    fn reset_restarts_the_origin() {
        let mut p = Pacer::new(10, 5);
        p.due(Ticks(0));
        p.reset(Ticks(1_000));
        assert_eq!(p.due(Ticks(1_005)), 0);
        assert_eq!(p.due(Ticks(1_020)), 2);
        let mut one = Pacer::new(0, 5);
        one.due(Ticks(0));
        assert_eq!(one.due(Ticks(3)), 3, "a step length of 0 is taken as 1");
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
