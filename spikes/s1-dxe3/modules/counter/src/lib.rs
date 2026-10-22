// A small module with its own state and floating point: sums the events' x values and reports the mean and a square root.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]

extern crate alloc;

use alloc::format;
use core::sync::atomic::{AtomicU32, Ordering};
use rt::{desk_app, log, DeskEvent};

static EVENTS: AtomicU32 = AtomicU32::new(100);
static SUM: rt::Slot<f64> = rt::Slot::new();

/// The square root, computed by the x87's `fsqrt` (`core` has none; see docs/TOOLCHAIN.md).
fn x87_sqrt(x: f64) -> f64 {
    let mut result: f64 = 0.0;
    // SAFETY: loads `x`, takes its root and stores it, leaving the x87 stack as found; the value moves through memory because
    // Rust does not allow an x87 register as an operand.
    unsafe {
        core::arch::asm!(
            "fld qword ptr [{x}]",
            "fsqrt",
            "fstp qword ptr [{r}]",
            x = in(reg) &x,
            r = in(reg) &mut result,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
            out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            options(nostack),
        );
    }
    result
}

fn start(app: u32, _args: &str) -> i32 {
    log(&format!(
        "counter: start app={} events={}",
        app,
        EVENTS.load(Ordering::Relaxed)
    ));
    SUM.set(0.0);
    0
}

fn event(_app: u32, e: &DeskEvent) {
    EVENTS.fetch_add(1, Ordering::Relaxed);
    SUM.with(|s| *s += f64::from(e.x));
}

fn stop(app: u32) {
    let events = EVENTS.load(Ordering::Relaxed);
    let sum = SUM.take().unwrap_or(0.0);
    let seen = f64::from(events - 100);
    let mean = if seen > 0.0 { sum / seen } else { 0.0 };
    log(&format!(
        "counter: stop app={} events={} sum={:.1} mean={:.3} sqrt={:.4}",
        app,
        events,
        sum,
        mean,
        x87_sqrt(sum)
    ));
}

desk_app!(start, event, stop);
