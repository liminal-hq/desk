// The widgets of the first window: a one-line label and a one-line text field with a caret.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Positions are relative to the content area of the window the widget is in; `paint` takes the content area's top-left as `origin`.

use crate::text::draw_text;
use crate::theme::{Role, Theme};
use alloc::string::String;
use desk_font::Font;
use desk_gfx::{Rect, Surface};
use desk_hal::KeyEvent;
use desk_hal::{KeyCode, Modifiers};

/// The height of a text field, outline included.
pub const FIELD_HEIGHT: u32 = 20;
/// The height of the caret.
pub const CARET_HEIGHT: u32 = 12;
// Where the text starts inside the field's outline, and how far down the cell's top and the caret's top are.
const TEXT_INSET: i32 = 6;
const TEXT_TOP: i32 = 3;
const CARET_TOP: i32 = 4;

/// A line of text in a colour role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub at: (i32, i32),
    pub text: String,
    pub role: Role,
}

/// A field: a `Border` outline, `Content` inside, the text in `Text`, and a caret in `Accent` after the text while its window is
/// focused. It is `width` wide and `FIELD_HEIGHT` high at `at`, and holds at most `max_chars` characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextLine {
    pub at: (i32, i32),
    pub width: u32,
    pub text: String,
    pub max_chars: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Widget {
    Label(Label),
    TextLine(TextLine),
}

impl TextLine {
    pub fn new(at: (i32, i32), width: u32, max_chars: u16) -> TextLine {
        TextLine {
            at,
            width,
            text: String::new(),
            max_chars,
        }
    }

    pub fn bounds(&self) -> Rect {
        Rect::new(self.at.0, self.at.1, self.width, FIELD_HEIGHT)
    }

    // The cell of `text` drawn in the field, and the caret after it.
    fn cell_and_caret(&self, font: &Font, text: &str) -> (Rect, Rect) {
        let x = self.at.0 + TEXT_INSET;
        let w = font.advance(text).min(i32::MAX as u32);
        let cell = Rect::new(x, self.at.1 + TEXT_TOP, w, font.height());
        (cell, Rect::new(cell.x1, self.at.1 + CARET_TOP, 1, CARET_HEIGHT))
    }

    // Everything `text` and its caret cover, for the text as it is now or as it was.
    fn ink(&self, font: &Font, text: &str) -> Rect {
        let (cell, caret) = self.cell_and_caret(font, text);
        cell.union(&caret)
    }

    fn paint(&self, s: &mut Surface, t: &Theme, font: &Font, origin: (i32, i32), focused: bool) {
        let field = self.bounds().offset(origin.0, origin.1);
        let outer_clip = s.clip();
        s.fill(field, t.slot(Role::Border));
        let inner = field.inflate(-1);
        s.fill(inner, t.slot(Role::Content));
        s.set_clip(outer_clip.intersect(&inner));
        let (cell, caret) = self.cell_and_caret(font, &self.text);
        draw_text(
            s,
            font,
            cell.x0 + origin.0,
            cell.y0 + origin.1,
            &self.text,
            t.slot(Role::Text),
        );
        if focused {
            s.fill(caret.offset(origin.0, origin.1), t.slot(Role::Accent));
        }
        s.set_clip(outer_clip);
    }

    fn key(&mut self, e: KeyEvent, font: &Font) -> Option<Rect> {
        if !e.pressed {
            return None;
        }
        let before = self.ink(font, &self.text);
        if e.code == KeyCode::Backspace {
            self.text.pop()?;
        } else {
            let chord = Modifiers::CTRL.union(Modifiers::ALT).union(Modifiers::GUI);
            if e.modifiers.0 & chord.0 != 0 || self.text.chars().count() >= usize::from(self.max_chars) {
                return None;
            }
            let c = desk_kbd::char_for(e.code, e.modifiers)?;
            if c != ' ' && !c.is_ascii_graphic() {
                return None;
            }
            self.text.push(c);
        }
        let after = self.ink(font, &self.text);
        Some(before.union(&after).intersect(&self.bounds()))
    }
}

impl Widget {
    /// The rectangle the widget covers, relative to the content area: a label's text cell, a field's whole outline.
    pub fn bounds(&self, font: &Font) -> Rect {
        match self {
            Widget::Label(l) => Rect::new(l.at.0, l.at.1, font.advance(&l.text), font.height()),
            Widget::TextLine(f) => f.bounds(),
        }
    }

    /// Paints the widget with the content area's top-left at `origin`. `focused` is whether its window is the focused one.
    pub fn paint(&self, s: &mut Surface, t: &Theme, font: &Font, origin: (i32, i32), focused: bool) {
        match self {
            Widget::Label(l) => {
                draw_text(s, font, l.at.0 + origin.0, l.at.1 + origin.1, &l.text, t.slot(l.role));
            }
            Widget::TextLine(f) => f.paint(s, t, font, origin, focused),
        }
    }

    /// A key for the focused widget; `Some(rect relative to the content area)` when it changed, covering the old and new text and
    /// caret. A label takes no keys.
    pub fn key(&mut self, e: KeyEvent, font: &Font) -> Option<Rect> {
        match self {
            Widget::Label(_) => None,
            Widget::TextLine(f) => f.key(e, font),
        }
    }
}
