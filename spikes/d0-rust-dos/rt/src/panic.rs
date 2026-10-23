// The panic handler: undo what the program set up, report where and why, exit with status 3.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Status 3 is the skeleton's: a status DOS can hand back that no ordinary run returns. The report goes to standard output because
// that is what the headless harness captures. The clean-up list runs first, so a hooked IRQ 0 is off the machine before any
// formatting happens (formatting can allocate, and allocation can fail, which panics again). A panic inside the handler, or inside
// a clean-up function, exits at once.
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

static PANICKING: AtomicBool = AtomicBool::new(false);

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if PANICKING.swap(true, Ordering::SeqCst) {
        crate::dos::exit_now(3);
    }
    crate::cleanup::run_all();
    crate::println!("RT PANIC: {}", info);
    crate::dos::exit_now(3)
}
