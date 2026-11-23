// The Sound Blaster's DSP: reset, version and the speaker switch, and the `BLASTER` environment value, written once against the HAL.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Written from the public Sound Blaster programming conventions (the DSP's ports at the base address plus 6, 0xA, 0xC and 0xE),
// not from any other program's source. Every wait is a bounded count of status reads, so an absent card costs a few milliseconds
// and never hangs the machine. An absent card's ports float to 0xFF: every check below treats that as "no card", never as success.
//
// No DMA playback here: it needs a buffer below 16 MiB that does not cross a 64 KiB page, the DMA controller and the card's IRQ,
// none of which is needed yet. Nothing in the repository uses this driver yet (roadmap R135); the audio service that will gets its
// own plan.
#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![forbid(unsafe_code)]

use desk_hal::{HalError, PortIo};

/// Offsets from the base address.
const RESET: u16 = 0x6;
const READ_DATA: u16 = 0xA;
const WRITE: u16 = 0xC;
const READ_STATUS: u16 = 0xE;

/// What a reset leaves in the read buffer when a DSP is there.
pub const RESET_ACK: u8 = 0xAA;
const CMD_VERSION: u8 = 0xE1;
const CMD_SPEAKER_ON: u8 = 0xD1;
const CMD_SPEAKER_OFF: u8 = 0xD3;

/// The most status reads any wait makes before it gives up. Each read of an ISA port takes about a microsecond, so this is about
/// 65 ms on a 486, far beyond the DSP's reset time (about 100 µs) and a command's turnaround.
pub const POLLS: u32 = 65_536;

/// Reads of the reset port (write-only, so it floats) between raising and lowering it. The DSP wants the line high for at least
/// 3 µs; each ISA read takes about 1 µs, so eight is more than twice that. The reads have no effect on the card.
pub const RESET_HOLD_READS: u32 = 8;

/// The base addresses a card can be jumpered to: 0x210 to 0x280 in steps of 0x10.
pub const fn valid_base(base: u16) -> bool {
    base >= 0x210 && base <= 0x280 && base & 0xF == 0
}

/// The settings in a `BLASTER` environment value, such as `A220 I5 D1 H5 T3`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Blaster {
    /// The base port (`A`), always valid by `valid_base`.
    pub base: u16,
    /// The IRQ (`I`), if given.
    pub irq: Option<u8>,
    /// The 8-bit DMA channel (`D`), if given.
    pub dma: Option<u8>,
    /// The 16-bit DMA channel (`H`), if given.
    pub high_dma: Option<u8>,
    /// The MIDI port (`P`), in hexadecimal, if given.
    pub midi: Option<u16>,
}

/// Parses a `BLASTER` value. The rules:
///
/// - The value is a list of fields separated by any amount of white space (or none: `A220I5` is fine). Each field is one letter,
///   in either case, followed by a number.
/// - `A` (the base port) and `P` (the MIDI port) are in hexadecimal; the others are in decimal. The numbers are digits only: a
///   sign, a prefix or a stray character makes the value malformed. Because a hexadecimal number swallows the hex letters, write
///   a space between `A220` or `P330` and a following field that starts with A to F (`A220 D1`; `A220D1` reads as base 0x220D1).
/// - `A` is required, and its value must satisfy `valid_base` (0x210 to 0x280 in steps of 0x10); otherwise `None`.
/// - `I`, `D`, `H`, `M` and `T` must fit a byte, and `A` and `P` a port; a number that does not fit, a letter with no number and a
///   number with no letter are malformed, and the whole value is refused with `None`.
/// - Letters other than A, I, D, H, M, P and T are skipped along with their number (a real value also carries `E`, for the
///   EMU8000 port, and others). `M` and `T` (the mixer and the card type) are checked and not kept.
/// - A repeated letter takes its last value.
pub fn parse_blaster(value: &str) -> Option<Blaster> {
    let bytes = value.as_bytes();
    let mut i = 0;
    let mut base = None;
    let mut out = Blaster {
        base: 0,
        irq: None,
        dma: None,
        high_dma: None,
        midi: None,
    };
    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let letter = bytes[i].to_ascii_uppercase();
        if !letter.is_ascii_uppercase() {
            return None;
        }
        i += 1;
        let radix: u32 = if letter == b'A' || letter == b'P' { 16 } else { 10 };
        let start = i;
        let mut n: u32 = 0;
        while let Some(d) = bytes.get(i).and_then(|&b| (b as char).to_digit(radix)) {
            n = n * radix + d;
            if n > 0xFFFF {
                return None;
            }
            i += 1;
        }
        if i == start {
            return None;
        }
        let small = u8::try_from(n).ok();
        match letter {
            b'A' => base = Some(n as u16),
            b'P' => out.midi = Some(n as u16),
            b'I' => out.irq = Some(small?),
            b'D' => out.dma = Some(small?),
            b'H' => out.high_dma = Some(small?),
            b'T' | b'M' => {
                small?;
            }
            _ => {}
        }
    }
    let base = base?;
    if !valid_base(base) {
        return None;
    }
    out.base = base;
    Some(out)
}

