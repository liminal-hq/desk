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
use desk_hal::{KeyCode, KeyEvent, Modifiers, MouseEvent};
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
    // An Alt+Tab in progress: how far down the stacking order (0 is the top, the focused window) the window that Alt's release will
    // focus is. Anything that changes the stacking order or the set of windows ends it.
    cycle: Option<usize>,
    // Alt+F4 has closed a window and F4 has not come up since: its typematic repeats must not close the next one too.
    closing: bool,
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
            cycle: None,
            closing: false,
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
        self.cycle = None;
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
        self.cycle = None;
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
        self.cycle = None;
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
        // A click while Alt is held ends the cycle, wherever it lands.
        self.cycle = None;
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

    /// The window that releasing Alt would focus, while an Alt+Tab is in progress.
    pub fn cycling(&self) -> Option<WindowId> {
        let d = self.cycle?;
        let i = self.windows.len().checked_sub(1 + d)?;
        self.windows.get(i).map(|w| w.id)
    }

    /// A key. In order: Alt+Tab (with Shift: the other way) moves the cycle one window along the stacking order and draws nothing;
    /// releasing Alt focuses the window the cycle has reached, as a click would; Escape cancels the cycle; Alt+F4, outside a cycle,
    /// closes the focused window; Tab with no Alt, Ctrl or Windows key moves to the focused window's next field (Shift: the previous),
    /// wrapping, labels skipped; anything else goes to the focused window's focused widget. Damage: as the change it makes (see
    /// `docs/LIVE-INPUT.md`); a step of the cycle adds none.
    pub fn key(&mut self, e: KeyEvent, fonts: &Fonts, damage: &mut Damage) {
        let alt = e.modifiers.contains(Modifiers::ALT);
        let shift = e.modifiers.contains(Modifiers::SHIFT);
        match (e.code, e.pressed) {
            (KeyCode::Tab, true) if alt => return self.step_cycle(shift),
            (KeyCode::LeftAlt | KeyCode::RightAlt, false) if !alt => {
                if let Some(id) = self.cycling() {
                    // Focus moved by the keyboard ends a title-bar drag and disarms a close box: the pointer's gesture was on a
                    // window that is no longer the focused one.
                    self.drag = None;
                    self.armed = None;
                    self.focus(id, damage);
                }
                self.cycle = None;
                return;
            }
            (KeyCode::Escape, true) if self.cycle.is_some() => {
                self.cycle = None;
                return;
            }
            (KeyCode::F4, true) if alt && self.cycle.is_none() => {
                if !self.closing {
                    self.closing = true;
                    if let Some(id) = self.focused() {
                        self.close(id, damage);
                    }
                }
                return;
            }
            (KeyCode::F4, false) => self.closing = false,
            (KeyCode::Tab, true) if e.modifiers.0 & (Modifiers::CTRL.0 | Modifiers::GUI.0) == 0 => {
                return self.step_field(shift, fonts, damage);
            }
            _ => {}
        }
        let Some(w) = self.windows.last_mut() else { return };
        let Some(i) = w.focus_widget else { return };
        let content = w.layout().content;
        let Some(widget) = w.widgets.get_mut(i) else { return };
        if let Some(r) = widget.key(e, &fonts.ui) {
            damage.add(r.offset(content.x0, content.y0).intersect(&content));
        }
    }

    // One Alt+Tab press: the target moves one window down the stacking order (Shift: up), wrapping.
    fn step_cycle(&mut self, back: bool) {
        let n = self.windows.len();
        if n == 0 {
            return;
        }
        let d = self.cycle.unwrap_or(0);
        self.cycle = Some(if back { (d + n - 1) % n } else { (d + 1) % n });
    }

    // Tab and Shift+Tab: the focused window's focus moves to its next (previous) field, wrapping.
    fn step_field(&mut self, back: bool, fonts: &Fonts, damage: &mut Damage) {
        let Some(w) = self.windows.last_mut() else { return };
        let fields: Vec<usize> = (0..w.widgets.len()).filter(|&i| w.widgets[i].takes_focus()).collect();
        if fields.is_empty() {
            return;
        }
        let at = w.focus_widget.and_then(|f| fields.iter().position(|&i| i == f));
        let next = match (at, back) {
            (Some(p), false) => (p + 1) % fields.len(),
            (Some(p), true) => (p + fields.len() - 1) % fields.len(),
            (None, false) => 0,
            (None, true) => fields.len() - 1,
        };
        let to = fields[next];
        if w.focus_widget == Some(to) {
            return;
        }
        let content = w.layout().content;
        for i in w.focus_widget.into_iter().chain(Some(to)) {
            if let Some(r) = w.widgets.get(i).and_then(|x| x.caret(&fonts.ui)) {
                damage.add(r.offset(content.x0, content.y0).intersect(&content));
            }
        }
        w.focus_widget = Some(to);
    }

    /// Paints the desktop and every window that meets `clip`, bottom to top, within `clip`; no pixel outside it changes.
    pub fn paint(&self, s: &mut Surface, clip: Rect, t: &Theme, fonts: &Fonts) {
        paint(&self.windows, s, clip, t, fonts);
    }
}
