// Damage accounting on `MemoryPresent`, at both depths: what a frame writes for the first frame, an idle frame, a cursor move, a
// raise, a drag and a key.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use desk_gfx::{Damage, Rect, MAX_RECTS};
use desk_hal::KeyCode;
use desk_shell::scenario::type_events;
use desk_ui::Depth;

#[test]
fn the_first_frame_writes_exactly_the_whole_screen() {
    for depth in DEPTHS {
        let mut s = shell(depth);
        let mut p = present(depth);
        let stats = s.frame(&mut p).unwrap();
        assert_eq!(stats.presented, W * H, "{depth:?}");
        assert_eq!(stats.repainted, W * H, "{depth:?}");
        assert_eq!(p.stats.pixels, W * H);
        assert_eq!(p.stats.frames, 1);
    }
}

#[test]
fn a_frame_with_no_events_writes_nothing() {
    for depth in DEPTHS {
        let (mut s, mut p) = started(depth);
        let before = p.stats;
        let stats = s.frame(&mut p).unwrap();
        assert_eq!(stats, Default::default());
        assert_eq!(p.stats, before, "an idle frame calls neither begin nor span");
        // Events that change nothing are idle too: a pointer report with no motion, and a key up.
        s.handle(mouse(0, 0, 0));
        s.handle(to(s.pointer().0, s.pointer().1));
        assert_eq!(s.frame(&mut p).unwrap(), Default::default());
        assert_eq!(p.stats, before);
    }
}

#[test]
fn a_cursor_move_writes_the_two_sprite_rectangles_and_repaints_nothing() {
    for depth in DEPTHS {
        let (mut s, mut p) = started(depth);
        s.handle(to(320, 240));
        s.frame(&mut p).unwrap();
        let back = s.back().pixels().to_vec();
        let before = p.stats;
        s.handle(to(420, 300));
        let stats = s.frame(&mut p).unwrap();
        let apart = area_of(&[cursor_rect(320, 240), cursor_rect(420, 300)], depth);
        assert_eq!(
            apart,
            if depth == Depth::Bits8 { 456 } else { 608 },
            "{depth:?}: the sprite rectangles are 12 × 19 apart, widened to 8-pixel columns at depth 4"
        );
        assert_eq!(stats.presented, apart, "{depth:?}");
        assert_eq!(stats.rects, 2);
        assert_eq!(p.stats.pixels - before.pixels, apart);
        assert_eq!(stats.repainted, 0, "{depth:?}: a cursor move repaints nothing");
        assert_eq!(s.back().pixels(), &back[..], "the cursor is never in the back buffer");
    }
}

#[test]
fn a_cursor_move_that_overlaps_writes_the_bounding_box() {
    for depth in DEPTHS {
        let (mut s, mut p) = started(depth);
        s.handle(to(320, 240));
        s.frame(&mut p).unwrap();
        s.handle(to(325, 245));
        let stats = s.frame(&mut p).unwrap();
        let bbox = Rect::new(320, 240, 17, 24);
        assert_eq!(stats.presented, area_of(&[bbox], depth), "{depth:?}");
        if depth == Depth::Bits8 {
            assert_eq!(stats.presented, 17 * 24);
        }
        assert_eq!(stats.rects, 1);
        assert_eq!(stats.repainted, 0);
    }
}

#[test]
fn a_cursor_at_the_edge_is_clipped_and_the_pointer_stays_on_the_screen() {
    for depth in DEPTHS {
        let (mut s, mut p) = started(depth);
        s.handle(to(10_000, 10_000));
        assert_eq!(s.pointer(), (W as i32 - 1, H as i32 - 1));
        let stats = s.frame(&mut p).unwrap();
        // The sprite at (639, 479) shows one column and one row; the old rectangle is the centre's.
        let expected = area_of(&[cursor_rect(320, 240), Rect::new(639, 479, 1, 1)], depth);
        assert_eq!(stats.presented, expected, "{depth:?}");
        s.handle(to(-5, -5));
        assert_eq!(s.pointer(), (0, 0));
        s.handle(mouse(-100, -100, 0));
        assert_eq!(s.pointer(), (0, 0));
        s.handle(mouse(i16::MAX, i16::MAX, 0));
        s.handle(mouse(i16::MAX, i16::MAX, 0));
        s.handle(mouse(i16::MAX, i16::MAX, 0));
        assert_eq!(s.pointer(), (W as i32 - 1, H as i32 - 1));
        s.frame(&mut p).unwrap();
    }
}

