// The scenario format: a text file of steps (windows opened, input events, frames captured, damage bounds) parsed into a `Scenario`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The format is `docs/FIRST-WINDOW.md` ("The scenario format"). The parser checks everything a later stage would trip over, so that
// a bad scenario is an error naming its line and never a wrong picture: names fit DOS's 8.3 and no frame name is a prefix of another
// (the native run waits for `shell: ready NAME` by substring), key names are `KeyCode` names, and the text of a `type` step can be
// typed. The parser is pure: it reads a string and builds a value.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use desk_gfx::Rect;
use desk_hal::{KeyCode, KeyEvent, Modifiers, MouseEvent};
use desk_kbd::char_for;
use desk_ui::Depth;

use crate::Event;

/// A parsed scenario file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    /// The screen size in pixels.
    pub screen: (u32, u32),
    /// The depths to run at, in the order the file lists them (both when the file has no `depths` line).
    pub depths: Vec<Depth>,
    pub steps: Vec<Step>,
}

/// One step of a scenario.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Open a window with this outer box and title; it is focused and on top.
    Open {
        name: String,
        outer: Rect,
        title: String,
    },
    /// A label in a window's content area, at `(x, y)` from the content's top-left.
    Label {
        window: String,
        x: i32,
        y: i32,
        text: String,
    },
    /// A text field, which takes the window's keys.
    Field {
        window: String,
        x: i32,
        y: i32,
        width: u32,
    },
    Close(String),
    /// Focus and raise.
    Focus(String),
    Event(Event),
    /// Each character as key events (see `type_events`).
    Type(String),
    /// Capture the screen as this frame.
    Frame(FrameName),
    /// At `depth` only: the frame just presented wrote at most `max_pixels` pixels, in at most `max_rects` rectangles.
    ExpectDamage {
        depth: Depth,
        max_pixels: u32,
        max_rects: Option<u8>,
    },
}

/// The name of a captured frame: 1 to 8 characters of `a-z`, `0-9` and `-`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameName(String);

