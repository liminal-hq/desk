// Drawing in VGA mode 12h: 640 × 480, four bit planes at A0000h, written through the sequencer map mask and the graphics
// controller's set/reset and bit mask, read back through the read map select.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A byte at A0000h + y * 80 + x / 8 holds eight pixels, one bit per pixel, and the same address in each of the four planes holds
// that pixel's bit 0 to bit 3 of colour. A CPU write goes to every plane the map mask enables, through the controller's logic:
//
// - **Set/reset** (write mode 0 with enable set/reset = 0Fh): each plane's bit comes from the set/reset register, not from the CPU
//   byte, wherever the **bit mask** has a 1; where it has a 0 the plane keeps what the **latches** hold. The latches are loaded
//   by any read of that address, so a partial byte is read first and then written; a whole byte (mask 0FFh) needs no read.
// - **Read map select** (read mode 0): a read returns one plane's byte, the plane chosen by graphics controller register 4. It is
//   the only way to see what is in the planes, one at a time.
//
// `Planar16` holds the controller in that state for as long as it lives. Dropping it switches set/reset off and opens the bit mask;
// the other registers it sets (write mode 0, no rotate, all planes in the map mask) are the standard mode 12h values, and the read
// map select is returned to plane 0 after every read.

use desk_hal::{HalError, MmioRegion, PortIo};

use crate::canvas::{clip_span, Canvas};
use crate::regs::{gc, gc_write, seq, sequencer_write};

/// Bytes in one row of one plane at 640 pixels.
pub const PITCH_640: u32 = 80;

pub struct Planar16<'a, P: PortIo> {
    ports: &'a P,
    region: &'a mut MmioRegion,
    width: u32,
    height: u32,
    pitch: u32,
}

impl<'a, P: PortIo> Planar16<'a, P> {
    /// Puts the controller in write mode 0 with replace logic, all four planes enabled and set/reset enabled on all of them.
    /// `InvalidArgument` if a row of `pitch` bytes cannot hold `width` pixels or the region is smaller than one plane
    /// (`pitch * height` bytes).
    pub fn new(
        ports: &'a P,
        region: &'a mut MmioRegion,
        width: u32,
        height: u32,
        pitch: u32,
    ) -> Result<Planar16<'a, P>, HalError> {
        if u64::from(pitch) * 8 < u64::from(width) || u64::from(pitch) * u64::from(height) > region.len() as u64 {
            return Err(HalError::InvalidArgument);
        }
        gc_write(ports, gc::MODE, 0x00);
        gc_write(ports, gc::DATA_ROTATE, 0x00);
        sequencer_write(ports, seq::MAP_MASK, 0x0F);
        gc_write(ports, gc::ENABLE_SET_RESET, 0x0F);
        gc_write(ports, gc::BIT_MASK, 0xFF);
        Ok(Planar16 {
            ports,
            region,
            width,
            height,
            pitch,
        })
    }

    fn offset(&self, x: u32, y: u32) -> usize {
        y as usize * self.pitch as usize + (x / 8) as usize
    }

    /// Writes the pixels of byte `offset` selected by `mask` with the colour already in the set/reset register. A partial mask
    /// reads the byte first to load the latches.
    fn put(&mut self, offset: usize, mask: u8) {
        gc_write(self.ports, gc::BIT_MASK, mask);
        if mask != 0xFF {
            let _ = self.region.read8(offset);
        }
        // The value written does not matter: with set/reset enabled on all planes the CPU byte is ignored.
        let _ = self.region.write8(offset, 0);
    }

    /// Fills pixels [x0, x0 + n) of row `y` with the set/reset colour.
    fn span(&mut self, x0: u32, y: u32, n: u32) {
        let last = x0 + n - 1;
        let (b0, b1) = (x0 / 8, last / 8);
        let left = 0xFFu8 >> (x0 % 8);
        let right = 0xFFu8 << (7 - last % 8);
        let row = self.offset(0, y);
        if b0 == b1 {
            self.put(row + b0 as usize, left & right);
            return;
        }
        self.put(row + b0 as usize, left);
        if b1 > b0 + 1 {
            gc_write(self.ports, gc::BIT_MASK, 0xFF);
            for b in b0 + 1..b1 {
                let _ = self.region.write8(row + b as usize, 0);
            }
        }
        self.put(row + b1 as usize, right);
    }
}

