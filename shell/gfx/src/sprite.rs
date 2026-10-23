// Two-ink sprites as string art, and the Desk's arrow.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::rect::Rect;

/// A two-ink sprite as string art: `b'X'` outline, `b'o'` fill, anything else transparent (the form of
/// `desk_vga::cursor::ARROW_ROWS`). Every row is `width` bytes and there are `height` rows.
pub struct Sprite {
    pub width: u8,
    pub height: u8,
    pub rows: &'static [&'static [u8]],
    pub hot_x: u8,
    pub hot_y: u8,
}

impl Sprite {
    /// The screen rectangle the sprite covers when its hot spot is at `(x, y)`.
    pub fn rect_at(&self, x: i32, y: i32) -> Rect {
        Rect::new(
            x.saturating_sub(self.hot_x as i32),
            y.saturating_sub(self.hot_y as i32),
            self.width as u32,
            self.height as u32,
        )
    }

    /// The ink index (0 outline, 1 fill) of the pixel at column `col`, row `row`, or `None` where the sprite is transparent.
    pub fn ink(&self, col: usize, row: usize) -> Option<usize> {
        match self.rows.get(row)?.get(col)? {
            b'X' => Some(0),
            b'o' => Some(1),
            _ => None,
        }
    }
}

static ARROW_ROWS: [&[u8]; 19] = [
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

/// The arrow: 12 × 19, hot spot at the top-left pixel.
pub const ARROW: Sprite = Sprite {
    width: 12,
    height: 19,
    rows: &ARROW_ROWS,
    hot_x: 0,
    hot_y: 0,
};

#[cfg(test)]
mod tests {
    use super::*;
    use desk_vga::cursor::{ARROW_HEIGHT, ARROW_ROWS as VGA_ROWS, ARROW_WIDTH};

    #[test]
    fn arrow_equals_the_vga_drivers_arrow() {
        assert_eq!((ARROW.width as usize, ARROW.height as usize), (ARROW_WIDTH, ARROW_HEIGHT));
        assert_eq!(ARROW.rows.len(), ARROW_HEIGHT);
        for (ours, theirs) in ARROW.rows.iter().zip(VGA_ROWS.iter()) {
            assert_eq!(*ours, &theirs[..]);
        }
        assert_eq!((ARROW.hot_x, ARROW.hot_y), (0, 0));
    }

    #[test]
    fn rows_are_as_wide_as_the_sprite() {
        assert!(ARROW.rows.iter().all(|r| r.len() == ARROW.width as usize));
    }

    #[test]
    fn rect_and_ink() {
        assert_eq!(ARROW.rect_at(10, 20), Rect::new(10, 20, 12, 19));
        assert_eq!(ARROW.ink(0, 0), Some(0));
        assert_eq!(ARROW.ink(1, 2), Some(1));
        assert_eq!(ARROW.ink(5, 0), None);
        assert_eq!(ARROW.ink(12, 0), None);
        assert_eq!(ARROW.ink(0, 19), None);
    }
}
