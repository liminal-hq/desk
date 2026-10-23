// A window: its name, outer box, title and widgets, and what a point on it means.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::vec::Vec;
use desk_gfx::Rect;
use desk_ui::{ChromeLayout, Control, Widget, HALO};

/// A window's identity. Ids are not reused while the window manager lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowId(pub u16);

/// What `Wm::open` needs to make a window. A box smaller than `desk_ui::MIN_WIDTH` × `MIN_HEIGHT` is widened to it.
pub struct WindowSpec<'s> {
    pub name: &'s str,
    pub outer: Rect,
    pub title: &'s str,
}

/// A window. `widgets` are positioned relative to the content area; `focus_widget` is the index of the one that takes keys. Whoever
/// adds a widget through `Wm::window_mut` also reports the damage for it.
#[derive(Clone, Debug)]
pub struct Window {
    pub id: WindowId,
    pub name: String,
    pub outer: Rect,
    pub title: String,
    pub widgets: Vec<Widget>,
    pub focus_widget: Option<usize>,
}

/// Which part of a window a point is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// The title bar, outside the three control boxes.
    Title,
    Control(Control),
    /// The rule under the title bar and the content area.
    Content,
    /// The halo outside the outer box: three pixels when the window is focused, the one-pixel hairline when it is not (the rest is
    /// not painted, so it belongs to whatever lies beneath).
    Ring,
}

impl Window {
    /// Where the window's parts are.
    pub fn layout(&self) -> ChromeLayout {
        ChromeLayout::new(self.outer)
    }

    /// What the window owns on screen in either focus state.
    pub fn extent(&self) -> Rect {
        self.outer.inflate(HALO)
    }

    /// The part of the window at `(x, y)`, for a window that is `focused` or not.
    pub(crate) fn hit(&self, x: i32, y: i32, focused: bool) -> Option<Hit> {
        let l = self.layout();
        if l.outer.contains(x, y) {
            for c in [Control::Minimise, Control::Maximise, Control::Close] {
                if l.button(c).contains(x, y) {
                    return Some(Hit::Control(c));
                }
            }
            return Some(if l.title_bar.contains(x, y) {
                Hit::Title
            } else {
                Hit::Content
            });
        }
        let reach = if focused { HALO } else { 1 };
        l.outer.inflate(reach).contains(x, y).then_some(Hit::Ring)
    }
}
