// The linear presenter: an 8-bit frame buffer (VBE 640 × 480 × 8, or mode 13h), one `write_bytes` per span.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_gfx::Present;
use desk_hal::{HalError, MmioRegion};

/// An 8-bit linear frame buffer: `pitch` bytes per row, `width` by `height` visible pixels.
pub struct LinearPresent<'a> {
    region: &'a mut MmioRegion,
    width: u32,
    height: u32,
    pitch: u32,
}

impl<'a> LinearPresent<'a> {
    /// `InvalidArgument` if the pitch is smaller than the width or the region is smaller than `pitch * height`, so every span
    /// that passes the checks in `span` is inside the region (the same rule as `desk_vga::Linear8::new`).
    pub fn new(region: &'a mut MmioRegion, width: u32, height: u32, pitch: u32) -> Result<LinearPresent<'a>, HalError> {
        if pitch < width || u64::from(pitch) * u64::from(height) > region.len() as u64 {
            return Err(HalError::InvalidArgument);
        }
        Ok(LinearPresent {
            region,
            width,
            height,
            pitch,
        })
    }
}

impl Present for LinearPresent<'_> {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    /// Refuses (`InvalidArgument`) a span that leaves the screen.
    fn span(&mut self, x: u32, y: u32, pixels: &[u8]) -> Result<(), HalError> {
        if y >= self.height || u64::from(x) + pixels.len() as u64 > u64::from(self.width) {
            return Err(HalError::InvalidArgument);
        }
        self.region.write_bytes(y as usize * self.pitch as usize + x as usize, pixels)
    }
}
