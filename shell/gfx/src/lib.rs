// The shell's graphics core: rectangles, the back buffer, sprites, damage, the compositor and frame digests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![cfg_attr(not(any(test, feature = "std")), no_std)]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

extern crate alloc;
