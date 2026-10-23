// Shared set-up for the desk-wm host tests: the screen, the faces and a few helpers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![allow(dead_code)]

use desk_font::Font;
use desk_gfx::{Damage, Rect};
use desk_hal::{KeyCode, KeyEvent, Modifiers, MouseEvent};
use desk_wm::{Fonts, WindowId, WindowSpec, Wm};

pub static UI: &[u8] = include_bytes!("../../../../fonts/generated/ui.dfn");
pub static TITLE: &[u8] = include_bytes!("../../../../fonts/generated/title.dfn");

pub const SCREEN: Rect = Rect::new(0, 0, 640, 480);
pub const LEFT: u8 = MouseEvent::LEFT;

pub fn fonts() -> Fonts<'static> {
    Fonts {
        ui: Font::parse(UI).unwrap(),
        title: Font::parse(TITLE).unwrap(),
    }
}

pub fn damage() -> Damage {
    Damage::new(SCREEN)
}

/// The damage's rectangles in a fixed order (top to bottom, then left to right).
pub fn rects(d: &Damage) -> Vec<Rect> {
    let mut v = d.rects().to_vec();
    v.sort_by_key(|r| (r.y0, r.x0));
    v
}

pub fn open(wm: &mut Wm, name: &str, outer: Rect) -> (WindowId, Rect) {
    let mut d = damage();
    let id = wm.open(
        WindowSpec {
            name,
            outer,
            title: name,
        },
        &mut d,
    );
    (id, outer.inflate(3))
}

pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        pressed: true,
        modifiers: Modifiers::NONE,
    }
}

/// Window A: outer (100, 100, 200, 120), extent (97, 97, 206, 126). Its title bar is y 100..122 and its close box
/// (274, 102)..(292, 120).
pub const A: Rect = Rect::new(100, 100, 200, 120);
/// Window B, apart from A: outer (400, 50, 150, 100).
pub const B: Rect = Rect::new(400, 50, 150, 100);
/// Window C, across A's lower right: outer (200, 150, 200, 120).
pub const C: Rect = Rect::new(200, 150, 200, 120);
