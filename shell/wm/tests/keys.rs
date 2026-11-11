// Host tests for desk-wm's keyboard window management: Alt+Tab and Shift+Alt+Tab (release picks), Alt+F4, and Tab between fields,
// row by row of the key table in docs/LIVE-INPUT.md, with the damage each reports.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use desk_gfx::{Damage, Rect, Surface};
use desk_hal::{KeyCode, KeyEvent, Modifiers};
use desk_ui::{Depth, Label, Role, TextLine, Theme, Widget};
use desk_wm::{WindowId, Wm};

const ALT: Modifiers = Modifiers::ALT;
const SHIFT: Modifiers = Modifiers::SHIFT;

fn ev(code: KeyCode, pressed: bool, modifiers: Modifiers) -> KeyEvent {
    KeyEvent {
        code,
        pressed,
        modifiers,
    }
}

/// Sends `e` and returns the damage it reported.
fn send(wm: &mut Wm, e: KeyEvent) -> Damage {
    let mut d = damage();
    wm.key(e, &fonts(), &mut d);
    d
}

fn alt_down(wm: &mut Wm) -> Damage {
    send(wm, ev(KeyCode::LeftAlt, true, ALT))
}

fn alt_up(wm: &mut Wm) -> Damage {
    send(wm, ev(KeyCode::LeftAlt, false, Modifiers::NONE))
}

fn tab(wm: &mut Wm, mods: Modifiers) -> Damage {
    send(wm, ev(KeyCode::Tab, true, mods))
}

/// Three windows, a at the bottom, then b, then c focused.
fn three() -> (Wm, [WindowId; 3]) {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let (b, _) = open(&mut wm, "b", B);
    let (c, _) = open(&mut wm, "c", C);
    (wm, [a, b, c])
}

fn order(wm: &Wm) -> Vec<WindowId> {
    wm.windows().iter().map(|w| w.id).collect()
}

fn extent(r: Rect) -> Rect {
    r.inflate(3)
}

#[test]
fn alt_tab_draws_nothing_until_alt_is_released_and_then_focuses_the_target() {
    let (mut wm, [a, b, c]) = three();
    assert!(alt_down(&mut wm).is_empty());
    assert!(tab(&mut wm, ALT).is_empty(), "a step of the cycle damages nothing");
    assert_eq!(wm.cycling(), Some(b), "one window down the stacking order");
    assert_eq!(wm.focused(), Some(c), "nothing is focused yet");
    assert!(send(&mut wm, ev(KeyCode::Tab, false, ALT)).is_empty());
    let d = alt_up(&mut wm);
    // As a focus that raises: the raised window's extent and the previously focused window's.
    assert_eq!(rects(&d), rects_of(&[extent(B), extent(C)]));
    assert_eq!(order(&wm), [a, c, b]);
    assert_eq!(wm.focused(), Some(b));
    assert_eq!(wm.cycling(), None);
}

fn rects_of(rs: &[Rect]) -> Vec<Rect> {
    let mut d = damage();
    for r in rs {
        d.add(*r);
    }
    rects(&d)
}

#[test]
fn a_second_alt_tab_goes_back_to_the_window_that_was_focused() {
    let (mut wm, [a, b, c]) = three();
    alt_down(&mut wm);
    tab(&mut wm, ALT);
    alt_up(&mut wm);
    assert_eq!(order(&wm), [a, c, b]);
    alt_down(&mut wm);
    tab(&mut wm, ALT);
    alt_up(&mut wm);
    assert_eq!(order(&wm), [a, b, c], "the focus toggles between the two most recent windows");
}

#[test]
fn each_further_tab_goes_one_deeper_and_wraps() {
    let (mut wm, [a, b, c]) = three();
    alt_down(&mut wm);
    for want in [b, a, c, b] {
        tab(&mut wm, ALT);
        assert_eq!(wm.cycling(), Some(want));
    }
    alt_up(&mut wm);
    assert_eq!(wm.focused(), Some(b));
}

#[test]
fn shift_alt_tab_goes_the_other_way() {
    let (mut wm, [a, b, c]) = three();
    let sa = SHIFT.union(ALT);
    send(&mut wm, ev(KeyCode::LeftShift, true, SHIFT));
    alt_down(&mut wm);
    assert!(tab(&mut wm, sa).is_empty());
    assert_eq!(
        wm.cycling(),
        Some(a),
        "the bottom window, one back up from the top (wrapping)"
    );
    tab(&mut wm, sa);
    assert_eq!(wm.cycling(), Some(b));
    tab(&mut wm, ALT);
    assert_eq!(wm.cycling(), Some(a));
    alt_up(&mut wm);
    assert_eq!(order(&wm), [b, c, a]);
}

