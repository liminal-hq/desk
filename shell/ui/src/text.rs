// Drawing text with a .DFN face, and cutting a line to a width with an ellipsis.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_font::Font;
use desk_gfx::Surface;

/// The character that ends a line cut short.
const ELLIPSIS: &str = "…";

/// Draws `text` with its cell's top at `top`, starting at `x`, advancing by each glyph's width with no kerning; a character the
/// face lacks draws as `?`. Returns the x after the text.
pub fn draw_text(s: &mut Surface, font: &Font, x: i32, top: i32, text: &str, colour: u8) -> i32 {
    let mut x = x;
    for c in text.chars() {
        let g = font.glyph_or_fallback(c);
        s.mono(
            x,
            top,
            u32::from(g.advance),
            u32::from(g.height),
            g.rows,
            usize::from(g.stride),
            colour,
        );
        x = x.saturating_add(i32::from(g.advance));
    }
    x
}

/// The longest prefix of `text` that fits in `max` pixels with an ellipsis after it, and whether `text` had to be cut. A text that
/// fits whole comes back as it is with `false`; the caller draws the prefix and then `…` when the flag is set. When not even the
/// ellipsis fits the prefix is empty.
pub fn fit_with_ellipsis<'t>(font: &Font, text: &'t str, max: u32) -> (&'t str, bool) {
    if font.advance(text) <= max {
        return (text, false);
    }
    let room = max.saturating_sub(font.advance(ELLIPSIS));
    let mut used = 0u32;
    let mut end = 0;
    for (at, c) in text.char_indices() {
        used = used.saturating_add(u32::from(font.glyph_or_fallback(c).advance));
        if used > room {
            break;
        }
        end = at + c.len_utf8();
    }
    (&text[..end], true)
}
