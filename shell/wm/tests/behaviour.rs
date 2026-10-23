// Host tests for desk-wm: hit testing, dragging and its clamps, the close box, keys and painting.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use desk_gfx::{Rect, Surface};
use desk_hal::{KeyCode, KeyEvent, Modifiers};
use desk_ui::{Control, Depth, Label, Role, TextLine, Theme, Widget};
use desk_wm::{Hit, Wm};

fn two_overlapping() -> (Wm, desk_wm::WindowId, desk_wm::WindowId) {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let (c, _) = open(&mut wm, "c", C);
    (wm, a, c)
}

// ---- hit testing ----------------------------------------------------------------------------------------------------------

#[test]
fn hits_name_each_part_of_a_window() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    // A: outer x 100..300, y 100..220; title bar y 100..122; boxes at y 102..120: minimise 234..252, maximise 254..272, close 274..292.
    assert_eq!(wm.hit(150, 105), Some((a, Hit::Title)));
    assert_eq!(wm.hit(100, 100), Some((a, Hit::Title)));
    assert_eq!(wm.hit(299, 121), Some((a, Hit::Title)));
    assert_eq!(wm.hit(233, 110), Some((a, Hit::Title)));
    assert_eq!(wm.hit(234, 110), Some((a, Hit::Control(Control::Minimise))));
    assert_eq!(wm.hit(251, 119), Some((a, Hit::Control(Control::Minimise))));
    assert_eq!(wm.hit(252, 110), Some((a, Hit::Title)));
    assert_eq!(wm.hit(254, 102), Some((a, Hit::Control(Control::Maximise))));
    assert_eq!(wm.hit(272, 110), Some((a, Hit::Title)));
    assert_eq!(wm.hit(274, 110), Some((a, Hit::Control(Control::Close))));
    assert_eq!(wm.hit(291, 119), Some((a, Hit::Control(Control::Close))));
    assert_eq!(wm.hit(292, 110), Some((a, Hit::Title)));
    assert_eq!(wm.hit(283, 120), Some((a, Hit::Title)));
    // The rule (y 122) and the content (y 123..220).
    assert_eq!(wm.hit(283, 122), Some((a, Hit::Content)));
    assert_eq!(wm.hit(150, 150), Some((a, Hit::Content)));
    assert_eq!(wm.hit(299, 219), Some((a, Hit::Content)));
    // The focused window's rings reach three pixels out.
    for (x, y) in [
        (99, 150),
        (97, 150),
        (300, 150),
        (302, 150),
        (150, 99),
        (150, 97),
        (150, 220),
        (150, 222),
        (97, 97),
        (302, 222),
    ] {
        assert_eq!(wm.hit(x, y), Some((a, Hit::Ring)), "({x}, {y})");
    }
    // Outside.
    for (x, y) in [(96, 150), (303, 150), (150, 96), (150, 223), (0, 0), (-1, -1), (639, 479)] {
        assert_eq!(wm.hit(x, y), None, "({x}, {y})");
    }
}

#[test]
fn an_unfocused_window_owns_only_its_hairline() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    open(&mut wm, "b", B);
    assert_eq!(wm.hit(99, 150), Some((a, Hit::Ring)));
    assert_eq!(wm.hit(300, 150), Some((a, Hit::Ring)));
    assert_eq!(wm.hit(98, 150), None);
    assert_eq!(wm.hit(302, 150), None);
}

#[test]
fn the_topmost_window_wins() {
    let (mut wm, a, c) = two_overlapping();
    // (250, 180) is in A's content and in C's content.
    assert_eq!(wm.hit(250, 180), Some((c, Hit::Content)));
    assert_eq!(wm.hit(150, 105), Some((a, Hit::Title)));
    // (198, 200) is in C's three-pixel ring (C is focused) and in A's content.
    assert_eq!(wm.hit(198, 200), Some((c, Hit::Ring)));
    // Raising A swaps the two.
    let mut d = damage();
    wm.focus(a, &mut d);
    assert_eq!(wm.hit(250, 180), Some((a, Hit::Content)));
    // C is unfocused now and owns only its one-pixel hairline, so (198, 200) is A's content.
    assert_eq!(wm.hit(198, 200), Some((a, Hit::Content)));
    assert_eq!(wm.hit(350, 262), Some((c, Hit::Content)));
}

// ---- pointer: focus, drag, close box -------------------------------------------------------------------------------------------

