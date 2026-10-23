// The shell's behaviour as a whole: incremental frames equal full redraws after every step, the pointer and its buttons, and the
// scenario runner.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use desk_gfx::{FrameDigest, FrameStats, MemoryPresent};
use desk_shell::scenario::{apply, parse, run, RunError, Scenario, Step};
use desk_shell::Shell;
use desk_ui::Depth;
use std::cell::Cell;

/// Opens, focuses, drags, types and closes, with windows that overlap and a field under another window's edge.
const HAND_WRITTEN: &str = r#"desk-scenario 1
screen 640 480
open notes 60 60 300 180 "Notes"
label notes 12 12 "The first window."
open welcome 220 150 340 200 "Welcome to Desk"
label welcome 12 12 "Type something:"
field welcome 12 36 300
pointer 320 240
frame open
move 100 60
frame moved
focus notes
frame focus
pointer 120 70
press left
release left
frame click
press left
move 40 30
move 200 -100
move -400 300
release left
frame dragged
pointer 400 300
press left
release left
type "Desk 486!"
frame typed
key Backspace down
key Backspace up
key Backspace down
key Backspace up
frame erased
open third 500 20 120 60 "Third"
close notes
frame shut
pointer 603 31
press left
release left
frame gone
"#;

fn scenario() -> Scenario {
    parse(HAND_WRITTEN).unwrap()
}

/// The shell's front buffer after `steps`, from a second shell that damages the whole screen before every frame.
fn full_redraw(sc: &Scenario, depth: Depth, upto: usize) -> (MemoryPresent, FrameDigest) {
    let mut s = shell(depth);
    let mut p = present(depth);
    for step in &sc.steps[..upto] {
        apply(&mut s, step).unwrap();
    }
    s.damage_all();
    s.frame(&mut p).unwrap();
    let d = s.digest();
    (p, d)
}

#[test]
fn incremental_equals_full_after_every_step() {
    let sc = scenario();
    for depth in DEPTHS {
        let (mut inc, mut p) = started(depth);
        for (i, step) in sc.steps.iter().enumerate() {
            apply(&mut inc, step).unwrap();
            inc.frame(&mut p).unwrap();
            let (full, digest) = full_redraw(&sc, depth, i + 1);
            assert!(
                p.pixels == full.pixels,
                "{depth:?}: step {} ({step:?}) leaves the front buffer different from a full redraw",
                i + 1
            );
            assert_eq!(inc.digest(), digest, "{depth:?}: step {}", i + 1);
            assert_eq!(
                digest_of(&p, depth),
                digest,
                "{depth:?}: the digest is of what is on the screen"
            );
        }
    }
}

#[test]
fn the_scenario_does_what_it_says() {
    use desk_ui::Widget;
    let sc = scenario();
    let mut s = shell(Depth::Bits8);
    for step in &sc.steps {
        apply(&mut s, step).unwrap();
    }
    let names: Vec<_> = s.wm().windows().iter().map(|w| w.name.clone()).collect();
    assert_eq!(names, ["welcome"], "notes was closed by name and third by its close box");
    let w = &s.wm().windows()[0];
    let Widget::TextLine(f) = &w.widgets[1] else {
        panic!("the field")
    };
    assert_eq!(f.text, "Desk 48");
    assert_eq!(w.outer, desk_gfx::Rect::new(220, 150, 340, 200));
    // Up to the drag's frame, notes has been raised by the click and moved by the drag.
    let mut s = shell(Depth::Bits8);
    let at = sc
        .steps
        .iter()
        .position(|s| matches!(s, Step::Frame(n) if n.as_str() == "dragged"))
        .unwrap();
    for step in &sc.steps[..at] {
        apply(&mut s, step).unwrap();
    }
    let notes = s.wm().windows().last().unwrap();
    assert_eq!(notes.name, "notes");
    assert_ne!(notes.outer, desk_gfx::Rect::new(60, 60, 300, 180));
}

