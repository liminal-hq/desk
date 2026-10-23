// Shared pieces of the s5-jsdos probes: the palette, a key wait, physical memory mapping and a small report writer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]

extern crate alloc;

use alloc::string::String;
use core::arch::asm;
use core::fmt::Write;
use rt::dos::{File, Mode};
use rt::dpmi;

/// The palette both probes load, so a reference image can be computed without knowing the emulator's default DAC: 6-bit
/// components, red rising with the index, green a sawtooth, blue falling.
pub fn palette(i: u8) -> (u8, u8, u8) {
    (i >> 2, (i.wrapping_mul(7)) & 63, 63 - (i >> 2))
}

/// A byte write to an I/O port.
pub fn out8(port: u16, value: u8) {
    // SAFETY: a port write touches no memory; the callers write to the VGA DAC, the PIT, the PC speaker gate, the OPL and the
    // Sound Blaster, which the DPMI host lets a client reach.
    unsafe { asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags)) };
}

/// A byte read from an I/O port.
pub fn in8(port: u16) -> u8 {
    let value: u8;
    // SAFETY: a port read touches no memory; see `out8`.
    unsafe { asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags)) };
    value
}

/// Waits about `ms` milliseconds by the BIOS tick counter (18.2 ticks a second), which the emulator advances in emulated time.
pub fn delay_ms(ms: u32) {
    let start = rt::timer::bios_ticks();
    let ticks = (ms * 182 / 10_000).max(1);
    while rt::timer::bios_ticks().wrapping_sub(start) < ticks {
        core::hint::spin_loop();
    }
}

/// Programs all 256 DAC entries with `palette`.
pub fn load_palette() {
    out8(0x3C8, 0);
    for i in 0..=255u8 {
        let (r, g, b) = palette(i);
        out8(0x3C9, r);
        out8(0x3C9, g);
        out8(0x3C9, b);
    }
}

/// Waits for a key (INT 16h AH=00h) and returns its scan code and ASCII.
pub fn wait_key() -> u16 {
    let ax: u32;
    // SAFETY: BIOS keyboard read, reflected to real mode by the host; AX in and out, no pointers.
    unsafe { asm!("int 0x16", inout("eax") 0u32 => ax, options(nomem, nostack)) };
    ax as u16
}

/// DPMI 0800h: maps physical memory (for a linear frame buffer) and returns its linear address, or the DPMI error code.
pub fn map_physical(phys: u32, size: u32) -> Result<u32, u16> {
    let mut r = dpmi::Regs {
        eax: 0x0800,
        ebx: phys >> 16,
        ecx: phys & 0xFFFF,
        esi: size >> 16,
        edi: size & 0xFFFF,
        ..dpmi::Regs::default()
    };
    dpmi::int31(&mut r);
    if r.carry != 0 {
        Err(r.eax as u16)
    } else {
        Ok(((r.ebx & 0xFFFF) << 16) | (r.ecx & 0xFFFF))
    }
}

/// Text that is printed as it is written and kept to be written to a file.
#[derive(Default)]
pub struct Report(pub String);

impl Report {
    pub fn line(&mut self, args: core::fmt::Arguments) {
        let start = self.0.len();
        let _ = self.0.write_fmt(args);
        self.0.push('\n');
        // DOS wants CR LF on the console; the file keeps plain LF.
        rt::print!("{}\r\n", &self.0[start..self.0.len() - 1]);
    }

    /// Writes the report to a file in the current directory (replacing it) and closes it.
    pub fn save(&self, name: &str) {
        if let Ok(mut f) = File::open(name, Mode::Create) {
            let _ = f.write_all(self.0.as_bytes());
        }
    }
}

#[macro_export]
macro_rules! say {
    ($r:expr, $($arg:tt)*) => { $r.line(core::format_args!($($arg)*)) };
}

/// Adler-32 over bytes, as `rt::vga::checksum` does it.
pub fn adler(bytes: impl Iterator<Item = u8>) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for v in bytes {
        a = (a + v as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
