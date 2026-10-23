// Host tests for desk-ui: the palettes and roles, text drawing, the window chrome and the widgets, against the committed faces.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_font::Font;
use desk_gfx::{Rect, Surface};
use desk_hal::{KeyCode, KeyEvent, Modifiers, Rgb6};
use desk_ui::{
    draw_text, fit_with_ellipsis, paint_chrome, palette, ChromeLayout, Control, Depth, Role, Theme, Widget, AFTERGLOW_SURFACES,
    DESK16, MIN_HEIGHT, MIN_WIDTH, TITLE_HEIGHT,
};

static UI: &[u8] = include_bytes!("../../../fonts/generated/ui.dfn");
static TITLE: &[u8] = include_bytes!("../../../fonts/generated/title.dfn");

const SENTINEL: u8 = 250;

fn rgb(hex: &str) -> Rgb6 {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap();
    Rgb6::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

// ---- palettes ----------------------------------------------------------------------------------------------------------------

const DESK16_HEX: [&str; 16] = [
    "#050507", "#0c0c10", "#111116", "#1b1e28", "#333847", "#7c8796", "#b4bfce", "#e0e0e0", "#ffaa40", "#7d5422", "#372612",
    "#f43f5e", "#a78bfa", "#22d3ee", "#2ec66a", "#60a5fa",
];
const SURFACES_HEX: [&str; 8] = [
    "#050507", "#0c0c10", "#111116", "#14161d", "#16161c", "#1b1e28", "#262a37", "#333847",
];

#[test]
fn the_constants_equal_the_hex_values_in_palette_md() {
    for (i, hex) in DESK16_HEX.iter().enumerate() {
        assert_eq!(DESK16[i], rgb(hex), "DESK16[{i}] {hex}");
    }
    for (i, hex) in SURFACES_HEX.iter().enumerate() {
        assert_eq!(AFTERGLOW_SURFACES[i], rgb(hex), "AFTERGLOW_SURFACES[{i}] {hex}");
    }
    // The rounding is the nearest DAC colour, not a multiple of four: #e0e0e0 is 55, not 56.
    assert_eq!(DESK16[7], Rgb6::new(55, 55, 55));
}

#[test]
fn depth_8_holds_the_sets_surfaces_and_cursor_and_black_elsewhere() {
    let p = palette(Depth::Bits8);
    for i in 0..256 {
        let want = match i {
            0..=15 => DESK16[i],
            97..=104 => AFTERGLOW_SURFACES[i - 97],
            240 => rgb("#050507"),
            241 => rgb("#e0e0e0"),
            _ => Rgb6::BLACK,
        };
        assert_eq!(p[i], want, "slot {i}");
    }
}

#[test]
fn depth_4_fills_only_the_first_sixteen() {
    let p = palette(Depth::Bits4);
    assert_eq!(&p[..16], &DESK16);
    assert!(p[16..].iter().all(|c| *c == Rgb6::BLACK));
}

// ---- roles ---------------------------------------------------------------------------------------------------------------------

// Role, hex at depth 8, slot at depth 8, hex at depth 4, slot at depth 4: the table in docs/FIRST-WINDOW.md.
const TABLE: [(Role, &str, u8, &str, u8); 15] = [
    (Role::Desktop, "#050507", 97, "#050507", 0),
    (Role::Chrome, "#0c0c10", 98, "#0c0c10", 1),
    (Role::Content, "#111116", 99, "#111116", 2),
    (Role::Raised, "#1b1e28", 102, "#1b1e28", 3),
    (Role::Line, "#262a37", 103, "#333847", 4),
    (Role::Border, "#333847", 104, "#333847", 4),
    (Role::TextMuted, "#7c8796", 5, "#7c8796", 5),
    (Role::TextSecondary, "#b4bfce", 6, "#b4bfce", 6),
    (Role::Text, "#e0e0e0", 7, "#e0e0e0", 7),
    (Role::Accent, "#ffaa40", 8, "#ffaa40", 8),
    (Role::Halo2, "#7d5422", 9, "#7d5422", 9),
    (Role::Halo3, "#372612", 10, "#372612", 10),
    (Role::Selection, "#372612", 10, "#372612", 10),
    (Role::CursorOutline, "#050507", 240, "#050507", 0),
    (Role::CursorFill, "#e0e0e0", 241, "#e0e0e0", 7),
];

#[test]
fn every_role_resolves_at_both_depths_as_the_table_says() {
    assert_eq!(TABLE.len(), Role::COUNT);
    assert_eq!(Role::ALL.len(), Role::COUNT);
    let (t8, t4) = (Theme::afterglow(Depth::Bits8), Theme::afterglow(Depth::Bits4));
    assert_eq!((t8.depth(), t4.depth()), (Depth::Bits8, Depth::Bits4));
    let (p8, p4) = (palette(Depth::Bits8), palette(Depth::Bits4));
    for (i, (role, hex8, slot8, hex4, slot4)) in TABLE.into_iter().enumerate() {
        assert_eq!(Role::ALL[i], role, "order of {role:?}");
        assert_eq!(t8.slot(role), slot8, "{role:?} at depth 8");
        assert_eq!(t4.slot(role), slot4, "{role:?} at depth 4");
        assert!(slot4 < 16 && t4.slot(role) < 16, "{role:?} is below 16 at depth 4");
        assert_eq!(p8[slot8 as usize], rgb(hex8), "{role:?} colour at depth 8");
        assert_eq!(p4[slot4 as usize], rgb(hex4), "{role:?} colour at depth 4");
    }
}

// ---- text ----------------------------------------------------------------------------------------------------------------------

#[test]
fn draw_text_equals_the_glyphs_drawn_one_by_one() {
    let font = Font::parse(UI).unwrap();
    let mut whole = Surface::new(80, 20, 0);
    let end = draw_text(&mut whole, &font, 3, 2, "Desk", 7);
    let mut parts = Surface::new(80, 20, 0);
    let mut x = 3;
    for c in "Desk".chars() {
        let g = font.glyph(c).unwrap();
        parts.mono(x, 2, g.advance as u32, g.height as u32, g.rows, g.stride as usize, 7);
        x += g.advance as i32;
    }
    assert_eq!(whole.pixels(), parts.pixels());
    assert_eq!(end, x);
    assert_eq!(end, 3 + font.advance("Desk") as i32);
    assert!(whole.pixels().contains(&7));
}

#[test]
fn a_missing_character_draws_a_question_mark() {
    let font = Font::parse(UI).unwrap();
    let missing = '\u{FFFD}';
    assert!(font.glyph(missing).is_none());
    let mut a = Surface::new(40, 20, 0);
    let mut b = Surface::new(40, 20, 0);
    let ax = draw_text(&mut a, &font, 1, 1, &missing.to_string(), 7);
    let bx = draw_text(&mut b, &font, 1, 1, "?", 7);
    assert_eq!(a.pixels(), b.pixels());
    assert_eq!(ax, bx);
}

#[test]
fn fit_with_ellipsis_keeps_the_longest_prefix_that_fits_with_the_ellipsis() {
    let font = Font::parse(TITLE).unwrap();
    let text = "A rather long title for a window";
    let full = font.advance(text);
    assert_eq!(fit_with_ellipsis(&font, text, full), (text, false));
    assert_eq!(fit_with_ellipsis(&font, "", 0), ("", false));
    let dots = font.advance("…");
    for max in [full - 1, 100, 60, dots + 1] {
        let (head, cut) = fit_with_ellipsis(&font, text, max);
        assert!(cut);
        assert!(text.starts_with(head));
        assert!(font.advance(head) + dots <= max, "{max}");
        // One more character would not fit.
        if let Some(next) = text[head.len()..].chars().next() {
            let longer = &text[..head.len() + next.len_utf8()];
            assert!(font.advance(longer) + dots > max, "{max}");
        }
    }
    assert_eq!(fit_with_ellipsis(&font, text, dots - 1), ("", true));
}

// ---- chrome ------------------------------------------------------------------------------------------------------------------

#[test]
fn the_layout_follows_the_chrome_table() {
    let l = ChromeLayout::new(Rect::new(10, 20, 300, 200));
    assert_eq!(l.outer, Rect::new(10, 20, 300, 200));
    assert_eq!(l.extent, Rect::new(7, 17, 306, 206));
    assert_eq!(l.title_bar, Rect::new(10, 20, 300, 22));
    assert_eq!(l.buttons[2], Rect::new(284, 22, 18, 18));
    assert_eq!(l.buttons[2].x1, 10 + 300 - 8);
    assert_eq!(l.buttons[1], Rect::new(264, 22, 18, 18));
    assert_eq!(l.buttons[0], Rect::new(244, 22, 18, 18));
    assert_eq!(l.button(Control::Close), l.buttons[2]);
    assert_eq!(l.button(Control::Minimise), l.buttons[0]);
    assert_eq!(l.title_text, Rect::new(18, 24, 244 - 8 - 18, 14));
    assert_eq!(l.rule, Rect::new(10, 42, 300, 1));
    assert_eq!(l.content, Rect::new(10, 43, 300, 177));

    let l = ChromeLayout::new(Rect::new(-5, 0, 120, 40));
    assert_eq!(l.buttons[2].x1, -5 + 120 - 8);
    assert_eq!(l.content, Rect::new(-5, 23, 120, 17));
    assert_eq!(l.extent, Rect::new(-8, -3, 126, 46));
    assert!(l.title_text.width() > 0);
    assert!(l.title_text.x1 + 8 <= l.buttons[0].x0);
}

#[test]
fn a_box_below_the_minimum_is_widened_to_it() {
    assert_eq!((MIN_WIDTH, MIN_HEIGHT, TITLE_HEIGHT), (120, 40, 22));
    let l = ChromeLayout::new(Rect::new(4, 5, 10, 10));
    assert_eq!(l.outer, Rect::new(4, 5, 120, 40));
    assert_eq!(l, ChromeLayout::new(Rect::new(4, 5, 120, 40)));
}

fn painted(depth: Depth, focused: bool, title: &str) -> (Surface, ChromeLayout, Theme) {
    let font = Font::parse(TITLE).unwrap();
    let t = Theme::afterglow(depth);
    let mut s = Surface::new(400, 300, SENTINEL);
    let l = ChromeLayout::new(Rect::new(50, 60, 200, 120));
    paint_chrome(&mut s, &t, &font, &l, title, focused);
    (s, l, t)
}

// The pixels `n` outside each edge of the outer box (the middle of each side and the four corners).
fn outside(l: &ChromeLayout, n: i32) -> Vec<(i32, i32)> {
    let o = l.outer.inflate(n - 1);
    let (cx, cy) = ((l.outer.x0 + l.outer.x1) / 2, (l.outer.y0 + l.outer.y1) / 2);
    vec![
        (cx, o.y0 - 1),
        (cx, o.y1),
        (o.x0 - 1, cy),
        (o.x1, cy),
        (o.x0 - 1, o.y0 - 1),
        (o.x1, o.y0 - 1),
        (o.x0 - 1, o.y1),
        (o.x1, o.y1),
    ]
}

#[test]
fn the_focused_halo_has_three_rings_and_nothing_at_four() {
    for depth in [Depth::Bits8, Depth::Bits4] {
        let (s, l, t) = painted(depth, true, "Notes");
        let want = [t.slot(Role::Accent), t.slot(Role::Halo2), t.slot(Role::Halo3), SENTINEL];
        for (i, want) in want.into_iter().enumerate() {
            for (x, y) in outside(&l, i as i32 + 1) {
                assert_eq!(s.get(x, y), Some(want), "{depth:?} ring {} at ({x}, {y})", i + 1);
            }
        }
    }
}

#[test]
fn the_unfocused_window_has_a_hairline_and_nothing_further_out() {
    for depth in [Depth::Bits8, Depth::Bits4] {
        let (s, l, t) = painted(depth, false, "Notes");
        let want = [t.slot(Role::Line), SENTINEL, SENTINEL, SENTINEL];
        for (i, want) in want.into_iter().enumerate() {
            for (x, y) in outside(&l, i as i32 + 1) {
                assert_eq!(s.get(x, y), Some(want), "{depth:?} ring {} at ({x}, {y})", i + 1);
            }
        }
    }
}

#[test]
fn the_title_bar_rule_and_content_have_their_colours() {
    for focused in [true, false] {
        let (s, l, t) = painted(Depth::Bits8, focused, "Notes");
        let o = l.outer;
        assert_eq!(s.get(o.x0, o.y0), Some(t.slot(Role::Chrome)));
        assert_eq!(s.get(o.x1 - 1, o.y0 + 21), Some(t.slot(Role::Chrome)));
        assert_eq!(s.get(o.x0, o.y0 + 22), Some(t.slot(Role::Line)));
        assert_eq!(s.get(o.x1 - 1, o.y0 + 22), Some(t.slot(Role::Line)));
        assert_eq!(s.get(o.x0, o.y0 + 23), Some(t.slot(Role::Content)));
        assert_eq!(s.get(o.x1 - 1, o.y1 - 1), Some(t.slot(Role::Content)));
        // Every pixel of the outer box is one of the four chrome colours or an ink.
        let inks = [Role::Text, Role::TextSecondary, Role::TextMuted].map(|r| t.slot(r));
        let flat = [Role::Chrome, Role::Line, Role::Content].map(|r| t.slot(r));
        for y in o.y0..o.y1 {
            for x in o.x0..o.x1 {
                let p = s.get(x, y).unwrap();
                assert!(flat.contains(&p) || inks.contains(&p), "({x}, {y}) is {p}");
            }
        }
    }
}

#[test]
fn the_titles_first_glyph_lands_at_x_plus_8_and_y_plus_4() {
    let font = Font::parse(TITLE).unwrap();
    for (focused, role) in [(true, Role::Text), (false, Role::TextSecondary)] {
        let (s, l, t) = painted(Depth::Bits8, focused, "Desk");
        let g = font.glyph('D').unwrap();
        let ink = t.slot(role);
        let mut set = 0;
        for gy in 0..g.height as u32 {
            for gx in 0..g.advance as u32 {
                let at = s.get(l.outer.x0 + 8 + gx as i32, l.outer.y0 + 4 + gy as i32);
                let want = if g.bit(gx, gy) { ink } else { t.slot(Role::Chrome) };
                assert_eq!(at, Some(want), "glyph pixel ({gx}, {gy})");
                set += g.bit(gx, gy) as u32;
            }
        }
        assert!(set > 0);
    }
}

#[test]
fn a_long_title_ends_in_an_ellipsis_before_the_control_boxes() {
    let font = Font::parse(TITLE).unwrap();
    let title = "A title far too long to fit in the bar of a small window";
    let (s, l, t) = painted(Depth::Bits8, true, title);
    let room = l.title_text.width();
    let (head, cut) = fit_with_ellipsis(&font, title, room);
    assert!(cut && !head.is_empty());
    // The title bar's text rows hold exactly `head` and `…` drawn from the title's origin.
    let mut want = Surface::new(400, 300, t.slot(Role::Chrome));
    let end = draw_text(&mut want, &font, l.title_text.x0, l.title_text.y0, head, t.slot(Role::Text));
    let end = draw_text(&mut want, &font, end, l.title_text.y0, "…", t.slot(Role::Text));
    for y in l.title_text.y0..l.title_text.y1 {
        for x in l.outer.x0..l.buttons[0].x0 {
            assert_eq!(s.get(x, y), want.get(x, y), "({x}, {y})");
        }
    }
    assert!(
        end <= l.buttons[0].x0 - 8,
        "the ellipsis ends {end}, the first box starts {}",
        l.buttons[0].x0
    );
    // No title ink within the 8 pixels before the first control box.
    for y in l.title_text.y0..l.title_text.y1 {
        for x in l.buttons[0].x0 - 8..l.buttons[0].x0 {
            assert_ne!(s.get(x, y), Some(t.slot(Role::Text)));
        }
    }
}

#[test]
fn the_control_icons_are_drawn_centred_in_each_box_in_the_icon_colour() {
    for (focused, role) in [(true, Role::TextSecondary), (false, Role::TextMuted)] {
        let (s, l, t) = painted(Depth::Bits8, focused, "");
        for b in l.buttons {
            let mut ink = 0;
            for y in b.y0..b.y1 {
                for x in b.x0..b.x1 {
                    let p = s.get(x, y).unwrap();
                    assert!(p == t.slot(Role::Chrome) || p == t.slot(role));
                    ink += (p == t.slot(role)) as u32;
                    if p == t.slot(role) {
                        let (cx, cy) = (x - b.x0 - 4, y - b.y0 - 4);
                        assert!((0..10).contains(&cx) && (0..10).contains(&cy));
                    }
                }
            }
            assert!(ink >= 8);
        }
    }
}

#[test]
fn painting_is_clipped_to_the_surfaces_clip() {
    let font = Font::parse(TITLE).unwrap();
    let t = Theme::afterglow(Depth::Bits8);
    let l = ChromeLayout::new(Rect::new(50, 60, 200, 120));
    let mut s = Surface::new(400, 300, SENTINEL);
    let clip = Rect::new(100, 70, 40, 40);
    s.set_clip(clip);
    paint_chrome(&mut s, &t, &font, &l, "Notes", true);
    for y in 0..300 {
        for x in 0..400 {
            if !clip.contains(x, y) {
                assert_eq!(s.get(x, y), Some(SENTINEL));
            }
        }
    }
}

// ---- widgets -------------------------------------------------------------------------------------------------------------------

fn key(code: KeyCode, modifiers: Modifiers) -> KeyEvent {
    KeyEvent {
        code,
        pressed: true,
        modifiers,
    }
}

fn field(max_chars: u16) -> Widget {
    Widget::TextLine(desk_ui::TextLine::new((10, 30), 120, max_chars))
}

fn text_of(w: &Widget) -> &str {
    match w {
        Widget::TextLine(f) => &f.text,
        Widget::Label(l) => &l.text,
    }
}

#[test]
fn letters_digits_and_space_append_and_shift_gives_capitals() {
    let font = Font::parse(UI).unwrap();
    let mut w = field(16);
    for (code, mods) in [
        (KeyCode::H, Modifiers::SHIFT),
        (KeyCode::I, Modifiers::NONE),
        (KeyCode::Space, Modifiers::NONE),
        (KeyCode::Num4, Modifiers::NONE),
        (KeyCode::Num2, Modifiers::NONE),
    ] {
        assert!(w.key(key(code, mods), &font).is_some(), "{code:?}");
    }
    assert_eq!(text_of(&w), "Hi 42");
}

#[test]
fn backspace_removes_one_character_and_does_nothing_on_an_empty_field() {
    let font = Font::parse(UI).unwrap();
    let mut w = field(16);
    assert_eq!(w.key(key(KeyCode::Backspace, Modifiers::NONE), &font), None);
    w.key(key(KeyCode::A, Modifiers::NONE), &font);
    w.key(key(KeyCode::B, Modifiers::NONE), &font);
    assert!(w.key(key(KeyCode::Backspace, Modifiers::NONE), &font).is_some());
    assert_eq!(text_of(&w), "a");
}

#[test]
fn a_key_at_max_chars_changes_nothing_and_returns_none() {
    let font = Font::parse(UI).unwrap();
    let mut w = field(2);
    w.key(key(KeyCode::A, Modifiers::NONE), &font);
    w.key(key(KeyCode::B, Modifiers::NONE), &font);
    assert_eq!(w.key(key(KeyCode::C, Modifiers::NONE), &font), None);
    assert_eq!(text_of(&w), "ab");
    // Backspace still works at the limit.
    assert!(w.key(key(KeyCode::Backspace, Modifiers::NONE), &font).is_some());
}

#[test]
fn keys_that_are_not_text_change_nothing() {
    let font = Font::parse(UI).unwrap();
    let mut w = field(8);
    let up = KeyEvent {
        code: KeyCode::A,
        pressed: false,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(w.key(up, &font), None);
    assert_eq!(w.key(key(KeyCode::A, Modifiers::CTRL), &font), None);
    assert_eq!(w.key(key(KeyCode::A, Modifiers::ALT), &font), None);
    assert_eq!(w.key(key(KeyCode::Enter, Modifiers::NONE), &font), None);
    assert_eq!(w.key(key(KeyCode::Left, Modifiers::NONE), &font), None);
    assert_eq!(text_of(&w), "");
    let mut label = Widget::Label(desk_ui::Label {
        at: (0, 0),
        text: "x".into(),
        role: Role::Text,
    });
    assert_eq!(label.key(key(KeyCode::A, Modifiers::NONE), &font), None);
}

#[test]
fn the_returned_rectangle_covers_the_old_and_new_text_and_caret_and_stays_in_the_field() {
    let font = Font::parse(UI).unwrap();
    let mut w = field(16);
    let Widget::TextLine(f) = &w else { unreachable!() };
    let bounds = f.bounds();
    let ink = |text: &str| {
        // The text cell and the caret as the field draws them (see FIRST-WINDOW.md "Widgets").
        let (x, y) = (10 + 6, 30);
        Rect::new(x, y + 3, font.advance(text), font.height()).union(&Rect::new(x + font.advance(text) as i32, y + 4, 1, 12))
    };
    let mut old = String::new();
    for (code, new) in [(KeyCode::W, "w"), (KeyCode::M, "wm"), (KeyCode::I, "wmi")] {
        let r = w.key(key(code, Modifiers::NONE), &font).unwrap();
        for part in [ink(&old), ink(new)] {
            assert_eq!(r.intersect(&part), part, "{old:?} to {new:?}");
        }
        assert_eq!(r.intersect(&bounds), r);
        old = new.to_string();
    }
    let r = w.key(key(KeyCode::Backspace, Modifiers::NONE), &font).unwrap();
    assert_eq!(r.intersect(&ink("wmi")), ink("wmi"));
    assert_eq!(r.intersect(&ink("wm")), ink("wm"));
}

#[test]
fn a_field_paints_its_outline_inside_text_and_a_caret_only_when_focused() {
    let font = Font::parse(UI).unwrap();
    let t = Theme::afterglow(Depth::Bits8);
    let mut w = field(16);
    w.key(key(KeyCode::O, Modifiers::NONE), &font);
    let origin = (100, 50);
    let (x, y) = (origin.0 + 10, origin.1 + 30);
    for focused in [true, false] {
        let mut s = Surface::new(300, 200, SENTINEL);
        w.paint(&mut s, &t, &font, origin, focused);
        assert_eq!(s.get(x, y), Some(t.slot(Role::Border)));
        assert_eq!(s.get(x + 119, y + 19), Some(t.slot(Role::Border)));
        assert_eq!(s.get(x + 1, y + 1), Some(t.slot(Role::Content)));
        assert_eq!(s.get(x - 1, y), Some(SENTINEL));
        assert_eq!(s.get(x + 120, y), Some(SENTINEL));
        let caret_x = x + 6 + font.advance("o") as i32;
        for dy in 0..12 {
            let p = s.get(caret_x, y + 4 + dy);
            assert_eq!(p == Some(t.slot(Role::Accent)), focused, "caret row {dy}");
        }
        assert!((1..19).any(|dy| (7..30).any(|dx| s.get(x + dx, y + dy) == Some(t.slot(Role::Text)))));
        assert_eq!(w.bounds(&font), Rect::new(10, 30, 120, 20));
    }
}

#[test]
fn a_label_draws_its_text_in_its_role_at_its_place_in_the_content_area() {
    let font = Font::parse(UI).unwrap();
    let t = Theme::afterglow(Depth::Bits8);
    let w = Widget::Label(desk_ui::Label {
        at: (8, 6),
        text: "Hello".into(),
        role: Role::TextSecondary,
    });
    assert_eq!(w.bounds(&font), Rect::new(8, 6, font.advance("Hello"), 14));
    let mut s = Surface::new(200, 60, 0);
    w.paint(&mut s, &t, &font, (20, 10), true);
    let mut want = Surface::new(200, 60, 0);
    draw_text(&mut want, &font, 28, 16, "Hello", t.slot(Role::TextSecondary));
    assert_eq!(s.pixels(), want.pixels());
}
