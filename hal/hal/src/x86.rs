// The real port instructions: the one audited `unsafe` boundary for port I/O, shared by the native kernel and the DOS runtime.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `in` and `out` are the same instructions in the kernel (ring 0) and in a DPMI client (ring 3 with the I/O permission CWSDPMI
// grants), so both platforms use this type. A host that traps port access (a Windows DOS box) emulates the instruction instead;
// that is the host's business, not this code's.

use crate::port::PortIo;
use core::arch::asm;

/// The machine's port bus. Zero-sized and `Copy`: holding one is the permission to touch hardware.
#[derive(Clone, Copy, Debug)]
pub struct RawPorts {
    _private: (),
}

impl RawPorts {
    /// # Safety
    ///
    /// The caller is the platform: it must run where `in` and `out` are permitted (ring 0, or a DPMI client with I/O permission),
    /// and it takes responsibility for which drivers it hands the bus to. Port writes can reprogram DMA, reset the machine or
    /// redirect interrupts, so they are as dangerous as raw pointer writes; this constructor is where that is accepted.
    pub const unsafe fn new() -> RawPorts {
        RawPorts { _private: () }
    }
}

// The `asm!` blocks omit `nomem` on purpose: the compiler then treats each one as possibly reading and writing memory, so it does
// not move ordinary loads and stores across a port access (a DMA buffer is filled before the port write that starts the transfer).
impl PortIo for RawPorts {
    #[inline]
    fn inb(&self, port: u16) -> u8 {
        let v: u8;
        // SAFETY: `RawPorts::new`'s contract; the instruction touches only AL, DX and the device.
        unsafe { asm!("in al, dx", in("dx") port, out("al") v, options(nostack, preserves_flags)) };
        v
    }

    #[inline]
    fn outb(&self, port: u16, value: u8) {
        // SAFETY: as `inb`.
        unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nostack, preserves_flags)) };
    }

    #[inline]
    fn inw(&self, port: u16) -> u16 {
        let v: u16;
        // SAFETY: as `inb`.
        unsafe { asm!("in ax, dx", in("dx") port, out("ax") v, options(nostack, preserves_flags)) };
        v
    }

    #[inline]
    fn outw(&self, port: u16, value: u16) {
        // SAFETY: as `inb`.
        unsafe { asm!("out dx, ax", in("dx") port, in("ax") value, options(nostack, preserves_flags)) };
    }

    #[inline]
    fn inl(&self, port: u16) -> u32 {
        let v: u32;
        // SAFETY: as `inb`.
        unsafe { asm!("in eax, dx", in("dx") port, out("eax") v, options(nostack, preserves_flags)) };
        v
    }

    #[inline]
    fn outl(&self, port: u16, value: u32) {
        // SAFETY: as `inb`.
        unsafe { asm!("out dx, eax", in("dx") port, in("eax") value, options(nostack, preserves_flags)) };
    }
}