/// The DSP at `base`, reached through `ports`.
#[derive(Clone, Copy, Debug)]
pub struct SoundBlaster<P> {
    ports: P,
    base: u16,
}

impl<P: PortIo> SoundBlaster<P> {
    pub const fn new(ports: P, base: u16) -> Self {
        SoundBlaster { ports, base }
    }

    pub const fn base(&self) -> u16 {
        self.base
    }

    /// Resets the DSP and checks that it answers 0xAA. False if it never does, which is what an empty slot gives: its ports read
    /// 0xFF, and 0xFF is not 0xAA. The sequence: write 1 to base+6, hold it for `RESET_HOLD_READS` reads of that port (at least
    /// 3 µs), write 0, then read base+0xE up to `POLLS` times until bit 7 (a byte is ready) and read base+0xA.
    pub fn reset(&self) -> bool {
        self.ports.outb(self.base + RESET, 1);
        for _ in 0..RESET_HOLD_READS {
            let _ = self.ports.inb(self.base + RESET);
        }
        self.ports.outb(self.base + RESET, 0);
        self.wait_read_ready().is_ok() && self.ports.inb(self.base + READ_DATA) == RESET_ACK
    }

    /// The DSP's version, major then minor (4.5, say), or `None` if it does not answer: the write buffer never frees (a floating
    /// bus reads 0xFF, bit 7 set) or the two bytes never come. Writes 0xE1 only once the write buffer is free.
    pub fn version(&self) -> Option<(u8, u8)> {
        self.write_command(CMD_VERSION).ok()?;
        self.wait_read_ready().ok()?;
        let major = self.ports.inb(self.base + READ_DATA);
        self.wait_read_ready().ok()?;
        let minor = self.ports.inb(self.base + READ_DATA);
        Some((major, minor))
    }

    /// Connects (0xD1) or disconnects (0xD3) the card's speaker output. `Timeout` if the write buffer never frees, in which case
    /// nothing was written.
    pub fn speaker(&self, on: bool) -> Result<(), HalError> {
        self.write_command(if on { CMD_SPEAKER_ON } else { CMD_SPEAKER_OFF })
    }

    /// Waits for base+0xC bit 7 to clear (the DSP will take a byte), then writes `value` there.
    fn write_command(&self, value: u8) -> Result<(), HalError> {
        for _ in 0..POLLS {
            if self.ports.inb(self.base + WRITE) & 0x80 == 0 {
                self.ports.outb(self.base + WRITE, value);
                return Ok(());
            }
        }
        Err(HalError::Timeout)
    }

