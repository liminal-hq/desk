// The DAC: loading and reading the palette, waiting for vertical retrace, and fades.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The Desk owns the palette and fades through the DAC, never through the frame buffer (docs/PALETTE.md): 768 six-bit values
// scaled over 16 frames. The DAC is addressed by writing an index to 0x3C8 (for writes) or 0x3C7 (for reads), then transferring
// red, green and blue in turn through 0x3C9; the index advances after each triple, so a whole palette is one index write and 768
// data transfers.

use desk_hal::{HalError, PortIo, Rgb6};

use crate::regs::INPUT_STATUS_1;

pub const PEL_MASK: u16 = 0x3C6;
pub const DAC_READ_INDEX: u16 = 0x3C7;
pub const DAC_WRITE_INDEX: u16 = 0x3C8;
pub const DAC_DATA: u16 = 0x3C9;

/// Bit 3 of the input status register 1: vertical retrace is in progress.
pub const VRETRACE: u8 = 0x08;

/// The number of reads of 0x3DA a wait for vertical retrace makes in each of its two phases before giving up. A frame at 60 Hz
/// is 16.7 ms and an ISA read takes about a microsecond, so a working display shows a retrace within roughly 17,000 reads; the
/// bound is far above that and still ends a dead one in about a second on a 486.
pub const VRETRACE_POLLS: u32 = 1_000_000;

/// The size of a full palette.
pub const ENTRIES: usize = 256;

/// Loads `colours` into the DAC from entry `first`. `InvalidArgument` if they run past entry 255; nothing is written then.
pub fn set_palette<P: PortIo>(ports: &P, first: u8, colours: &[Rgb6]) -> Result<(), HalError> {
    if usize::from(first) + colours.len() > ENTRIES {
        return Err(HalError::InvalidArgument);
    }
    if colours.is_empty() {
        return Ok(());
    }
    ports.outb(DAC_WRITE_INDEX, first);
    for c in colours {
        ports.outb(DAC_DATA, c.r & 63);
        ports.outb(DAC_DATA, c.g & 63);
        ports.outb(DAC_DATA, c.b & 63);
    }
    Ok(())
}

/// Reads `out.len()` entries from the DAC starting at `first`.
pub fn read_palette<P: PortIo>(ports: &P, first: u8, out: &mut [Rgb6]) -> Result<(), HalError> {
    if usize::from(first) + out.len() > ENTRIES {
        return Err(HalError::InvalidArgument);
    }
    if out.is_empty() {
        return Ok(());
    }
    ports.outb(DAC_READ_INDEX, first);
    for c in out.iter_mut() {
        c.r = ports.inb(DAC_DATA) & 63;
        c.g = ports.inb(DAC_DATA) & 63;
        c.b = ports.inb(DAC_DATA) & 63;
    }
    Ok(())
}

/// Waits for the start of the next vertical retrace: first until any retrace in progress has ended, then until the next one begins,
/// so that a write that follows has the whole retrace to itself. `Timeout` if either phase makes `VRETRACE_POLLS` reads without
/// seeing the bit change (no display, a stuck register, a card that does not implement the bit).
pub fn wait_vretrace<P: PortIo>(ports: &P) -> Result<(), HalError> {
    wait_vretrace_bounded(ports, VRETRACE_POLLS)
}

/// As `wait_vretrace`, with the number of polls per phase chosen by the caller.
pub fn wait_vretrace_bounded<P: PortIo>(ports: &P, polls: u32) -> Result<(), HalError> {
    let mut n = 0;
    while ports.inb(INPUT_STATUS_1) & VRETRACE != 0 {
        n += 1;
        if n >= polls {
            return Err(HalError::Timeout);
        }
    }
    n = 0;
    while ports.inb(INPUT_STATUS_1) & VRETRACE == 0 {
        n += 1;
        if n >= polls {
            return Err(HalError::Timeout);
        }
    }
    Ok(())
}

/// The Desk's fade length: 16 frames.
pub const FADE_STEPS: u32 = 16;

/// Loads `base` with every channel scaled by `num / den` (rounded down) into entries 0 to 255. `num = den` is `base` exactly and
/// `num = 0` is black. 768 data transfers after one index write.
pub fn set_palette_scaled<P: PortIo>(ports: &P, base: &[Rgb6; ENTRIES], num: u32, den: u32) {
    ports.outb(DAC_WRITE_INDEX, 0);
    for c in base {
        let c = c.scaled(num, den);
        ports.outb(DAC_DATA, c.r);
        ports.outb(DAC_DATA, c.g);
        ports.outb(DAC_DATA, c.b);
    }
}

/// How a fade ended: how many of its frames waited the full retrace and how many gave up (a display with no retrace bit still
/// fades, just without sync).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FadeReport {
    pub synced: u32,
    pub timed_out: u32,
}

/// Fades the palette from `base` to black over `steps` frames, one `wait_vretrace` before each write; the last frame is black.
/// Frame `i` (1-based) shows `base` scaled by `(steps - i) / steps`.
pub fn fade_out<P: PortIo>(ports: &P, base: &[Rgb6; ENTRIES], steps: u32) -> FadeReport {
    fade(ports, base, steps, true)
}

/// Fades from black up to `base` over `steps` frames; the last frame is `base` exactly.
pub fn fade_in<P: PortIo>(ports: &P, base: &[Rgb6; ENTRIES], steps: u32) -> FadeReport {
    fade(ports, base, steps, false)
}

fn fade<P: PortIo>(ports: &P, base: &[Rgb6; ENTRIES], steps: u32, out: bool) -> FadeReport {
    let mut report = FadeReport { synced: 0, timed_out: 0 };
    for i in 1..=steps {
        match wait_vretrace(ports) {
            Ok(()) => report.synced += 1,
            Err(_) => report.timed_out += 1,
        }
        let num = if out { steps - i } else { i };
        set_palette_scaled(ports, base, num, steps);
    }
    report
}
