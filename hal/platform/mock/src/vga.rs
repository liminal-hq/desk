// A model of the VGA's register file at 0x3C0..=0x3DF: attribute controller, sequencer, graphics controller, CRTC, DAC and the
// input status register.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// What it models, because the driver's correctness depends on it: the attribute controller's index/data flip-flop (reset by a read
// of 0x3DA), the CRTC's write protection of registers 0 to 7 (bit 7 of register 0x11), the DAC's index auto-increment and
// three-phase transfers with separate read and write indices, six-bit DAC values, and a vertical retrace bit that toggles every
// `retrace_period` reads of 0x3DA (or never, to test the bounded wait). What it does not model: the memory, the display timing,
// the monochrome I/O range (0x3B4/0x3B5/0x3BA), or the effect of any register on anything.

use crate::bus::{Device, Width};

pub const AC_INDEX: u16 = 0x3C0;
pub const AC_READ: u16 = 0x3C1;
pub const MISC_WRITE: u16 = 0x3C2;
pub const SEQ_INDEX: u16 = 0x3C4;
pub const SEQ_DATA: u16 = 0x3C5;
pub const DAC_MASK: u16 = 0x3C6;
pub const DAC_READ_INDEX: u16 = 0x3C7;
pub const DAC_WRITE_INDEX: u16 = 0x3C8;
pub const DAC_DATA: u16 = 0x3C9;
pub const MISC_READ: u16 = 0x3CC;
pub const GC_INDEX: u16 = 0x3CE;
pub const GC_DATA: u16 = 0x3CF;
pub const CRTC_INDEX: u16 = 0x3D4;
pub const CRTC_DATA: u16 = 0x3D5;
pub const STATUS: u16 = 0x3DA;

/// The register file. All fields are public so a test can set the state it wants and read what a driver programmed.
#[derive(Debug)]
pub struct VgaModel {
    pub misc: u8,
    pub seq: [u8; 5],
    pub crtc: [u8; 25],
    pub gc: [u8; 9],
    /// Attribute registers 0 to 20 (16 palette entries, mode control, overscan, plane enable, horizontal pan, colour select).
    pub ac: [u8; 21],
    /// The palette address bit (0x20 in the index byte): set when video is enabled.
    pub ac_video_enabled: bool,
    pub dac: [[u8; 3]; 256],
    pub pel_mask: u8,
    // Index registers and flip-flops.
    seq_index: u8,
    crtc_index: u8,
    gc_index: u8,
    ac_index: u8,
    ac_flip: bool,
    dac_read_index: u8,
    dac_write_index: u8,
    dac_read_phase: u8,
    dac_write_phase: u8,
    dac_write_pending: [u8; 3],
    dac_last_op_was_read: bool,
    status_reads: u32,
    /// Reads of 0x3DA between toggles of the retrace bit; 0 means the bit never sets (a stuck or absent display).
    pub retrace_period: u32,
    /// Every write the model accepted or refused, as (what, value): a CRTC write refused by the protect bit is counted here.
    pub refused_crtc_writes: u32,
}

impl Default for VgaModel {
    fn default() -> VgaModel {
        VgaModel {
            misc: 0,
            seq: [0; 5],
            crtc: [0; 25],
            gc: [0; 9],
            ac: [0; 21],
            ac_video_enabled: false,
            dac: [[0; 3]; 256],
            pel_mask: 0xFF,
            seq_index: 0,
            crtc_index: 0,
            gc_index: 0,
            ac_index: 0,
            ac_flip: false,
            dac_read_index: 0,
            dac_write_index: 0,
            dac_read_phase: 0,
            dac_write_phase: 0,
            dac_write_pending: [0; 3],
            dac_last_op_was_read: false,
            status_reads: 0,
            retrace_period: 4,
            refused_crtc_writes: 0,
        }
    }
}

impl VgaModel {
    pub fn new() -> VgaModel {
        VgaModel::default()
    }

    /// The attribute controller's flip-flop: false means the next write to 0x3C0 is an index.
    pub fn ac_expects_data(&self) -> bool {
        self.ac_flip
    }

    fn retrace(&self) -> bool {
        self.retrace_period != 0 && (self.status_reads / self.retrace_period) % 2 == 1
    }
}

