// The test picture, written once against `Canvas` so the same calls draw it on a linear 8-bit surface (mode 13h, VBE) and on mode
// 12h's planes, and in memory for the reference checksum.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// From the top: the Afterglow horizon as stepped bands (docs/PALETTE.md: no dithering; seven bands in 256 colours, slots 10, 9 and
// 8 in 16), a palette ramp (every one of the 256 slots, or the 16), and a box drawn with 1-pixel lines holding the six accent
// colours and a grid of single-pixel lines.

use desk_hal::Rgb6;
use desk_vga::Canvas;

/// Which palette the picture is drawn for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    Colours256,
    Colours16,
}

/// The Desk's 16-colour set (docs/PALETTE.md): eight greys, then amber, two browns, rose, purple, cyan, green, blue.
pub const DESK16: [Rgb6; 16] = [
    Rgb6::from_rgb8(0x05, 0x05, 0x07),
    Rgb6::from_rgb8(0x0c, 0x0c, 0x10),
    Rgb6::from_rgb8(0x11, 0x11, 0x16),
    Rgb6::from_rgb8(0x1b, 0x1e, 0x28),
    Rgb6::from_rgb8(0x33, 0x38, 0x47),
    Rgb6::from_rgb8(0x7c, 0x87, 0x96),
    Rgb6::from_rgb8(0xb4, 0xbf, 0xce),
    Rgb6::from_rgb8(0xe0, 0xe0, 0xe0),
    Rgb6::from_rgb8(0xff, 0xaa, 0x40),
    Rgb6::from_rgb8(0x7d, 0x54, 0x22),
    Rgb6::from_rgb8(0x37, 0x26, 0x12),
    Rgb6::from_rgb8(0xf4, 0x3f, 0x5e),
    Rgb6::from_rgb8(0xa7, 0x8b, 0xfa),
    Rgb6::from_rgb8(0x22, 0xd3, 0xee),
    Rgb6::from_rgb8(0x2e, 0xc6, 0x6a),
    Rgb6::from_rgb8(0x60, 0xa5, 0xfa),
];

/// The horizon in 256 colours, purple to red to orange. PALETTE.md names the ramp steps (purple-800, red-700, orange-300) without
/// giving their values, so these seven are an illustration of the same sweep, not the shipped ramp.
const HORIZON: [Rgb6; 7] = [
    Rgb6::from_rgb8(0x2a, 0x1f, 0x4d),
    Rgb6::from_rgb8(0x4a, 0x24, 0x68),
    Rgb6::from_rgb8(0x7a, 0x2a, 0x6a),
    Rgb6::from_rgb8(0xa8, 0x30, 0x5a),
    Rgb6::from_rgb8(0xc4, 0x40, 0x3f),
    Rgb6::from_rgb8(0xe0, 0x70, 0x2f),
    Rgb6::from_rgb8(0xff, 0xaa, 0x40),
];

fn triangle(x: u32) -> u8 {
    let m = x % 256;
    let v = if m < 128 { m } else { 255 - m };
    (v * 63 / 127) as u8
}

/// The 256-entry palette: slots 0 to 15 the Desk's 16, 16 to 22 the horizon, 23 to 31 greys, 32 to 255 a three-phase hue sweep.
pub fn palette256() -> [Rgb6; 256] {
    let mut p = [Rgb6::BLACK; 256];
    p[..16].copy_from_slice(&DESK16);
    p[16..23].copy_from_slice(&HORIZON);
    for (i, c) in p.iter_mut().enumerate().skip(23) {
        *c = if i < 32 {
            let g = ((i - 23) * 7) as u8;
            Rgb6::new(g, g, g)
        } else {
            Rgb6::new(
                triangle(i as u32 * 3),
                triangle(i as u32 * 3 + 85),
                triangle(i as u32 * 3 + 170),
            )
        };
    }
    p
}

/// Draws the picture. Every coordinate is a fraction of the surface, so it fits 320 × 200, 640 × 480 and anything else.
pub fn draw<C: Canvas>(c: &mut C, depth: Depth) {
    let (w, h) = (c.width(), c.height());
    c.clear(0);

    // The horizon: equal bands, one palette slot each.
    let bands: &[u8] = match depth {
        Depth::Colours256 => &[16, 17, 18, 19, 20, 21, 22],
        Depth::Colours16 => &[10, 9, 8],
    };
    let top = h * 45 / 100;
    let n = bands.len() as u32;
    for (i, slot) in bands.iter().enumerate() {
        let (y0, y1) = (top * i as u32 / n, top * (i as u32 + 1) / n);
        c.fill_rect(0, y0 as i32, w, y1 - y0, *slot);
    }

    // The ramp: column x shows slot x * slots / w.
    let slots = if depth == Depth::Colours256 { 256 } else { 16 };
    let (ramp_y, ramp_h) = (h * 50 / 100, (h * 8 / 100).max(4));
    for x in 0..w {
        c.vline(x as i32, ramp_y as i32, ramp_h, (x * slots / w) as u8);
    }

    // The box: a 1-pixel outline, a second one inside it, a grid of single-pixel lines, diagonals, and six swatches.
    let (bx, by, bw, bh) = (w / 16, h * 62 / 100, w - w / 8, h * 32 / 100);
    c.rect(bx as i32, by as i32, bw, bh, 7);
    c.rect(bx as i32 + 3, by as i32 + 3, bw - 6, bh - 6, 4);
    let step = (w / 40).max(4);
    let mut x = bx + 8;
    while x < bx + bw - 8 {
        c.vline(x as i32, by as i32 + 5, bh - 10, 3);
        x += step;
    }
    let mut y = by + 8;
    while y < by + bh - 8 {
        c.hline(bx as i32 + 5, y as i32, bw - 10, 2);
        y += step;
    }
    for i in 0..bh.min(bw) - 8 {
        c.pixel((bx + 4 + i) as i32, (by + 4 + i) as i32, 11);
        c.pixel((bx + bw - 5 - i) as i32, (by + 4 + i) as i32, 12);
    }
    let sw = (bw / 12).max(6);
    for (i, slot) in [8u8, 11, 12, 13, 14, 15].iter().enumerate() {
        c.fill_rect(
            (bx + bw / 4 + i as u32 * (sw + 2)) as i32,
            (by + bh / 3) as i32,
            sw,
            sw,
            *slot,
        );
    }
}
