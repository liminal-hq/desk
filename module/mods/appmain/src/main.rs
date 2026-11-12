// appmain: an application (kind 2) that only logs a line. The spike never runs it: an application needs a task (R77), so the
// loaders refuse the file, which is what the tests use it for.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

use desk_guest::log;

#[no_mangle]
pub extern "C" fn desk_main(_args: *const u8, _args_len: u32) -> i32 {
    log("appmain: desk_main");
    0
}
