// Shared set-up for the desk-shell host tests: the faces, shells at each depth, the presenter and a few event helpers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![allow(dead_code)]

use desk_font::Font;
use desk_gfx::{Damage, FrameDigest, MemoryPresent, Rect};
use desk_hal::{MouseEvent, Rgb6};
use desk_shell::{Event, Shell};
use desk_ui::{palette, Depth};
use desk_wm::Fonts;

pub static UI: &[u8] = include_bytes!("../../../../fonts/generated/ui.dfn");
pub static TITLE: &[u8] = include_bytes!("../../../../fonts/generated/title.dfn");

pub const W: u32 = 640;
pub const H: u32 = 480;
pub const LEFT: u8 = MouseEvent::LEFT;
pub const DEPTHS: [Depth; 2] = [Depth::Bits8, Depth::Bits4];

/// Window A: outer (100, 100, 200, 120), extent (97, 97, 206, 126); its title bar is y 100..122.
pub const A: Rect = Rect::new(100, 100, 200, 120);
/// Window B, apart from A: outer (400, 50, 150, 100).
pub const B: Rect = Rect::new(400, 50, 150, 100);
/// Window C, across A's lower right: outer (200, 150, 200, 120).
pub const C: Rect = Rect::new(200, 150, 200, 120);

pub fn fonts() -> Fonts<'static> {
    Fonts {
        ui: Font::parse(UI).unwrap(),
        title: Font::parse(TITLE).unwrap(),
    }
}

pub fn shell(depth: Depth) -> Shell<'static> {
    Shell::new(W, H, depth, fonts()).unwrap()
}

/// A presenter for `depth`: spans on multiples of 8 columns at depth 4, as mode 12h's planes want them.
pub fn present(depth: Depth) -> MemoryPresent {
    MemoryPresent::new(W, H, if depth == Depth::Bits4 { 8 } else { 1 })
}

pub fn align(depth: Depth) -> u32 {
    if depth == Depth::Bits4 {
        8
    } else {
        1
    }
}

/// A shell at `depth` with its first frame presented, so later frames show only what changed.
pub fn started(depth: Depth) -> (Shell<'static>, MemoryPresent) {
    let mut s = shell(depth);
    let mut p = present(depth);
    s.frame(&mut p).unwrap();
    (s, p)
}

pub fn extent(r: Rect) -> Rect {
    r.inflate(3)
}

/// The pixels a frame presents for these rectangles damaged together on a presenter that wants `depth`'s alignment: the area of
/// the merged, aligned damage, which is what an independent reading of the geometry predicts.
pub fn area_of(rects: &[Rect], depth: Depth) -> u32 {
    let mut d = Damage::new(Rect::new(0, 0, W, H));
    for r in rects {
        d.add(*r);
    }
    d.align_x(align(depth));
    d.area()
}

/// The 12 × 19 rectangle of the cursor with its hot spot at `(x, y)`.
pub fn cursor_rect(x: i32, y: i32) -> Rect {
    Rect::new(x, y, 12, 19)
}

pub fn to(x: i32, y: i32) -> Event {
    Event::PointerTo(x, y)
}

pub fn mouse(dx: i16, dy: i16, buttons: u8) -> Event {
    Event::Pointer(MouseEvent {
        dx,
        dy,
        buttons,
        wheel: 0,
    })
}

pub fn pal(depth: Depth) -> [Rgb6; 256] {
    palette(depth)
}

/// The digests of a presenter's pixels, to compare with `Shell::digest`.
pub fn digest_of(p: &MemoryPresent, depth: Depth) -> FrameDigest {
    FrameDigest::from_rows(W, H, &palette(depth), |y, buf| {
        let start = y as usize * W as usize;
        buf.copy_from_slice(&p.pixels[start..start + W as usize]);
    })
}
