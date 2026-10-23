// The shell's graphics core: rectangles, the back buffer, sprites, damage, the compositor and frame digests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

extern crate alloc;

mod compose;
mod damage;
mod digest;
mod present;
mod rect;
mod sprite;
mod surface;

pub use compose::{Compositor, CursorLayer, FrameStats, MAX_WIDTH};
pub use damage::{Damage, MAX_RECTS};
pub use digest::{crc32, Crc32, FrameDigest};
pub use present::{MemoryPresent, Present, PresentStats};
pub use rect::Rect;
pub use sprite::{Sprite, ARROW};
pub use surface::Surface;
