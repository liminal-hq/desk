// The Yamaha OPL2 (YM3812, the AdLib) and OPL3 (YMF262) FM synthesis chips: register writes with the classic settling, detection,
// the OPL3 mode bit, a reset and a minimal tone.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Nothing in the repository uses this driver yet: the audio service that will (the 560 Hz tick that feeds the chip) has its own
// plan. It is written fresh from the chip's public programming conventions, against `PortIo`, so the same code runs under DOS, on
// the native kernel and against the mock's `OplModel`.
//
// The ports, from `base` (0x388 on an AdLib and on a Sound Blaster):
//
// - `base`: write the register index for bank 0; read the status.
// - `base + 1`: write the data for bank 0.
// - `base + 2`: write the register index for bank 1 (OPL3 only).
// - `base + 3`: write the data for bank 1 (OPL3 only).
#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![forbid(unsafe_code)]

use desk_hal::{HalError, PortIo};

/// The usual base port: the AdLib's index and status port (the data port is 0x389; an OPL3's second bank is 0x38A and 0x38B).
pub const BASE: u16 = 0x388;

/// Status reads after writing a register index. The chip needs about 3.3 microseconds before it takes the data. There is no clock
/// in a driver written against `PortIo`, so the wait is a count of bus cycles, as the classic AdLib programming practice does it:
/// each read of the status port is an ISA I/O cycle, which that practice reckons at about 0.66 microseconds, so 6 reads is 3.96
/// microseconds, at least the 3.3 needed. The value read is thrown away.
pub const INDEX_SETTLE_READS: u32 = 6;

/// Status reads after writing a register's data. The chip needs about 23 microseconds before the next index; 35 reads at about
/// 0.66 microseconds is 23.1 microseconds, the classic count.
pub const DATA_SETTLE_READS: u32 = 35;

/// Status reads that `detect` waits for timer 1 to run out. Timer 1 counts in 80 microsecond steps from the value in register 2, so
/// from 0xFF it expires after one step, 80 microseconds. 200 reads at the same reckoning is 132 microseconds, a margin of more than
/// half over the 80 needed, and the settling after the start write (35 reads) counts towards it. The wait is bounded: there is no
/// loop on the status.
pub const TIMER_WAIT_READS: u32 = 200;

/// Register 4: the timer control and the interrupt flag reset (bank 0).
const REG_TIMER_CONTROL: u8 = 0x04;
/// Register 2: timer 1's count (bank 0).
const REG_TIMER1: u8 = 0x02;
/// Register 5 of bank 1: bit 0 is the NEW bit, which turns the OPL3's extra features on (OPL3 only).
const REG_NEW: u8 = 0x05;
/// The frequency's low byte is 0xA0..=0xA8; the key-on, block and high bits are 0xB0..=0xB8.
const REG_FNUM_LOW: u8 = 0xA0;
const REG_KEY_BLOCK: u8 = 0xB0;
/// The first and last registers the chips decode.
const REG_FIRST: u8 = 0x01;
const REG_LAST: u8 = 0xF5;

/// Timer control: mask both timers (bits 6 and 5), the first half of the conventional reset.
const TIMER_MASK_BOTH: u8 = 0x60;
/// Timer control: reset the interrupt flags (bit 7).
const TIMER_RESET_FLAGS: u8 = 0x80;
/// Timer control: start timer 1 (bit 0) with timer 2 masked (bit 5).
const TIMER1_START: u8 = 0x21;

/// Status bits 7 (interrupt request), 6 (timer 1 expired) and 5 (timer 2 expired).
const STATUS_TIMER_BITS: u8 = 0xE0;
/// What the timer bits read after timer 1 alone has expired.
const STATUS_TIMER1_EXPIRED: u8 = 0xC0;
/// Status bits 1 and 2: both clear on an OPL3, set on an OPL2.
const STATUS_CHIP_BITS: u8 = 0x06;

/// The frequency number is 10 bits wide, the block (octave) 3.
pub const MAX_FNUM: u16 = 0x3FF;
pub const MAX_BLOCK: u8 = 7;

/// What `detect` found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chip {
    /// Nothing answered the timer test (a floating bus reads as 0xFF).
    None,
    /// An OPL2: nine channels, one register bank.
    Opl2,
    /// An OPL3: eighteen channels, two register banks.
    Opl3,
}