#[test]
fn the_comparison_catches_a_change_with_no_damage() {
    // A widget added behind the shell's back reports no damage, so the incremental picture misses it: this is the failure the
    // incremental-equals-full test exists to see.
    let (mut s, mut p) = started(Depth::Bits8);
    s.open("a", A, "a");
    s.frame(&mut p).unwrap();
    let id = s.wm().by_name("a").unwrap();
    s.wm()
        .window_mut(id)
        .unwrap()
        .widgets
        .push(desk_ui::Widget::TextLine(desk_ui::TextLine::new((10, 10), 100, 8)));
    assert_eq!(s.frame(&mut p).unwrap(), Default::default());
    s.damage_all();
    let mut q = present(Depth::Bits8);
    s.frame(&mut q).unwrap();
    assert!(p.pixels != q.pixels);
}

#[test]
fn depth_4_never_holds_an_index_above_15() {
    let sc = scenario();
    let (mut s, mut p) = started(Depth::Bits4);
    for step in &sc.steps {
        apply(&mut s, step).unwrap();
        s.frame(&mut p).unwrap();
        assert!(s.back().pixels().iter().all(|&b| b <= 15));
        assert!(p.pixels.iter().all(|&b| b <= 15));
    }
}

/// Runs `sc` and collects (name, digest) per captured frame.
fn digests(sc: &Scenario, depth: Depth) -> Vec<(String, FrameDigest)> {
    let mut s = shell(depth);
    let mut p = present(depth);
    let mut out = Vec::new();
    run(sc, depth, &mut s, &mut p, |name, sh, _| {
        out.push((name.as_str().to_string(), sh.digest()));
        Ok(())
    })
    .unwrap();
    assert_eq!(digest_of(&p, depth), s.digest());
    out
}

#[test]
fn two_runs_give_the_same_digests() {
    let sc = scenario();
    for depth in DEPTHS {
        let a = digests(&sc, depth);
        let b = digests(&sc, depth);
        assert_eq!(a.len(), 9);
        assert_eq!(a, b, "{depth:?}");
    }
    // The two depths draw different pictures.
    assert_ne!(digests(&sc, Depth::Bits8)[0].1, digests(&sc, Depth::Bits4)[0].1);
}

#[test]
fn pointer_and_pointer_to_agree() {
    for depth in DEPTHS {
        let moves = [
            mouse(0, 0, 0),
            mouse(30, -20, 0),
            mouse(0, 0, LEFT),
            mouse(40, 30, LEFT),
            mouse(-10, 5, LEFT),
            mouse(0, 0, 0),
        ];
        let (mut a, mut pa) = started(depth);
        let (mut b, mut pb) = started(depth);
        for s in [&mut a, &mut b] {
            s.open("a", A, "a");
            s.open("b", B, "b");
            s.handle(to(150, 105));
        }
        // `a` moves by deltas; `b` is told where each move ends, with the buttons from the report before it.
        for m in moves {
            a.handle(m);
            let (x, y) = a.pointer();
            b.handle(desk_shell::Event::Pointer(desk_hal::MouseEvent {
                dx: 0,
                dy: 0,
                buttons: match m {
                    desk_shell::Event::Pointer(m) => m.buttons,
                    _ => 0,
                },
                wheel: 0,
            }));
            b.handle(to(x, y));
            assert_eq!(a.pointer(), b.pointer());
            a.frame(&mut pa).unwrap();
            b.frame(&mut pb).unwrap();
            assert!(pa.pixels == pb.pixels, "{depth:?}");
            assert_eq!(a.digest(), b.digest());
        }
    }
}

