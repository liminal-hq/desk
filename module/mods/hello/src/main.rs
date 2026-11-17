// hello: the spike's first module. `app_start` prints two lines and returns 0, `app_stop` prints one; it has no `app_event`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

use core::fmt::Write;
use core::sync::atomic::{AtomicU32, Ordering};

use desk_guest::{desk_app, log, Lines};

/// A static that starts at 7: each instance has its own copy, and the number printed shows it.
static COUNT: AtomicU32 = AtomicU32::new(7);

fn start(app: u32, _args: &[u8]) -> i32 {
    log("hello from a loaded program");
    let count = COUNT.fetch_add(1, Ordering::Relaxed);
    let _ = writeln!(Lines::new(), "hello: app={app} count={count}");
    0
}

fn stop(app: u32) {
    let _ = writeln!(Lines::new(), "hello: stop app={app}");
}

desk_app!(start = start, stop = stop);
