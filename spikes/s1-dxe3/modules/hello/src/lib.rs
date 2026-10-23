// A small module: logs what it is asked, keeps a history in a Vec of Strings, and opens a (stub) window.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use rt::{desk_app, log, window_new, DeskEvent};

// Static data starts from this value every time the module is loaded: a reload must show 7 again, not the last run's count.
static EVENTS: AtomicU32 = AtomicU32::new(7);
// The history lives on the core's heap through `desk_alloc`; `stop` frees it.
static HISTORY: rt::Slot<Vec<String>> = rt::Slot::new();

fn start(app: u32, args: &str) -> i32 {
    let window = window_new(app, "Hello", 200, 100);
    log(&format!(
        "hello: start app={} args='{}' window={} events={}",
        app,
        args,
        window,
        EVENTS.load(Ordering::Relaxed)
    ));
    HISTORY.set(Vec::new());
    0
}

fn event(app: u32, e: &DeskEvent) {
    let n = EVENTS.fetch_add(1, Ordering::Relaxed) + 1;
    let line = format!("event {} kind={} x={} y={}", n, e.kind, e.x, e.y);
    log(&format!("hello[{}]: {}", app, line));
    HISTORY.with(|h| h.push(line));
}

fn stop(app: u32) {
    let kept = HISTORY.take().map_or(0, |h| h.len());
    log(&format!(
        "hello: stop app={} events={} kept={}",
        app,
        EVENTS.load(Ordering::Relaxed),
        kept
    ));
}

desk_app!(start, event, stop);
