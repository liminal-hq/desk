// The shell's presenters: the linear and planar (mode 12h) frame buffer writers, and the read-back the DOS captures use.
// This is the one `shell/` crate that calls `desk-vga`'s register-level API; the core crates see only `desk_gfx::Present`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

extern crate alloc;

mod linear;
mod planar;
mod readback;

pub use linear::LinearPresent;
pub use planar::{planarise, PlanarPresent, PlaneSink, VgaPlanes, MAX_PLANE_BYTES};
pub use readback::{read_back_linear, read_back_planar};
