// Host tests for desk-wm: the damage each change reports, one test per row of the design's table.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use desk_gfx::Rect;
use desk_hal::{KeyCode, MouseEvent};
use desk_ui::{TextLine, Widget};
use desk_wm::{WindowSpec, Wm};

#[test]
fn open_reports_the_new_windows_extent() {
    let mut wm = Wm::new(SCREEN);
    let mut d = damage();
    wm.open(
        WindowSpec {
            name: "a",
            outer: A,
            title: "A",
        },
        &mut d,
    );
    assert_eq!(rects(&d), [Rect::new(97, 97, 206, 126)]);
}

#[test]
fn open_over_another_window_also_repaints_the_one_that_loses_focus() {
    let mut wm = Wm::new(SCREEN);
    open(&mut wm, "a", A);
    let mut d = damage();
    wm.open(
        WindowSpec {
            name: "b",
            outer: B,
            title: "B",
        },
        &mut d,
    );
    assert_eq!(rects(&d), [Rect::new(397, 47, 156, 106), Rect::new(97, 97, 206, 126)]);
}

#[test]
fn close_of_a_window_that_is_not_focused_reports_its_extent() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    open(&mut wm, "b", B);
    let mut d = damage();
    wm.close(a, &mut d);
    assert_eq!(rects(&d), [Rect::new(97, 97, 206, 126)]);
}

#[test]
fn close_of_the_focused_window_reports_its_extent_and_the_new_tops() {
    let mut wm = Wm::new(SCREEN);
    open(&mut wm, "a", A);
    let (b, _) = open(&mut wm, "b", B);
    let mut d = damage();
    wm.close(b, &mut d);
    assert_eq!(rects(&d), [Rect::new(397, 47, 156, 106), Rect::new(97, 97, 206, 126)]);
    assert_eq!(wm.focused(), wm.by_name("a"));
}

#[test]
fn close_of_the_only_window_reports_its_extent_and_leaves_nothing_focused() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let mut d = damage();
    wm.close(a, &mut d);
    assert_eq!(rects(&d), [Rect::new(97, 97, 206, 126)]);
    assert_eq!(wm.focused(), None);
}

#[test]
fn focus_of_the_window_already_on_top_reports_nothing() {
    let mut wm = Wm::new(SCREEN);
    open(&mut wm, "a", A);
    let (b, _) = open(&mut wm, "b", B);
    let mut d = damage();
    wm.focus(b, &mut d);
    assert!(d.is_empty());
}

#[test]
fn focus_that_raises_reports_the_raised_extent_and_the_previous_focus() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    open(&mut wm, "b", B);
    let mut d = damage();
    wm.focus(a, &mut d);
    assert_eq!(rects(&d), [Rect::new(397, 47, 156, 106), Rect::new(97, 97, 206, 126)]);
    assert_eq!(wm.focused(), Some(a));
    // The two windows overlap: the merged damage is their bounding box.
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    open(&mut wm, "c", C);
    let mut d = damage();
    wm.focus(a, &mut d);
    assert_eq!(rects(&d), [Rect::new(97, 97, 306, 176)]);
}

#[test]
fn drag_reports_the_old_and_new_extents_merged() {
    let mut wm = Wm::new(SCREEN);
    open(&mut wm, "a", A);
    let mut d = damage();
    wm.pointer(150, 105, LEFT, &mut d);
    assert!(d.is_empty(), "pressing the title of the top window changes nothing");
    wm.pointer(160, 110, LEFT, &mut d);
    // (97, 97, 206, 126) and (107, 102, 206, 126) overlap.
    assert_eq!(rects(&d), [Rect::new(97, 97, 216, 131)]);
    // A move that leaves the old extent behind reports both.
    let mut d = damage();
    wm.pointer(400, 105, LEFT, &mut d);
    assert_eq!(rects(&d), [Rect::new(347, 97, 206, 126), Rect::new(107, 102, 206, 126)]);
    let mut d = damage();
    wm.pointer(400, 405, LEFT, &mut d);
    // The new extent is cut to the screen's 480 rows.
    assert_eq!(rects(&d), [Rect::new(347, 97, 206, 126), Rect::new(347, 397, 206, 83)]);
}

#[test]
fn a_drag_that_does_not_move_the_window_reports_nothing() {
    let mut wm = Wm::new(SCREEN);
    open(&mut wm, "a", A);
    let mut d = damage();
    wm.pointer(150, 105, LEFT, &mut d);
    wm.pointer(150, 105, LEFT, &mut d);
    // The window goes to the top edge (y 0), and a pointer further up leaves it there.
    wm.pointer(150, 5, LEFT, &mut d);
    let mut d = damage();
    wm.pointer(150, -50, LEFT, &mut d);
    assert!(d.is_empty());
}

#[test]
fn a_key_into_a_text_field_reports_the_fields_changed_part_on_screen() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let f = fonts();
    {
        let w = wm.window_mut(a).unwrap();
        w.widgets.push(Widget::TextLine(TextLine::new((10, 30), 120, 20)));
        w.focus_widget = Some(0);
    }
    let mut d = damage();
    wm.key(key(KeyCode::A), &f, &mut d);
    // The content area starts at (100, 123); the field at (10, 30) in it is at (110, 153). The text cell starts 6 in and 3 down, the
    // caret is 1 × 12 starting 4 down, before and after the character.
    let adv = f.ui.advance("a") as i32;
    let (x, y) = (110 + 6, 153 + 3);
    assert_eq!(rects(&d), [Rect::new(x, y, adv as u32 + 1, f.ui.height().max(13))]);
    // A key that changes nothing reports nothing.
    let mut d = damage();
    wm.key(key(KeyCode::Enter), &f, &mut d);
    assert!(d.is_empty());
}

#[test]
fn pointer_motion_with_no_drag_reports_nothing() {
    let mut wm = Wm::new(SCREEN);
    open(&mut wm, "a", A);
    open(&mut wm, "c", C);
    let mut d = damage();
    for (x, y) in [(10, 10), (150, 105), (250, 160), (283, 110), (700, 700), (-5, -5)] {
        wm.pointer(x, y, 0, &mut d);
    }
    assert!(d.is_empty());
    // A right-button press is not a left press.
    wm.pointer(150, 105, MouseEvent::RIGHT, &mut d);
    assert!(d.is_empty());
    assert_eq!(wm.focused(), wm.by_name("c"));
}
