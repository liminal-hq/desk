// Ending the run through QEMU's isa-debug-exit device.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::hal::PORTS;
use core::arch::asm;
use desk_hal::PortIo;

const DEBUG_EXIT: u16 = 0xF4;

// QEMU turns a value `v` written to the device into the process exit status `(v << 1) | 1`.
const SUCCESS: u8 = 0x10; // status 33
const FAILURE: u8 = 0x11; // status 35

fn stop(code: u8) -> ! {
    PORTS.outb(DEBUG_EXIT, code);
    // Real hardware has no such device: halt for good.
    loop {
        // SAFETY: stops the CPU.
        unsafe { asm!("cli", "hlt") };
    }
}

pub fn success() -> ! {
    stop(SUCCESS)
}

pub fn failure() -> ! {
    stop(FAILURE)
}
