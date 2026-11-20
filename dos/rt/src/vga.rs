// VGA mode 13h (320x200, 256 colours) and the frame buffer at A0000h.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The mode switch is INT 10h, which the host reflects to the BIOS. The frame buffer is in the first megabyte, which CWSDPMI maps
// at linear 0, so a pointer into it is A0000h minus the image base through the 4 GiB data segment (the same arithmetic DJGPP's
// "near pointer" trick uses). No selector, and no physical mapping (DPMI 0800h), is needed below 1 MiB.
use crate::{cleanup, dpmi};
use core::arch::asm;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;

fn int10(ax: u32) -> u32 {
    let out: u32;
    // SAFETY: INT 10h video services are reflected to the BIOS by the host; AX in, AX out, EBX saved by hand.
    unsafe {
        asm!("push ebx", "int 0x10", "pop ebx", inout("eax") ax => out, out("ecx") _, out("edx") _);
    }
    out
}

/// The current video mode (INT 10h AH=0Fh).
pub fn get_mode() -> u8 {
    int10(0x0F00) as u8
}

/// Sets a video mode (INT 10h AH=00h).
pub fn set_mode(mode: u8) {
    int10(mode as u32);
}

/// Switches to mode 13h and registers the return to text mode with the clean-up list.
pub fn enter_mode13() {
    set_mode(0x13);
    cleanup::register(leave);
}

/// Returns to 80x25 text mode (mode 3). Safe to call twice.
pub fn leave() {
    if get_mode() != 3 {
        set_mode(3);
    }
    cleanup::forget(leave);
}

/// The frame buffer, 320 x 200 bytes.
pub fn framebuffer() -> *mut u8 {
    dpmi::linear_to_ptr::<u8>(0xA0000)
}

/// The test pattern's colour at (x, y): vertical bars of 16 columns' worth in the top half, a diagonal in the bottom half.
pub fn pattern(x: usize, y: usize) -> u8 {
    if y < HEIGHT / 2 {
        (x / 20 * 16 + y / 12) as u8
    } else {
        (x ^ y) as u8
    }
}

/// Writes the test pattern to the frame buffer.
pub fn draw_pattern() {
    let fb = framebuffer();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            // SAFETY: in range of the 64000-byte frame buffer; the pointer is valid in mode 13h.
            unsafe { fb.add(y * WIDTH + x).write_volatile(pattern(x, y)) };
        }
    }
}

/// A checksum (Adler-32) of the frame buffer as read back.
pub fn checksum() -> u32 {
    let fb = framebuffer();
    let (mut a, mut b) = (1u32, 0u32);
    for i in 0..WIDTH * HEIGHT {
        // SAFETY: as `draw_pattern`.
        let v = unsafe { fb.add(i).read_volatile() } as u32;
        a = (a + v) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// The same checksum over the pattern computed in memory, without touching the frame buffer.
pub fn expected_checksum() -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            a = (a + pattern(x, y) as u32) % 65521;
            b = (b + a) % 65521;
        }
    }
    (b << 16) | a
}
