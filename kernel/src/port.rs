// Port I/O instructions.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::arch::asm;

/// Writes a byte to an I/O port.
#[inline]
pub fn outb(port: u16, value: u8) {
    // SAFETY: port writes have no memory effects; callers pass the port of a device they own.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)) };
}

/// Reads a byte from an I/O port.
#[inline]
pub fn inb(port: u16) -> u8 {
    let v: u8;
    // SAFETY: as for `outb`.
    unsafe { asm!("in al, dx", in("dx") port, out("al") v, options(nomem, nostack, preserves_flags)) };
    v
}

/// A short delay for slow devices such as the PIC: a write to the unused POST port 0x80.
pub fn io_wait() {
    outb(0x80, 0);
}
