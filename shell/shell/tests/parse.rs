// The scenario parser: the design's example, each kind of malformed line, and what `type` expands to.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_gfx::Rect;
use desk_hal::{KeyCode, KeyEvent, Modifiers, MouseEvent};
use desk_shell::scenario::{key_code, parse, type_events, FrameName, ParseErrorKind as K, Scenario, Step};
use desk_shell::Event;
use desk_ui::Depth;

/// The first window's scenario, as `docs/FIRST-WINDOW.md` gives it.
const EXAMPLE: &str = r#"desk-scenario 1
screen 640 480
depths 8 4
open notes 60 60 300 180 "Notes"
label notes 12 12 "The first window."
open welcome 220 150 340 200 "Welcome to Desk"
label welcome 12 12 "Type something:"
field welcome 12 36 300
pointer 320 240
frame open
move 100 60
frame moved
expect-damage 8 456 2
expect-damage 4 608 2
pointer 120 70
press left
release left
frame focus
press left
move 40 30
release left
frame dragged
pointer 400 300
press left
release left
type "desk"
frame typed
pointer 543 161
press left
release left
frame closed
"#;

fn frame(name: &str) -> Step {
    Step::Frame(FrameName::new(name).unwrap())
}

fn mouse(dx: i16, dy: i16, buttons: u8) -> Step {
    Step::Event(Event::Pointer(MouseEvent {
        dx,
        dy,
        buttons,
        wheel: 0,
    }))
}

fn click() -> [Step; 2] {
    [mouse(0, 0, 1), mouse(0, 0, 0)]
}

/// A scenario with `body` after a valid header, `screen` and `depths`.
fn with(body: &str) -> String {
    format!("desk-scenario 1\nscreen 640 480\ndepths 8 4\n{body}\n")
}

/// The error of `text`: its line and kind.
fn err(text: &str) -> (u32, K) {
    let e = parse(text).expect_err("the text should not parse");
    (e.line, e.kind)
}

#[test]
fn accepts_the_designs_example_and_gives_its_steps() {
    let sc = parse(EXAMPLE).expect("the example parses");
    assert_eq!(sc.screen, (640, 480));
    assert_eq!(sc.depths, [Depth::Bits8, Depth::Bits4]);
    let mut want = vec![
        Step::Open {
            name: "notes".into(),
            outer: Rect::new(60, 60, 300, 180),
            title: "Notes".into(),
        },
        Step::Label {
            window: "notes".into(),
            x: 12,
            y: 12,
            text: "The first window.".into(),
        },
        Step::Open {
            name: "welcome".into(),
            outer: Rect::new(220, 150, 340, 200),
            title: "Welcome to Desk".into(),
        },
        Step::Label {
            window: "welcome".into(),
            x: 12,
            y: 12,
            text: "Type something:".into(),
        },
        Step::Field {
            window: "welcome".into(),
            x: 12,
            y: 36,
            width: 300,
        },
        Step::Event(Event::PointerTo(320, 240)),
        frame("open"),
        mouse(100, 60, 0),
        frame("moved"),
        Step::ExpectDamage {
            depth: Depth::Bits8,
            max_pixels: 456,
            max_rects: Some(2),
        },
        Step::ExpectDamage {
            depth: Depth::Bits4,
            max_pixels: 608,
            max_rects: Some(2),
        },
        Step::Event(Event::PointerTo(120, 70)),
    ];
    want.extend(click());
    want.push(frame("focus"));
    want.push(mouse(0, 0, 1));
    want.push(mouse(40, 30, 1));
    want.push(mouse(0, 0, 0));
    want.push(frame("dragged"));
    want.push(Step::Event(Event::PointerTo(400, 300)));
    want.extend(click());
    want.push(Step::Type("desk".into()));
    want.push(frame("typed"));
    want.push(Step::Event(Event::PointerTo(543, 161)));
    want.extend(click());
    want.push(frame("closed"));
    assert_eq!(sc.steps, want);
}

