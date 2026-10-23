// A file of the Desk platform layer for std, copied over the toolchain's library sources by patch/mkstd.sh.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! System bindings for Desk programs: the "unsupported" platform plus a small C interface to the runtime.

pub mod abi;

#[path = "../unsupported/common.rs"]
#[deny(unsafe_op_in_unsafe_fn)]
mod common;

pub use common::{cleanup, init, unsupported, unsupported_err};

pub fn abort_internal() -> ! {
    // SAFETY: the runtime's exit never returns.
    unsafe { abi::desk_exit(134) }
}