impl Device for VgaModel {
    fn read(&mut self, port: u16, _: Width) -> u32 {
        let v = match port {
            AC_INDEX => self.ac_index | if self.ac_video_enabled { 0x20 } else { 0 },
            AC_READ => self.ac.get(usize::from(self.ac_index)).copied().unwrap_or(0),
            MISC_READ => self.misc,
            SEQ_INDEX => self.seq_index,
            SEQ_DATA => self.seq.get(usize::from(self.seq_index)).copied().unwrap_or(0),
            DAC_MASK => self.pel_mask,
            // 0 after a write to the write index, 3 after a write to the read index; the last access decides.
            DAC_READ_INDEX => {
                if self.dac_last_op_was_read {
                    3
                } else {
                    0
                }
            }
            DAC_WRITE_INDEX => self.dac_write_index,
            DAC_DATA => {
                let v = self.dac[usize::from(self.dac_read_index)][usize::from(self.dac_read_phase)];
                self.dac_read_phase += 1;
                if self.dac_read_phase == 3 {
                    self.dac_read_phase = 0;
                    self.dac_read_index = self.dac_read_index.wrapping_add(1);
                }
                v
            }
            GC_INDEX => self.gc_index,
            GC_DATA => self.gc.get(usize::from(self.gc_index)).copied().unwrap_or(0),
            CRTC_INDEX => self.crtc_index,
            CRTC_DATA => self.crtc.get(usize::from(self.crtc_index)).copied().unwrap_or(0),
            STATUS => {
                self.ac_flip = false;
                self.status_reads = self.status_reads.wrapping_add(1);
                // Bit 3 is vertical retrace; bit 0 (display disabled) is set during retrace too.
                if self.retrace() {
                    0x09
                } else {
                    0
                }
            }
            _ => 0xFF,
        };
        v.into()
    }

    fn write(&mut self, port: u16, _: Width, value: u32) {
        let v = value as u8;
        match port {
            AC_INDEX => {
                if !self.ac_flip {
                    self.ac_index = v & 0x1F;
                    self.ac_video_enabled = v & 0x20 != 0;
                } else if let Some(r) = self.ac.get_mut(usize::from(self.ac_index)) {
                    *r = v;
                }
                self.ac_flip = !self.ac_flip;
            }
            MISC_WRITE => self.misc = v,
            SEQ_INDEX => self.seq_index = v,
            SEQ_DATA => {
                if let Some(r) = self.seq.get_mut(usize::from(self.seq_index)) {
                    *r = v;
                }
            }
            DAC_MASK => self.pel_mask = v,
            DAC_READ_INDEX => {
                self.dac_read_index = v;
                self.dac_read_phase = 0;
                self.dac_last_op_was_read = true;
            }
            DAC_WRITE_INDEX => {
                self.dac_write_index = v;
                self.dac_write_phase = 0;
                self.dac_last_op_was_read = false;
            }
            DAC_DATA => {
                self.dac_write_pending[usize::from(self.dac_write_phase)] = v & 63;
                self.dac_write_phase += 1;
                if self.dac_write_phase == 3 {
                    self.dac_write_phase = 0;
                    self.dac[usize::from(self.dac_write_index)] = self.dac_write_pending;
                    self.dac_write_index = self.dac_write_index.wrapping_add(1);
                }
                self.dac_last_op_was_read = false;
            }
            GC_INDEX => self.gc_index = v,
            GC_DATA => {
                if let Some(r) = self.gc.get_mut(usize::from(self.gc_index)) {
                    *r = v;
                }
            }
            CRTC_INDEX => self.crtc_index = v,
            CRTC_DATA => {
                let i = usize::from(self.crtc_index);
                // Bit 7 of register 0x11 protects registers 0 to 7 (bit 4 of register 7, line compare, is the one exception).
                let protected = self.crtc[0x11] & 0x80 != 0 && i < 8;
                if protected && i == 7 {
                    self.crtc[7] = (self.crtc[7] & !0x10) | (v & 0x10);
                } else if protected {
                    self.refused_crtc_writes += 1;
                } else if let Some(r) = self.crtc.get_mut(i) {
                    *r = v;
                }
            }
            _ => {}
        }
    }
}
