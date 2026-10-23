// The shell: events in, the cursor and the window manager updated, damage collected, frames presented.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `Shell` owns everything a frame is a function of: the compositor (back buffer, damage, cursor), the window manager, the theme and
// the faces. It holds no platform type; a platform drains its input into `handle`, calls `frame` with its presenter, and reads
// `digest` for the frame's checksums. The window manager reports its damage to the caller, so every change that goes through the
// shell (events, and `open`, `close`, `focus`, `label` and `field`) hands that damage on to the compositor.

use crate::Event;
use alloc::string::String;
use desk_gfx::{Compositor, Damage, FrameDigest, FrameStats, Present, Rect, Surface};
use desk_hal::{HalError, KeyEvent};
use desk_ui::{palette, Depth, Label, Role, TextLine, Theme, Widget};
use desk_wm::{Fonts, WindowId, WindowSpec, Wm};

/// How many characters a scenario's text field holds, before its width stops further ones.
const FIELD_CHARS: u16 = 64;

pub struct Shell<'f> {
    compositor: Compositor,
    wm: Wm,
    theme: Theme,
    fonts: Fonts<'f>,
    pointer: (i32, i32),
    buttons: u8,
}

impl<'f> Shell<'f> {
    /// A shell for a `width × height` screen at `depth`, the cursor at the centre and nothing open. Refused with
    /// `InvalidArgument` for a size the compositor does not take (zero, or wider than `desk_gfx::MAX_WIDTH`).
    pub fn new(width: u32, height: u32, depth: Depth, fonts: Fonts<'f>) -> Result<Shell<'f>, HalError> {
        let theme = Theme::afterglow(depth);
        let mut compositor = Compositor::new(width, height, theme.slot(Role::Desktop))?;
        compositor.set_cursor_inks([theme.slot(Role::CursorOutline), theme.slot(Role::CursorFill)]);
        let pointer = ((width / 2) as i32, (height / 2) as i32);
        compositor.move_cursor(pointer.0, pointer.1);
        Ok(Shell {
            compositor,
            wm: Wm::new(Rect::new(0, 0, width, height)),
            theme,
            fonts,
            pointer,
            buttons: 0,
        })
    }

    fn screen(&self) -> Rect {
        self.compositor.back().bounds()
    }

    fn damage_set(&self) -> Damage {
        Damage::new(self.screen())
    }

    fn flush(&mut self, d: &Damage) {
        for r in d.rects() {
            self.compositor.damage(*r);
        }
    }

    /// Applies one input event. A pointer report moves the cursor (clamped to the screen) and tells the window manager the position
    /// and the buttons; it acts on a change of the left button, never on its level. A key goes to the focused window's focused field.
    pub fn handle(&mut self, e: Event) {
        match e {
            Event::Pointer(m) => {
                self.buttons = m.buttons;
                self.point(
                    self.pointer.0.saturating_add(i32::from(m.dx)),
                    self.pointer.1.saturating_add(i32::from(m.dy)),
                );
            }
            Event::PointerTo(x, y) => self.point(x, y),
            Event::Key(k) => self.key(k),
        }
    }

    fn point(&mut self, x: i32, y: i32) {
        let s = self.screen();
        let x = x.clamp(0, s.x1 - 1);
        let y = y.clamp(0, s.y1 - 1);
        self.pointer = (x, y);
        self.compositor.move_cursor(x, y);
        let mut d = self.damage_set();
        self.wm.pointer(x, y, self.buttons, &mut d);
        self.flush(&d);
    }

    fn key(&mut self, e: KeyEvent) {
        let mut d = self.damage_set();
        self.wm.key(e, &self.fonts, &mut d);
        self.flush(&d);
    }

    /// Opens a window on top and focuses it.
    pub fn open(&mut self, name: &str, outer: Rect, title: &str) -> WindowId {
        let mut d = self.damage_set();
        let id = self.wm.open(WindowSpec { name, outer, title }, &mut d);
        self.flush(&d);
        id
    }

    /// Closes the named window; false when there is none.
    pub fn close(&mut self, name: &str) -> bool {
        let Some(id) = self.wm.by_name(name) else { return false };
        let mut d = self.damage_set();
        self.wm.close(id, &mut d);
        self.flush(&d);
        true
    }

    /// Focuses and raises the named window; false when there is none.
    pub fn focus(&mut self, name: &str) -> bool {
        let Some(id) = self.wm.by_name(name) else { return false };
        let mut d = self.damage_set();
        self.wm.focus(id, &mut d);
        self.flush(&d);
        true
    }

    /// Adds a label to the named window's content area, at `(x, y)` from its top-left; false when there is no such window.
    pub fn label(&mut self, window: &str, x: i32, y: i32, text: &str) -> bool {
        self.add_widget(
            window,
            Widget::Label(Label {
                at: (x, y),
                text: String::from(text),
                role: Role::Text,
            }),
            false,
        )
    }

    /// Adds a text field to the named window and makes it the window's focused widget; false when there is no such window.
    pub fn field(&mut self, window: &str, x: i32, y: i32, width: u32) -> bool {
        self.add_widget(window, Widget::TextLine(TextLine::new((x, y), width, FIELD_CHARS)), true)
    }

    fn add_widget(&mut self, window: &str, widget: Widget, focus: bool) -> bool {
        let Some(id) = self.wm.by_name(window) else { return false };
        let Some(w) = self.wm.window_mut(id) else { return false };
        let content = w.layout().content;
        let bounds = widget
            .bounds(&self.fonts.ui)
            .offset(content.x0, content.y0)
            .intersect(&content);
        if focus {
            w.focus_widget = Some(w.widgets.len());
        }
        w.widgets.push(widget);
        self.compositor.damage(bounds);
        true
    }

    /// Marks the whole screen for repaint, which is what the next frame then presents: the reference a damage-driven picture is
    /// compared with.
    pub fn damage_all(&mut self) {
        let s = self.screen();
        self.compositor.damage(s);
    }

    /// Repaints what changed and presents it, cursor included. A frame with nothing changed writes nothing.
    pub fn frame<P: Present>(&mut self, out: &mut P) -> Result<FrameStats, HalError> {
        let (wm, theme, fonts) = (&self.wm, &self.theme, &self.fonts);
        self.compositor.frame(out, |s: &mut Surface, r| wm.paint(s, r, theme, fonts))
    }

    /// True when `frame` would draw something: damage is pending or the cursor is not where it was last presented. A live loop
    /// asks this instead of presenting to find out.
    pub fn is_dirty(&self) -> bool {
        self.compositor.is_dirty()
    }

    /// The digests of what is on the screen once the last frame has been presented.
    pub fn digest(&self) -> FrameDigest {
        self.compositor.digest(&palette(self.theme.depth()))
    }

    /// The window manager, for looking at the windows. A change made through it directly is not damaged: use the shell's own
    /// methods, which are the ones that report to the compositor.
    pub fn wm(&mut self) -> &mut Wm {
        &mut self.wm
    }

    /// The back buffer: the scene without the cursor.
    pub fn back(&self) -> &Surface {
        self.compositor.back()
    }

    /// Where the pointer is, always on the screen.
    pub fn pointer(&self) -> (i32, i32) {
        self.pointer
    }

    pub fn depth(&self) -> Depth {
        self.theme.depth()
    }
}
