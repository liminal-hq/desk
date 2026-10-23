// The window manager: z-order, focus, hit testing, dragging, the close box and the damage each change reports.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `windows` runs bottom to top and its last window is the focused one: focus always raises, and closing the top window focuses the
// next. The damage each change adds is the table in `docs/FIRST-WINDOW.md` ("desk-wm"). A focus change repaints the title bar and the
// four rings, which `Damage` merges into the whole `extent` anyway, so the extent is what is added.

use crate::paint::paint;
use crate::window::{Hit, Window, WindowId, WindowSpec};
use crate::Fonts;
use alloc::string::String;
use alloc::vec::Vec;
use desk_gfx::{Damage, Rect, Surface};
use desk_hal::{KeyEvent, MouseEvent};
use desk_ui::{Control, Theme, TITLE_HEIGHT};

/// How much of the title bar must stay on screen horizontally.
const REACH: i32 = 24;

// A title-bar drag: the window and where in its outer box the pointer took hold.
#[derive(Clone, Copy)]
struct Drag {
    id: WindowId,
    grab: (i32, i32),
}

pub struct Wm {
    windows: Vec<Window>,
    drag: Option<Drag>,
    // The window whose close box the left button went down on, until the button comes up.
    armed: Option<WindowId>,
    screen: Rect,
    buttons: u8,
    next_id: u16,
}

impl Wm {
    pub fn new(screen: Rect) -> Wm {
        Wm {
            windows: Vec::new(),
            drag: None,
            armed: None,
            screen,
            buttons: 0,
            next_id: 0,
        }
    }

    /// The windows, bottom to top.
    pub fn windows(&self) -> &[Window] {
        &self.windows
    }

    /// The focused window, which is always the top one.
    pub fn focused(&self) -> Option<WindowId> {
        self.windows.last().map(|w| w.id)
    }

    fn index(&self, id: WindowId) -> Option<usize> {
        self.windows.iter().position(|w| w.id == id)
    }

    /// Opens a window on top and focuses it. Damage: its `extent` (and the old top window's, which loses focus).
    pub fn open(&mut self, spec: WindowSpec, damage: &mut Damage) -> WindowId {
        while self.windows.iter().any(|w| w.id.0 == self.next_id) {
            self.next_id = self.next_id.wrapping_add(1);
        }
        let id = WindowId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        if let Some(old) = self.windows.last() {
            damage.add(old.extent());
        }
        let mut w = Window {
            id,
            name: String::from(spec.name),
            outer: spec.outer,
            title: String::from(spec.title),
            widgets: Vec::new(),
            focus_widget: None,
        };
        w.outer = w.layout().outer;
        damage.add(w.extent());
        self.windows.push(w);
        id
    }

    /// Closes a window. Damage: its `extent`; when it was the focused one, also the extent of the window that becomes the top.
    pub fn close(&mut self, id: WindowId, damage: &mut Damage) {
        let Some(i) = self.index(id) else { return };
        let w = self.windows.remove(i);
        damage.add(w.extent());
        if i == self.windows.len() {
            if let Some(top) = self.windows.last() {
                damage.add(top.extent());
            }
        }
        if self.drag.is_some_and(|d| d.id == id) {
            self.drag = None;
        }
        if self.armed == Some(id) {
            self.armed = None;
        }
    }

    /// Focuses a window and raises it. Damage: none when it is already the top window; otherwise its `extent` and the previously
    /// focused window's.
    pub fn focus(&mut self, id: WindowId, damage: &mut Damage) {
        let Some(i) = self.index(id) else { return };
        if i + 1 == self.windows.len() {
            return;
        }
        if let Some(old) = self.windows.last() {
            damage.add(old.extent());
        }
        let w = self.windows.remove(i);
        damage.add(w.extent());
        self.windows.push(w);
    }