#[test]
fn button_edges_not_levels_drive_presses() {
    // b opens first and a on top of it, apart: b is not focused.
    let setup = || {
        let (mut s, p) = started(Depth::Bits8);
        s.open("b", B, "b");
        s.open("a", A, "a");
        (s, p)
    };
    let a = |s: &mut Shell| s.wm().by_name("a");

    // The button goes down on the desktop and stays down while the pointer moves over b's title bar: no edge there, no press.
    let (mut s, _) = setup();
    s.handle(to(20, 440));
    s.handle(mouse(0, 0, LEFT));
    s.handle(to(450, 60));
    s.handle(mouse(0, 0, LEFT));
    s.handle(mouse(5, 5, LEFT));
    assert_eq!(
        s.wm().focused(),
        a(&mut s),
        "a held button arriving over a title bar does not focus or raise it"
    );
    s.handle(mouse(-100, 0, LEFT));
    assert_eq!(s.wm().windows()[1].outer, A, "and nothing is dragged");
    s.handle(mouse(0, 0, 0));
    // The release was not an edge on a window either; a fresh press is.
    s.handle(to(450, 60));
    s.handle(mouse(0, 0, LEFT));
    assert_eq!(s.wm().focused(), s.wm().by_name("b"));

    // A level repeated over the close box neither arms nor closes it; an edge on it, then release, closes.
    let (mut s, _) = setup();
    s.handle(to(20, 440));
    s.handle(mouse(0, 0, LEFT));
    s.handle(to(283, 111));
    s.handle(mouse(0, 0, 0));
    assert!(
        s.wm().by_name("a").is_some(),
        "releasing over a close box that was never pressed does not close"
    );
    s.handle(mouse(0, 0, LEFT));
    s.handle(mouse(0, 0, LEFT));
    assert!(s.wm().by_name("a").is_some(), "pressed is not closed");
    s.handle(mouse(0, 0, 0));
    assert!(s.wm().by_name("a").is_none());

    // Other buttons are levels the window manager ignores.
    let (mut s, _) = setup();
    s.handle(to(450, 60));
    s.handle(mouse(0, 0, desk_hal::MouseEvent::RIGHT));
    assert_eq!(s.wm().focused(), a(&mut s));
}

/// Counts the presenter's frames, so a test can tell when the capture callback ran relative to them.
struct Spy<'c> {
    inner: MemoryPresent,
    ended: &'c Cell<u32>,
}

impl desk_gfx::Present for Spy<'_> {
    fn width(&self) -> u32 {
        self.inner.width()
    }
    fn height(&self) -> u32 {
        self.inner.height()
    }
    fn align(&self) -> u32 {
        self.inner.align()
    }
    fn begin(&mut self) -> Result<(), desk_hal::HalError> {
        self.inner.begin()
    }
    fn span(&mut self, x: u32, y: u32, pixels: &[u8]) -> Result<(), desk_hal::HalError> {
        self.inner.span(x, y, pixels)
    }
    fn end(&mut self) -> Result<(), desk_hal::HalError> {
        self.ended.set(self.ended.get() + 1);
        self.inner.end()
    }
}

#[test]
fn run_captures_once_per_frame_step_after_presenting() {
    use desk_gfx::Present;
    let sc = scenario();
    let frames = sc.steps.iter().filter(|s| matches!(s, Step::Frame(_))).count();
    for depth in DEPTHS {
        let ended = Cell::new(0);
        let mut spy = Spy {
            inner: present(depth),
            ended: &ended,
        };
        let mut s = shell(depth);
        let mut seen = Vec::new();
        run(&sc, depth, &mut s, &mut spy, |name, _, stats: FrameStats| {
            // Every frame so far, this one included, has been presented and ended.
            seen.push((name.as_str().to_string(), ended.get(), stats.presented));
            Ok(())
        })
        .unwrap();
        assert_eq!(seen.len(), frames);
        // One captured frame per presented frame, in order, each seeing its own presented frame as finished.
        let counts: Vec<u32> = seen.iter().map(|s| s.1).collect();
        assert_eq!(counts, (1..=frames as u32).collect::<Vec<_>>());
        assert_eq!(seen[0].0, "open");
        assert_eq!(seen[0].2, W * H);
        assert_eq!(spy.inner.stats.frames, frames as u32);
        assert!(spy.width() == W);
    }
}

