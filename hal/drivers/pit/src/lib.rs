// The 8254 programmable interval timer: channel 0 as the periodic tick, its counter for sub-tick time, and the tick clock.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The reference is the native kernel's `kernel/src/pit.rs`. For the same frequency `program_periodic` computes the same divisor
// (rounded to the nearest, 11,932 for 100 Hz) and writes the same three bytes. Beyond the kernel's version: frequencies the chip
// cannot produce are refused instead of truncated, the BIOS rate (divisor 65,536, written as 0) can be restored, the counter can
// be latched and read for sub-tick timing, and the tick count lives in `PitClock`, a `Clock` the core reads. The kernel's handler
// also sent the end-of-interrupt; here that is the platform's job (`desk_hal::irq`), because under DOS the chained BIOS handler
// sends it.
#![cfg_attr(not(any(test, feature = "std")), no_std)]

use core::sync::atomic::{AtomicU32, Ordering};

use desk_hal::irq::IrqService;
use desk_hal::time::{Clock, TickCounter, Ticks};
use desk_hal::{HalError, PortIo};

/// The PIT's input clock in hertz (a third of the 3.579545 MHz NTSC colour-burst crystal, 14.31818 MHz / 12).
pub const INPUT_HZ: u32 = 1_193_182;

pub const CHANNEL0: u16 = 0x40;
pub const COMMAND: u16 = 0x43;

/// The largest divisor, written to the chip as 0. The BIOS programs it: about 18.2 Hz.
pub const MAX_DIVISOR: u32 = 65_536;
/// Mode 2 (rate generator) needs a divisor of at least 2.
pub const MIN_DIVISOR: u32 = 2;

/// Channel 0, low byte then high byte, mode 2 (rate generator), binary.
const CMD_CH0_LOHI_RATE: u8 = 0x34;
/// Channel 0, counter latch.
const CMD_CH0_LATCH: u8 = 0x00;

/// The divisor that comes closest to `hz`: `(INPUT_HZ + hz / 2) / hz`, as the kernel computes it. `InvalidArgument` if that falls
/// outside 2..=65,536, which leaves 19 Hz to 795,454 Hz in whole hertz (18 Hz would need 66,288; the BIOS's 18.2 Hz is
/// `program_divisor(MAX_DIVISOR)`).
pub const fn divisor_for(hz: u32) -> Result<u32, HalError> {
    if hz == 0 {
        return Err(HalError::InvalidArgument);
    }
    let divisor = (INPUT_HZ + hz / 2) / hz;
    if divisor < MIN_DIVISOR || divisor > MAX_DIVISOR {
        return Err(HalError::InvalidArgument);
    }
    Ok(divisor)
}

/// The rate a divisor really gives, in millihertz, rounded to the nearest (100 Hz asks for 11,932 and gets 99,998 mHz).
pub const fn actual_millihertz(divisor: u32) -> u32 {
    ((INPUT_HZ as u64 * 1000 + divisor as u64 / 2) / divisor as u64) as u32
}

/// The chip, reached through `ports`.
#[derive(Clone, Copy, Debug)]
pub struct Pit<P> {
    ports: P,
}

impl<P: PortIo> Pit<P> {
    pub const fn new(ports: P) -> Self {
        Pit { ports }
    }

    /// Programs channel 0 as a rate generator for the divisor (2..=65,536; 65,536 is written as 0).
    pub fn program_divisor(&self, divisor: u32) -> Result<(), HalError> {
        if !(MIN_DIVISOR..=MAX_DIVISOR).contains(&divisor) {
            return Err(HalError::InvalidArgument);
        }
        let reload = (divisor % MAX_DIVISOR) as u16;
        self.ports.outb(COMMAND, CMD_CH0_LOHI_RATE);
        self.ports.outb(CHANNEL0, reload as u8);
        self.ports.outb(CHANNEL0, (reload >> 8) as u8);
        Ok(())
    }

    /// Programs channel 0 to interrupt at about `hz` and returns the divisor used. Writes nothing if `hz` is out of range.
    pub fn program_periodic(&self, hz: u32) -> Result<u32, HalError> {
        let divisor = divisor_for(hz)?;
        self.program_divisor(divisor)?;
        Ok(divisor)
    }

    /// Puts back the BIOS rate (about 18.2 Hz). The hosted platform runs this on exit if Desk changed the rate.
    pub fn restore_bios_rate(&self) {
        // MAX_DIVISOR is in range, so this cannot fail.
        let _ = self.program_divisor(MAX_DIVISOR);
    }

    /// Latches channel 0 and reads its current count, which runs down from the divisor once per input clock (838 ns). With the
    /// tick count this gives sub-tick time without a TSC. Run with interrupts off: an interrupt handler that touches the PIT
    /// between the latch and the two reads would scramble them.
    pub fn read_counter(&self) -> u16 {
        self.ports.outb(COMMAND, CMD_CH0_LATCH);
        let lo = self.ports.inb(CHANNEL0);
        let hi = self.ports.inb(CHANNEL0);
        u16::from_le_bytes([lo, hi])
    }
}

/// The monotonic clock the PIT drives: the interrupt side ticks it, the core reads it. Lives in a `static`.
#[derive(Debug, Default)]
pub struct PitClock {
    counter: TickCounter,
    millihertz: AtomicU32,
}

impl PitClock {
    pub const fn new() -> PitClock {
        PitClock {
            counter: TickCounter::new(),
            millihertz: AtomicU32::new(0),
        }
    }

    /// Records the divisor channel 0 was programmed with (the BIOS rate is `MAX_DIVISOR`), so the clock reports the real rate.
    /// Call before registering the handler.
    pub fn set_divisor(&self, divisor: u32) {
        self.millihertz.store(actual_millihertz(divisor), Ordering::Relaxed);
    }
}