    /// The first window with this name.
    pub fn by_name(&self, name: &str) -> Option<WindowId> {
        self.windows.iter().find(|w| w.name == name).map(|w| w.id)
    }

    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    /// The topmost window at `(x, y)` and the part of it there.
    pub fn hit(&self, x: i32, y: i32) -> Option<(WindowId, Hit)> {
        let top = self.windows.len().checked_sub(1)?;
        self.windows
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, w)| w.hit(x, y, i == top).map(|h| (w.id, h)))
    }

    /// The pointer is now at `(x, y)` with `buttons` (`MouseEvent::LEFT` and the others). A drag follows the motion first; then a left
    /// press focuses and raises the window under the pointer, starts a drag on its title and arms its close box, and a release ends
    /// the drag and closes the window if the pointer is still over the armed close box. Motion with no drag adds no damage.
    pub fn pointer(&mut self, x: i32, y: i32, buttons: u8, damage: &mut Damage) {
        if let Some(d) = self.drag {
            self.drag_to(d, x, y, damage);
        }
        let was = self.buttons & MouseEvent::LEFT != 0;
        let is = buttons & MouseEvent::LEFT != 0;
        self.buttons = buttons;
        if is && !was {
            self.press(x, y, damage);
        } else if was && !is {
            self.release(x, y, damage);
        }
    }

    fn press(&mut self, x: i32, y: i32, damage: &mut Damage) {
        let Some((id, hit)) = self.hit(x, y) else { return };
        self.focus(id, damage);
        match hit {
            Hit::Title => {
                let outer = self.windows.last().map(|w| w.outer).unwrap_or_default();
                self.drag = Some(Drag {
                    id,
                    grab: (x - outer.x0, y - outer.y0),
                });
            }
            Hit::Control(Control::Close) => self.armed = Some(id),
            _ => {}
        }
    }

    fn release(&mut self, x: i32, y: i32, damage: &mut Damage) {
        self.drag = None;
        if let Some(id) = self.armed.take() {
            if self.hit(x, y) == Some((id, Hit::Control(Control::Close))) {
                self.close(id, damage);
            }
        }
    }

    // Moves the dragged window so the grabbed point is under the pointer, kept where its title bar can still be reached.
    fn drag_to(&mut self, d: Drag, x: i32, y: i32, damage: &mut Damage) {
        let screen = self.screen;
        let Some(w) = self.window_mut(d.id) else { return };
        let width = w.outer.width() as i64;
        let height = TITLE_HEIGHT as i64;
        let clamp = |v: i32, lo: i64, hi: i64| (v as i64).max(lo).min(hi) as i32;
        let nx = clamp(
            x - d.grab.0,
            screen.x0 as i64 + REACH as i64 - width,
            screen.x1 as i64 - REACH as i64,
        );
        let ny = clamp(y - d.grab.1, screen.y0 as i64, screen.y1 as i64 - height);
        if (nx, ny) == (w.outer.x0, w.outer.y0) {
            return;
        }
        let old = w.extent();
        w.outer = w.outer.offset(nx - w.outer.x0, ny - w.outer.y0);
        damage.add(old);
        damage.add(w.extent());
    }

    /// A key for the focused window's focused widget, and for nothing else. Damage: the rectangle the widget reports, on screen.
    pub fn key(&mut self, e: KeyEvent, fonts: &Fonts, damage: &mut Damage) {
        let Some(w) = self.windows.last_mut() else { return };
        let Some(i) = w.focus_widget else { return };
        let content = w.layout().content;
        let Some(widget) = w.widgets.get_mut(i) else { return };
        if let Some(r) = widget.key(e, &fonts.ui) {
            damage.add(r.offset(content.x0, content.y0).intersect(&content));
        }
    }

    /// Paints the desktop and every window that meets `clip`, bottom to top, within `clip`; no pixel outside it changes.
    pub fn paint(&self, s: &mut Surface, clip: Rect, t: &Theme, fonts: &Fonts) {
        paint(&self.windows, s, clip, t, fonts);
    }
}