#[test]
fn alt_pressed_and_released_with_no_tab_changes_nothing() {
    let (mut wm, ids) = three();
    assert!(alt_down(&mut wm).is_empty());
    assert!(alt_up(&mut wm).is_empty());
    assert_eq!(order(&wm), ids);
    assert_eq!(wm.cycling(), None);
}

#[test]
fn alt_tab_with_one_window_changes_nothing() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    alt_down(&mut wm);
    assert!(tab(&mut wm, ALT).is_empty());
    assert_eq!(wm.cycling(), Some(a));
    assert!(alt_up(&mut wm).is_empty());
    assert_eq!(wm.focused(), Some(a));
    // And with none.
    let mut wm = Wm::new(SCREEN);
    alt_down(&mut wm);
    assert!(tab(&mut wm, ALT).is_empty());
    assert_eq!(wm.cycling(), None);
    assert!(alt_up(&mut wm).is_empty());
}

#[test]
fn escape_cancels_the_cycle() {
    let (mut wm, ids) = three();
    alt_down(&mut wm);
    tab(&mut wm, ALT);
    assert!(send(&mut wm, ev(KeyCode::Escape, true, ALT)).is_empty());
    assert_eq!(wm.cycling(), None);
    assert!(alt_up(&mut wm).is_empty());
    assert_eq!(order(&wm), ids);
}

#[test]
fn a_click_while_alt_is_held_ends_the_cycle_and_leaves_no_stale_index() {
    let (mut wm, [a, b, c]) = three();
    alt_down(&mut wm);
    tab(&mut wm, ALT);
    tab(&mut wm, ALT);
    assert_eq!(wm.cycling(), Some(a));
    // Click on b's title bar: b is focused by the click, and the cycle is gone.
    let mut d = damage();
    wm.pointer(450, 60, LEFT, &mut d);
    wm.pointer(450, 60, 0, &mut d);
    assert_eq!(wm.cycling(), None);
    assert_eq!(order(&wm), [a, c, b]);
    assert!(
        alt_up(&mut wm).is_empty(),
        "releasing Alt after the click does not pick the old target"
    );
    assert_eq!(order(&wm), [a, c, b]);
    // A new cycle starts from the top again.
    alt_down(&mut wm);
    tab(&mut wm, ALT);
    assert_eq!(wm.cycling(), Some(c));
    // A click that lands on nothing ends it too.
    wm.pointer(5, 5, LEFT, &mut d);
    assert_eq!(wm.cycling(), None);
}

#[test]
fn a_window_opened_or_closed_during_a_cycle_ends_it() {
    let (mut wm, [_, _, c]) = three();
    alt_down(&mut wm);
    tab(&mut wm, ALT);
    let mut d = damage();
    wm.close(c, &mut d);
    assert_eq!(wm.cycling(), None);
    assert!(alt_up(&mut wm).is_empty());
}

#[test]
fn releasing_one_alt_while_the_other_is_held_keeps_the_cycle() {
    let (mut wm, [_, b, _]) = three();
    send(&mut wm, ev(KeyCode::LeftAlt, true, ALT));
    send(&mut wm, ev(KeyCode::RightAlt, true, ALT));
    tab(&mut wm, ALT);
    send(&mut wm, ev(KeyCode::LeftAlt, false, ALT));
    assert_eq!(wm.cycling(), Some(b));
    alt_up(&mut wm);
    assert_eq!(wm.focused(), Some(b));
}

#[test]
fn alt_f4_closes_the_focused_window() {
    let (mut wm, [a, b, c]) = three();
    alt_down(&mut wm);
    let d = send(&mut wm, ev(KeyCode::F4, true, ALT));
    // As the close box: the extent of the window and of the one that becomes the top.
    assert_eq!(rects(&d), rects_of(&[extent(C), extent(B)]));
    assert_eq!(order(&wm), [a, b]);
    assert_eq!(wm.focused(), Some(b));
    assert!(wm.windows().iter().all(|w| w.id != c));
    // F4 with no Alt is the widget's, which ignores it.
    assert!(send(&mut wm, ev(KeyCode::F4, true, Modifiers::NONE)).is_empty());
    assert_eq!(order(&wm), [a, b]);
    // Closing the last windows leaves nothing and does not fail.
    for _ in 0..2 {
        send(&mut wm, ev(KeyCode::F4, false, ALT));
        send(&mut wm, ev(KeyCode::F4, true, ALT));
    }
    send(&mut wm, ev(KeyCode::F4, false, ALT));
    assert!(send(&mut wm, ev(KeyCode::F4, true, ALT)).is_empty());
    assert_eq!(wm.focused(), None);
}