impl FrameName {
    /// The name, or `None` if it is not 1 to 8 characters of `a-z`, `0-9` and `-`.
    pub fn new(name: &str) -> Option<FrameName> {
        valid_name(name).then(|| FrameName(name.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FrameName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn valid_name(name: &str) -> bool {
    (1..=8).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// What was wrong with a line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// The first line is not `desk-scenario 1`.
    MissingHeader,
    /// The format version is not 1.
    BadVersion(String),
    UnknownCommand(String),
    /// The line ended before this argument.
    MissingArgument(&'static str),
    /// The line has more than its command takes.
    ExtraArgument(String),
    /// Text where a number is wanted, or a number out of range for its place.
    BadNumber(String),
    /// A bare word where a quoted string is wanted.
    ExpectedString(&'static str),
    /// A quoted string where a bare word is wanted.
    ExpectedWord(&'static str),
    UnterminatedString,
    /// A backslash followed by something other than `"` or `\`.
    BadEscape(char),
    /// A closing quote followed directly by more text.
    TextAfterString,
    /// Not 1 to 8 characters of `a-z`, `0-9` and `-`.
    BadName(String),
    DuplicateFrame(String),
    /// One frame name is the start of the other (the first string is the new one).
    FramePrefix(String, String),
    DuplicateWindow(String),
    /// A window name that is not open at this point.
    UnknownWindow(String),
    /// `screen` after a step.
    ScreenAfterStep,
    /// `depths` after a step.
    DepthsAfterStep,
    /// `screen` or `depths` a second time.
    Duplicate(&'static str),
    /// A step with no `screen` line before it (or no steps and no screen at all).
    MissingScreen,
    /// A depth that is not 8 or 4, or one listed twice.
    BadDepth(String),
    /// A key name that is not a `desk_hal::KeyCode` name.
    BadKey(String),
    /// Not `down` or `up`.
    BadKeyState(String),
    /// Not `left`, `right` or `middle`.
    BadButton(String),
    /// A character of a `type` step that no key types.
    UntypeableChar(char),
}

/// A malformed line: its number, counted from 1, and what is wrong with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: u32,
    pub kind: ParseErrorKind,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {:?}", self.line, self.kind)
    }
}

macro_rules! key_table {
    ($($name:ident),* $(,)?) => {
        &[$((stringify!($name), KeyCode::$name)),*]
    };
}

/// Every named key, in `KeyCode`'s order (so `Backslash` is found before `IntlBackslash`). `Unknown` has no name.
const KEYS: &[(&str, KeyCode)] = key_table!(
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Escape,
    Backspace,
    Tab,
    Enter,
    Space,
    Minus,
    Equals,
    LeftBracket,
    RightBracket,
    Backslash,
    Semicolon,
    Apostrophe,
    Grave,
    Comma,
    Period,
    Slash,
    IntlBackslash,
    CapsLock,
    NumLock,
    ScrollLock,
    LeftShift,
    RightShift,
    LeftCtrl,
    RightCtrl,
    LeftAlt,
    RightAlt,
    LeftGui,
    RightGui,
    Menu,
    Up,
    Down,
    Left,
    Right,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    PrintScreen,
    SysRq,
    Pause,
    Break,
    Kp0,
    Kp1,
    Kp2,
    Kp3,
    Kp4,
    Kp5,
    Kp6,
    Kp7,
    Kp8,
    Kp9,
    KpPeriod,
    KpPlus,
    KpMinus,
    KpStar,
    KpSlash,
    KpEnter,
);

/// The key a scenario names (`A`, `Backspace`, `Enter`, …), exactly as `KeyCode` spells it.
pub fn key_code(name: &str) -> Option<KeyCode> {
    KEYS.iter().find(|(n, _)| *n == name).map(|&(_, code)| code)
}

fn modifier_of(code: KeyCode) -> Modifiers {
    match code {
        KeyCode::LeftShift | KeyCode::RightShift => Modifiers::SHIFT,
        KeyCode::LeftCtrl | KeyCode::RightCtrl => Modifiers::CTRL,
        KeyCode::LeftAlt | KeyCode::RightAlt => Modifiers::ALT,
        KeyCode::LeftGui | KeyCode::RightGui => Modifiers::GUI,
        _ => Modifiers::NONE,
    }
}

/// The key and whether Shift is held that type `c` on a US layout, preferring the unshifted key.
fn key_for_char(c: char) -> Option<(KeyCode, bool)> {
    if !(c == ' ' || c.is_ascii_graphic()) {
        return None;
    }
    let find = |mods| {
        KEYS.iter()
            .map(|&(_, code)| code)
            .find(|&code| char_for(code, mods) == Some(c))
    };
    find(Modifiers::NONE)
        .map(|code| (code, false))
        .or_else(|| find(Modifiers::SHIFT).map(|code| (code, true)))
}

/// The key events that type `text`: for each character a key down and a key up, with Left Shift pressed around the key for a capital
/// or a shifted symbol. Each event's modifiers are the state after it, as the HAL reports them. `Err` holds the first character
/// that no key types (anything but printable ASCII).
pub fn type_events(text: &str) -> Result<Vec<Event>, char> {
    let mut events = Vec::new();
    let key = |code, pressed, modifiers| {
        Event::Key(KeyEvent {
            code,
            pressed,
            modifiers,
        })
    };
    for c in text.chars() {
        let (code, shift) = key_for_char(c).ok_or(c)?;
        let held = if shift { Modifiers::SHIFT } else { Modifiers::NONE };
        if shift {
            events.push(key(KeyCode::LeftShift, true, Modifiers::SHIFT));
        }
        events.push(key(code, true, held));
        events.push(key(code, false, held));
        if shift {
            events.push(key(KeyCode::LeftShift, false, Modifiers::NONE));
        }
    }
    Ok(events)
}

#[derive(Debug)]
enum Tok {
    Word(String),
    Str(String),
}

/// Splits a line into words and quoted strings; a `#` outside a string starts a comment.
fn tokenize(line: &str) -> Result<Vec<Tok>, ParseErrorKind> {
    let mut toks = Vec::new();
    let mut chars = line.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '#' {
            break;
        } else if c == '"' {
            chars.next();
            let mut s = String::new();
            loop {
                match chars.next() {
                    None => return Err(ParseErrorKind::UnterminatedString),
                    Some('"') => break,
                    Some('\\') => match chars.next() {
                        Some(e @ ('"' | '\\')) => s.push(e),
                        Some(e) => return Err(ParseErrorKind::BadEscape(e)),
                        None => return Err(ParseErrorKind::UnterminatedString),
                    },
                    Some(other) => s.push(other),
                }
            }
            if chars.peek().is_some_and(|n| !n.is_whitespace()) {
                return Err(ParseErrorKind::TextAfterString);
            }
            toks.push(Tok::Str(s));
        } else {
            let mut w = String::new();
            while let Some(&n) = chars.peek() {
                if n.is_whitespace() || n == '#' {
                    break;
                }
                w.push(n);
                chars.next();
            }
            toks.push(Tok::Word(w));
        }
    }
    Ok(toks)
}

type Kind = ParseErrorKind;

struct Args(alloc::vec::IntoIter<Tok>);

impl Args {
    fn word(&mut self, what: &'static str) -> Result<String, Kind> {
        match self.0.next() {
            Some(Tok::Word(w)) => Ok(w),
            Some(Tok::Str(_)) => Err(Kind::ExpectedWord(what)),
            None => Err(Kind::MissingArgument(what)),
        }
    }

    fn string(&mut self, what: &'static str) -> Result<String, Kind> {
        match self.0.next() {
            Some(Tok::Str(s)) => Ok(s),
            Some(Tok::Word(_)) => Err(Kind::ExpectedString(what)),
            None => Err(Kind::MissingArgument(what)),
        }
    }

    fn name(&mut self, what: &'static str) -> Result<String, Kind> {
        let w = self.word(what)?;
        if valid_name(&w) {
            Ok(w)
        } else {
            Err(Kind::BadName(w))
        }
    }

    fn number<T: core::str::FromStr>(&mut self, what: &'static str) -> Result<T, Kind> {
        let w = self.word(what)?;
        w.parse().map_err(|_| Kind::BadNumber(w))
    }

    /// A number of at least 1.
    fn positive(&mut self, what: &'static str) -> Result<u32, Kind> {
        let w = self.word(what)?;
        match w.parse::<u32>() {
            Ok(n) if n > 0 => Ok(n),
            _ => Err(Kind::BadNumber(w)),
        }
    }

    fn done(mut self) -> Result<(), Kind> {
        match self.0.next() {
            None => Ok(()),
            Some(Tok::Word(w) | Tok::Str(w)) => Err(Kind::ExtraArgument(w)),
        }
    }
}

fn depth_of(w: &str) -> Result<Depth, Kind> {
    match w {
        "8" => Ok(Depth::Bits8),
        "4" => Ok(Depth::Bits4),
        _ => Err(Kind::BadDepth(w.to_string())),
    }
}

struct Parser {
    screen: Option<(u32, u32)>,
    depths: Option<Vec<Depth>>,
    steps: Vec<Step>,
    windows: Vec<String>,
    frames: Vec<FrameName>,
    buttons: u8,
    modifiers: Modifiers,
}

impl Parser {
    fn window(&self, name: String) -> Result<String, Kind> {
        if self.windows.contains(&name) {
            Ok(name)
        } else {
            Err(Kind::UnknownWindow(name))
        }
    }

    fn button(&mut self, args: &mut Args, down: bool) -> Result<Step, Kind> {
        let w = args.word("button")?;
        let bit = match w.as_str() {
            "left" => MouseEvent::LEFT,
            "right" => MouseEvent::RIGHT,
            "middle" => MouseEvent::MIDDLE,
            _ => return Err(Kind::BadButton(w)),
        };
        self.buttons = if down { self.buttons | bit } else { self.buttons & !bit };
        Ok(Step::Event(Event::Pointer(MouseEvent {
            dx: 0,
            dy: 0,
            buttons: self.buttons,
            wheel: 0,
        })))
    }

    /// Parses one command line (`cmd` and what follows it) and records its effect.
    fn line(&mut self, cmd: &str, mut a: Args) -> Result<(), Kind> {
        match cmd {
            "screen" => {
                if !self.steps.is_empty() {
                    return Err(Kind::ScreenAfterStep);
                }
                if self.screen.is_some() {
                    return Err(Kind::Duplicate("screen"));
                }
                let (w, h) = (a.positive("width")?, a.positive("height")?);
                a.done()?;
                self.screen = Some((w, h));
            }
            "depths" => {
                if !self.steps.is_empty() {
                    return Err(Kind::DepthsAfterStep);
                }
                if self.depths.is_some() {
                    return Err(Kind::Duplicate("depths"));
                }
                let mut list = Vec::new();
                loop {
                    match a.0.next() {
                        None => break,
                        Some(Tok::Word(w)) | Some(Tok::Str(w)) => {
                            let d = depth_of(&w)?;
                            if list.contains(&d) {
                                return Err(Kind::BadDepth(w));
                            }
                            list.push(d);
                        }
                    }
                }
                if list.is_empty() {
                    return Err(Kind::MissingArgument("depth"));
                }
                self.depths = Some(list);
            }
            _ => {
                if self.screen.is_none() {
                    return Err(Kind::MissingScreen);
                }
                let step = self.step(cmd, &mut a)?;
                a.done()?;
                self.steps.push(step);
            }
        }
        Ok(())
    }

    fn step(&mut self, cmd: &str, a: &mut Args) -> Result<Step, Kind> {
        Ok(match cmd {
            "open" => {
                let name = a.name("name")?;
                let (x, y): (i32, i32) = (a.number("x")?, a.number("y")?);
                let (w, h): (u32, u32) = (a.number("width")?, a.number("height")?);
                let title = a.string("title")?;
                if self.windows.contains(&name) {
                    return Err(Kind::DuplicateWindow(name));
                }
                self.windows.push(name.clone());
                Step::Open {
                    name,
                    outer: Rect::new(x, y, w, h),
                    title,
                }
            }
            "label" => {
                let window = a.name("name")?;
                let (x, y): (i32, i32) = (a.number("x")?, a.number("y")?);
                let text = a.string("text")?;
                Step::Label {
                    window: self.window(window)?,
                    x,
                    y,
                    text,
                }
            }
            "field" => {
                let window = a.name("name")?;
                let (x, y): (i32, i32) = (a.number("x")?, a.number("y")?);
                let width: u32 = a.number("width")?;
                Step::Field {
                    window: self.window(window)?,
                    x,
                    y,
                    width,
                }
            }
            "close" => {
                let name = self.window(a.name("name")?)?;
                self.windows.retain(|w| *w != name);
                Step::Close(name)
            }
            "focus" => Step::Focus(self.window(a.name("name")?)?),
            "pointer" => {
                let (x, y): (i32, i32) = (a.number("x")?, a.number("y")?);
                Step::Event(Event::PointerTo(x, y))
            }
            "move" => {
                let (dx, dy): (i16, i16) = (a.number("dx")?, a.number("dy")?);
                Step::Event(Event::Pointer(MouseEvent {
                    dx,
                    dy,
                    buttons: self.buttons,
                    wheel: 0,
                }))
            }
            "press" => self.button(a, true)?,
            "release" => self.button(a, false)?,
            "key" => {
                let name = a.word("key")?;
                let code = key_code(&name).ok_or(Kind::BadKey(name))?;
                let state = a.word("down or up")?;
                let pressed = match state.as_str() {
                    "down" => true,
                    "up" => false,
                    _ => return Err(Kind::BadKeyState(state)),
                };
                let bit = modifier_of(code);
                self.modifiers = if pressed {
                    self.modifiers.union(bit)
                } else {
                    self.modifiers.without(bit)
                };
                Step::Event(Event::Key(KeyEvent {
                    code,
                    pressed,
                    modifiers: self.modifiers,
                }))
            }
            "type" => {
                let text = a.string("text")?;
                type_events(&text).map_err(Kind::UntypeableChar)?;
                Step::Type(text)
            }
            "frame" => {
                let name = a.name("name")?;
                for seen in &self.frames {
                    if seen.as_str() == name {
                        return Err(Kind::DuplicateFrame(name));
                    }
                    if seen.as_str().starts_with(name.as_str()) || name.starts_with(seen.as_str()) {
                        return Err(Kind::FramePrefix(name, seen.to_string()));
                    }
                }
                let frame = FrameName(name);
                self.frames.push(frame.clone());
                Step::Frame(frame)
            }
            "expect-damage" => {
                let depth = depth_of(&a.word("depth")?)?;
                let max_pixels: u32 = a.number("pixels")?;
                let max_rects = match a.0.next() {
                    None => None,
                    Some(Tok::Word(w)) => Some(w.parse::<u8>().map_err(|_| Kind::BadNumber(w))?),
                    Some(Tok::Str(s)) => return Err(Kind::BadNumber(s)),
                };
                Step::ExpectDamage {
                    depth,
                    max_pixels,
                    max_rects,
                }
            }
            other => return Err(Kind::UnknownCommand(other.to_string())),
        })
    }
}

/// Parses a scenario file. The first error stops the parse.
pub fn parse(text: &str) -> Result<Scenario, ParseError> {
    let mut parser = Parser {
        screen: None,
        depths: None,
        steps: Vec::new(),
        windows: Vec::new(),
        frames: Vec::new(),
        buttons: 0,
        modifiers: Modifiers::NONE,
    };
    let mut header = false;
    let mut last = 1;
    for (i, raw) in text.lines().enumerate() {
        let line = i as u32 + 1;
        last = line;
        let at = |kind| ParseError { line, kind };
        let toks = tokenize(raw).map_err(at)?;
        let mut it = toks.into_iter();
        let cmd = match it.next() {
            None => continue,
            Some(Tok::Word(w)) => w,
            Some(Tok::Str(s)) => return Err(at(Kind::UnknownCommand(s))),
        };
        let args = Args(it);
        if !header {
            if cmd != "desk-scenario" {
                return Err(at(Kind::MissingHeader));
            }
            let mut args = args;
            let version = args.word("version").map_err(at)?;
            if version != "1" {
                return Err(at(Kind::BadVersion(version)));
            }
            args.done().map_err(at)?;
            header = true;
            continue;
        }
        parser.line(&cmd, args).map_err(at)?;
    }
    if !header {
        return Err(ParseError {
            line: last,
            kind: Kind::MissingHeader,
        });
    }
    let screen = parser.screen.ok_or(ParseError {
        line: last,
        kind: Kind::MissingScreen,
    })?;
    let depths = parser.depths.unwrap_or_else(|| alloc::vec![Depth::Bits8, Depth::Bits4]);
    Ok(Scenario {
        screen,
        depths,
        steps: parser.steps,
    })
}
