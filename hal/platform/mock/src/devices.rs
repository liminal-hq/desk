// Small models of real chips, enough to check what a driver programs: the 16550 UART and the 8254 PIT.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::bus::{Device, Width};

/// A 16550A at `base..=base + 7`: the divisor latch, line and modem control, the FIFO control, the scratch register, loopback, and
/// a transmitter that can be held busy. Bytes written to the transmitter are kept in `sent`; bytes in `incoming` are what the
/// receiver has. Interrupts and FIFO depth are not modelled.
#[derive(Debug, Default)]
pub struct Uart16550 {
    pub base: u16,
    pub ier: u8,
    pub lcr: u8,
    pub mcr: u8,
    pub fcr: u8,
    pub scratch: u8,
    pub divisor: u16,
    /// What reached the transmitter, in order (in loopback mode it goes to `incoming` instead).
    pub sent: Vec<u8>,
    /// What the receiver holds.
    pub incoming: VecDeque<u8>,
    /// While set, the line status reports the transmitter busy, as a stuck or missing UART would.
    pub busy: bool,
}

const DLAB: u8 = 0x80;
const LOOPBACK: u8 = 0x10;

impl Uart16550 {
    pub fn new(base: u16) -> Uart16550 {
        Uart16550 {
            base,
            ..Uart16550::default()
        }
    }

    fn dlab(&self) -> bool {
        self.lcr & DLAB != 0
    }
}

impl Device for Uart16550 {
    fn read(&mut self, port: u16, _: Width) -> u32 {
        let v = match port - self.base {
            0 if self.dlab() => self.divisor as u8,
            0 => self.incoming.pop_front().unwrap_or(0),
            1 if self.dlab() => (self.divisor >> 8) as u8,
            1 => self.ier,
            2 => 0xC1, // interrupt identification: FIFOs enabled, no interrupt pending
            3 => self.lcr,
            4 => self.mcr,
            5 => {
                let data_ready = u8::from(!self.incoming.is_empty());
                let empty = if self.busy { 0 } else { 0x60 };
                data_ready | empty
            }
            6 => 0,
            _ => self.scratch,
        };
        v.into()
    }

    fn write(&mut self, port: u16, _: Width, value: u32) {
        let v = value as u8;
        match port - self.base {
            0 if self.dlab() => self.divisor = (self.divisor & 0xFF00) | u16::from(v),
            0 if self.mcr & LOOPBACK != 0 => self.incoming.push_back(v),
            0 => self.sent.push(v),
            1 if self.dlab() => self.divisor = (self.divisor & 0x00FF) | (u16::from(v) << 8),
            1 => self.ier = v,
            2 => self.fcr = v,
            3 => self.lcr = v,
            4 => self.mcr = v,
            5 | 6 => {}
            _ => self.scratch = v,
        }
    }
}

/// One counter of the 8254: its mode and the reload value written to it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PitChannel {
    /// The access mode from the command (1 low byte, 2 high byte, 3 low then high).
    pub access: u8,
    /// The counting mode from the command (0 to 5).
    pub mode: u8,
    pub bcd: bool,
    /// The reload value as written (0 means 65,536).
    pub reload: u16,
    /// Whether a complete reload value has been written since the last command.
    pub loaded: bool,
    /// Bytes written so far towards the reload value, for the low-then-high access mode.
    pending_low: Option<u8>,
    latched: Option<u16>,
    read_high_next: bool,
}

/// An 8254 at 0x40..=0x43. Counters do not run: a latch command (or a read) returns `count`, which a test sets.
#[derive(Debug, Default)]
pub struct Pit8254 {
    pub channels: [PitChannel; 3],
    /// The value a counter-latch command captures.
    pub count: u16,
}

impl Device for Pit8254 {
    fn read(&mut self, port: u16, _: Width) -> u32 {
        let Some(ch) = self.channels.get_mut(usize::from(port - 0x40)) else {
            return 0xFF; // the command port is write-only
        };
        let value = ch.latched.unwrap_or(self.count);
        let byte = match ch.access {
            1 => value as u8,
            2 => (value >> 8) as u8,
            _ => {
                let b = if ch.read_high_next { (value >> 8) as u8 } else { value as u8 };
                ch.read_high_next = !ch.read_high_next;
                b
            }
        };
        if ch.access != 3 || !ch.read_high_next {
            ch.latched = None;
        }
        byte.into()
    }

    fn write(&mut self, port: u16, _: Width, value: u32) {
        let v = value as u8;
        if port == 0x43 {
            let sel = usize::from(v >> 6);
            let access = (v >> 4) & 3;
            if sel == 3 {
                return; // read-back command, not modelled
            }
            let ch = &mut self.channels[sel];
            if access == 0 {
                ch.latched = Some(self.count);
                ch.read_high_next = false;
            } else {
                *ch = PitChannel {
                    access,
                    mode: (v >> 1) & 7,
                    bcd: v & 1 != 0,
                    ..PitChannel::default()
                };
            }
            return;
        }
        let ch = &mut self.channels[usize::from(port - 0x40)];
        match ch.access {
            1 => (ch.reload, ch.loaded) = ((ch.reload & 0xFF00) | u16::from(v), true),
            2 => (ch.reload, ch.loaded) = ((ch.reload & 0x00FF) | (u16::from(v) << 8), true),
            _ => match ch.pending_low.take() {
                None => ch.pending_low = Some(v),
                Some(lo) => (ch.reload, ch.loaded) = (u16::from(lo) | (u16::from(v) << 8), true),
            },
        }
    }
}
