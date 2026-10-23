// The shell's look: theme roles and palettes, text drawing, the window chrome with its stepped halo, and the label and text-field widgets.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

extern crate alloc;

mod chrome;
mod icons;
mod palette;
mod text;
mod theme;
mod widget;

pub use chrome::{paint_chrome, ChromeLayout, Control, BOX, HALO, MIN_HEIGHT, MIN_WIDTH, TITLE_HEIGHT};
pub use icons::{Icon, CLOSE, MAXIMISE, MINIMISE};
pub use palette::{palette, AFTERGLOW_SURFACES, DESK16};
pub use text::{draw_text, fit_with_ellipsis};
pub use theme::{Depth, Role, Theme};
pub use widget::{Label, TextLine, Widget, CARET_HEIGHT, FIELD_HEIGHT};