impl Clock for PitClock {
    fn now(&self) -> Ticks {
        self.counter.now()
    }
    fn millihertz(&self) -> u32 {
        self.millihertz.load(Ordering::Relaxed)
    }
}

/// The IRQ 0 work: one tick. The handler's state is just the shared reference, so `IrqHandler::from_static` takes a leaked
/// `&'static mut &'static PitClock` (or a `static mut` the platform owns).
impl IrqService for &'static PitClock {
    fn on_irq(&mut self) {
        self.counter.tick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use desk_hal::irq::{Chain, Irq, IrqHandler, IrqRegistry};
    use desk_hal_mock::devices::Pit8254;
    use desk_hal_mock::{Access, MockIrq, MockPortBus};

    #[test]
    fn the_divisor_is_rounded_to_the_nearest_as_in_the_kernel() {
        assert_eq!(divisor_for(100), Ok(11_932));
        assert_eq!(divisor_for(1_000), Ok(1_193));
        assert_eq!(divisor_for(60), Ok(19_886));
        assert_eq!(divisor_for(19), Ok(62_799));
        assert_eq!(divisor_for(397_728), Ok(3));
        assert_eq!(divisor_for(477_273), Ok(2));
        assert_eq!(divisor_for(795_454), Ok(2));
    }

    #[test]
    fn rates_the_chip_cannot_produce_are_refused() {
        for hz in [0, 1, 18, 795_455, 1_193_182, u32::MAX] {
            assert_eq!(divisor_for(hz), Err(HalError::InvalidArgument), "{hz} Hz");
        }
    }

    #[test]
    fn the_actual_rate_is_reported() {
        assert_eq!(actual_millihertz(11_932), 99_998);
        assert_eq!(actual_millihertz(MAX_DIVISOR), 18_207);
        assert_eq!(actual_millihertz(1_193), 1_000_153);
    }

    #[test]
    fn programming_100_hz_writes_what_the_kernel_writes() {
        // kernel/src/pit.rs: outb(0x43, 0x34); outb(0x40, divisor as u8); outb(0x40, (divisor >> 8) as u8), divisor 11,932 = 0x2E9C.
        let bus = MockPortBus::scripted([Access::outb(0x43, 0x34), Access::outb(0x40, 0x9C), Access::outb(0x40, 0x2E)]);
        assert_eq!(Pit::new(&bus).program_periodic(100), Ok(11_932));
        assert!(bus.script_done());
    }

    #[test]
    fn the_chip_model_sees_mode_2_and_the_reload_value() {
        let bus = MockPortBus::new();
        let chip = bus.attach(0x40, 0x43, Pit8254::default());
        let pit = Pit::new(&bus);
        pit.program_periodic(1_000).unwrap();
        let ch0 = chip.borrow().channels[0];
        assert_eq!(
            (ch0.access, ch0.mode, ch0.bcd, ch0.reload, ch0.loaded),
            (3, 2, false, 1_193, true)
        );
        pit.restore_bios_rate();
        assert_eq!(chip.borrow().channels[0].reload, 0, "65,536 is written as 0");
        assert_eq!(bus.writes()[3..], [(0x43, 0x34), (0x40, 0), (0x40, 0)]);
    }

    #[test]
    fn an_out_of_range_rate_writes_nothing() {
        let bus = MockPortBus::scripted([]);
        let pit = Pit::new(&bus);
        assert_eq!(pit.program_periodic(18), Err(HalError::InvalidArgument));
        assert_eq!(pit.program_divisor(1), Err(HalError::InvalidArgument));
        assert_eq!(pit.program_divisor(MAX_DIVISOR + 1), Err(HalError::InvalidArgument));
        assert!(bus.log().is_empty());
    }

    #[test]
    fn the_counter_is_latched_then_read_low_then_high() {
        let bus = MockPortBus::scripted([Access::outb(0x43, 0x00), Access::inb(0x40, 0x34), Access::inb(0x40, 0x12)]);
        assert_eq!(Pit::new(&bus).read_counter(), 0x1234);
        assert!(bus.script_done());

        let bus = MockPortBus::new();
        let chip = bus.attach(0x40, 0x43, Pit8254::default());
        let pit = Pit::new(&bus);
        pit.program_periodic(100).unwrap();
        chip.borrow_mut().count = 5_000;
        assert_eq!(pit.read_counter(), 5_000);
    }

    #[test]
    fn ticks_arrive_through_a_registered_handler() {
        static CLOCK: PitClock = PitClock::new();
        let bus = MockPortBus::new();
        let irq = MockIrq::new();
        let divisor = Pit::new(&bus).program_periodic(100).unwrap();
        CLOCK.set_divisor(divisor);
        assert_eq!(CLOCK.millihertz(), 99_998);
        let state: &'static mut &'static PitClock = Box::leak(Box::new(&CLOCK));
        irq.register(Irq::TIMER, IrqHandler::from_static(state), Chain::None).unwrap();
        let start = CLOCK.now();
        for _ in 0..25 {
            assert!(irq.raise(Irq::TIMER));
        }
        assert_eq!(CLOCK.now().since(start), 25);
        assert_eq!(CLOCK.millis_since(start), 250);
        assert_eq!(
            irq.eois(),
            [Irq::TIMER; 25],
            "the platform, not the driver, sends the end-of-interrupt"
        );
        assert_eq!(
            irq.register(Irq::TIMER, IrqHandler::from_static(Box::leak(Box::new(&CLOCK))), Chain::None),
            Err(HalError::Busy)
        );
        irq.unregister(Irq::TIMER).unwrap();
        assert!(!irq.raise(Irq::TIMER));
        assert_eq!(CLOCK.now().since(start), 25);
    }
}
