// The three 10 × 10 one-bit control icons, drawn for the Desk.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_gfx::Surface;

/// A 10 × 10 one-bit icon in the `.DFN` row form: two bytes a row, the leftmost pixel in the highest bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Icon {
    pub bits: [u8; Icon::SIZE as usize * 2],
}

impl Icon {
    pub const SIZE: u32 = 10;

    /// Packs ten rows of ten characters, `X` for a set pixel and anything else clear.
    pub const fn from_art(rows: [&[u8; 10]; 10]) -> Icon {
        let mut bits = [0u8; 20];
        let mut y = 0;
        while y < 10 {
            let mut x = 0;
            while x < 10 {
                if rows[y][x] == b'X' {
                    bits[y * 2 + x / 8] |= 0x80 >> (x % 8);
                }
                x += 1;
            }
            y += 1;
        }
        Icon { bits }
    }

    /// Whether the pixel at column `x`, row `y` is set.
    pub fn bit(&self, x: u32, y: u32) -> bool {
        x < Icon::SIZE && y < Icon::SIZE && self.bits[(y * 2 + x / 8) as usize] & (0x80 >> (x % 8)) != 0
    }

    /// Draws the set pixels in `colour` with the icon's top-left corner at `(x, y)`.
    pub fn draw(&self, s: &mut Surface, x: i32, y: i32, colour: u8) {
        s.mono(x, y, Icon::SIZE, Icon::SIZE, &self.bits, 2, colour);
    }
}

/// A bar along the bottom.
pub const MINIMISE: Icon = Icon::from_art([
    b"..........",
    b"..........",
    b"..........",
    b"..........",
    b"..........",
    b"..........",
    b"..........",
    b".XXXXXXXX.",
    b".XXXXXXXX.",
    b"..........",
]);

/// A square with a heavier top edge.
pub const MAXIMISE: Icon = Icon::from_art([
    b"..........",
    b".XXXXXXXX.",
    b".XXXXXXXX.",
    b".X......X.",
    b".X......X.",
    b".X......X.",
    b".X......X.",
    b".X......X.",
    b".XXXXXXXX.",
    b"..........",
]);

/// A cross, drawn two pixels thick so it holds its weight against the box.
pub const CLOSE: Icon = Icon::from_art([
    b"..........",
    b".XX....XX.",
    b"..XX..XX..",
    b"...XXXX...",
    b"....XX....",
    b"....XX....",
    b"...XXXX...",
    b"..XX..XX..",
    b".XX....XX.",
    b"..........",
]);
