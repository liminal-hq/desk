// The PC speaker: a square wave from PIT channel 2, switched by two bits of port 0x61, written once against the HAL.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Written from the public 8254 and PC conventions, not from any other program's source. Nothing in the repository uses this driver
// yet (roadmap R135); the audio service that will gets its own plan.
//
// Port 0x61 is the keyboard controller's port B: bit 0 gates PIT channel 2, bit 1 connects its output to the speaker, and the other
// bits (the parity and I/O-check enables, the refresh toggle) are not the speaker's. Every write to it here is a read-modify-write
// that changes bits 0 and 1 only.
#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![forbid(unsafe_code)]

use desk_hal::{HalError, PortIo};
use desk_pit::{check_divisor, COMMAND, INPUT_HZ, MAX_DIVISOR};

/// PIT channel 2's counter port.
pub const CHANNEL2: u16 = 0x42;
/// The keyboard controller's port B, which also carries the speaker's gate and data lines.
pub const PORT_B: u16 = 0x61;
/// Bit 0 (the channel 2 gate) and bit 1 (the speaker data line): the only bits of `PORT_B` this driver changes.
pub const SPEAKER_BITS: u8 = 0b11;

/// Channel 2, low byte then high byte, mode 3 (square wave), binary.
const CMD_CH2_LOHI_SQUARE: u8 = 0xB6;

/// The divisor for `hz`, by integer division (440 Hz gives 2,711). `InvalidArgument` for 0 Hz and for anything outside what the
/// 16-bit counter can count in mode 3 under `desk_pit::check_divisor` (2 to 65,536, as the one copy of the rule has it): in whole
/// hertz, 19 Hz to 596,591 Hz. Divisor 1 is not valid in mode 3 on the 8254, so 596,592 Hz and up are refused; 18 Hz would need
/// 66,287.
pub const fn divisor_for(hz: u32) -> Result<u32, HalError> {
    if hz == 0 {
        return Err(HalError::InvalidArgument);
    }
    check_divisor(INPUT_HZ / hz)
}

/// The speaker, reached through `ports`.
#[derive(Clone, Copy, Debug)]
pub struct Speaker<P> {
    ports: P,
}

impl<P: PortIo> Speaker<P> {
    pub const fn new(ports: P) -> Self {
        Speaker { ports }
    }

    /// Sounds a tone of about `hz` and returns the divisor used. Programs channel 2 in mode 3 (command 0xB6 to port 0x43, then the
    /// divisor's low and high bytes to port 0x42; 65,536 would be written as 0, but whole hertz cannot reach it), then sets bits 0
    /// and 1 of port 0x61, keeping the others. `InvalidArgument` outside the range of `divisor_for`, with nothing written.
    pub fn tone(&self, hz: u32) -> Result<u32, HalError> {
        let divisor = divisor_for(hz)?;
        let reload = (divisor % MAX_DIVISOR) as u16;
        self.ports.outb(COMMAND, CMD_CH2_LOHI_SQUARE);
        self.ports.outb(CHANNEL2, reload as u8);
        self.ports.outb(CHANNEL2, (reload >> 8) as u8);
        self.update(SPEAKER_BITS, SPEAKER_BITS);
        Ok(divisor)
    }

    /// Silences the speaker: clears bits 0 and 1 of port 0x61 and keeps the others. Leaves channel 2 as programmed.
    pub fn off(&self) {
        self.update(SPEAKER_BITS, 0);
    }

    /// Whether the gate and the data line are both set, which is when a tone is audible.
    pub fn is_on(&self) -> bool {
        self.ports.inb(PORT_B) & SPEAKER_BITS == SPEAKER_BITS
    }

