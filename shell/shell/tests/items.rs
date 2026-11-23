// Retained fill and text items: what a window's content clip keeps out, and the cap on how many a window holds.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use desk_gfx::Rect;
use desk_shell::scenario::{apply, parse, RunError};
use desk_shell::MAX_ITEMS;
use desk_ui::{ChromeLayout, Depth, Role, Theme};

/// Where window A's content area is on the screen.
fn content() -> Rect {
    ChromeLayout::new(A).content
}

/// Whether every pixel outside `keep` is the same in `before` and `after`.
fn outside_equal(before: &[u8], after: &[u8], keep: Rect) -> bool {
    (0..H as i32).all(|y| {
        (0..W as i32).all(|x| {
            let i = (y as u32 * W + x as u32) as usize;
            keep.contains(x, y) || before[i] == after[i]
        })
    })
}

#[test]
fn a_huge_fill_changes_nothing_outside_the_content_area() {
    for depth in DEPTHS {
        let (mut s, mut p) = started(depth);
        s.open("a", A, "a");
        s.handle(to(20, 440));
        s.frame(&mut p).unwrap();
        let before = s.back().pixels().to_vec();
        assert!(s.fill("a", Rect::new(-50, -50, 10000, 10000), Role::Accent));
        s.frame(&mut p).unwrap();
        let after = s.back().pixels().to_vec();
        let c = content();
        assert!(
            outside_equal(&before, &after, c),
            "{depth:?}: a pixel outside the content area changed"
        );
        let slot = Theme::afterglow(depth).slot(Role::Accent);
        for y in c.y0..c.y1 {
            for x in c.x0..c.x1 {
                assert_eq!(s.back().get(x, y), Some(slot), "{depth:?}: ({x}, {y}) is not filled");
            }
        }
        assert_ne!(before, after);
    }
}

#[test]
fn text_and_fills_at_the_extremes_of_i32_do_not_panic_or_leave_the_content_area() {
    for depth in DEPTHS {
        let (mut s, mut p) = started(depth);
        s.open("a", A, "a");
        s.handle(to(20, 440));
        s.frame(&mut p).unwrap();
        let before = s.back().pixels().to_vec();
        for (x, y) in [
            (i32::MAX, 0),
            (0, i32::MAX),
            (i32::MIN, i32::MIN),
            (i32::MAX, i32::MAX),
            (i32::MIN, 0),
        ] {
            assert!(s.text("a", x, y, "x", Role::Text));
        }
        assert!(s.fill("a", Rect::new(i32::MAX, i32::MAX, u32::MAX, u32::MAX), Role::Accent));
        assert!(s.fill("a", Rect::new(i32::MIN, i32::MIN, u32::MAX, u32::MAX), Role::Accent));
        assert!(s.fill("a", Rect::new(5, 5, 0, 0), Role::Accent));
        s.frame(&mut p).unwrap();
        let after = s.back().pixels().to_vec();
        assert!(
            outside_equal(&before, &after, content()),
            "{depth:?}: a pixel outside the content area changed"
        );
    }
}

#[test]
fn an_overhanging_fill_and_text_stay_inside_the_content_area() {
    for depth in DEPTHS {
        let (mut s, mut p) = started(depth);
        s.open("a", A, "a");
        s.handle(to(20, 440));
        s.frame(&mut p).unwrap();
        let before = s.back().pixels().to_vec();
        assert!(s.fill("a", Rect::new(-10, -10, 30, 20), Role::Raised));
        assert!(s.fill("a", Rect::new(150, 80, i32::MAX as u32, u32::MAX), Role::Selection));
        assert!(s.text("a", -20, -5, "Text hanging off the top left", Role::Text));
        assert!(s.text("a", 190, 90, "Text hanging off the bottom right", Role::Text));
        s.frame(&mut p).unwrap();
        let after = s.back().pixels().to_vec();
        assert!(outside_equal(&before, &after, content()), "{depth:?}");
        assert_ne!(before, after);
    }
}

#[test]
fn items_for_a_window_that_is_not_there_are_refused() {
    let mut s = shell(Depth::Bits8);
    assert!(!s.fill("none", Rect::new(0, 0, 5, 5), Role::Accent));
    assert!(!s.text("none", 0, 0, "x", Role::Text));
}

#[test]
fn a_window_refuses_the_item_past_the_cap() {
    assert_eq!(MAX_ITEMS, 32);
    for depth in DEPTHS {
        let (mut s, mut p) = started(depth);
        s.open("a", A, "a");
        s.open("b", B, "b");
        for i in 0..MAX_ITEMS as i32 {
            let ok = if i % 2 == 0 {
                s.fill("a", Rect::new(i, i, 4, 4), Role::Accent)
            } else {
                s.text("a", i, i, "t", Role::Text)
            };
            assert!(ok, "item {i}");
        }
        assert!(!s.fill("a", Rect::new(0, 0, 4, 4), Role::Accent), "the 33rd fill");
        assert!(!s.text("a", 0, 0, "t", Role::Text), "the 33rd text");
        assert!(!s.label("a", 0, 0, "t"), "the 33rd label");
        assert!(!s.field("a", 0, 0, 50), "the 33rd field");
        let id = s.wm().by_name("a").unwrap();
        assert_eq!(s.wm().window_mut(id).unwrap().widgets.len(), MAX_ITEMS);
        // The cap is per window.
        assert!(s.fill("b", Rect::new(0, 0, 4, 4), Role::Accent));
        s.frame(&mut p).unwrap();
    }
}

#[test]
fn a_step_past_the_cap_says_so_and_a_missing_window_says_that() {
    let sc =
        parse("desk-scenario 1\nscreen 640 480\nopen w 0 0 200 100 \"w\"\nfill w 0 0 1 1 Accent\ntext w 0 0 Text \"t\"").unwrap();
    let mut s = shell(Depth::Bits8);
    for step in &sc.steps {
        apply(&mut s, step).unwrap();
    }
    for _ in 0..MAX_ITEMS - 2 {
        apply(&mut s, &sc.steps[1]).unwrap();
    }
    assert_eq!(apply(&mut s, &sc.steps[1]), Err(RunError::TooManyItems("w".into())));
    assert_eq!(apply(&mut s, &sc.steps[2]), Err(RunError::TooManyItems("w".into())));
    let mut empty = shell(Depth::Bits8);
    assert_eq!(apply(&mut empty, &sc.steps[1]), Err(RunError::UnknownWindow("w".into())));
}