/// An OPL2 or OPL3 at `base`.
pub struct Opl<P: PortIo> {
    ports: P,
    base: u16,
    /// Whether `enter_opl3` has been called (and not `leave_opl3`): `reset` and the tone helpers then use bank 1 as well.
    opl3: bool,
}

impl<P: PortIo> Opl<P> {
    /// A driver for the chip at `base` (normally `BASE`). Writes nothing.
    pub const fn new(ports: P, base: u16) -> Opl<P> {
        Opl {
            ports,
            base,
            opl3: false,
        }
    }

    /// Writes `value` to register `reg` of bank 0, waiting `INDEX_SETTLE_READS` status reads after the index and
    /// `DATA_SETTLE_READS` after the data.
    pub fn write(&self, reg: u8, value: u8) {
        self.write_bank(0, reg, value);
    }

    /// The same for bank 1, ports `base + 2` and `base + 3`. Only an OPL3 has this bank: on an OPL2 the ports are unconnected (or,
    /// on a dual-OPL2 card, not this chip), so the write is harmless at best and the driver does not check; use `detect` first.
    pub fn write_hi(&self, reg: u8, value: u8) {
        self.write_bank(2, reg, value);
    }

    fn write_bank(&self, port_offset: u16, reg: u8, value: u8) {
        let index = self.base + port_offset;
        self.ports.outb(index, reg);
        self.settle(INDEX_SETTLE_READS);
        self.ports.outb(index + 1, value);
        self.settle(DATA_SETTLE_READS);
    }

    fn settle(&self, reads: u32) {
        for _ in 0..reads {
            let _ = self.ports.inb(self.base);
        }
    }

    /// The status register: bit 7 any timer expired, bit 6 timer 1, bit 5 timer 2.
    pub fn status(&self) -> u8 {
        self.ports.inb(self.base)
    }

    /// Looks for a chip by the standard timer test and tells an OPL2 from an OPL3. The chip is left with its timers reset.
    ///
    /// 1. Mask both timers and reset the interrupt flags (0x60 then 0x80 to register 4); the status must then have bits 7 and 6
    ///    clear (mask 0xE0 equal to 0). A floating bus reads 0xFF and fails here.
    /// 2. Write 0xFF to register 2 and start timer 1 (0x21 to register 4); wait `TIMER_WAIT_READS` status reads.
    /// 3. Read the status: mask 0xE0 must equal 0xC0 (timer 1 expired, timer 2 not).
    /// 4. Reset the timers again.
    ///
    /// The same status read tells the chip: on an OPL3 bits 1 and 2 are clear (mask 0x06 equal to 0), on an OPL2 they read set.
    /// That is the conventional test, not a guarantee: some clones and some Sound Blaster compatibles answer differently (an OPL3
    /// clone with the low bits set reads as an OPL2, which is a safe mistake: the driver then stays in bank 0).
    pub fn detect(&self) -> Chip {
        self.reset_timers();
        if self.status() & STATUS_TIMER_BITS != 0 {
            return Chip::None;
        }
        self.write(REG_TIMER1, 0xFF);
        self.write(REG_TIMER_CONTROL, TIMER1_START);
        self.settle(TIMER_WAIT_READS);
        let status = self.status();
        self.reset_timers();
        if status & STATUS_TIMER_BITS != STATUS_TIMER1_EXPIRED {
            return Chip::None;
        }
        if status & STATUS_CHIP_BITS == 0 {
            Chip::Opl3
        } else {
            Chip::Opl2
        }
    }

    fn reset_timers(&self) {
        self.write(REG_TIMER_CONTROL, TIMER_MASK_BOTH);
        self.write(REG_TIMER_CONTROL, TIMER_RESET_FLAGS);
    }

    /// Sets the NEW bit (bit 0 of register 0x05 of bank 1): the OPL3's second bank and extra channels come alive. Writes the whole
    /// register, 1. Only call it after `detect` returned `Opl3`.
    pub fn enter_opl3(&mut self) {
        self.write_hi(REG_NEW, 1);
        self.opl3 = true;
    }