impl<P: PortIo> Drop for Planar16<'_, P> {
    /// Switches set/reset off and opens the bit mask again, the state the BIOS and other drivers expect.
    fn drop(&mut self) {
        gc_write(self.ports, gc::ENABLE_SET_RESET, 0x00);
        gc_write(self.ports, gc::BIT_MASK, 0xFF);
    }
}

impl<P: PortIo> Canvas for Planar16<'_, P> {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn pixel(&mut self, x: i32, y: i32, colour: u8) {
        if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
            gc_write(self.ports, gc::SET_RESET, colour & 0x0F);
            let o = self.offset(x as u32, y as u32);
            self.put(o, 0x80 >> (x % 8));
        }
    }

    fn get_pixel(&mut self, x: i32, y: i32) -> Option<u8> {
        if x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height {
            return None;
        }
        let o = self.offset(x as u32, y as u32);
        let bit = 7 - (x % 8);
        let mut colour = 0u8;
        for plane in 0..4u8 {
            gc_write(self.ports, gc::READ_MAP_SELECT, plane);
            // In range: the coordinates were checked against the geometry `new` validated against the region.
            let byte = self.region.read8(o).unwrap_or(0);
            colour |= ((byte >> bit) & 1) << plane;
        }
        gc_write(self.ports, gc::READ_MAP_SELECT, 0);
        Some(colour)
    }

    fn hline(&mut self, x: i32, y: i32, len: u32, colour: u8) {
        if y < 0 || y as u32 >= self.height {
            return;
        }
        if let Some((x0, n)) = clip_span(x, len, self.width) {
            gc_write(self.ports, gc::SET_RESET, colour & 0x0F);
            self.span(x0, y as u32, n);
        }
    }

    fn vline(&mut self, x: i32, y: i32, len: u32, colour: u8) {
        if x < 0 || x as u32 >= self.width {
            return;
        }
        if let Some((y0, n)) = clip_span(y, len, self.height) {
            gc_write(self.ports, gc::SET_RESET, colour & 0x0F);
            gc_write(self.ports, gc::BIT_MASK, 0x80 >> (x % 8));
            for row in y0..y0 + n {
                let o = self.offset(x as u32, row);
                let _ = self.region.read8(o);
                let _ = self.region.write8(o, 0);
            }
        }
    }

    fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, colour: u8) {
        let (Some((x0, nw)), Some((y0, nh))) = (clip_span(x, w, self.width), clip_span(y, h, self.height)) else {
            return;
        };
        gc_write(self.ports, gc::SET_RESET, colour & 0x0F);
        for row in y0..y0 + nh {
            self.span(x0, row, nw);
        }
    }

    fn read_row(&mut self, y: u32, out: &mut [u8]) -> Result<(), HalError> {
        if y >= self.height || out.len() < self.width as usize {
            return Err(HalError::InvalidArgument);
        }
        let out = &mut out[..self.width as usize];
        out.fill(0);
        let row = self.offset(0, y);
        let mut result = Ok(());
        'planes: for plane in 0..4u8 {
            gc_write(self.ports, gc::READ_MAP_SELECT, plane);
            for (i, px) in out.iter_mut().enumerate() {
                match self.region.read8(row + i / 8) {
                    Ok(byte) => *px |= ((byte >> (7 - i % 8)) & 1) << plane,
                    Err(e) => {
                        result = Err(e);
                        break 'planes;
                    }
                }
            }
        }
        // Plane 0 again whether or not a read failed.
        gc_write(self.ports, gc::READ_MAP_SELECT, 0);
        result
    }
}
