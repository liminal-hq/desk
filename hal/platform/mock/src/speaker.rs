// Port 0x61 as the PC speaker uses it, for the driver tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The tone's counter is channel 2 of the 8254, which `devices::Pit8254` already models (its `channels[2]` records the mode and the
// reload value); this file holds the port the gate and the data line are on.

use alloc::vec::Vec;

use crate::bus::{Device, Width};

/// Port 0x61: the keyboard controller's port B, which also carries the PC speaker's gate (bit 0) and data (bit 1) lines. The other
/// bits (the parity-check enables, the refresh toggle and the like) belong to other things, and a driver must leave them as found.
#[derive(Debug, Default)]
pub struct Port61 {
    /// The whole byte as last written (or set by a test).
    pub value: u8,
    /// Every byte written, in order.
    pub writes: Vec<u8>,
}

impl Port61 {
    pub fn with_value(value: u8) -> Port61 {
        Port61 {
            value,
            writes: Vec::new(),
        }
    }
    /// Bit 0: the PIT channel 2 gate.
    pub fn gate(&self) -> bool {
        self.value & 1 != 0
    }
    /// Bit 1: the speaker data line.
    pub fn data(&self) -> bool {
        self.value & 2 != 0
    }
}

impl Device for Port61 {
    fn read(&mut self, _: u16, _: Width) -> u32 {
        self.value.into()
    }
    fn write(&mut self, _: u16, _: Width, value: u32) {
        self.value = value as u8;
        self.writes.push(value as u8);
    }
}
