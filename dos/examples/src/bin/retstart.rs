// A program whose `_start` returns instead of exiting, to see what the stub leaves behind it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

// No runtime here: the entry is two instructions. The stub left the stub's own stack in use, so the `ret` pops whatever is on it.
core::arch::global_asm!(".section .text.start,\"ax\"", ".global _start", "_start:", "ret");

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