#[test]
fn alt_f4_during_a_cycle_does_nothing() {
    let (mut wm, ids) = three();
    alt_down(&mut wm);
    tab(&mut wm, ALT);
    assert!(send(&mut wm, ev(KeyCode::F4, true, ALT)).is_empty());
    assert_eq!(order(&wm), ids);
    assert!(wm.cycling().is_some());
}

fn two_fields() -> (Wm, WindowId) {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let w = wm.window_mut(a).unwrap();
    w.widgets.push(Widget::TextLine(TextLine::new((10, 10), 150, 20)));
    w.widgets.push(Widget::Label(Label {
        at: (10, 40),
        text: "between".into(),
        role: Role::Text,
    }));
    w.widgets.push(Widget::TextLine(TextLine::new((10, 60), 150, 20)));
    w.focus_widget = Some(0);
    (wm, a)
}

fn focus_of(wm: &mut Wm, a: WindowId) -> Option<usize> {
    wm.window_mut(a).unwrap().focus_widget
}

fn text_of(wm: &mut Wm, a: WindowId, i: usize) -> String {
    match &wm.window_mut(a).unwrap().widgets[i] {
        Widget::TextLine(f) => f.text.clone(),
        _ => panic!("not a field"),
    }
}

#[test]
fn tab_moves_between_fields_skipping_labels_and_wrapping() {
    let (mut wm, a) = two_fields();
    // The content area starts at (100, 123); the carets are 1 × 12 at 4 down and 6 in after an empty text.
    let caret0 = Rect::new(100 + 10 + 6, 123 + 10 + 4, 1, 12);
    let caret2 = Rect::new(100 + 10 + 6, 123 + 60 + 4, 1, 12);
    let d = tab(&mut wm, Modifiers::NONE);
    assert_eq!(focus_of(&mut wm, a), Some(2), "the label is skipped");
    assert_eq!(rects(&d), rects_of(&[caret0, caret2]), "the two fields' carets");
    tab(&mut wm, Modifiers::NONE);
    assert_eq!(focus_of(&mut wm, a), Some(0), "wraps");
    send(&mut wm, ev(KeyCode::LeftShift, true, SHIFT));
    tab(&mut wm, SHIFT);
    assert_eq!(focus_of(&mut wm, a), Some(2), "Shift+Tab goes back, wrapping");
    tab(&mut wm, SHIFT);
    assert_eq!(focus_of(&mut wm, a), Some(0));
}

#[test]
fn typing_goes_to_the_field_tab_chose() {
    let (mut wm, a) = two_fields();
    send(&mut wm, key(KeyCode::A));
    tab(&mut wm, Modifiers::NONE);
    send(&mut wm, key(KeyCode::B));
    send(&mut wm, key(KeyCode::C));
    assert_eq!(text_of(&mut wm, a, 0), "a");
    assert_eq!(text_of(&mut wm, a, 2), "bc");
}

#[test]
fn tab_with_one_field_or_none_changes_nothing() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    assert!(tab(&mut wm, Modifiers::NONE).is_empty(), "no widgets");
    {
        let w = wm.window_mut(a).unwrap();
        w.widgets.push(Widget::TextLine(TextLine::new((10, 10), 150, 20)));
        w.focus_widget = Some(0);
    }
    assert!(tab(&mut wm, Modifiers::NONE).is_empty(), "the only field keeps the focus");
    assert!(tab(&mut wm, SHIFT).is_empty());
    assert_eq!(focus_of(&mut wm, a), Some(0));
}

#[test]
fn tab_with_ctrl_or_the_windows_key_is_the_widgets_not_the_focus_cycles() {
    let (mut wm, a) = two_fields();
    assert!(tab(&mut wm, Modifiers::CTRL).is_empty());
    assert!(tab(&mut wm, Modifiers::GUI).is_empty());
    assert_eq!(focus_of(&mut wm, a), Some(0));
    assert_eq!(text_of(&mut wm, a, 0), "");
}