#[test]
fn comments_blank_lines_and_crlf_are_ignored() {
    let text =
        "# a scenario\r\n\r\ndesk-scenario 1  # the version\r\nscreen 320 200\r\n\r\n# nothing else\r\nframe a # captured\r\n";
    let sc = parse(text).unwrap();
    assert_eq!(sc.screen, (320, 200));
    assert_eq!(
        sc.depths,
        [Depth::Bits8, Depth::Bits4],
        "both depths when the file has no depths line"
    );
    assert_eq!(sc.steps, [frame("a")]);
    let one = parse("desk-scenario 1\nscreen 640 480\ndepths 4\nframe a\n").unwrap();
    assert_eq!(one.depths, [Depth::Bits4]);
}

#[test]
fn strings_take_escapes_and_keep_spaces_and_hashes() {
    let sc = parse(&with(r#"open w 0 0 100 50 "a \"b\" \\ #c  d""#)).unwrap();
    match &sc.steps[0] {
        Step::Open { title, .. } => assert_eq!(title, r#"a "b" \ #c  d"#),
        other => panic!("{other:?}"),
    }
}

#[test]
fn negative_coordinates_are_accepted() {
    let sc = parse(&with("open w -20 -5 100 50 \"T\"\npointer -3 7\nmove -4 -2")).unwrap();
    assert_eq!(
        sc.steps[0],
        Step::Open {
            name: "w".into(),
            outer: Rect::new(-20, -5, 100, 50),
            title: "T".into()
        }
    );
    assert_eq!(sc.steps[1], Step::Event(Event::PointerTo(-3, 7)));
    assert_eq!(sc.steps[2], mouse(-4, -2, 0));
}

#[test]
fn buttons_are_tracked_across_steps() {
    let sc = parse(&with(
        "press left\npress right\nmove 1 1\nrelease left\nrelease right\nrelease middle\npress middle",
    ))
    .unwrap();
    let buttons: Vec<u8> = sc
        .steps
        .iter()
        .map(|s| match s {
            Step::Event(Event::Pointer(m)) => m.buttons,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(buttons, [1, 3, 3, 2, 0, 0, 4]);
}

#[test]
fn key_steps_name_key_codes_and_track_modifiers() {
    let sc = parse(&with(
        "key LeftShift down\nkey A down\nkey A up\nkey LeftShift up\nkey Backspace down",
    ))
    .unwrap();
    let key = |code, pressed, modifiers| {
        Step::Event(Event::Key(KeyEvent {
            code,
            pressed,
            modifiers,
        }))
    };
    assert_eq!(
        sc.steps,
        [
            key(KeyCode::LeftShift, true, Modifiers::SHIFT),
            key(KeyCode::A, true, Modifiers::SHIFT),
            key(KeyCode::A, false, Modifiers::SHIFT),
            key(KeyCode::LeftShift, false, Modifiers::NONE),
            key(KeyCode::Backspace, true, Modifiers::NONE),
        ]
    );
    assert_eq!(key_code("Enter"), Some(KeyCode::Enter));
    assert_eq!(key_code("enter"), None);
    assert_eq!(key_code("Unknown"), None);
}

#[test]
fn a_modifier_stays_held_while_either_of_its_keys_is_down() {
    let sc = parse(&with(
        "key LeftShift down\nkey RightShift down\nkey RightShift up\nkey A down\nkey LeftShift up\nkey A up",
    ))
    .unwrap();
    let mods: Vec<Modifiers> = sc
        .steps
        .iter()
        .map(|s| match s {
            Step::Event(Event::Key(k)) => k.modifiers,
            other => panic!("{other:?}"),
        })
        .collect();
    let s = Modifiers::SHIFT;
    assert_eq!(mods, [s, s, s, s, Modifiers::NONE, Modifiers::NONE]);
}

#[test]
fn closed_windows_can_be_opened_again() {
    let text = with("open w 0 0 50 50 \"A\"\nclose w\nopen w 0 0 50 50 \"B\"\nfocus w");
    assert_eq!(parse(&text).unwrap().steps.len(), 4);
}

#[test]
fn each_malformed_line_is_an_error_naming_its_line() {
    // The body starts on line 4, after the header, `screen` and `depths`.
    let cases: &[(&str, K)] = &[
        ("frame ok\nbogus 1 2", K::UnknownCommand("bogus".into())),
        ("frame ok\nopen", K::MissingArgument("name")),
        ("frame ok\nopen w 1 2 3", K::MissingArgument("height")),
        ("frame ok\nopen w 1 2 3 4", K::MissingArgument("title")),
        ("frame ok\npointer 5", K::MissingArgument("y")),
        ("frame ok\nkey A", K::MissingArgument("down or up")),
        ("frame ok\ntype", K::MissingArgument("text")),
        ("frame ok\nframe", K::MissingArgument("name")),
        ("frame ok\npointer x 5", K::BadNumber("x".into())),
        ("frame ok\npointer 1.5 5", K::BadNumber("1.5".into())),
        ("frame ok\nmove 40000 0", K::BadNumber("40000".into())),
        ("frame ok\nopen w 1 2 -3 4 \"T\"", K::BadNumber("-3".into())),
        ("frame ok\nopen w 1 2 3 4 \"T", K::UnterminatedString),
        ("frame ok\nlabel w 1 2 \"ends with a backslash\\", K::UnterminatedString),
        ("frame ok\nopen w 1 2 3 4 \"T\\n\"", K::BadEscape('n')),
        ("frame ok\nopen w 1 2 3 4 \"T\"x", K::TextAfterString),
        ("frame ok\nopen w 1 2 3 4 T", K::ExpectedString("title")),
        ("frame ok\nopen \"w\" 1 2 3 4 \"T\"", K::ExpectedWord("name")),
        ("frame ok\nframe abcdefghi", K::BadName("abcdefghi".into())),
        ("frame ok\nframe Open", K::BadName("Open".into())),
        ("frame ok\nframe a_b", K::BadName("a_b".into())),
        ("frame ok\nopen Notes 1 2 3 4 \"T\"", K::BadName("Notes".into())),
        ("frame ok\nframe ok", K::DuplicateFrame("ok".into())),
        ("frame ok\nframe okay", K::FramePrefix("okay".into(), "ok".into())),
        ("frame okay\nframe ok", K::FramePrefix("ok".into(), "okay".into())),
        ("frame ok\nexpect-damage", K::MissingArgument("depth")),
        ("frame ok\nexpect-damage 8", K::MissingArgument("pixels")),
        ("frame ok\nexpect-damage 16 10", K::BadDepth("16".into())),
        ("frame ok\nexpect-damage 8 10 300", K::BadNumber("300".into())),
        ("frame ok\nscreen 100 100", K::ScreenAfterStep),
        ("frame ok\ndepths 8", K::DepthsAfterStep),
        ("frame ok\nkey Bogus down", K::BadKey("Bogus".into())),
        ("frame ok\nkey a down", K::BadKey("a".into())),
        ("frame ok\nkey A sideways", K::BadKeyState("sideways".into())),
        ("frame ok\npress forward", K::BadButton("forward".into())),
        ("frame ok\nclose nowhere", K::UnknownWindow("nowhere".into())),
        ("frame ok\nlabel nowhere 1 2 \"T\"", K::UnknownWindow("nowhere".into())),
        (
            "frame ok\nopen w 1 2 3 4 \"T\"\nopen w 1 2 3 4 \"T\"",
            K::DuplicateWindow("w".into()),
        ),
        (
            "frame ok\nopen w 1 2 3 4 \"T\"\nclose w\nfocus w",
            K::UnknownWindow("w".into()),
        ),
        ("frame ok\nframe two extra", K::ExtraArgument("extra".into())),
        ("frame ok\ntype \"caf\u{e9}\"", K::UntypeableChar('\u{e9}')),
        ("frame ok\ntype \"tab\there\"", K::UntypeableChar('\t')),
    ];
    for (body, kind) in cases {
        let (line, got) = err(&with(body));
        let expected_line = 4 + body.lines().count() as u32 - 1;
        assert_eq!((line, &got), (expected_line, kind), "for {body:?}");
    }
}

#[test]
fn the_header_screen_and_depths_lines_are_checked() {
    assert_eq!(err(""), (1, K::MissingHeader));
    assert_eq!(err("# only a comment\n"), (1, K::MissingHeader));
    assert_eq!(err("screen 640 480\n"), (1, K::MissingHeader));
    assert_eq!(err("# c\n\ndesk-scenario 2\n"), (3, K::BadVersion("2".into())));
    assert_eq!(err("desk-scenario\n"), (1, K::MissingArgument("version")));
    assert_eq!(err("desk-scenario 1 now\n"), (1, K::ExtraArgument("now".into())));
    assert_eq!(
        err("desk-scenario 1\nscreen 640 480\nscreen 640 480\n"),
        (3, K::Duplicate("screen"))
    );
    assert_eq!(err("desk-scenario 1\nscreen 640\n"), (2, K::MissingArgument("height")));
    assert_eq!(err("desk-scenario 1\nscreen 0 480\n"), (2, K::BadNumber("0".into())));
    assert_eq!(
        err("desk-scenario 1\nscreen 640 480\ndepths\n"),
        (3, K::MissingArgument("depth"))
    );
    assert_eq!(
        err("desk-scenario 1\nscreen 640 480\ndepths 8 8\n"),
        (3, K::BadDepth("8".into()))
    );
    assert_eq!(
        err("desk-scenario 1\nscreen 640 480\ndepths 8\ndepths 4\n"),
        (4, K::Duplicate("depths"))
    );
    assert_eq!(err("desk-scenario 1\nframe a\n"), (2, K::MissingScreen));
    assert_eq!(err("desk-scenario 1\n"), (1, K::MissingScreen));
}

#[test]
fn the_error_prints_its_line() {
    let e = parse("desk-scenario 1\nscreen 640 480\nfly\n").unwrap_err();
    assert!(e.to_string().starts_with("line 3: "), "{e}");
}

#[test]
fn frame_names_are_limited_to_eight_characters_of_the_set() {
    for good in ["a", "open", "0", "a-b-c-d-", "12345678", "-"] {
        assert_eq!(FrameName::new(good).unwrap().as_str(), good);
    }
    for bad in ["", "abcdefghi", "A", "a b", "a.b", "é", "a_b"] {
        assert!(FrameName::new(bad).is_none(), "{bad:?}");
    }
}

fn key(code: KeyCode, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key(KeyEvent {
        code,
        pressed,
        modifiers,
    })
}

#[test]
fn type_expands_to_key_events_with_shift_for_capitals() {
    let ev = type_events("aB").unwrap();
    assert_eq!(
        ev,
        [
            key(KeyCode::A, true, Modifiers::NONE),
            key(KeyCode::A, false, Modifiers::NONE),
            key(KeyCode::LeftShift, true, Modifiers::SHIFT),
            key(KeyCode::B, true, Modifiers::SHIFT),
            key(KeyCode::B, false, Modifiers::SHIFT),
            key(KeyCode::LeftShift, false, Modifiers::NONE),
        ]
    );
}

#[test]
fn type_covers_letters_digits_space_and_punctuation() {
    let ev = type_events("a 1.-").unwrap();
    let codes: Vec<KeyCode> = ev
        .iter()
        .step_by(2)
        .map(|e| if let Event::Key(k) = e { k.code } else { unreachable!() })
        .collect();
    assert_eq!(
        codes,
        [KeyCode::A, KeyCode::Space, KeyCode::Num1, KeyCode::Period, KeyCode::Minus]
    );
    assert!(ev
        .iter()
        .all(|e| matches!(e, Event::Key(k) if k.modifiers == Modifiers::NONE)));
    // A shifted symbol takes Shift too, and every printable ASCII character can be typed.
    let ev = type_events("!").unwrap();
    assert_eq!(ev.len(), 4);
    assert_eq!(ev[1], key(KeyCode::Num1, true, Modifiers::SHIFT));
    for c in (0x20u8..0x7f).map(char::from) {
        assert!(type_events(&c.to_string()).is_ok(), "{c:?}");
    }
    assert_eq!(type_events("").unwrap(), []);
    assert_eq!(type_events("ab\n"), Err('\n'));
}

#[test]
fn a_typed_step_produces_the_characters_it_was_given() {
    let text = "Desk 486!";
    let mut typed = String::new();
    let mut mods = Modifiers::NONE;
    for e in type_events(text).unwrap() {
        let Event::Key(k) = e else { unreachable!() };
        mods = k.modifiers;
        if k.pressed {
            if let Some(c) = desk_kbd::char_for(k.code, k.modifiers) {
                typed.push(c);
            }
        }
    }
    assert_eq!(typed, text);
    assert_eq!(mods, Modifiers::NONE, "Shift is released at the end");
}

#[test]
fn the_scenario_type_is_public_data() {
    let sc: Scenario = parse(&with("type \"Hi\"")).unwrap();
    assert_eq!(sc.steps, [Step::Type("Hi".into())]);
}