/// A started shell with windows `a` and `b` open (b on top, focused) and the pointer parked on the empty desktop.
fn two_windows(depth: Depth, second: Rect) -> (desk_shell::Shell<'static>, desk_gfx::MemoryPresent) {
    let (mut s, mut p) = started(depth);
    s.open("a", A, "a");
    s.open("b", second, "b");
    s.handle(to(20, 440));
    s.frame(&mut p).unwrap();
    (s, p)
}

#[test]
fn a_focus_change_that_raises_writes_the_two_extents() {
    for depth in DEPTHS {
        for (second, overlap) in [(B, false), (C, true)] {
            let (mut s, mut p) = two_windows(depth, second);
            // Put the pointer on a's title bar, then press: the press raises a.
            s.handle(to(150, 105));
            s.frame(&mut p).unwrap();
            s.handle(mouse(0, 0, LEFT));
            let stats = s.frame(&mut p).unwrap();
            let (ea, eb) = (extent(A), extent(second));
            let al = align(depth);
            let exact = area_of(&[ea, eb], depth);
            assert_eq!(stats.presented, exact, "{depth:?} overlap={overlap}");
            if overlap {
                // Overlapping extents merge into their bounding box, (97, 97, 306, 176).
                assert_eq!(stats.rects, 1);
                assert_eq!(stats.presented, area_of(&[Rect::new(97, 97, 306, 176)], depth));
            } else {
                // At most the raised window's extent and the previous focus's (widened to the presenter's columns), nothing else.
                assert_eq!(stats.rects, 2);
                assert!(stats.presented <= ea.align_x(al).area() + eb.align_x(al).area());
                assert_eq!(
                    stats.repainted,
                    ea.area() + eb.area(),
                    "no pixel outside the two extents is repainted"
                );
            }
            assert_eq!(s.wm().focused(), s.wm().by_name("a"));
        }
    }
}

#[test]
fn pressing_the_window_that_is_already_on_top_writes_nothing() {
    for depth in DEPTHS {
        let (mut s, mut p) = two_windows(depth, B);
        s.handle(to(450, 60));
        s.frame(&mut p).unwrap();
        s.handle(mouse(0, 0, LEFT));
        assert_eq!(s.frame(&mut p).unwrap(), Default::default(), "{depth:?}");
    }
}

#[test]
fn a_drag_writes_the_old_and_new_extents_and_the_cursor() {
    for depth in DEPTHS {
        let (mut s, mut p) = started(depth);
        s.open("a", A, "a");
        s.handle(to(150, 105));
        s.frame(&mut p).unwrap();
        s.handle(mouse(0, 0, LEFT));
        assert_eq!(s.frame(&mut p).unwrap(), Default::default());
        for (dx, dy) in [(40i16, 30i16), (300, 5), (-200, 200)] {
            let (px, py) = s.pointer();
            let old = extent(s.wm().windows()[0].outer);
            s.handle(mouse(dx, dy, LEFT));
            let new = extent(s.wm().windows()[0].outer);
            let (nx, ny) = s.pointer();
            let stats = s.frame(&mut p).unwrap();
            let union = old.union(&new);
            let exact = area_of(&[old, new, cursor_rect(px, py), cursor_rect(nx, ny)], depth);
            assert_eq!(stats.presented, exact, "{depth:?} by ({dx}, {dy})");
            // The documented bound: the old and new extents (their bounding box when they overlap), plus the cursor's two
            // rectangles, each widened to the presenter's columns.
            let a = align(depth);
            let widen = |r: Rect| r.align_x(a);
            let extents = if old.overlaps(&new) {
                widen(union).area()
            } else {
                widen(old).area() + widen(new).area()
            };
            let bound = extents + widen(cursor_rect(px, py)).area() + widen(cursor_rect(nx, ny)).area();
            assert!(stats.presented <= bound, "{depth:?}: {} > {bound}", stats.presented);
        }
        s.handle(mouse(0, 0, 0));
    }
}

