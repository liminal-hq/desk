// A program that hooks IRQ 0 and then panics: the clean-up list must put the vector back before the report.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use rt::{dpmi, println, timer};

static mut BEFORE: (u16, u32) = (0, 0);

/// Runs after the timer's own clean-up (the list is last in, first out) and says whether the vector is back where it was.
fn report() {
    // SAFETY: single-threaded; BEFORE was set before this was registered.
    let before = unsafe { BEFORE };
    let now = dpmi::get_pm_vector(8).unwrap();
    println!("panicky: IRQ 0 vector restored by the clean-up list: {}", now == before);
}

pub fn main() -> i32 {
    let vec = 8;
    let before = dpmi::get_pm_vector(vec).unwrap();
    // SAFETY: single-threaded, before the hook.
    unsafe { BEFORE = before };
    rt::cleanup::register(report);
    timer::hook().expect("hook");
    let hooked = dpmi::get_pm_vector(vec).unwrap();
    println!("panicky: vector changed by the hook: {}", before != hooked);
    let n = timer::ticks();
    let _ = n;
    let v: alloc::vec::Vec<u32> = alloc::vec![1, 2, 3];
    let i = v.len() + core::hint::black_box(2);
    println!("panicky: about to index {} of {}", i, v.len());
    let x = v[i];
    println!("unreachable {}", x);
    0
}