#[test]
fn run_stops_when_capture_fails() {
    let sc = scenario();
    let mut s = shell(Depth::Bits8);
    let mut p = present(Depth::Bits8);
    let mut n = 0;
    let r = run(&sc, Depth::Bits8, &mut s, &mut p, |_, _, _| {
        n += 1;
        Err(RunError::Capture("no".into()))
    });
    assert_eq!(r, Err(RunError::Capture("no".into())));
    assert_eq!(n, 1);
}

const EXPECT: &str = "desk-scenario 1\nscreen 640 480\npointer 320 240\nframe one\nmove 100 60\nframe two\nexpect-damage 8 456 2\nexpect-damage 4 608 2\n";

#[test]
fn run_checks_expect_damage_and_fails_one_that_is_exceeded() {
    for depth in DEPTHS {
        let sc = parse(EXPECT).unwrap();
        let mut s = shell(depth);
        let mut p = present(depth);
        run(&sc, depth, &mut s, &mut p, |_, _, _| Ok(())).unwrap();

        // One pixel less than the move writes, at the depth's own step, fails; the step for the other depth is skipped.
        let tight = if depth == Depth::Bits8 { 455 } else { 607 };
        let text = EXPECT
            .replace(
                "expect-damage 8 456 2",
                &format!("expect-damage 8 {} 2", if depth == Depth::Bits8 { tight } else { 456 }),
            )
            .replace(
                "expect-damage 4 608 2",
                &format!("expect-damage 4 {} 2", if depth == Depth::Bits4 { tight } else { 608 }),
            );
        let mut s = shell(depth);
        let mut p = present(depth);
        let err = run(&parse(&text).unwrap(), depth, &mut s, &mut p, |_, _, _| Ok(())).unwrap_err();
        assert_eq!(
            err,
            RunError::DamageExceeded {
                depth,
                pixels: tight + 1,
                max_pixels: tight,
                rects: 2,
                max_rects: Some(2)
            }
        );
        assert!(err.to_string().contains("wrote"));

        // The rectangle count is checked too.
        let text = EXPECT.replace("456 2", "456 1").replace("608 2", "608 1");
        let mut s = shell(depth);
        let mut p = present(depth);
        assert!(matches!(
            run(&parse(&text).unwrap(), depth, &mut s, &mut p, |_, _, _| Ok(())),
            Err(RunError::DamageExceeded {
                rects: 2,
                max_rects: Some(1),
                ..
            })
        ));
    }
}

#[test]
fn run_reports_unknown_windows_and_a_misplaced_expect() {
    // The parser refuses these, so the steps are built by hand.
    let mut sc = scenario();
    sc.steps = vec![Step::Close("nope".into())];
    let mut s = shell(Depth::Bits8);
    let mut p = present(Depth::Bits8);
    assert_eq!(
        run(&sc, Depth::Bits8, &mut s, &mut p, |_, _, _| Ok(())),
        Err(RunError::UnknownWindow("nope".into()))
    );
    sc.steps = vec![Step::ExpectDamage {
        depth: Depth::Bits8,
        max_pixels: 1,
        max_rects: None,
    }];
    assert_eq!(
        run(&sc, Depth::Bits8, &mut s, &mut p, |_, _, _| Ok(())),
        Err(RunError::NoFrame)
    );
    sc.steps = vec![Step::Type("é".into())];
    assert_eq!(
        run(&sc, Depth::Bits8, &mut s, &mut p, |_, _, _| Ok(())),
        Err(RunError::Untypable('é'))
    );
}

#[test]
fn a_presenter_of_the_wrong_size_is_refused_before_anything_is_painted() {
    let mut s = shell(Depth::Bits8);
    let mut p = MemoryPresent::new(320, 200, 1);
    assert_eq!(s.frame(&mut p), Err(desk_hal::HalError::InvalidArgument));
    assert!(Shell::new(0, 10, Depth::Bits8, fonts()).is_err());
    assert!(Shell::new(1281, 10, Depth::Bits8, fonts()).is_err());
}
