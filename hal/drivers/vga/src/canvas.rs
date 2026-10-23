// The drawing surface the Desk's primitives are written against, and the checksum the checks use.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Coordinates are signed and everything is clipped to the surface, so a window that is half off the screen needs no special case.
// A colour is a palette index: 0 to 255 on a linear 8-bit surface, 0 to 15 on a planar one (the high bits are ignored).

use desk_hal::HalError;

/// The widest surface `checksum` handles: it reads a row at a time into a stack buffer.
pub const MAX_ROW: usize = 1280;

pub trait Canvas {
    fn width(&self) -> u32;
    fn height(&self) -> u32;

    /// One pixel; outside the surface is ignored.
    fn pixel(&mut self, x: i32, y: i32, colour: u8);
    /// One pixel read back; `None` outside the surface.
    fn get_pixel(&mut self, x: i32, y: i32) -> Option<u8>;
    /// `len` pixels rightwards from (x, y).
    fn hline(&mut self, x: i32, y: i32, len: u32, colour: u8);
    /// `len` pixels downwards from (x, y).
    fn vline(&mut self, x: i32, y: i32, len: u32, colour: u8);
    /// A solid rectangle.
    fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, colour: u8);
    /// Row `y`, left to right, one byte per pixel, into the start of `out` (which must hold `width()` bytes). Reads the surface
    /// as the hardware holds it, so it is what a checksum of the frame buffer sees.
    fn read_row(&mut self, y: u32, out: &mut [u8]) -> Result<(), HalError>;

    /// A 1-pixel outline of the rectangle whose top-left pixel is (x, y).
    fn rect(&mut self, x: i32, y: i32, w: u32, h: u32, colour: u8) {
        if w == 0 || h == 0 {
            return;
        }
        self.hline(x, y, w, colour);
        self.vline(x, y, h, colour);
        if h > 1 {
            self.hline(x, y.saturating_add(h.min(i32::MAX as u32) as i32 - 1), w, colour);
        }
        if w > 1 {
            self.vline(x.saturating_add(w.min(i32::MAX as u32) as i32 - 1), y, h, colour);
        }
    }

    /// The whole surface in one colour.
    fn clear(&mut self, colour: u8) {
        let (w, h) = (self.width(), self.height());
        self.fill_rect(0, 0, w, h, colour);
    }

    /// Copies a `w` by `h` block of one-byte pixels from `src` (rows `src_pitch` bytes apart) to (x, y), clipped. Pixels of the
    /// block that `src` is too short to hold are skipped.
    fn blit(&mut self, x: i32, y: i32, w: u32, h: u32, src: &[u8], src_pitch: usize) {
        for row in 0..h {
            for col in 0..w {
                if let Some(v) = src.get(row as usize * src_pitch + col as usize) {
                    self.pixel(x.saturating_add(col as i32), y.saturating_add(row as i32), *v);
                }
            }
        }
    }
}

/// Adler-32, as zlib defines it, over a byte stream.
#[derive(Clone, Copy, Debug)]
pub struct Adler32 {
    a: u32,
    b: u32,
}

impl Default for Adler32 {
    fn default() -> Adler32 {
        Adler32 { a: 1, b: 0 }
    }
}

impl Adler32 {
    pub fn new() -> Adler32 {
        Adler32::default()
    }

    pub fn update(&mut self, bytes: &[u8]) {
        for v in bytes {
            self.a = (self.a + u32::from(*v)) % 65521;
            self.b = (self.b + self.a) % 65521;
        }
    }

    pub fn finish(&self) -> u32 {
        (self.b << 16) | self.a
    }
}

/// The Adler-32 of the surface as read back, row by row, one byte per pixel: the number `hw` prints for mode 13h, now for any
/// surface, so a linear 8-bit frame buffer and a planar one that show the same picture have the same checksum.
pub fn checksum<C: Canvas>(canvas: &mut C) -> Result<u32, HalError> {
    let width = canvas.width() as usize;
    if width > MAX_ROW {
        return Err(HalError::InvalidArgument);
    }
    let mut row = [0u8; MAX_ROW];
    let mut sum = Adler32::new();
    for y in 0..canvas.height() {
        canvas.read_row(y, &mut row[..width])?;
        sum.update(&row[..width]);
    }
    Ok(sum.finish())
}

/// Clips the span [start, start + len) on one axis to [0, limit). Returns (first, count), or `None` if nothing is left.
pub(crate) fn clip_span(start: i32, len: u32, limit: u32) -> Option<(u32, u32)> {
    let end = i64::from(start) + i64::from(len);
    let first = i64::from(start).max(0);
    let last = end.min(i64::from(limit));
    if last <= first {
        None
    } else {
        Some((first as u32, (last - first) as u32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adler32_matches_the_reference_values() {
        let mut a = Adler32::new();
        assert_eq!(a.finish(), 1);
        a.update(b"Wikipedia");
        assert_eq!(a.finish(), 0x11E6_0398);
    }

    #[test]
    fn spans_clip_on_both_sides() {
        assert_eq!(clip_span(-3, 10, 100), Some((0, 7)));
        assert_eq!(clip_span(95, 10, 100), Some((95, 5)));
        assert_eq!(clip_span(10, 5, 100), Some((10, 5)));
        assert_eq!(clip_span(-5, 5, 100), None);
        assert_eq!(clip_span(100, 1, 100), None);
        assert_eq!(clip_span(0, 0, 100), None);
        assert_eq!(clip_span(i32::MIN, u32::MAX, 100), Some((0, 100)));
    }
}