    /// Writes 0 to register 0x05 of bank 1: the chip is back in OPL2 mode, as an OPL2-only program expects it. A program that
    /// entered OPL3 mode puts this back when it finishes. It does not key off or clear bank 1: call `reset` first, while still in OPL3
    /// mode, or the bank 1 channels and the 4-operator bits stay as they were.
    pub fn leave_opl3(&mut self) {
        self.write_hi(REG_NEW, 0);
        self.opl3 = false;
    }

    /// Silences the chip and clears it: key-off for every channel (registers 0xB0..=0xB8, and the same in bank 1 after
    /// `enter_opl3`), then 0 to every register from 0x01 to 0xF5 of bank 0 (and of bank 1, except the NEW bit at 0x05, which
    /// stays as `enter_opl3` and `leave_opl3` left it). That is 9 (OPL2) or 18 (OPL3) key-offs and then 245 or 489 more writes,
    /// each about 41 status reads, so a reset takes about 7 ms on an OPL2 and about 20 ms on an OPL3 (at the classic figure of 0.66 µs a read, not measured). It also
    /// zeroes registers 0xC0..=0xC8, which clears the OPL3's left and right output enables: in OPL3 mode a channel is silent until
    /// it is set up again.
    pub fn reset(&self) {
        for ch in 0..9 {
            self.write(REG_KEY_BLOCK + ch, 0);
        }
        if self.opl3 {
            for ch in 0..9 {
                self.write_hi(REG_KEY_BLOCK + ch, 0);
            }
        }
        for reg in REG_FIRST..=REG_LAST {
            self.write(reg, 0);
        }
        if self.opl3 {
            for reg in REG_FIRST..=REG_LAST {
                if reg != REG_NEW {
                    self.write_hi(reg, 0);
                }
            }
        }
    }

    /// Sets channel `channel`'s frequency number (10 bits) and block (octave, 0 to 7) and keys it on: register 0xA0 + n gets the
    /// low 8 bits of the number, then 0xB0 + n gets key-on (bit 5), the block (bits 4 to 2) and the number's top 2 bits. Channels
    /// 0 to 8 are in bank 0, 9 to 17 in bank 1 (OPL3 mode only). The tone is audible only if the operators of the channel are set
    /// up (volume, attack, waveform), which this does not do. `InvalidArgument` for a channel, number or block out of range, and
    /// for a bank 1 channel before `enter_opl3`; nothing is written then.
    pub fn tone(&self, channel: u8, fnum: u16, block: u8) -> Result<(), HalError> {
        if fnum > MAX_FNUM || block > MAX_BLOCK {
            return Err(HalError::InvalidArgument);
        }
        let (hi, n) = self.channel(channel)?;
        let high = 0x20 | (block << 2) | (fnum >> 8) as u8;
        self.write_either(hi, REG_FNUM_LOW + n, fnum as u8);
        self.write_either(hi, REG_KEY_BLOCK + n, high);
        Ok(())
    }

    /// Keys channel `channel` off (writes 0 to its 0xB0 register, so the block and number are cleared too).
    pub fn silence(&self, channel: u8) -> Result<(), HalError> {
        let (hi, n) = self.channel(channel)?;
        self.write_either(hi, REG_KEY_BLOCK + n, 0);
        Ok(())
    }

    fn channel(&self, channel: u8) -> Result<(bool, u8), HalError> {
        match channel {
            0..=8 => Ok((false, channel)),
            9..=17 if self.opl3 => Ok((true, channel - 9)),
            _ => Err(HalError::InvalidArgument),
        }
    }