#[test]
fn a_press_focuses_and_raises_the_window_under_the_pointer() {
    let (mut wm, a, c) = two_overlapping();
    assert_eq!(wm.focused(), Some(c));
    let mut d = damage();
    // (150, 150) is in A's content only.
    wm.pointer(150, 150, LEFT, &mut d);
    assert_eq!(wm.focused(), Some(a));
    assert_eq!(wm.windows().last().unwrap().id, a);
    // A press on the desktop changes nothing.
    wm.pointer(150, 150, 0, &mut d);
    wm.pointer(20, 20, LEFT, &mut d);
    assert_eq!(wm.focused(), Some(a));
}

#[test]
fn press_motion_release_on_a_title_moves_the_window_by_the_motion() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let mut d = damage();
    wm.pointer(150, 105, LEFT, &mut d);
    wm.pointer(180, 145, LEFT, &mut d);
    assert_eq!(wm.window_mut(a).unwrap().outer, Rect::new(130, 140, 200, 120));
    wm.pointer(170, 135, LEFT, &mut d);
    assert_eq!(wm.window_mut(a).unwrap().outer, Rect::new(120, 130, 200, 120));
    wm.pointer(170, 135, 0, &mut d);
    // After the release the window stays where it is.
    wm.pointer(300, 300, 0, &mut d);
    assert_eq!(wm.window_mut(a).unwrap().outer, Rect::new(120, 130, 200, 120));
}

#[test]
fn a_drag_raises_and_focuses_a_window_below() {
    let (mut wm, a, _) = two_overlapping();
    let mut d = damage();
    wm.pointer(150, 105, LEFT, &mut d);
    wm.pointer(155, 107, LEFT, &mut d);
    assert_eq!(wm.focused(), Some(a));
    assert_eq!(wm.window_mut(a).unwrap().outer, Rect::new(105, 102, 200, 120));
}

#[test]
fn pressing_content_or_a_control_does_not_drag() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let mut d = damage();
    wm.pointer(150, 150, LEFT, &mut d);
    wm.pointer(250, 250, LEFT, &mut d);
    wm.pointer(250, 250, 0, &mut d);
    wm.pointer(240, 110, LEFT, &mut d);
    wm.pointer(300, 200, LEFT, &mut d);
    wm.pointer(300, 200, 0, &mut d);
    assert_eq!(wm.window_mut(a).unwrap().outer, A);
    assert!(d.is_empty());
}

// Where A's outer box is after pressing its title at (150, 105) and moving the pointer to `pointer`.
fn drag_to(pointer: (i32, i32)) -> Rect {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let mut d = damage();
    wm.pointer(150, 105, LEFT, &mut d);
    wm.pointer(pointer.0, pointer.1, LEFT, &mut d);
    wm.window_mut(a).unwrap().outer
}

#[test]
fn the_drag_is_clamped_at_all_four_screen_edges() {
    // Left: at least 24 pixels of the title bar stay on screen, so x1 >= 24 and x0 = 24 - 200.
    assert_eq!(drag_to((-5000, 105)), Rect::new(-176, 100, 200, 120));
    assert_eq!(drag_to((-5000, 105)).x1, 24);
    // Right: x0 <= 640 - 24.
    assert_eq!(drag_to((5000, 105)), Rect::new(616, 100, 200, 120));
    // Top: y0 >= 0.
    assert_eq!(drag_to((150, -5000)), Rect::new(100, 0, 200, 120));
    // Bottom: y0 <= 480 - 22.
    assert_eq!(drag_to((150, 5000)), Rect::new(100, 458, 200, 120));
    // Corners clamp both axes.
    assert_eq!(drag_to((-5000, -5000)), Rect::new(-176, 0, 200, 120));
    assert_eq!(drag_to((5000, 5000)), Rect::new(616, 458, 200, 120));
    // Just inside the limits nothing is clamped, and one step past them is.
    assert_eq!(drag_to((150 - 100 - 175, 105)), Rect::new(-175, 100, 200, 120));
    assert_eq!(drag_to((150 - 100 - 176, 105)), Rect::new(-176, 100, 200, 120));
    assert_eq!(drag_to((150 - 100 - 177, 105)), Rect::new(-176, 100, 200, 120));
    assert_eq!(drag_to((150 + 516, 105 + 358)), Rect::new(616, 458, 200, 120));
    assert_eq!(drag_to((150 + 517, 105 + 359)), Rect::new(616, 458, 200, 120));
}

#[test]
fn a_clamped_window_follows_the_pointer_back() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let mut d = damage();
    wm.pointer(150, 105, LEFT, &mut d);
    wm.pointer(-3000, 105, LEFT, &mut d);
    wm.pointer(200, 105, LEFT, &mut d);
    assert_eq!(wm.window_mut(a).unwrap().outer, Rect::new(150, 100, 200, 120));
}

