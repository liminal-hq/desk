// Painting: the desktop, then each window that meets the clip, bottom to top.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::window::Window;
use crate::Fonts;
use desk_gfx::{Rect, Surface};
use desk_ui::{paint_chrome, Role, Theme};

/// Paints `windows` (bottom to top; the last is the focused one) into `s` within `clip`, and puts the surface's clip back.
pub(crate) fn paint(windows: &[Window], s: &mut Surface, clip: Rect, t: &Theme, fonts: &Fonts) {
    let saved = s.clip();
    let clip = clip.intersect(&saved);
    s.set_clip(clip);
    s.fill(clip, t.slot(Role::Desktop));
    for (i, w) in windows.iter().enumerate() {
        if w.extent().overlaps(&clip) {
            paint_window(w, i + 1 == windows.len(), s, clip, t, fonts);
        }
    }
    s.set_clip(saved);
}

fn paint_window(w: &Window, focused: bool, s: &mut Surface, clip: Rect, t: &Theme, fonts: &Fonts) {
    let l = w.layout();
    paint_chrome(s, t, &fonts.title, &l, &w.title, focused);
    // Widgets stay inside the content area.
    s.set_clip(clip.intersect(&l.content));
    let origin = (l.content.x0, l.content.y0);
    for widget in &w.widgets {
        widget.paint(s, t, &fonts.ui, origin, focused);
    }
    s.set_clip(clip);
}