    fn write_either(&self, hi: bool, reg: u8, value: u8) {
        if hi {
            self.write_hi(reg, value);
        } else {
            self.write(reg, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use desk_hal_mock::{MockPortBus, OplKind, OplModel};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn rig(kind: OplKind) -> (MockPortBus, Rc<RefCell<OplModel>>) {
        let bus = MockPortBus::new();
        let model = bus.attach(BASE, BASE + 3, OplModel::new(BASE, kind));
        (bus, model)
    }

    #[test]
    fn a_bank_0_write_waits_six_reads_after_the_index_and_thirty_five_after_the_data() {
        let (bus, model) = rig(OplKind::Opl3);
        Opl::new(&bus, BASE).write(0xB0, 0x2A);
        let m = model.borrow();
        assert_eq!(m.writes.len(), 1);
        let w = m.writes[0];
        assert_eq!((w.bank, w.reg, w.value), (0, 0xB0, 0x2A));
        assert_eq!(w.reads_after_index, 6);
        assert_eq!(w.reads_after_data, 35);
        assert_eq!((INDEX_SETTLE_READS, DATA_SETTLE_READS), (6, 35));
    }

    #[test]
    fn a_bank_1_write_settles_the_same_and_uses_the_second_pair_of_ports() {
        let (bus, model) = rig(OplKind::Opl3);
        Opl::new(&bus, BASE).write_hi(0x40, 0x3F);
        let w = model.borrow().writes[0];
        assert_eq!((w.bank, w.reg, w.value), (1, 0x40, 0x3F));
        assert_eq!((w.reads_after_index, w.reads_after_data), (6, 35));
        assert_eq!(bus.writes(), vec![(0x38A, 0x40), (0x38B, 0x3F)]);
    }

    #[test]
    fn the_settling_holds_across_a_run_of_writes() {
        let (bus, model) = rig(OplKind::Opl2);
        let opl = Opl::new(&bus, BASE);
        for i in 0..5 {
            opl.write(0x20 + i, i);
        }
        let m = model.borrow();
        assert_eq!(m.writes.len(), 5);
        assert!(m.writes.iter().all(|w| w.reads_after_index == 6 && w.reads_after_data == 35));
    }

    #[test]
    fn detect_tells_an_opl2_from_an_opl3_from_nothing() {
        assert_eq!(Opl::new(&rig(OplKind::Opl2).0, BASE).detect(), Chip::Opl2);
        assert_eq!(Opl::new(&rig(OplKind::Opl3).0, BASE).detect(), Chip::Opl3);
        assert_eq!(Opl::new(&rig(OplKind::Absent).0, BASE).detect(), Chip::None);
        // No device attached at all: the bus floats at 0xFF.
        assert_eq!(Opl::new(&MockPortBus::new(), BASE).detect(), Chip::None);
    }

    #[test]
    fn detect_runs_the_timer_test_in_order_and_leaves_the_timers_reset() {
        let (bus, model) = rig(OplKind::Opl2);
        assert_eq!(Opl::new(&bus, BASE).detect(), Chip::Opl2);
        let m = model.borrow();
        let seq: Vec<_> = m.writes.iter().map(|w| (w.bank, w.reg, w.value)).collect();
        assert_eq!(
            seq,
            vec![
                (0, 4, 0x60),
                (0, 4, 0x80),
                (0, 2, 0xFF),
                (0, 4, 0x21),
                (0, 4, 0x60),
                (0, 4, 0x80)
            ]
        );
        assert_eq!(m.status_flags(), 0, "the flags are cleared again");
        assert!(!m.timer1_running());
    }

    #[test]
    fn detect_needs_the_timer_to_have_run_out() {
        let (bus, model) = rig(OplKind::Opl2);
        model.borrow_mut().timer_delay = 10_000;
        assert_eq!(Opl::new(&bus, BASE).detect(), Chip::None);
        // A timer that runs out within the wait is found.
        let (bus, model) = rig(OplKind::Opl3);
        model.borrow_mut().timer_delay = TIMER_WAIT_READS;
        assert_eq!(Opl::new(&bus, BASE).detect(), Chip::Opl3);
    }

    #[test]
    fn detect_refuses_a_chip_whose_flags_will_not_clear() {
        let (bus, model) = rig(OplKind::Opl2);
        model.borrow_mut().stuck_flags = 0xC0;
        assert_eq!(Opl::new(&bus, BASE).detect(), Chip::None);
    }

    #[test]
    fn entering_and_leaving_opl3_write_only_register_5_of_bank_1() {
        let (bus, model) = rig(OplKind::Opl3);
        let mut opl = Opl::new(&bus, BASE);
        opl.enter_opl3();
        assert!(model.borrow().new_mode());
        opl.leave_opl3();
        assert!(!model.borrow().new_mode());
        let seq: Vec<_> = model.borrow().writes.iter().map(|w| (w.bank, w.reg, w.value)).collect();
        assert_eq!(seq, vec![(1, 5, 1), (1, 5, 0)]);
    }

    #[test]
    fn reset_keys_off_every_channel_and_zeroes_the_registers() {
        let (bus, model) = rig(OplKind::Opl2);
        let opl = Opl::new(&bus, BASE);
        for reg in 0x01..=0xF5u8 {
            opl.write(reg, 0xA5);
        }
        model.borrow_mut().writes.clear();
        opl.reset();
        let m = model.borrow();
        let first: Vec<_> = m.writes.iter().take(9).map(|w| (w.bank, w.reg, w.value)).collect();
        assert_eq!(first, (0..9).map(|c| (0, 0xB0 + c, 0)).collect::<Vec<_>>());
        assert!(
            m.writes.iter().all(|w| w.bank == 0 && w.value == 0),
            "an OPL2 reset never touches bank 1"
        );
        assert!(m.regs[0].iter().all(|&r| r == 0));
    }

    #[test]
    fn reset_in_opl3_mode_clears_both_banks_and_keeps_the_new_bit() {
        let (bus, model) = rig(OplKind::Opl3);
        let mut opl = Opl::new(&bus, BASE);
        opl.enter_opl3();
        for reg in 0x06..=0xF5u8 {
            opl.write_hi(reg, 0x5A);
            opl.write(reg, 0xA5);
        }
        model.borrow_mut().writes.clear();
        opl.reset();
        let m = model.borrow();
        let keyoffs: Vec<_> = m.writes.iter().take(18).map(|w| (w.bank, w.reg)).collect();
        let expect: Vec<_> = (0..9).map(|c| (0, 0xB0 + c)).chain((0..9).map(|c| (1, 0xB0 + c))).collect();
        assert_eq!(keyoffs, expect);
        assert!(m.regs[0].iter().all(|&r| r == 0));
        for (reg, &v) in m.regs[1].iter().enumerate() {
            assert_eq!(v, if reg == 5 { 1 } else { 0 }, "bank 1 register {reg:#04x}");
        }
        assert!(m.new_mode());
    }

    #[test]
    fn a_bank_1_write_to_an_opl2_is_recorded_but_changes_nothing() {
        let (bus, model) = rig(OplKind::Opl2);
        Opl::new(&bus, BASE).write_hi(0x40, 0x3F);
        let m = model.borrow();
        assert_eq!(m.writes.len(), 1);
        assert!(m.regs[1].iter().all(|&r| r == 0));
        assert!(!m.new_mode());
    }

    #[test]
    fn a_tone_sets_the_frequency_block_and_key_and_silence_clears_it() {
        let (bus, model) = rig(OplKind::Opl3);
        let mut opl = Opl::new(&bus, BASE);
        opl.tone(2, 0x241, 4).unwrap();
        assert_eq!(model.borrow().regs[0][0xA2], 0x41);
        assert_eq!(model.borrow().regs[0][0xB2], 0x20 | (4 << 2) | 0x02);
        opl.silence(2).unwrap();
        assert_eq!(model.borrow().regs[0][0xB2], 0);
        // Bank 1 channels need OPL3 mode.
        assert_eq!(opl.tone(9, 0x100, 0), Err(HalError::InvalidArgument));
        opl.enter_opl3();
        opl.tone(10, 0x100, 1).unwrap();
        assert_eq!(model.borrow().regs[1][0xA1], 0x00);
        assert_eq!(model.borrow().regs[1][0xB1], 0x20 | (1 << 2) | 0x01);
    }

    #[test]
    fn a_tone_out_of_range_writes_nothing() {
        let (bus, model) = rig(OplKind::Opl2);
        let opl = Opl::new(&bus, BASE);
        assert_eq!(opl.tone(0, 0x400, 0), Err(HalError::InvalidArgument));
        assert_eq!(opl.tone(0, 0x100, 8), Err(HalError::InvalidArgument));
        assert_eq!(opl.tone(18, 0x100, 0), Err(HalError::InvalidArgument));
        assert_eq!(opl.silence(9), Err(HalError::InvalidArgument));
        assert!(model.borrow().writes.is_empty());
    }
}
