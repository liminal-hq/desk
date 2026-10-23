// The x87 floating-point unit: enabling it and a first calculation.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::println;
use core::arch::asm;

/// Gets the FPU ready: CR0.EM off (execute x87 instructions rather than trap them), CR0.MP on (WAIT respects TS), CR0.NE on
/// (report x87 errors as exception 16 instead of the legacy IRQ13 wiring), CR0.TS off (no lazy-switch trap), then `fninit`.
pub fn init() {
    // SAFETY: edits only the FPU-related CR0 bits.
    unsafe {
        asm!(
            "mov {t}, cr0",
            "and {t}, {clear}",
            "or {t}, {set}",
            "mov cr0, {t}",
            "fninit",
            t = out(reg) _,
            clear = const !((1u32 << 2) | (1 << 3)),
            set = const (1u32 << 1) | (1 << 5),
        );
    }
}

fn fsqrt(x: f64) -> f64 {
    let mut out = 0.0f64;
    // SAFETY: loads, takes the square root and stores through valid references; the x87 stack is balanced.
    unsafe {
        asm!("fld qword ptr [{x}]", "fsqrt", "fstp qword ptr [{o}]", x = in(reg) &x, o = in(reg) &mut out);
    }
    out
}

pub fn demo() {
    let cw: u16 = {
        let mut cw = 0u16;
        // SAFETY: stores the control word to a valid location.
        unsafe { asm!("fnstcw [{0}]", in(reg) &mut cw) };
        cw
    };
    // `fninit` leaves the control word at 0x037F; anything else means no working FPU.
    assert_eq!(cw, 0x037F, "x87 control word after fninit");
    let root = fsqrt(2.0);
    // The compiler emits x87 code for ordinary f64 arithmetic because SSE is off in the target.
    let mut harmonic = 0.0f64;
    for i in 1..=10 {
        harmonic += 1.0 / f64::from(i);
    }
    println!("fpu: control word {:#06x}, sqrt(2) = {:.6}, H(10) = {:.6}", cw, root, harmonic);
    assert!((root * root - 2.0).abs() < 1e-12);
    assert!((harmonic - 2.928_968_253_968_254).abs() < 1e-12);
    println!("fpu: results check out");
}
