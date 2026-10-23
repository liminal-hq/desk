// Where finished pixels go: the `Present` trait and a frame buffer in memory that counts what it is given.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec;
use alloc::vec::Vec;
use desk_hal::HalError;

/// Where finished pixels go: a frame buffer, or memory in tests.
pub trait Present {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    /// Spans start and end on multiples of this: 1 for a linear frame buffer, 8 for mode 12h's planes.
    fn align(&self) -> u32 {
        1
    }
    /// Before the first span of a frame (the planar presenter sets the graphics controller here).
    fn begin(&mut self) -> Result<(), HalError> {
        Ok(())
    }
    /// Palette indices for row `y` from column `x`. The caller keeps the span on the screen and aligned.
    fn span(&mut self, x: u32, y: u32, pixels: &[u8]) -> Result<(), HalError>;
    /// After the last span of a frame.
    fn end(&mut self) -> Result<(), HalError> {
        Ok(())
    }
}

/// What a `MemoryPresent` has been given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PresentStats {
    /// Calls to `begin`.
    pub frames: u32,
    /// Calls to `span`.
    pub spans: u32,
    /// Pixels in those spans.
    pub pixels: u32,
}

/// A frame buffer in RAM that counts what it is given: the host's display, and the damage accounting's meter.
pub struct MemoryPresent {
    /// `width × height` palette indices, row-major.
    pub pixels: Vec<u8>,
    width: u32,
    height: u32,
    align: u32,
    pub stats: PresentStats,
}

impl MemoryPresent {
    /// A cleared frame buffer that wants spans on multiples of `align` columns (0 is taken as 1).
    pub fn new(width: u32, height: u32, align: u32) -> MemoryPresent {
        MemoryPresent {
            pixels: vec![0; width as usize * height as usize],
            width,
            height,
            align: align.max(1),
            stats: PresentStats::default(),
        }
    }
}

impl Present for MemoryPresent {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn align(&self) -> u32 {
        self.align
    }

    fn begin(&mut self) -> Result<(), HalError> {
        self.stats.frames += 1;
        Ok(())
    }

    /// Refuses (`InvalidArgument`) a span off the screen, or one whose start or length is not a multiple of `align()`.
    fn span(&mut self, x: u32, y: u32, pixels: &[u8]) -> Result<(), HalError> {
        let n = pixels.len() as u64;
        if y >= self.height
            || x as u64 + n > self.width as u64
            || !x.is_multiple_of(self.align)
            || !(pixels.len() as u32).is_multiple_of(self.align)
        {
            return Err(HalError::InvalidArgument);
        }
        let start = y as usize * self.width as usize + x as usize;
        self.pixels[start..start + pixels.len()].copy_from_slice(pixels);
        self.stats.spans += 1;
        self.stats.pixels += pixels.len() as u32;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_land_and_are_counted() {
        let mut p = MemoryPresent::new(8, 3, 1);
        p.begin().unwrap();
        p.span(2, 1, &[1, 2, 3]).unwrap();
        p.span(0, 2, &[9; 8]).unwrap();
        p.end().unwrap();
        assert_eq!(&p.pixels[8..16], &[0, 0, 1, 2, 3, 0, 0, 0]);
        assert_eq!(&p.pixels[16..], &[9; 8]);
        assert_eq!(
            p.stats,
            PresentStats {
                frames: 1,
                spans: 2,
                pixels: 11
            }
        );
        assert_eq!((p.width(), p.height(), p.align()), (8, 3, 1));
    }

    #[test]
    fn bad_spans_are_refused_and_not_counted() {
        let mut p = MemoryPresent::new(16, 2, 8);
        assert_eq!(p.span(4, 0, &[0; 8]), Err(HalError::InvalidArgument));
        assert_eq!(p.span(0, 0, &[0; 5]), Err(HalError::InvalidArgument));
        assert_eq!(p.span(8, 0, &[0; 16]), Err(HalError::InvalidArgument));
        assert_eq!(p.span(0, 2, &[0; 8]), Err(HalError::InvalidArgument));
        assert_eq!(p.span(u32::MAX - 7, 0, &[0; 8]), Err(HalError::InvalidArgument));
        assert_eq!(p.stats, PresentStats::default());
        p.span(8, 1, &[3; 8]).unwrap();
        assert_eq!(p.pixels[31], 3);
    }
}
