// A Sound Blaster DSP model, for the driver tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Just enough of the DSP to check what `desk-sb` programs: the reset handshake, the version command, the speaker commands and the
// two status ports, with configurable delays. There is no DMA, no sample output and no timing: "delays" count status reads, which is
// how the driver waits.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::bus::{Device, Width};

/// The DSP at `base..=base + 0xF`.
#[derive(Debug)]
pub struct SbModel {
    pub base: u16,
    /// Every read returns 0xFF and every write is ignored, as in an empty slot.
    pub absent: bool,
    /// A reset never produces the 0xAA acknowledgement.
    pub never_answers: bool,
    /// How many reads of the read-buffer status (base+0xE) report "empty" after a reset or a command that has an answer, before
    /// bit 7 comes up.
    pub ready_delay: u32,
    /// How many reads of the write-buffer status (base+0xC) report busy (bit 7 set) after each accepted write.
    pub write_busy: u32,
    pub major: u8,
    pub minor: u8,
    /// Every command byte accepted at base+0xC, in order.
    pub commands: Vec<u8>,
    /// The state the speaker commands (0xD1, 0xD3) left.
    pub speaker_on: bool,
    /// Writes to base+0xC that arrived while the buffer was busy (a driver that does not wait loses these on a real DSP).
    pub dropped_writes: u32,
    /// Reads of base+0xC and of base+0xE, to check that waits are bounded.
    pub write_status_reads: u32,
    pub read_status_reads: u32,
    #[doc(hidden)]
    pub reset_high: bool,
    #[doc(hidden)]
    pub output: VecDeque<u8>,
    #[doc(hidden)]
    pub ready_left: u32,
    #[doc(hidden)]
    pub busy_left: u32,
}

impl SbModel {
    /// A present DSP, version 4.5.
    pub fn new(base: u16) -> SbModel {
        SbModel {
            base,
            absent: false,
            never_answers: false,
            ready_delay: 0,
            write_busy: 0,
            major: 4,
            minor: 5,
            commands: Vec::new(),
            speaker_on: false,
            dropped_writes: 0,
            write_status_reads: 0,
            read_status_reads: 0,
            reset_high: false,
            output: VecDeque::new(),
            ready_left: 0,
            busy_left: 0,
        }
    }

    fn queue(&mut self, bytes: &[u8]) {
        self.output.extend(bytes.iter().copied());
        self.ready_left = self.ready_delay;
    }
}

impl Device for SbModel {
    fn read(&mut self, port: u16, _: Width) -> u32 {
        if self.absent {
            return 0xFF;
        }
        match port - self.base {
            0xA => self.output.pop_front().map_or(0xFF, u32::from),
            0xC => {
                self.write_status_reads += 1;
                if self.busy_left > 0 {
                    self.busy_left -= 1;
                    0xFF
                } else {
                    0x7F
                }
            }
            0xE => {
                self.read_status_reads += 1;
                if self.output.is_empty() {
                    0x7F
                } else if self.ready_left > 0 {
                    self.ready_left -= 1;
                    0x7F
                } else {
                    0xFF
                }
            }
            _ => 0xFF, // the write-only reset port and the unused offsets
        }
    }

    fn write(&mut self, port: u16, _: Width, value: u32) {
        if self.absent {
            return;
        }
        let v = value as u8;
        match port - self.base {
            0x6 => {
                if v == 1 {
                    self.reset_high = true;
                } else if v == 0 && self.reset_high {
                    self.reset_high = false;
                    self.output.clear();
                    self.speaker_on = false;
                    if !self.never_answers {
                        self.queue(&[0xAA]);
                    }
                }
            }
            0xC => {
                if self.busy_left > 0 {
                    self.dropped_writes += 1;
                    return;
                }
                self.busy_left = self.write_busy;
                self.commands.push(v);
                match v {
                    0xE1 => {
                        let (major, minor) = (self.major, self.minor);
                        self.queue(&[major, minor]);
                    }
                    0xD1 => self.speaker_on = true,
                    0xD3 => self.speaker_on = false,
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