    /// Waits for base+0xE bit 7 (a byte is waiting at base+0xA).
    fn wait_read_ready(&self) -> Result<(), HalError> {
        for _ in 0..POLLS {
            if self.ports.inb(self.base + READ_STATUS) & 0x80 != 0 {
                return Ok(());
            }
        }
        Err(HalError::Timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use desk_hal_mock::{Access, MockPortBus, SbModel};
    use std::cell::RefCell;
    use std::rc::Rc;

    const BASE: u16 = 0x220;

    fn card(model: SbModel) -> (MockPortBus, Rc<RefCell<SbModel>>) {
        let bus = MockPortBus::new();
        let dev = bus.attach(BASE, BASE + 0xF, model);
        (bus, dev)
    }

    fn blaster(base: u16, irq: Option<u8>, dma: Option<u8>, high_dma: Option<u8>, midi: Option<u16>) -> Blaster {
        Blaster {
            base,
            irq,
            dma,
            high_dma,
            midi,
        }
    }

    #[test]
    fn parses_the_usual_values() {
        assert_eq!(
            parse_blaster("A220 I5 D1 T3"),
            Some(blaster(0x220, Some(5), Some(1), None, None))
        );
        assert_eq!(
            parse_blaster("a220 i7 d3 h5 t6"),
            Some(blaster(0x220, Some(7), Some(3), Some(5), None))
        );
        assert_eq!(
            parse_blaster("A240 I10 D1 H5 P330 T6"),
            Some(blaster(0x240, Some(10), Some(1), Some(5), Some(0x330)))
        );
        assert_eq!(
            parse_blaster("  A220   I5\tD1  "),
            Some(blaster(0x220, Some(5), Some(1), None, None))
        );
        assert_eq!(parse_blaster("A220I5"), Some(blaster(0x220, Some(5), None, None, None)));
        assert_eq!(parse_blaster("A210"), Some(blaster(0x210, None, None, None, None)));
        assert_eq!(parse_blaster("A280"), Some(blaster(0x280, None, None, None, None)));
        assert_eq!(parse_blaster("A220 I5 I7").unwrap().irq, Some(7), "the last repeat wins");
    }

    #[test]
    fn skips_unknown_letters() {
        let got = parse_blaster("A220 I5 D1 E620 Z9 T4");
        assert_eq!(got, Some(blaster(0x220, Some(5), Some(1), None, None)));
    }

    #[test]
    fn refuses_what_it_cannot_trust() {
        assert_eq!(parse_blaster(""), None);
        assert_eq!(parse_blaster("   "), None);
        assert_eq!(parse_blaster("I5 D1 T3"), None, "no A");
        for bad in ["A200", "A290", "A225", "A300", "A0", "A21", "A2210"] {
            assert_eq!(parse_blaster(bad), None, "{bad}");
        }
        for bad in [
            "A220 I",
            "A220 I5x",
            "A220 I-5",
            "A220 I+5",
            "A220 I0x",
            "A220 I256",
            "A220 D99999",
            "220 I5",
            "A220 5",
            "A220 ;",
        ] {
            assert_eq!(parse_blaster(bad), None, "{bad}");
        }
        assert_eq!(parse_blaster("A220 T300"), None, "T does not fit a byte");
        assert_eq!(parse_blaster("A220 P12345"), None, "P does not fit a port");
    }

    #[test]
    fn reset_works_on_a_present_card() {
        let (bus, dev) = card(SbModel::new(BASE));
        assert!(SoundBlaster::new(&bus, BASE).reset());
        assert_eq!(bus.writes(), [(BASE + 6, 1), (BASE + 6, 0)], "a 1 then a 0 on the reset port");
        assert!(dev.borrow().commands.is_empty());
    }

    #[test]
    fn reset_waits_for_a_slow_card() {
        let (bus, _) = card(SbModel {
            ready_delay: 300,
            ..SbModel::new(BASE)
        });
        assert!(SoundBlaster::new(&bus, BASE).reset());
    }

    #[test]
    fn reset_holds_the_line_before_lowering_it() {
        let (bus, _) = card(SbModel::new(BASE));
        SoundBlaster::new(&bus, BASE).reset();
        let log = bus.log();
        let high = log.iter().position(|a| *a == Access::outb(BASE + 6, 1)).unwrap();
        let low = log.iter().position(|a| *a == Access::outb(BASE + 6, 0)).unwrap();
        assert!(
            low - high > RESET_HOLD_READS as usize,
            "at least {RESET_HOLD_READS} reads between"
        );
    }

    #[test]
    fn an_empty_slot_is_not_a_card() {
        // A floating bus reads 0xFF everywhere: bit 7 is set (a byte "ready") and the byte is 0xFF, not 0xAA.
        let bus = MockPortBus::new();
        let sb = SoundBlaster::new(&bus, BASE);
        assert!(!sb.reset());
        assert_eq!(sb.version(), None);
        assert_eq!(sb.speaker(true), Err(HalError::Timeout));
        assert!(
            bus.writes().iter().all(|&(p, _)| p == BASE + 6),
            "only the reset port is written: {:?}",
            bus.writes()
        );
        // The model's own "absent" mode behaves the same.
        let (bus, _) = card(SbModel {
            absent: true,
            ..SbModel::new(BASE)
        });
        let sb = SoundBlaster::new(&bus, BASE);
        assert!(!sb.reset());
        assert_eq!(sb.version(), None);
    }

    #[test]
    fn a_card_that_never_answers_ends_the_wait() {
        let (bus, dev) = card(SbModel {
            never_answers: true,
            ..SbModel::new(BASE)
        });
        assert!(!SoundBlaster::new(&bus, BASE).reset());
        assert_eq!(dev.borrow().read_status_reads, POLLS, "bounded, and no more");
    }

    #[test]
    fn version_reads_major_then_minor() {
        let (bus, dev) = card(SbModel::new(BASE));
        assert_eq!(SoundBlaster::new(&bus, BASE).version(), Some((4, 5)));
        assert_eq!(dev.borrow().commands, [0xE1]);
        let (bus, _) = card(SbModel {
            major: 3,
            minor: 1,
            ..SbModel::new(BASE)
        });
        assert_eq!(SoundBlaster::new(&bus, BASE).version(), Some((3, 1)));
    }

    #[test]
    fn version_waits_for_a_busy_write_buffer_and_a_slow_answer() {
        let (bus, dev) = card(SbModel {
            write_busy: 50,
            ready_delay: 40,
            ..SbModel::new(BASE)
        });
        let sb = SoundBlaster::new(&bus, BASE);
        // The previous command leaves the buffer busy for 50 reads.
        sb.speaker(true).unwrap();
        assert_eq!(sb.version(), Some((4, 5)));
        let dev = dev.borrow();
        assert_eq!(dev.commands, [0xD1, 0xE1]);
        assert_eq!(dev.dropped_writes, 0, "nothing was written while the buffer was busy");
        assert!(dev.write_status_reads > 50);
    }

    #[test]
    fn a_stuck_write_buffer_times_out_without_writing() {
        let (bus, dev) = card(SbModel {
            write_busy: u32::MAX,
            ..SbModel::new(BASE)
        });
        let sb = SoundBlaster::new(&bus, BASE);
        sb.speaker(true).unwrap();
        assert_eq!(sb.version(), None);
        assert_eq!(sb.speaker(false), Err(HalError::Timeout));
        let dev = dev.borrow();
        assert_eq!(dev.commands, [0xD1]);
        assert_eq!(dev.dropped_writes, 0);
    }

    #[test]
    fn speaker_commands_are_recorded() {
        let (bus, dev) = card(SbModel::new(BASE));
        let sb = SoundBlaster::new(&bus, BASE);
        sb.speaker(true).unwrap();
        assert!(dev.borrow().speaker_on);
        sb.speaker(false).unwrap();
        assert!(!dev.borrow().speaker_on);
        assert_eq!(dev.borrow().commands, [0xD1, 0xD3]);
    }
}