#[test]
fn a_window_whose_focus_widget_is_unset_takes_the_first_field_on_tab() {
    let (mut wm, a) = two_fields();
    wm.window_mut(a).unwrap().focus_widget = None;
    tab(&mut wm, Modifiers::NONE);
    assert_eq!(focus_of(&mut wm, a), Some(0));
    wm.window_mut(a).unwrap().focus_widget = None;
    tab(&mut wm, SHIFT);
    assert_eq!(focus_of(&mut wm, a), Some(2));
}

#[test]
fn the_caret_is_drawn_only_in_the_focused_windows_focused_field() {
    let (mut wm, a) = two_fields();
    let (t, f) = (Theme::afterglow(Depth::Bits8), fonts());
    let accent = t.slot(Role::Accent);
    let caret0 = (100 + 10 + 6, 123 + 10 + 4 + 2);
    let caret2 = (100 + 10 + 6, 123 + 60 + 4 + 2);
    let shows = |wm: &Wm, at: (i32, i32)| {
        let mut s = Surface::new(640, 480, 0);
        wm.paint(&mut s, SCREEN, &t, &f);
        s.get(at.0, at.1) == Some(accent)
    };
    assert!(shows(&wm, caret0));
    assert!(!shows(&wm, caret2), "the window is focused but this field is not");
    tab(&mut wm, Modifiers::NONE);
    assert!(!shows(&wm, caret0));
    assert!(shows(&wm, caret2));
    // A window that is not focused shows neither caret.
    let (b, _) = open(&mut wm, "b", B);
    assert!(!shows(&wm, caret0) && !shows(&wm, caret2));
    let mut d = damage();
    wm.focus(a, &mut d);
    assert!(shows(&wm, caret2));
    assert_ne!(Some(b), wm.focused());
}

#[test]
fn other_keys_still_reach_the_focused_widget() {
    let (mut wm, a) = two_fields();
    let d = send(&mut wm, key(KeyCode::A));
    assert!(!d.is_empty());
    assert_eq!(text_of(&mut wm, a, 0), "a");
    let d = send(&mut wm, key(KeyCode::Backspace));
    assert!(!d.is_empty());
    // Escape with no cycle is the widget's, which ignores it.
    assert!(send(&mut wm, key(KeyCode::Escape)).is_empty());
}

#[test]
fn holding_alt_f4_closes_one_window_not_one_per_repeat() {
    let (mut wm, [a, b, _]) = three();
    alt_down(&mut wm);
    send(&mut wm, ev(KeyCode::F4, true, ALT));
    for _ in 0..5 {
        assert!(
            send(&mut wm, ev(KeyCode::F4, true, ALT)).is_empty(),
            "a typematic repeat closes nothing"
        );
    }
    assert_eq!(order(&wm), [a, b]);
    // After F4 comes up, the next press closes the next window.
    send(&mut wm, ev(KeyCode::F4, false, ALT));
    send(&mut wm, ev(KeyCode::F4, true, ALT));
    assert_eq!(order(&wm), [a]);
}

#[test]
fn alt_tab_during_a_drag_ends_the_drag_and_disarms_the_close_box() {
    let (mut wm, [a, _, c]) = three();
    let mut d = damage();
    // Press on C's title bar (a drag begins), then Alt+Tab to the window under it and release Alt.
    wm.pointer(300, 160, LEFT, &mut d);
    alt_down(&mut wm);
    tab(&mut wm, ALT);
    tab(&mut wm, ALT);
    alt_up(&mut wm);
    assert_eq!(wm.focused(), Some(a));
    let before = wm.windows().iter().find(|w| w.id == c).unwrap().outer;
    let mut d = damage();
    wm.pointer(340, 190, LEFT, &mut d);
    assert!(d.is_empty(), "the motion no longer drags the window that lost the focus");
    assert_eq!(wm.windows().iter().find(|w| w.id == c).unwrap().outer, before);
    // A close box armed before the focus moved does not close on release either.
    let (mut wm, [a, _, c]) = three();
    let close = Rect::new(C.x1 - 26 + 3, C.y0 + 2 + 3, 1, 1);
    wm.pointer(close.x0, close.y0, LEFT, &mut d);
    alt_down(&mut wm);
    tab(&mut wm, ALT);
    tab(&mut wm, ALT);
    alt_up(&mut wm);
    let mut d = damage();
    wm.pointer(close.x0, close.y0, 0, &mut d);
    assert!(wm.windows().iter().any(|w| w.id == c), "the armed close box was dropped");
    assert_eq!(wm.focused(), Some(a));
}
