// Getting back to text mode without the BIOS: the font in plane 2 and the DAC entries the text attribute palette points at.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `regs::set_mode_03h` programs the registers, but a graphics mode has overwritten what the BIOS loaded for text: plane 2 held the
// character generator (256 glyphs, each in a 32-byte slot of which the first 16 bytes are the 8 × 16 bitmap) and the DAC held the 16
// text colours. Both have to be written again, from a font the caller keeps (the native loader copies the BIOS's 8 × 16 font to
// low memory before it leaves real mode, because after that nothing can ask the BIOS for it).

use desk_hal::{HalError, MmioRegion, PortIo, Rgb6};

use crate::dac::set_palette;
use crate::regs::{gc, gc_write, seq, sequencer_write, MODE_03H};

/// Bytes in an 8 × 16 font: 256 glyphs of 16 rows.
pub const FONT_BYTES: usize = 4096;
/// Bytes one glyph takes in plane 2.
pub const GLYPH_SLOT: usize = 32;
/// Bytes of plane 2 the font occupies.
pub const PLANE_BYTES: usize = 256 * GLYPH_SLOT;

/// The 16 text colours as (DAC entry, colour) pairs, in the BIOS's arrangement: attribute palette entries 6 and 8 to 15 point at DAC
/// entries 14h and 38h to 3Fh (`regs::MODE_03H.attr`), the rest at their own number. The values are the standard VGA text colours
/// (two-thirds and one-third intensity, with brown as orange-ish 42, 21, 0).
pub const TEXT_DAC: [(u8, Rgb6); 16] = [
    (0x00, Rgb6::new(0, 0, 0)),
    (0x01, Rgb6::new(0, 0, 42)),
    (0x02, Rgb6::new(0, 42, 0)),
    (0x03, Rgb6::new(0, 42, 42)),
    (0x04, Rgb6::new(42, 0, 0)),
    (0x05, Rgb6::new(42, 0, 42)),
    (0x14, Rgb6::new(42, 21, 0)),
    (0x07, Rgb6::new(42, 42, 42)),
    (0x38, Rgb6::new(21, 21, 21)),
    (0x39, Rgb6::new(21, 21, 63)),
    (0x3A, Rgb6::new(21, 63, 21)),
    (0x3B, Rgb6::new(21, 63, 63)),
    (0x3C, Rgb6::new(63, 21, 21)),
    (0x3D, Rgb6::new(63, 21, 63)),
    (0x3E, Rgb6::new(63, 63, 21)),
    (0x3F, Rgb6::new(63, 63, 63)),
];

/// Writes the 16 text colours to their DAC entries.
pub fn set_text_palette<P: PortIo>(ports: &P) -> Result<(), HalError> {
    for (index, colour) in TEXT_DAC {
        set_palette(ports, index, &[colour])?;
    }
    Ok(())
}

/// Writes an 8 × 16 font (`FONT_BYTES`, glyph after glyph) into plane 2 and puts the sequencer and graphics controller back to the
/// text-mode values of `MODE_03H`. Call it after `set_mode_03h`.
///
/// The CPU only reaches plane 2 in the odd/even text layout through the odd/even trick, so for the copy the controller is set up
/// the way the BIOS does it: sequencer map mask = plane 2 only, memory mode = sequential (no odd/even, no chain-4), graphics controller
/// read map = 2, mode = 0 and memory map = A0000h for 64 KiB. `plane` must be the 64 KiB window at A0000h.
///
/// `InvalidArgument` if the font is not `FONT_BYTES` long or the window is smaller than `PLANE_BYTES`; nothing is written then.
pub fn load_font<P: PortIo>(ports: &P, plane: &mut MmioRegion, font: &[u8]) -> Result<(), HalError> {
    if font.len() != FONT_BYTES || plane.len() < PLANE_BYTES {
        return Err(HalError::InvalidArgument);
    }
    sequencer_write(ports, seq::MAP_MASK, 0x04);
    sequencer_write(ports, seq::MEMORY_MODE, 0x07);
    gc_write(ports, gc::READ_MAP_SELECT, 0x02);
    gc_write(ports, gc::MODE, 0x00);
    gc_write(ports, gc::MISC, 0x04);
    let mut slot = [0u8; GLYPH_SLOT];
    let (glyphs, _) = font.as_chunks::<16>();
    for (c, glyph) in glyphs.iter().enumerate() {
        slot[..16].copy_from_slice(glyph);
        plane.write_bytes(c * GLYPH_SLOT, &slot)?;
    }
    sequencer_write(ports, seq::MAP_MASK, MODE_03H.seq[2]);
    sequencer_write(ports, seq::MEMORY_MODE, MODE_03H.seq[4]);
    gc_write(ports, gc::READ_MAP_SELECT, 0x00);
    gc_write(ports, gc::MODE, MODE_03H.gc[5]);
    gc_write(ports, gc::MISC, MODE_03H.gc[6]);
    Ok(())
}
