// A file of the Desk platform layer for std, copied over the toolchain's library sources by patch/mkstd.sh.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::sys::pal::abi;
use crate::time::Duration;

pub fn sleep(dur: Duration) {
    let ns = u64::try_from(dur.as_nanos()).unwrap_or(u64::MAX);
    unsafe { abi::desk_sleep_ns(ns) }
}
