// Drawing on an 8-bit linear frame buffer: VGA mode 13h, or a VBE mode with a linear frame buffer and one byte per pixel.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_hal::{HalError, MmioRegion};

use crate::canvas::{clip_span, Canvas};

/// A view of an 8-bit frame buffer: `pitch` bytes per row, `width` by `height` visible pixels.
pub struct Linear8<'a> {
    region: &'a mut MmioRegion,
    width: u32,
    height: u32,
    pitch: u32,
}

impl<'a> Linear8<'a> {
    /// `InvalidArgument` if the pitch is smaller than the width or the region is smaller than `pitch * height`, so every
    /// access the drawing methods make is inside it.
    pub fn new(region: &'a mut MmioRegion, width: u32, height: u32, pitch: u32) -> Result<Linear8<'a>, HalError> {
        let needed = u64::from(pitch) * u64::from(height);
        if pitch < width || needed > region.len() as u64 {
            return Err(HalError::InvalidArgument);
        }
        Ok(Linear8 {
            region,
            width,
            height,
            pitch,
        })
    }

    fn offset(&self, x: u32, y: u32) -> usize {
        y as usize * self.pitch as usize + x as usize
    }

    fn fill_span(&mut self, x: u32, y: u32, len: u32, colour: u8) {
        let base = self.offset(x, y);
        for i in 0..len as usize {
            // Inside the region: `new` checked pitch * height and the callers clip to width and height.
            let _ = self.region.write8(base + i, colour);
        }
    }
}

impl Canvas for Linear8<'_> {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn pixel(&mut self, x: i32, y: i32, colour: u8) {
        if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
            let o = self.offset(x as u32, y as u32);
            let _ = self.region.write8(o, colour);
        }
    }

    fn get_pixel(&mut self, x: i32, y: i32) -> Option<u8> {
        if x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height {
            self.region.read8(self.offset(x as u32, y as u32)).ok()
        } else {
            None
        }
    }

    fn hline(&mut self, x: i32, y: i32, len: u32, colour: u8) {
        if y < 0 || y as u32 >= self.height {
            return;
        }
        if let Some((x0, n)) = clip_span(x, len, self.width) {
            self.fill_span(x0, y as u32, n, colour);
        }
    }

    fn vline(&mut self, x: i32, y: i32, len: u32, colour: u8) {
        if x < 0 || x as u32 >= self.width {
            return;
        }
        if let Some((y0, n)) = clip_span(y, len, self.height) {
            for row in y0..y0 + n {
                let o = self.offset(x as u32, row);
                let _ = self.region.write8(o, colour);
            }
        }
    }

    fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, colour: u8) {
        let (Some((x0, nw)), Some((y0, nh))) = (clip_span(x, w, self.width), clip_span(y, h, self.height)) else {
            return;
        };
        for row in y0..y0 + nh {
            self.fill_span(x0, row, nw, colour);
        }
    }

    fn read_row(&mut self, y: u32, out: &mut [u8]) -> Result<(), HalError> {
        if y >= self.height || out.len() < self.width as usize {
            return Err(HalError::InvalidArgument);
        }
        let base = self.offset(0, y);
        for (i, o) in out.iter_mut().take(self.width as usize).enumerate() {
            *o = self.region.read8(base + i)?;
        }
        Ok(())
    }

    /// Each visible row is copied with one `write_bytes`, which is the fast path a platform can replace.
    fn blit(&mut self, x: i32, y: i32, w: u32, h: u32, src: &[u8], src_pitch: usize) {
        let (Some((x0, nw)), Some((y0, nh))) = (clip_span(x, w, self.width), clip_span(y, h, self.height)) else {
            return;
        };
        let skip_x = (i64::from(x0) - i64::from(x)) as usize;
        let skip_y = (i64::from(y0) - i64::from(y)) as usize;
        for row in 0..nh as usize {
            let from = (skip_y + row) * src_pitch + skip_x;
            let Some(bytes) = src.get(from..) else { return };
            let n = (nw as usize).min(bytes.len());
            let dst = self.offset(x0, y0 + row as u32);
            let _ = self.region.write_bytes(dst, &bytes[..n]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(buf: &mut [u8]) -> MmioRegion {
        // SAFETY: the slice outlives the region in every test and is not otherwise touched while the region is in use.
        unsafe { MmioRegion::new(buf.as_mut_ptr(), buf.len()) }
    }

    #[test]
    fn the_view_is_validated() {
        let mut buf = [0u8; 100];
        let mut r = region(&mut buf);
        assert!(Linear8::new(&mut r, 10, 10, 10).is_ok());
        assert_eq!(Linear8::new(&mut r, 10, 11, 10).err(), Some(HalError::InvalidArgument));
        assert_eq!(Linear8::new(&mut r, 10, 5, 9).err(), Some(HalError::InvalidArgument));
    }

    #[test]
    fn primitives_draw_and_clip() {
        let mut buf = [0u8; 8 * 6];
        {
            let mut r = region(&mut buf);
            let mut c = Linear8::new(&mut r, 8, 6, 8).unwrap();
            c.pixel(1, 1, 5);
            c.pixel(-1, 0, 9);
            c.pixel(8, 0, 9);
            c.hline(-2, 0, 5, 1);
            c.vline(7, 3, 99, 2);
            c.fill_rect(3, 2, 3, 2, 7);
            c.rect(0, 4, 4, 2, 3);
            assert_eq!(c.get_pixel(1, 1), Some(5));
            assert_eq!(c.get_pixel(8, 1), None);
        }
        let row = |y: usize| buf[y * 8..y * 8 + 8].to_vec();
        assert_eq!(row(0), [1, 1, 1, 0, 0, 0, 0, 0]);
        assert_eq!(row(1), [0, 5, 0, 0, 0, 0, 0, 0]);
        assert_eq!(row(2), [0, 0, 0, 7, 7, 7, 0, 0]);
        assert_eq!(row(3), [0, 0, 0, 7, 7, 7, 0, 2]);
        assert_eq!(row(4), [3, 3, 3, 3, 0, 0, 0, 2]);
        assert_eq!(row(5), [3, 3, 3, 3, 0, 0, 0, 2]);
    }

    #[test]
    fn clearing_and_reading_back_agree() {
        let mut buf = [0u8; 16 * 4];
        let mut r = region(&mut buf);
        let mut c = Linear8::new(&mut r, 10, 4, 16).unwrap();
        c.clear(0xAB);
        let mut row = [0u8; 10];
        c.read_row(3, &mut row).unwrap();
        assert_eq!(row, [0xAB; 10]);
        assert_eq!(c.read_row(4, &mut row), Err(HalError::InvalidArgument));
        assert_eq!(c.read_row(0, &mut row[..9]), Err(HalError::InvalidArgument));
        assert_eq!(
            crate::canvas::checksum(&mut c),
            Ok({
                let mut a = crate::canvas::Adler32::new();
                a.update(&[0xAB; 40]);
                a.finish()
            })
        );
    }

    #[test]
    fn blit_clips_all_four_edges() {
        let mut buf = [0u8; 6 * 4];
        let src = [1, 2, 3, 4, 5, 6, 7, 8, 9]; // 3 x 3
        {
            let mut r = region(&mut buf);
            let mut c = Linear8::new(&mut r, 6, 4, 6).unwrap();
            c.blit(-1, -1, 3, 3, &src, 3);
            c.blit(4, 3, 3, 3, &src, 3);
        }
        let row = |y: usize| buf[y * 6..y * 6 + 6].to_vec();
        assert_eq!(row(0), [5, 6, 0, 0, 0, 0]);
        assert_eq!(row(1), [8, 9, 0, 0, 0, 0]);
        assert_eq!(row(2), [0, 0, 0, 0, 0, 0]);
        assert_eq!(row(3), [0, 0, 0, 0, 1, 2]);
    }
}