#[test]
fn a_close_press_and_release_on_the_box_closes_the_window_and_focuses_the_new_top() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let (b, _) = open(&mut wm, "b", B);
    let mut d = damage();
    // B's close box: outer x 400..550, so 524..542 at y 52..70.
    wm.pointer(530, 60, LEFT, &mut d);
    assert!(wm.by_name("b").is_some(), "closing waits for the release");
    wm.pointer(530, 60, 0, &mut d);
    assert_eq!(wm.by_name("b"), None);
    assert_eq!(wm.focused(), Some(a));
    assert!(wm.window_mut(b).is_none());
    // The pointer may leave the box and come back before the release.
    wm.pointer(283, 110, LEFT, &mut d);
    wm.pointer(150, 150, LEFT, &mut d);
    wm.pointer(280, 115, LEFT, &mut d);
    wm.pointer(280, 115, 0, &mut d);
    assert_eq!(wm.by_name("a"), None);
    assert_eq!(wm.focused(), None);
}

#[test]
fn a_release_outside_the_close_box_does_not_close() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let mut d = damage();
    for release in [(150, 150), (283, 125), (272, 110), (292, 110), (700, 700)] {
        wm.pointer(283, 110, LEFT, &mut d);
        wm.pointer(release.0, release.1, LEFT, &mut d);
        wm.pointer(release.0, release.1, 0, &mut d);
        assert_eq!(wm.by_name("a"), Some(a), "released at {release:?}");
    }
    // The press is not remembered: a press elsewhere and a release over the box closes nothing.
    wm.pointer(150, 150, LEFT, &mut d);
    wm.pointer(283, 110, LEFT, &mut d);
    wm.pointer(283, 110, 0, &mut d);
    assert_eq!(wm.by_name("a"), Some(a));
}

#[test]
fn a_close_press_on_a_window_below_raises_it_and_closes_it_on_release() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let (b, _) = open(&mut wm, "b", B);
    let mut d = damage();
    wm.pointer(283, 110, LEFT, &mut d);
    assert_eq!(wm.focused(), Some(a));
    wm.pointer(283, 110, 0, &mut d);
    assert_eq!(wm.by_name("a"), None);
    assert_eq!(wm.focused(), Some(b));
}

#[test]
fn minimise_and_maximise_are_inert() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let mut d = damage();
    for x in [240, 260] {
        wm.pointer(x, 110, LEFT, &mut d);
        wm.pointer(x + 3, 112, LEFT, &mut d);
        wm.pointer(x + 3, 112, 0, &mut d);
    }
    assert_eq!(wm.window_mut(a).unwrap().outer, A);
    assert!(d.is_empty());
}

// ---- keys ------------------------------------------------------------------------------------------------------------------

fn with_field(wm: &mut Wm, id: desk_wm::WindowId) {
    let w = wm.window_mut(id).unwrap();
    w.widgets.push(Widget::TextLine(TextLine::new((10, 30), 120, 20)));
    w.focus_widget = Some(0);
}

fn text(wm: &mut Wm, id: desk_wm::WindowId) -> String {
    match &wm.window_mut(id).unwrap().widgets[0] {
        Widget::TextLine(f) => f.text.clone(),
        Widget::Label(_) => unreachable!(),
    }
}

#[test]
fn keys_reach_only_the_focused_windows_field() {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    let (b, _) = open(&mut wm, "b", B);
    with_field(&mut wm, a);
    with_field(&mut wm, b);
    let f = fonts();
    let mut d = damage();
    wm.key(key(KeyCode::X), &f, &mut d);
    wm.key(key(KeyCode::Y), &f, &mut d);
    assert_eq!((text(&mut wm, a), text(&mut wm, b)), (String::new(), "xy".into()));
    wm.focus(a, &mut d);
    wm.key(key(KeyCode::Z), &f, &mut d);
    assert_eq!((text(&mut wm, a), text(&mut wm, b)), ("z".into(), "xy".into()));
    // A key release changes nothing, and Backspace removes one character.
    wm.key(
        KeyEvent {
            pressed: false,
            ..key(KeyCode::Q)
        },
        &f,
        &mut d,
    );
    wm.key(key(KeyCode::Backspace), &f, &mut d);
    assert_eq!((text(&mut wm, a), text(&mut wm, b)), (String::new(), "xy".into()));
    wm.key(
        KeyEvent {
            modifiers: Modifiers::SHIFT,
            ..key(KeyCode::Q)
        },
        &f,
        &mut d,
    );
    assert_eq!(text(&mut wm, a), "Q");
}

