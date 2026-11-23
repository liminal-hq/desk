// A model of the Yamaha OPL2 and OPL3: the register file, the settling reads and timer 1.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Modelled: the register file of both banks (256 bytes each), every write in order with the number of status reads that followed
// its index and its data (so a test can assert the driver's settling), timer 1 (started by bit 0 of register 4, it sets status
// bits 7 and 6 after `timer_delay` status reads, unless masked; bit 7 of register 4 clears the flags), the OPL3's NEW bit, and the
// status bits 1 and 2 that tell the chips apart. Not modelled: timer 2, any sound, the registers' effects (apart from register 4
// and the NEW bit), real elapsed time (a read counts as one unit, whatever it would take on a bus).

use alloc::vec::Vec;

use crate::bus::{Device, Width};

/// Which chip, or none, the model stands in for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OplKind {
    /// Reads return 0xFF and writes are ignored (and not recorded), as an empty port does.
    Absent,
    /// An OPL2: status bits 1 and 2 read set; the bank 1 ports are not connected (writes are recorded but change nothing).
    Opl2,
    /// An OPL3: status bits 1 and 2 read clear; both banks.
    Opl3,
}

/// One register write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OplWrite {
    /// 0 for the first bank (ports `base`, `base + 1`), 1 for the second (`base + 2`, `base + 3`).
    pub bank: u8,
    pub reg: u8,
    pub value: u8,
    /// Status reads between the index write and this data write.
    pub reads_after_index: u32,
    /// Status reads between this data write and the next index write (or the end of the run so far).
    pub reads_after_data: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Nothing written yet, or a data write is the last access: status reads count towards that write's `reads_after_data`.
    AfterData,
    /// An index was written: status reads count towards `reads_after_index`.
    AfterIndex { bank: u8, reg: u8, reads: u32 },
}

pub struct OplModel {
    pub base: u16,
    pub kind: OplKind,
    /// Every register write, in order (it can be cleared by the test).
    pub writes: Vec<OplWrite>,
    /// The register file, by bank. Register 4 of bank 0 holds the last value written, as a plain latch.
    pub regs: [[u8; 256]; 2],
    /// Status reads after timer 1 starts before it expires.
    pub timer_delay: u32,
    /// Status bits forced on whatever the chip does (a chip whose flags will not clear).
    pub stuck_flags: u8,
    /// Every status read so far.
    pub status_reads: u32,
    phase: Phase,
    flags: u8,
    timer1: Option<u32>,
    timer1_masked: bool,
}

impl OplModel {
    /// A model at `base..=base + 3`. The timer expires after 100 status reads (more than the driver's data settling, less than its
    /// timer wait).
    pub fn new(base: u16, kind: OplKind) -> OplModel {
        OplModel {
            base,
            kind,
            writes: Vec::new(),
            regs: [[0; 256]; 2],
            timer_delay: 100,
            stuck_flags: 0,
            status_reads: 0,
            phase: Phase::AfterData,
            flags: 0,
            timer1: None,
            timer1_masked: false,
        }
    }

    /// The timer bits of the status (bits 7 and 6) as the chip holds them.
    pub fn status_flags(&self) -> u8 {
        self.flags
    }

    pub fn timer1_running(&self) -> bool {
        self.timer1.is_some()
    }

    /// The NEW bit (bit 0 of register 5 of bank 1): always off on an OPL2.
    pub fn new_mode(&self) -> bool {
        self.regs[1][5] & 1 != 0
    }

    fn status(&mut self) -> u8 {
        self.status_reads += 1;
        match self.phase {
            Phase::AfterData => {
                if let Some(last) = self.writes.last_mut() {
                    last.reads_after_data += 1;
                }
            }
            Phase::AfterIndex { bank, reg, reads } => {
                self.phase = Phase::AfterIndex {
                    bank,
                    reg,
                    reads: reads + 1,
                };
            }
        }
        if let Some(n) = self.timer1 {
            let n = n + 1;
            self.timer1 = Some(n);
            if n >= self.timer_delay && !self.timer1_masked {
                self.flags |= 0xC0;
            }
        }
        let chip = if self.kind == OplKind::Opl2 { 0x06 } else { 0 };
        self.flags | self.stuck_flags | chip
    }

    fn write_data(&mut self, bank: u8, reg: u8, value: u8, reads_after_index: u32) {
        self.writes.push(OplWrite {
            bank,
            reg,
            value,
            reads_after_index,
            reads_after_data: 0,
        });
        if self.kind == OplKind::Opl2 && bank == 1 {
            return;
        }
        self.regs[bank as usize][reg as usize] = value;
        if bank == 0 && reg == 4 {
            if value & 0x80 != 0 {
                self.flags = 0;
            }
            self.timer1_masked = value & 0x40 != 0;
            self.timer1 = if value & 0x01 != 0 && value & 0x80 == 0 {
                Some(0)
            } else {
                None
            };
        }
    }
}

impl Device for OplModel {
    fn read(&mut self, port: u16, _: Width) -> u32 {
        if self.kind == OplKind::Absent {
            return 0xFF;
        }
        match port - self.base {
            0 => self.status().into(),
            2 if self.kind == OplKind::Opl3 => self.status().into(),
            _ => 0xFF,
        }
    }

    fn write(&mut self, port: u16, _: Width, value: u32) {
        if self.kind == OplKind::Absent {
            return;
        }
        let value = value as u8;
        let offset = port - self.base;
        if offset % 2 == 0 {
            self.phase = Phase::AfterIndex {
                bank: (offset / 2) as u8,
                reg: value,
                reads: 0,
            };
        } else if let Phase::AfterIndex { bank: b, reg, reads } = self.phase {
            // The data goes to the bank whose index was written last.
            self.phase = Phase::AfterData;
            self.write_data(b, reg, value, reads);
        }
    }
}