#[test]
fn a_key_into_a_field_writes_at_most_the_field() {
    for depth in DEPTHS {
        let (mut s, mut p) = started(depth);
        s.open("a", A, "a");
        s.label("a", 10, 10, "Type:");
        s.field("a", 10, 30, 120);
        s.handle(to(20, 440));
        s.frame(&mut p).unwrap();
        // The content area starts at (100, 123); the field is 120 × 20 at (10, 30) in it.
        let field = Rect::new(110, 153, 120, 20);
        let mut total = 0;
        for e in type_events("desk Z").unwrap() {
            s.handle(e);
            let stats = s.frame(&mut p).unwrap();
            assert!(stats.presented <= area_of(&[field], depth), "{depth:?}: {}", stats.presented);
            assert!(stats.rects <= 1);
            total += stats.presented;
        }
        assert!(total > 0, "the typed text reached the screen");
        // A key that changes nothing writes nothing: Enter, and a key release.
        s.handle(desk_shell::Event::Key(desk_hal::KeyEvent {
            code: KeyCode::Enter,
            pressed: true,
            modifiers: desk_hal::Modifiers::NONE,
        }));
        assert_eq!(s.frame(&mut p).unwrap(), Default::default());
    }
}

#[test]
fn opening_and_closing_write_the_extents() {
    for depth in DEPTHS {
        let (mut s, mut p) = started(depth);
        s.open("a", A, "a");
        let stats = s.frame(&mut p).unwrap();
        assert_eq!(stats.presented, area_of(&[extent(A)], depth));
        s.close("a");
        let stats = s.frame(&mut p).unwrap();
        assert_eq!(stats.presented, area_of(&[extent(A)], depth));
        assert_eq!(s.frame(&mut p).unwrap(), Default::default());
    }
}

#[test]
fn damage_holds_its_invariants_over_random_adds() {
    // A fixed-seed generator (a 32-bit LCG), so the run is the same everywhere.
    let mut state = 0x2545_f491u32;
    let mut next = |n: u32| {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 8) % n
    };
    let bounds = Rect::new(0, 0, W, H);
    for align in [1, 8] {
        let mut d = Damage::new(bounds);
        let mut added = Vec::new();
        for _ in 0..10_000 {
            if next(40) == 0 {
                d.clear();
                added.clear();
            }
            let r = Rect::new(next(700) as i32 - 30, next(520) as i32 - 20, next(120), next(90));
            d.add(r);
            added.push(r);
            let mut view = d;
            view.align_x(align);
            let rs = view.rects();
            assert!(rs.len() <= MAX_RECTS);
            for (i, a) in rs.iter().enumerate() {
                assert!(!a.is_empty() && a.intersect(&bounds) == *a);
                assert!(rs[i + 1..].iter().all(|b| !a.overlaps(b)), "disjoint");
            }
            if let Some(r) = added.last().map(|r| r.intersect(&bounds)).filter(|r| !r.is_empty()) {
                // The newest add is covered: every pixel on its corners and centre is inside some rectangle.
                for (x, y) in [(r.x0, r.y0), (r.x1 - 1, r.y1 - 1), ((r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2)] {
                    assert!(rs.iter().any(|q| q.contains(x, y)));
                }
            }
        }
    }
}