    /// Port 0x61 with `mask`'s bits replaced by `bits` and every other bit as read.
    fn update(&self, mask: u8, bits: u8) {
        let old = self.ports.inb(PORT_B);
        self.ports.outb(PORT_B, (old & !mask) | (bits & mask));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use desk_hal_mock::devices::Pit8254;
    use desk_hal_mock::{Access, MockPortBus, Port61};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn rig(port61: u8) -> (MockPortBus, Rc<RefCell<Pit8254>>, Rc<RefCell<Port61>>) {
        let bus = MockPortBus::new();
        let pit = bus.attach(0x40, 0x43, Pit8254::default());
        let b = bus.attach(PORT_B, PORT_B, Port61::with_value(port61));
        (bus, pit, b)
    }

    #[test]
    fn the_range_is_the_counters() {
        assert_eq!(divisor_for(440), Ok(2_711));
        assert_eq!(divisor_for(1_000), Ok(1_193));
        assert_eq!(divisor_for(19), Ok(62_799));
        assert_eq!(divisor_for(596_591), Ok(2));
        for hz in [0, 1, 18, 596_592, 1_193_182, u32::MAX] {
            assert_eq!(divisor_for(hz), Err(HalError::InvalidArgument), "{hz}");
        }
    }

    #[test]
    fn tone_programs_channel_2_in_square_wave_mode() {
        for (hz, divisor) in [(440, 2_711u32), (1_000, 1_193), (19, 62_799), (596_591, 2)] {
            let (bus, pit, _) = rig(0);
            assert_eq!(Speaker::new(&bus).tone(hz), Ok(divisor));
            let ch = pit.borrow().channels[2];
            assert_eq!((ch.access, ch.mode, ch.loaded), (3, 3, true), "{hz} Hz");
            assert_eq!(u32::from(ch.reload), divisor, "{hz} Hz");
        }
        let (bus, _, _) = rig(0);
        Speaker::new(&bus).tone(440).unwrap();
        assert_eq!(
            bus.writes(),
            [(0x43, 0xB6), (0x42, 0x97), (0x42, 0x0A), (0x61, 0x03)],
            "2,711 is 0x0A97: command, low, high, then the gate and data bits"
        );
    }

    #[test]
    fn a_refused_tone_writes_nothing() {
        let (bus, _, _) = rig(0xF0);
        let sp = Speaker::new(&bus);
        for hz in [0, 18, 596_592, 1_193_182, 2_000_000] {
            assert_eq!(sp.tone(hz), Err(HalError::InvalidArgument), "{hz}");
        }
        assert!(bus.log().is_empty(), "{:?}", bus.log());
    }

    #[test]
    fn tone_and_off_change_only_bits_0_and_1() {
        for other in [0x00, 0x0C, 0x30, 0xFC, 0xA4] {
            for start in [0x00, 0x01, 0x02, 0x03] {
                let (bus, _, port) = rig(other | start);
                let sp = Speaker::new(&bus);
                sp.tone(440).unwrap();
                assert_eq!(port.borrow().value, other | 0x03, "tone over {:#04x}", other | start);
                assert!(sp.is_on());
                sp.off();
                assert_eq!(port.borrow().value, other, "off over {:#04x}", other | 0x03);
                assert!(!sp.is_on());
                assert!(
                    port.borrow().writes.iter().all(|w| w & 0xFC == other),
                    "every write kept the other bits"
                );
            }
        }
    }

    #[test]
    fn is_on_needs_both_bits() {
        for (value, on) in [
            (0x00, false),
            (0x01, false),
            (0x02, false),
            (0x03, true),
            (0xFF, true),
            (0xFC, false),
        ] {
            let (bus, _, _) = rig(value);
            assert_eq!(Speaker::new(&bus).is_on(), on, "{value:#04x}");
        }
    }

    #[test]
    fn off_reads_before_it_writes() {
        let (bus, _, _) = rig(0x4F);
        Speaker::new(&bus).off();
        assert_eq!(bus.log(), [Access::inb(0x61, 0x4F), Access::outb(0x61, 0x4C)]);
    }
}
