// Mode 03h and the text restore: the register image, the font layout in plane 2 and the registers left behind.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_hal::{HalError, MmioRegion};
use desk_hal_mock::{MockPortBus, VgaModel};
use desk_vga::regs::{self, MODE_03H};
use desk_vga::text::{self, FONT_BYTES, GLYPH_SLOT, PLANE_BYTES};

fn region(buf: &mut [u8]) -> MmioRegion {
    // SAFETY: the buffer outlives the region in every test and is not otherwise touched while it is in use.
    unsafe { MmioRegion::new(buf.as_mut_ptr(), buf.len()) }
}

#[test]
fn mode_03h_programs_the_text_registers() {
    let bus = MockPortBus::new();
    let vga = bus.attach(0x3C0, 0x3DF, VgaModel::new());
    regs::set_mode_03h(&bus);
    let vga = vga.borrow();
    assert_eq!(vga.misc, 0x67);
    assert_eq!(vga.seq, MODE_03H.seq);
    assert_eq!(vga.crtc, MODE_03H.crtc);
    assert_eq!(vga.gc, MODE_03H.gc);
    assert_eq!(vga.ac, MODE_03H.attr);
    assert_eq!(vga.refused_crtc_writes, 0);
    assert!(vga.ac_video_enabled);
    // 80 columns of 9 dots, 25 rows of 16 lines: the numbers the table has to agree with.
    assert_eq!(MODE_03H.crtc[1] + 1, 80, "horizontal display end");
    assert_eq!(MODE_03H.crtc[9] & 0x1F, 15, "maximum scan line: 16-line cells");
    assert_eq!(MODE_03H.seq[1] & 1, 0, "9-dot characters");
    assert_eq!(MODE_03H.gc[6] & 0x0C, 0x0C, "memory map B8000h");
}

#[test]
fn the_font_lands_in_32_byte_slots_and_the_registers_return_to_text_values() {
    let bus = MockPortBus::new();
    let vga = bus.attach(0x3C0, 0x3DF, VgaModel::new());
    regs::set_mode_03h(&bus);
    let font: Vec<u8> = (0..FONT_BYTES).map(|i| (i % 251) as u8 | 1).collect();
    let mut plane = vec![0xEEu8; 0x10000];
    text::load_font(&bus, &mut region(&mut plane), &font).unwrap();
    for c in 0..256 {
        assert_eq!(
            &plane[c * GLYPH_SLOT..c * GLYPH_SLOT + 16],
            &font[c * 16..c * 16 + 16],
            "glyph {c}"
        );
        assert!(
            plane[c * GLYPH_SLOT + 16..(c + 1) * GLYPH_SLOT].iter().all(|b| *b == 0),
            "slot {c} padding is cleared"
        );
    }
    assert_eq!(plane[PLANE_BYTES], 0xEE, "nothing past the font is touched");
    let vga = vga.borrow();
    assert_eq!(vga.seq, MODE_03H.seq, "map mask and memory mode are back");
    assert_eq!(vga.gc, MODE_03H.gc, "read map, mode and memory map are back");
}

#[test]
fn a_font_of_the_wrong_size_or_a_small_window_writes_nothing() {
    let bus = MockPortBus::new();
    let mut plane = vec![0u8; 0x10000];
    assert_eq!(
        text::load_font(&bus, &mut region(&mut plane), &[0; FONT_BYTES - 1]),
        Err(HalError::InvalidArgument)
    );
    let mut small = vec![0u8; PLANE_BYTES - 1];
    assert_eq!(
        text::load_font(&bus, &mut region(&mut small), &[0; FONT_BYTES]),
        Err(HalError::InvalidArgument)
    );
    assert!(bus.writes().is_empty());
}

#[test]
fn the_text_palette_reaches_the_dac_entries_the_attribute_palette_uses() {
    let bus = MockPortBus::new();
    let vga = bus.attach(0x3C0, 0x3DF, VgaModel::new());
    text::set_text_palette(&bus).unwrap();
    let vga = vga.borrow();
    // Light grey (attribute colour 7) is DAC entry 7; bright white (15) goes through attribute entry 15 = 3Fh.
    assert_eq!(MODE_03H.attr[7], 0x07);
    assert_eq!(MODE_03H.attr[15], 0x3F);
    assert_eq!(MODE_03H.attr[6], 0x14);
    let entry = |i: usize| vga.dac[i];
    assert_eq!(entry(0x07), [42, 42, 42]);
    assert_eq!(entry(0x3F), [63, 63, 63]);
    assert_eq!(entry(0x14), [42, 21, 0]);
    for (i, (dac_index, _)) in text::TEXT_DAC.iter().enumerate() {
        assert_eq!(*dac_index, MODE_03H.attr[i], "colour {i}");
    }
}
