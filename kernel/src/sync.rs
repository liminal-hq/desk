// Critical sections for single-processor code that shares state with interrupt handlers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::arch::asm;

/// Runs `f` with interrupts off and restores the previous interrupt flag afterwards.
pub fn without_interrupts<R>(f: impl FnOnce() -> R) -> R {
    let flags: u32;
    // SAFETY: reads EFLAGS and clears IF; the flag is restored below.
    unsafe { asm!("pushfd", "pop {0}", "cli", out(reg) flags, options(preserves_flags)) };
    let r = f();
    if flags & (1 << 9) != 0 {
        // SAFETY: interrupts were enabled when we came in.
        unsafe { asm!("sti") };
    }
    r
}