#[test]
fn keys_go_nowhere_when_nothing_can_take_them() {
    let f = fonts();
    let mut d = damage();
    let mut wm = Wm::new(SCREEN);
    wm.key(key(KeyCode::A), &f, &mut d);
    assert!(d.is_empty());
    // A focused window with no focused widget, and one whose widget is a label, take nothing.
    let (a, _) = open(&mut wm, "a", A);
    let (b, _) = open(&mut wm, "b", B);
    with_field(&mut wm, a);
    wm.window_mut(b).unwrap().widgets.push(Widget::Label(Label {
        at: (4, 4),
        text: "hi".into(),
        role: Role::Text,
    }));
    wm.key(key(KeyCode::A), &f, &mut d);
    wm.window_mut(b).unwrap().focus_widget = Some(0);
    wm.key(key(KeyCode::A), &f, &mut d);
    wm.window_mut(b).unwrap().focus_widget = Some(9);
    wm.key(key(KeyCode::A), &f, &mut d);
    assert!(d.is_empty());
    assert_eq!(text(&mut wm, a), "");
    // After the last window closes no window is focused, and a key does nothing.
    wm.close(b, &mut d);
    wm.close(a, &mut d);
    let mut d = damage();
    wm.key(key(KeyCode::A), &f, &mut d);
    assert!(d.is_empty());
}

// ---- painting --------------------------------------------------------------------------------------------------------------

const SENTINEL: u8 = 250;

fn busy_wm() -> Wm {
    let mut wm = Wm::new(SCREEN);
    let (a, _) = open(&mut wm, "a", A);
    open(&mut wm, "c", C);
    let w = wm.window_mut(a).unwrap();
    w.widgets.push(Widget::Label(Label {
        at: (8, 8),
        text: "Name".into(),
        role: Role::Text,
    }));
    let mut f = TextLine::new((8, 30), 120, 20);
    f.text.push_str("hello");
    w.widgets.push(Widget::TextLine(f));
    wm
}

#[test]
fn paint_changes_no_pixel_outside_its_clip() {
    let wm = busy_wm();
    let (t, f) = (Theme::afterglow(Depth::Bits8), fonts());
    let clips = [
        Rect::new(120, 90, 150, 100),
        Rect::new(97, 97, 206, 126),
        Rect::new(190, 140, 30, 30),
        Rect::new(-20, -20, 60, 60),
        Rect::new(600, 440, 100, 100),
        Rect::new(0, 0, 640, 480),
    ];
    for clip in clips {
        let mut s = Surface::new(640, 480, SENTINEL);
        wm.paint(&mut s, clip, &t, &f);
        let visible = clip.intersect(&s.bounds());
        for y in 0..480 {
            for x in 0..640 {
                let v = s.get(x, y).unwrap();
                if visible.contains(x, y) {
                    assert_ne!(v, SENTINEL, "({x}, {y}) inside {clip:?} was left unpainted");
                } else {
                    assert_eq!(v, SENTINEL, "({x}, {y}) outside {clip:?} was painted");
                }
            }
        }
        assert_eq!(s.clip(), s.bounds(), "the surface's clip is put back");
    }
}

#[test]
fn painting_a_clip_gives_the_pixels_a_full_paint_gives_there() {
    let wm = busy_wm();
    let (t, f) = (Theme::afterglow(Depth::Bits8), fonts());
    let mut full = Surface::new(640, 480, SENTINEL);
    wm.paint(&mut full, SCREEN, &t, &f);
    for clip in [
        Rect::new(120, 90, 150, 100),
        Rect::new(190, 140, 30, 30),
        Rect::new(97, 97, 206, 126),
        Rect::new(380, 250, 30, 30),
    ] {
        let mut part = Surface::new(640, 480, SENTINEL);
        wm.paint(&mut part, clip, &t, &f);
        for y in clip.y0..clip.y1 {
            for x in clip.x0..clip.x1 {
                assert_eq!(part.get(x, y), full.get(x, y), "({x}, {y}) in {clip:?}");
            }
        }
    }
    // The desktop is `Desktop`, and the focused window (C, on top) paints over A where they overlap.
    assert_eq!(full.get(5, 5), Some(t.slot(Role::Desktop)));
    assert_eq!(full.get(250, 200), Some(t.slot(Role::Content)));
    assert_eq!(full.get(199, 200), Some(t.slot(Role::Accent)));
}
