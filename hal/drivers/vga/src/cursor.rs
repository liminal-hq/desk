// A software mouse cursor: an arrow sprite drawn over any `Canvas`, with the pixels under it saved and put back.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The Desk draws its own cursor (docs/SPEC.md, "Input"): the DOS mouse driver's cursor is never shown and the native kernel has none.
// Save-under suits a frame buffer that is slow to read and write over the bus and has no hardware sprite; the Desk's compositor
// will replace this with damage-aware drawing, but the sprite and the clipping stay. The hot spot is the top-left pixel of the
// arrow, and the cursor may be partly or wholly off the surface (the sprite is clipped, as every `Canvas` call is).

use crate::canvas::Canvas;

pub const ARROW_WIDTH: usize = 12;
pub const ARROW_HEIGHT: usize = 19;

/// The arrow: `X` the outline, `o` the fill, `.` transparent.
const ARROW: [&[u8; ARROW_WIDTH]; ARROW_HEIGHT] = [
    b"X...........",
    b"XX..........",
    b"XoX.........",
    b"XooX........",
    b"XoooX.......",
    b"XooooX......",
    b"XoooooX.....",
    b"XooooooX....",
    b"XoooooooX...",
    b"XooooooooX..",
    b"XoooooXXXXX.",
    b"XooXooX.....",
    b"XoX.XooX....",
    b"XX..XooX....",
    b"X....XooX...",
    b".....XooX...",
    b"......XooX..",
    b"......XooX..",
    b".......XX...",
];

/// The cursor's state: where it is drawn, if it is, and what was under it.
pub struct SoftCursor {
    x: i32,
    y: i32,
    under: [u8; ARROW_WIDTH * ARROW_HEIGHT],
    /// Which of the saved pixels were on the surface (the rest are not put back).
    valid: [bool; ARROW_WIDTH * ARROW_HEIGHT],
    drawn: bool,
}

impl Default for SoftCursor {
    fn default() -> Self {
        SoftCursor::new()
    }
}

impl SoftCursor {
    pub const fn new() -> SoftCursor {
        SoftCursor {
            x: 0,
            y: 0,
            under: [0; ARROW_WIDTH * ARROW_HEIGHT],
            valid: [false; ARROW_WIDTH * ARROW_HEIGHT],
            drawn: false,
        }
    }

    /// Whether the arrow is on the surface now.
    pub fn is_drawn(&self) -> bool {
        self.drawn
    }

    /// The hot spot, where the arrow was last drawn.
    pub fn position(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    /// Puts back what the arrow covered. Does nothing if it is not drawn. Call it before drawing anything else under the cursor.
    pub fn hide<C: Canvas>(&mut self, canvas: &mut C) {
        if !self.drawn {
            return;
        }
        for row in 0..ARROW_HEIGHT {
            for col in 0..ARROW_WIDTH {
                let i = row * ARROW_WIDTH + col;
                if self.valid[i] {
                    canvas.pixel(
                        self.x.wrapping_add(col as i32),
                        self.y.wrapping_add(row as i32),
                        self.under[i],
                    );
                }
            }
        }
        self.drawn = false;
    }

    /// Moves the arrow to `(x, y)`: restores the old place, saves the new one and draws. `outline` and `fill` are palette indices.
    pub fn show<C: Canvas>(&mut self, canvas: &mut C, x: i32, y: i32, outline: u8, fill: u8) {
        self.hide(canvas);
        self.x = x;
        self.y = y;
        for (row, line) in ARROW.iter().enumerate() {
            for (col, &shape) in line.iter().enumerate() {
                let i = row * ARROW_WIDTH + col;
                let (px, py) = (x.wrapping_add(col as i32), y.wrapping_add(row as i32));
                match canvas.get_pixel(px, py) {
                    Some(v) => {
                        self.under[i] = v;
                        self.valid[i] = true;
                    }
                    None => self.valid[i] = false,
                }
                match shape {
                    b'X' => canvas.pixel(px, py, outline),
                    b'o' => canvas.pixel(px, py, fill),
                    _ => {}
                }
            }
        }
        self.drawn = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::Linear8;
    use desk_hal::MmioRegion;

    fn surface(buf: &mut [u8]) -> MmioRegion {
        // SAFETY: the slice outlives the region and nothing else touches it while the region is in use.
        unsafe { MmioRegion::new(buf.as_mut_ptr(), buf.len()) }
    }

    fn patterned(w: usize, h: usize) -> Vec<u8> {
        (0..w * h).map(|i| (i * 7 % 251) as u8).collect()
    }

    #[test]
    fn the_arrow_is_drawn_and_hiding_it_restores_every_pixel() {
        let mut buf = patterned(64, 48);
        let before = buf.clone();
        let mut region = surface(&mut buf);
        let mut c = Linear8::new(&mut region, 64, 48, 64).unwrap();
        let mut cur = SoftCursor::new();
        cur.show(&mut c, 10, 5, 254, 255);
        assert_eq!(c.get_pixel(10, 5), Some(254), "the hot spot is the outline's first pixel");
        assert_eq!(c.get_pixel(11, 7), Some(255), "the fill");
        assert!(cur.is_drawn());
        cur.hide(&mut c);
        assert_eq!(buf, before);
    }

    #[test]
    fn moving_restores_the_old_place_before_drawing_the_new() {
        let mut buf = patterned(64, 48);
        let before = buf.clone();
        let mut region = surface(&mut buf);
        let mut c = Linear8::new(&mut region, 64, 48, 64).unwrap();
        let mut cur = SoftCursor::new();
        cur.show(&mut c, 3, 3, 254, 255);
        cur.show(&mut c, 20, 20, 254, 255);
        // Overlapping positions must not leave the first arrow's pixels behind or save them as the background.
        cur.show(&mut c, 22, 21, 254, 255);
        cur.hide(&mut c);
        assert_eq!(buf, before);
    }

    #[test]
    fn at_the_edges_the_arrow_is_clipped_not_wrapped() {
        for (x, y) in [(-5, -5), (60, 40), (63, 47), (-100, 10), (10, 1000), (i32::MAX, i32::MIN)] {
            let mut buf = patterned(64, 48);
            let before = buf.clone();
            let mut region = surface(&mut buf);
            let mut c = Linear8::new(&mut region, 64, 48, 64).unwrap();
            let mut cur = SoftCursor::new();
            cur.show(&mut c, x, y, 254, 255);
            cur.hide(&mut c);
            assert_eq!(buf, before, "{x},{y}");
        }
        // Hot spot in the last column: one pixel, the outline, and nothing wraps to the next row.
        let mut buf = vec![0u8; 64 * 48];
        let mut region = surface(&mut buf);
        let mut c = Linear8::new(&mut region, 64, 48, 64).unwrap();
        let mut cur = SoftCursor::new();
        cur.show(&mut c, 63, 0, 254, 255);
        assert_eq!(
            buf.iter().filter(|&&p| p != 0).count(),
            15,
            "only the arrow's first column survives the clip"
        );
        assert_eq!(buf[63], 254);
        assert_eq!(
            buf[64 + 63],
            254,
            "the next row's outline is in the same column, not wrapped to the left edge"
        );
        assert_eq!(buf[64], 0);
    }
}
