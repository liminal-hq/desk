// The shell itself: input events, the cursor, the window manager and frames, and the scenario format and runner that script them, so
// the same steps run on the host and on both tracks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

extern crate alloc;

mod run;
pub mod scenario;
mod shell;

pub use shell::Shell;

use desk_hal::{KeyEvent, MouseEvent};

/// One thing that happened on an input device. A platform drains its input source into these; a scenario writes them as steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// A mouse report: motion added to the pointer, and the state of the buttons now.
    Pointer(MouseEvent),
    /// The pointer set to a position, with the buttons unchanged.
    PointerTo(i32, i32),
    /// A key going down or up.
    Key(KeyEvent),
}
