// The BIOS keyboard state in the BIOS data area, for a program that takes IRQ 1 from the BIOS.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// While the BIOS keyboard handler is not running, nothing keeps these bytes up to date, and DOS reads them when the program is
// over: a Shift whose release the BIOS never saw would stay "down" at the prompt. The policy (docs/B-DOS-INPUT.md): read the lock
// flags at start so the decoder agrees with the LEDs, and at exit put the bytes back as found, with the momentary bits cleared and
// the type-ahead buffer emptied. All of it is plain memory in the first megabyte, reached through the 4 GiB data segment.
use crate::dpmi;
use core::ptr::{read_volatile, write_volatile};
use desk_hal::Modifiers;

/// 0040:0017, the shift flags: bit 0 right Shift, 1 left Shift, 2 Ctrl, 3 Alt, 4 Scroll Lock, 5 Num Lock, 6 Caps Lock, 7 Insert.
const FLAGS: u32 = 0x417;
/// 0040:0018, more flags: bit 0 left Ctrl, 1 left Alt, 2 SysRq, 3 Pause, bits 4 to 7 "key is down" for the four toggles.
const FLAGS2: u32 = 0x418;
/// 0040:001A and 001C, the head and tail of the type-ahead ring (offsets into the segment, 001Eh to 003Dh).
const HEAD: u32 = 0x41A;
const TAIL: u32 = 0x41C;
/// Bits of the flags that are locks (and Insert), which survive; the rest are keys held down.
const STATE_BITS: u8 = 0xF0;

fn byte(linear: u32) -> *mut u8 {
    dpmi::linear_to_ptr::<u8>(linear)
}

fn word(linear: u32) -> *mut u16 {
    dpmi::linear_to_ptr::<u16>(linear)
}

/// What the BIOS data area held when the program took the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saved {
    flags: u8,
}

/// The shift flag byte now.
pub fn flags() -> u8 {
    // SAFETY: the BIOS data area is conventional memory, mapped at linear 0.
    unsafe { read_volatile(byte(FLAGS)) }
}

/// The lock keys as `Modifiers`, for `Decoder::with_locks`.
pub fn locks() -> Modifiers {
    let f = flags();
    let mut m = Modifiers::NONE;
    if f & 0x10 != 0 {
        m = m.union(Modifiers::SCROLL);
    }
    if f & 0x20 != 0 {
        m = m.union(Modifiers::NUM);
    }
    if f & 0x40 != 0 {
        m = m.union(Modifiers::CAPS);
    }
    m
}

pub fn save() -> Saved {
    Saved { flags: flags() }
}

impl Saved {
    /// The flag byte that was found.
    pub fn flags(self) -> u8 {
        self.flags
    }

    /// A saved state from a flag byte the caller kept.
    pub fn from_flags(flags: u8) -> Saved {
        Saved { flags }
    }
}

/// The number of keys in the BIOS type-ahead buffer.
pub fn buffered() -> usize {
    // SAFETY: as `flags`.
    let (head, tail) = unsafe { (read_volatile(word(HEAD)), read_volatile(word(TAIL))) };
    let span = 0x20u16; // 001Eh to 003Dh, in bytes: 16 keys of two bytes
    ((tail.wrapping_sub(head)) % span / 2) as usize
}

/// Empties the type-ahead buffer by moving the head to the tail (what INT 16h AH=0Ch does). A BIOS handler that runs in the
/// middle can add a key we then miss; the next call takes it.
pub fn flush() {
    // SAFETY: as `flags`.
    unsafe { write_volatile(word(HEAD), read_volatile(word(TAIL))) };
}

/// Puts the keyboard state back as `saved` found it: locks and Insert as they were (the LEDs were not touched, so they agree), no
/// key held, an empty buffer.
pub fn restore(saved: Saved) {
    // SAFETY: as `flags`.
    unsafe {
        write_volatile(byte(FLAGS), saved.flags & STATE_BITS);
        write_volatile(byte(FLAGS2), 0);
    }
    flush();
}
