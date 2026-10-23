// The window chrome: the layout of a window's parts and the painting of its stepped halo, title bar, control boxes, rule and content.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The measurements are `docs/FIRST-WINDOW.md` ("Window chrome"). They are all taken from the window's outer box, which excludes
// the halo; the halo is drawn outside it.

use crate::icons::{Icon, CLOSE, MAXIMISE, MINIMISE};
use crate::text::{draw_text, fit_with_ellipsis};
use crate::theme::{Role, Theme};
use desk_font::Font;
use desk_gfx::{Rect, Surface};

/// The height of the title bar.
pub const TITLE_HEIGHT: u32 = 22;
/// How far the halo reaches outside the outer box.
pub const HALO: i32 = 3;
/// The side of a control box.
pub const BOX: u32 = 18;
/// The smallest outer box; `ChromeLayout::new` widens a smaller one to it, so callers take `outer` and `extent` from the layout.
pub const MIN_WIDTH: u32 = 120;
/// See `MIN_WIDTH`.
pub const MIN_HEIGHT: u32 = 40;

const TITLE_INSET: i32 = 8;
const TITLE_TOP: i32 = 4;
const BOX_TOP: i32 = 2;
const BOX_GAP: i32 = 2;
const BOX_MARGIN: i32 = 8;
// The cell of Desk Title, the face the title is set in.
const TITLE_CELL: u32 = 14;

/// The control boxes, left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Minimise,
    Maximise,
    Close,
}

/// Where each part of a window is. `extent` is what the window owns on screen in either focus state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChromeLayout {
    pub outer: Rect,
    pub extent: Rect,
    pub title_bar: Rect,
    /// The room for the title: from 8 pixels in to 8 pixels before the first control box.
    pub title_text: Rect,
    /// Minimise, maximise and close, in the order of `Control`.
    pub buttons: [Rect; 3],
    pub rule: Rect,
    pub content: Rect,
}

impl ChromeLayout {
    /// The layout of a window whose outer box is `outer`, widened to `MIN_WIDTH` × `MIN_HEIGHT` when it is smaller.
    pub fn new(outer: Rect) -> ChromeLayout {
        let w = outer.width().max(MIN_WIDTH);
        let h = outer.height().max(MIN_HEIGHT);
        let outer = Rect::new(outer.x0, outer.y0, w, h);
        let (x, y) = (outer.x0, outer.y0);
        let step = BOX as i32 + BOX_GAP;
        let close_x = outer.x1 - BOX_MARGIN - BOX as i32;
        let button = |at: i32| Rect::new(at, y + BOX_TOP, BOX, BOX);
        let buttons = [button(close_x - 2 * step), button(close_x - step), button(close_x)];
        let text_x = x + TITLE_INSET;
        let text_end = buttons[0].x0 - BOX_MARGIN;
        ChromeLayout {
            outer,
            extent: outer.inflate(HALO),
            title_bar: Rect::new(x, y, w, TITLE_HEIGHT),
            title_text: Rect::new(text_x, y + TITLE_TOP, (text_end - text_x).max(0) as u32, TITLE_CELL),
            buttons,
            rule: Rect::new(x, y + TITLE_HEIGHT as i32, w, 1),
            content: Rect::new(x, y + TITLE_HEIGHT as i32 + 1, w, h - TITLE_HEIGHT - 1),
        }
    }

    /// The box of one control.
    pub fn button(&self, c: Control) -> Rect {
        self.buttons[c as usize]
    }
}

/// Paints the window into `s` (within its clip): the halo (three rings when `focused`, a hairline otherwise, and nothing at two and
/// three pixels so what lies beneath shows), the title bar with the title cut to fit, the control boxes and their icons, the rule and
/// the content area.
pub fn paint_chrome(s: &mut Surface, t: &Theme, title_font: &Font, l: &ChromeLayout, title: &str, focused: bool) {
    let rings = [Role::Accent, Role::Halo2, Role::Halo3];
    let shown = if focused { rings.len() } else { 1 };
    for (i, role) in rings.iter().enumerate().take(shown) {
        let role = if focused { *role } else { Role::Line };
        s.outline(l.outer.inflate(i as i32 + 1), t.slot(role));
    }
    s.fill(l.title_bar, t.slot(Role::Chrome));
    s.fill(l.rule, t.slot(Role::Line));
    s.fill(l.content, t.slot(Role::Content));

    let (title_role, icon_role) = if focused {
        (Role::Text, Role::TextSecondary)
    } else {
        (Role::TextSecondary, Role::TextMuted)
    };
    let room = l.title_text.width();
    let (head, cut) = fit_with_ellipsis(title_font, title, room);
    let (x, top) = (l.title_text.x0, l.title_text.y0);
    let x = draw_text(s, title_font, x, top, head, t.slot(title_role));
    if cut && title_font.advance("…") <= room {
        draw_text(s, title_font, x, top, "…", t.slot(title_role));
    }

    let icons = [&MINIMISE, &MAXIMISE, &CLOSE];
    let pad = (BOX - Icon::SIZE) as i32 / 2;
    for (b, icon) in l.buttons.iter().zip(icons) {
        s.fill(*b, t.slot(Role::Chrome));
        icon.draw(s, b.x0 + pad, b.y0 + pad, t.slot(icon_role));
    }
}
