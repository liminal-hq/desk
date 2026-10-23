// The back buffer: palette indices in a row-major byte array, with clipped drawing primitives.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::rect::Rect;
use alloc::vec;
use alloc::vec::Vec;

/// The back buffer: palette indices, row-major, pitch = width. Every drawing call is clipped to `clip()`.
pub struct Surface {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    clip: Rect,
}

impl Surface {
    /// A surface of `width × height` pixels, every one `fill`.
    pub fn new(width: u32, height: u32, fill: u8) -> Surface {
        let mut s = Surface {
            width,
            height,
            pixels: Vec::new(),
            clip: Rect::default(),
        };
        s.pixels = vec![fill; width as usize * height as usize];
        s.clip = s.bounds();
        s
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// The whole surface as a rectangle.
    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Row `y`. Panics if `y` is not on the surface.
    pub fn row(&self, y: u32) -> &[u8] {
        let w = self.width as usize;
        &self.pixels[y as usize * w..(y as usize + 1) * w]
    }

    /// Limits drawing to `r`, intersected with the bounds.
    pub fn set_clip(&mut self, r: Rect) {
        self.clip = r.intersect(&self.bounds());
    }

    pub fn clip(&self) -> Rect {
        self.clip
    }

    /// The pixel at `(x, y)`, or `None` off the surface. The clip does not apply to reads.
    pub fn get(&self, x: i32, y: i32) -> Option<u8> {
        if x < 0 || y < 0 || x as u32 >= self.width || y as u32 >= self.height {
            return None;
        }
        Some(self.pixels[y as usize * self.width as usize + x as usize])
    }

    fn put(&mut self, x: i32, y: i32, colour: u8) {
        let i = y as usize * self.width as usize + x as usize;
        self.pixels[i] = colour;
    }

    pub fn fill(&mut self, r: Rect, colour: u8) {
        let r = r.intersect(&self.clip);
        if r.is_empty() {
            return;
        }
        let w = self.width as usize;
        for y in r.y0..r.y1 {
            let start = y as usize * w;
            self.pixels[start + r.x0 as usize..start + r.x1 as usize].fill(colour);
        }
    }

    pub fn hline(&mut self, x: i32, y: i32, len: u32, colour: u8) {
        self.fill(Rect::new(x, y, len, 1), colour);
    }

    pub fn vline(&mut self, x: i32, y: i32, len: u32, colour: u8) {
        self.fill(Rect::new(x, y, 1, len), colour);
    }

    /// A one-pixel ring just inside `r`.
    pub fn outline(&mut self, r: Rect, colour: u8) {
        if r.is_empty() {
            return;
        }
        let (w, h) = (r.width(), r.height());
        self.hline(r.x0, r.y0, w, colour);
        self.hline(r.x0, r.y1 - 1, w, colour);
        self.vline(r.x0, r.y0, h, colour);
        self.vline(r.x1 - 1, r.y0, h, colour);
    }

    /// Copies a `w × h` block of indices from `src` (rows `src_pitch` apart) to `(x, y)`. Drawing stops at the first row
    /// that `src` is too short to hold.
    pub fn blit(&mut self, x: i32, y: i32, w: u32, h: u32, src: &[u8], src_pitch: usize) {
        let r = Rect::new(x, y, w, h).intersect(&self.clip);
        if r.is_empty() {
            return;
        }
        let n = r.width() as usize;
        let sx = (r.x0 as i64 - x as i64) as usize;
        for dy in r.y0..r.y1 {
            let sy = (dy as i64 - y as i64) as usize;
            let from = sy * src_pitch + sx;
            let Some(row) = src.get(from..from + n) else { return };
            let start = dy as usize * self.width as usize + r.x0 as usize;
            self.pixels[start..start + n].copy_from_slice(row);
        }
    }

    /// A 1-bit bitmap, leftmost pixel in the highest bit (the `.DFN` row form): set bits are drawn in `colour`, clear bits
    /// are left alone. `stride` is the bytes per bitmap row. Bits past the end of `bits` read as clear.
    #[allow(clippy::too_many_arguments)]
    pub fn mono(&mut self, x: i32, y: i32, w: u32, h: u32, bits: &[u8], stride: usize, colour: u8) {
        let r = Rect::new(x, y, w, h).intersect(&self.clip);
        if r.is_empty() {
            return;
        }
        for dy in r.y0..r.y1 {
            let row = (dy as i64 - y as i64) as usize;
            for dx in r.x0..r.x1 {
                let col = (dx as i64 - x as i64) as usize;
                let Some(byte) = bits.get(row * stride + col / 8) else {
                    continue;
                };
                if byte & (0x80 >> (col % 8)) != 0 {
                    self.put(dx, dy, colour);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: u32 = 12;
    const H: u32 = 9;

    /// Applies `draw` to a fresh surface and checks every pixel against `inside`, for a shape placed at every offset around
    /// the surface, with and without a clip rectangle.
    fn check(draw: impl Fn(&mut Surface, i32, i32), inside: impl Fn(i32, i32, i32, i32) -> bool) {
        let clips = [
            Rect::new(0, 0, W, H),
            Rect::new(3, 2, 5, 4),
            Rect::new(-4, -4, 100, 100),
            Rect::new(6, 0, 20, 3),
        ];
        for clip in clips {
            for ox in -(W as i32 + 2)..(W as i32 + 2) {
                for oy in -(H as i32 + 2)..(H as i32 + 2) {
                    let mut s = Surface::new(W, H, 0);
                    s.set_clip(clip);
                    draw(&mut s, ox, oy);
                    let effective = s.clip();
                    for y in 0..H as i32 {
                        for x in 0..W as i32 {
                            let want = if effective.contains(x, y) && inside(x, y, ox, oy) {
                                7
                            } else {
                                0
                            };
                            assert_eq!(s.get(x, y), Some(want), "at ({x}, {y}) shape at ({ox}, {oy}) clip {clip:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn new_fills_and_reads() {
        let s = Surface::new(4, 3, 5);
        assert_eq!((s.width(), s.height(), s.pixels().len()), (4, 3, 12));
        assert!(s.pixels().iter().all(|&p| p == 5));
        assert_eq!(s.get(3, 2), Some(5));
        assert_eq!(
            (s.get(-1, 0), s.get(4, 0), s.get(0, 3), s.get(0, -1)),
            (None, None, None, None)
        );
        assert_eq!(s.row(2), &[5, 5, 5, 5]);
    }

    #[test]
    fn set_clip_is_limited_to_the_bounds() {
        let mut s = Surface::new(10, 10, 0);
        s.set_clip(Rect::new(-5, -5, 100, 100));
        assert_eq!(s.clip(), s.bounds());
        s.set_clip(Rect::new(20, 20, 5, 5));
        assert!(s.clip().is_empty());
        s.fill(s.bounds(), 9);
        assert!(s.pixels().iter().all(|&p| p == 0));
    }

    #[test]
    fn fill_clips_at_every_edge_and_to_the_clip() {
        check(
            |s, ox, oy| s.fill(Rect::new(ox, oy, 5, 4), 7),
            |x, y, ox, oy| Rect::new(ox, oy, 5, 4).contains(x, y),
        );
    }

    #[test]
    fn hline_and_vline_clip() {
        check(
            |s, ox, oy| s.hline(ox, oy, 7, 7),
            |x, y, ox, oy| y == oy && x >= ox && x < ox + 7,
        );
        check(
            |s, ox, oy| s.vline(ox, oy, 6, 7),
            |x, y, ox, oy| x == ox && y >= oy && y < oy + 6,
        );
    }

    #[test]
    fn outline_is_a_ring_just_inside() {
        check(
            |s, ox, oy| s.outline(Rect::new(ox, oy, 6, 5), 7),
            |x, y, ox, oy| {
                let r = Rect::new(ox, oy, 6, 5);
                r.contains(x, y) && (x == r.x0 || x == r.x1 - 1 || y == r.y0 || y == r.y1 - 1)
            },
        );
        let mut s = Surface::new(4, 4, 0);
        s.outline(Rect::new(0, 0, 4, 4), 1);
        assert_eq!(s.get(1, 1), Some(0));
        s.outline(Rect::new(0, 0, 1, 1), 2);
        assert_eq!(s.get(0, 0), Some(2));
        s.outline(Rect::default(), 3);
    }

    #[test]
    fn blit_clips_and_reads_from_the_right_source_pixel() {
        let src: Vec<u8> = (0..5 * 4).map(|i| 100 + i as u8).collect();
        for ox in -7..W as i32 + 3 {
            for oy in -6..H as i32 + 3 {
                let mut s = Surface::new(W, H, 0);
                s.blit(ox, oy, 4, 4, &src, 5);
                for y in 0..H as i32 {
                    for x in 0..W as i32 {
                        let (sx, sy) = (x - ox, y - oy);
                        let want = if (0..4).contains(&sx) && (0..4).contains(&sy) {
                            100 + (sy * 5 + sx) as u8
                        } else {
                            0
                        };
                        assert_eq!(s.get(x, y), Some(want), "at ({x}, {y}) blit at ({ox}, {oy})");
                    }
                }
            }
        }
        check(
            |s, ox, oy| s.blit(ox, oy, 5, 4, &[7; 20], 5),
            |x, y, ox, oy| Rect::new(ox, oy, 5, 4).contains(x, y),
        );
    }

    #[test]
    fn blit_with_a_short_source_stops_at_its_end() {
        let mut s = Surface::new(4, 4, 0);
        s.blit(0, 0, 4, 4, &[1; 6], 4);
        assert_eq!(s.row(0), &[1, 1, 1, 1]);
        assert_eq!(s.row(1), &[0, 0, 0, 0]);
        assert_eq!(s.row(2), &[0, 0, 0, 0]);
    }

    #[test]
    fn mono_draws_a_known_pattern() {
        // 10 × 3: two bytes per row, leftmost pixel in the high bit.
        let bits = [0b1010_0000, 0b0100_0000, 0b0000_0001, 0b1000_0000, 0b1111_1111, 0b1100_0000];
        let mut s = Surface::new(12, 5, 0);
        s.mono(1, 1, 10, 3, &bits, 2, 9);
        let row = |y: u32| s.row(y).to_vec();
        assert_eq!(row(0), [0; 12]);
        // Row 0 of the bitmap: bits 0, 2 and 9 set.
        assert_eq!(row(1), [0, 9, 0, 9, 0, 0, 0, 0, 0, 0, 9, 0]);
        // Row 1: bits 7 and 8 set.
        assert_eq!(row(2), [0, 0, 0, 0, 0, 0, 0, 0, 9, 9, 0, 0]);
        // Row 2: all ten set.
        assert_eq!(row(3), [0, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 0]);
        assert_eq!(row(4), [0; 12]);
        // Clear bits leave what is under them.
        let mut t = Surface::new(8, 1, 3);
        t.mono(0, 0, 8, 1, &[0b1000_0001], 1, 6);
        assert_eq!(t.row(0), &[6, 3, 3, 3, 3, 3, 3, 6]);
    }

    #[test]
    fn mono_clips_at_every_edge_and_to_the_clip() {
        // A 6 × 4 bitmap with a checkerboard of set bits.
        let bits = [0b1010_1000, 0b0101_0100, 0b1010_1000, 0b0101_0100];
        check(
            |s, ox, oy| s.mono(ox, oy, 6, 4, &bits, 1, 7),
            |x, y, ox, oy| {
                let (cx, cy) = (x - ox, y - oy);
                (0..6).contains(&cx) && (0..4).contains(&cy) && (cx + cy) % 2 == 0
            },
        );
    }
}
