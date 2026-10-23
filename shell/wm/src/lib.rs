// The shell's window manager: windows with z-order and focus, hit testing, title-bar dragging, the close box and the damage each
// change reports.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

extern crate alloc;

mod paint;
mod window;
mod wm;

pub use window::{Hit, Window, WindowId, WindowSpec};
pub use wm::Wm;

use desk_font::Font;

/// The two faces the window manager paints with: the UI face for widgets and the title face for title bars.
pub struct Fonts<'a> {
    pub ui: Font<'a>,
    pub title: Font<'a>,
}
