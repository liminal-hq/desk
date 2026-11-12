// A small no_std module for the packer's real-linker test: two import slots, the three entry points, and data that needs relocating.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// This file is not part of the `module` workspace: `tests/real.rs` copies it into a scratch package and builds it for
// `kernel/i486-desk-none.json` with `module.ld`. The optional entry points are behind the feature `bare`.
#![no_std]
#![no_main]

use core::sync::atomic::{AtomicUsize, Ordering};

#[no_mangle]
#[used]
#[link_section = ".desk.imp"]
static __desk_imp_desk_log: AtomicUsize = AtomicUsize::new(0);
#[no_mangle]
#[used]
#[link_section = ".desk.imp"]
static __desk_imp_desk_alloc: AtomicUsize = AtomicUsize::new(0);

static GREETING: &[u8] = b"hello from a module\n";
static TABLE: [&[u8]; 2] = [GREETING, b"second\n"];
static mut STATE: [u32; 16] = [0; 16];

fn log(text: &[u8]) {
    let f = __desk_imp_desk_log.load(Ordering::Relaxed);
    if f != 0 {
        // SAFETY: the loader stores the address of `desk_log` in the slot before the module runs.
        let f: extern "C" fn(*const u8, usize) = unsafe { core::mem::transmute(f) };
        f(text.as_ptr(), text.len());
    }
}

#[no_mangle]
pub extern "C" fn app_start(_app: u32, _args: *const u8, _len: u32) -> i32 {
    for t in TABLE.iter() {
        log(t);
    }
    // SAFETY: single-threaded test module.
    unsafe { STATE[0] += 1 };
    0
}

#[cfg(not(feature = "bare"))]
#[no_mangle]
pub extern "C" fn app_event(_app: u32, _event: *const u8) {
    log(b"event\n");
}

#[cfg(not(feature = "bare"))]
#[no_mangle]
pub extern "C" fn app_stop(_app: u32) {
    log(b"stop\n");
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
