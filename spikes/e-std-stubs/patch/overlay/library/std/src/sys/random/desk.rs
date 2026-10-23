// A file of the Desk platform layer for std, copied over the toolchain's library sources by patch/mkstd.sh.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::sys::pal::abi;

pub fn fill_bytes(bytes: &mut [u8]) {
    unsafe { abi::desk_random(bytes.as_mut_ptr(), bytes.len()) }
}
