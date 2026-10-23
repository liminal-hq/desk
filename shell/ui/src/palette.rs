// The Desk's palettes: the 16-colour set, the Afterglow surfaces, and the DAC contents for each depth.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The hex values are `docs/PALETTE.md`'s, converted with `Rgb6::from_rgb8` (the nearest DAC colour), not rounded to a multiple of four.

use crate::theme::Depth;
use desk_hal::Rgb6;

const fn hex(rgb: u32) -> Rgb6 {
    Rgb6::from_rgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// The 16-colour set: slots 0–15 at both depths, and the only colours in mode 12h.
pub const DESK16: [Rgb6; 16] = [
    hex(0x050507),
    hex(0x0c0c10),
    hex(0x111116),
    hex(0x1b1e28),
    hex(0x333847),
    hex(0x7c8796),
    hex(0xb4bfce),
    hex(0xe0e0e0),
    hex(0xffaa40),
    hex(0x7d5422),
    hex(0x372612),
    hex(0xf43f5e),
    hex(0xa78bfa),
    hex(0x22d3ee),
    hex(0x2ec66a),
    hex(0x60a5fa),
];

/// The Afterglow look's surfaces, slots 97–104 at depth 8.
pub const AFTERGLOW_SURFACES: [Rgb6; 8] = [
    hex(0x050507),
    hex(0x0c0c10),
    hex(0x111116),
    hex(0x14161d),
    hex(0x16161c),
    hex(0x1b1e28),
    hex(0x262a37),
    hex(0x333847),
];

/// The first slot of `AFTERGLOW_SURFACES` at depth 8.
const SURFACES_AT: usize = 97;
/// The cursor's outline and fill slots at depth 8.
const CURSOR_AT: usize = 240;

/// The DAC contents for `depth`; entries the depth does not use are black. Depth 8 holds the 16-colour set in 0–15, the surfaces in
/// 97–104 and the cursor in 240–241; depth 4 holds only the 16-colour set.
pub fn palette(depth: Depth) -> [Rgb6; 256] {
    let mut p = [Rgb6::BLACK; 256];
    p[..DESK16.len()].copy_from_slice(&DESK16);
    if depth == Depth::Bits8 {
        p[SURFACES_AT..SURFACES_AT + AFTERGLOW_SURFACES.len()].copy_from_slice(&AFTERGLOW_SURFACES);
        p[CURSOR_AT] = DESK16[0];
        p[CURSOR_AT + 1] = DESK16[7];
    }
    p
}
