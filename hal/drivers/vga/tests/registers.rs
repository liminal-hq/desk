// The exact register writes that set VGA mode 13h and mode 12h, checked against the model of the register file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_hal::PortIo;
use desk_hal_mock::{Access, MockPortBus, VgaModel};
use desk_vga::regs::{self, ModeRegs, MODE_12H, MODE_13H};

fn bus() -> (MockPortBus, std::rc::Rc<std::cell::RefCell<VgaModel>>) {
    let bus = MockPortBus::new();
    let vga = bus.attach(0x3C0, 0x3DF, VgaModel::new());
    (bus, vga)
}

/// The writes a mode set makes, in order, built from the table and the sequence the documentation gives (not from the driver's
/// loops), so the two can disagree.
fn expected_writes(m: &ModeRegs) -> Vec<(u16, u32)> {
    let mut w: Vec<(u16, u32)> = vec![(0x3C0, 0x00), (0x3C4, 0), (0x3C5, 1), (0x3C2, m.misc.into())];
    for i in 1..5 {
        w.push((0x3C4, i));
        w.push((0x3C5, m.seq[i as usize].into()));
    }
    w.push((0x3C4, 0));
    w.push((0x3C5, m.seq[0].into()));
    w.push((0x3D4, 0x11));
    w.push((0x3D5, (m.crtc[0x11] & 0x7F).into()));
    for i in 0..25 {
        w.push((0x3D4, i));
        w.push((0x3D5, m.crtc[i as usize].into()));
    }
    for i in 0..9 {
        w.push((0x3CE, i));
        w.push((0x3CF, m.gc[i as usize].into()));
    }
    for i in 0..21 {
        w.push((0x3C0, i));
        w.push((0x3C0, m.attr[i as usize].into()));
    }
    w.push((0x3C0, 0x20));
    w.push((0x3C6, 0xFF));
    w
}

#[test]
fn mode_13h_writes_the_documented_sequence() {
    let (bus, vga) = bus();
    regs::set_mode_13h(&bus);
    let w = bus.writes();
    assert_eq!(w.len(), 128);
    assert_eq!(w, expected_writes(&MODE_13H));
    // The first writes, spelled out: video off, sequencer in reset, misc output, then the clocking mode.
    assert_eq!(
        &w[..8],
        &[
            (0x3C0, 0x00),
            (0x3C4, 0),
            (0x3C5, 1),
            (0x3C2, 0x63),
            (0x3C4, 1),
            (0x3C5, 0x01),
            (0x3C4, 2),
            (0x3C5, 0x0F)
        ]
    );
    // The last: attribute registers 0x14, the video-on write and the DAC mask.
    assert_eq!(
        &w[w.len() - 4..],
        &[(0x3C0, 0x14), (0x3C0, 0x00), (0x3C0, 0x20), (0x3C6, 0xFF)]
    );
    let vga = vga.borrow();
    assert_eq!(vga.misc, 0x63);
    assert_eq!(vga.seq, MODE_13H.seq);
    assert_eq!(vga.crtc, MODE_13H.crtc);
    assert_eq!(vga.gc, MODE_13H.gc);
    assert_eq!(vga.ac, MODE_13H.attr);
    assert!(vga.ac_video_enabled);
    assert_eq!(vga.refused_crtc_writes, 0, "the CRTC was unlocked before registers 0 to 7");
    assert_eq!(vga.seq[4], 0x0E, "chain-4 and no odd/even");
    assert_eq!(vga.gc[5], 0x40, "256-colour shift register");
}

#[test]
fn mode_12h_writes_the_documented_sequence() {
    let (bus, vga) = bus();
    regs::set_mode_12h(&bus);
    assert_eq!(bus.writes(), expected_writes(&MODE_12H));
    let vga = vga.borrow();
    assert_eq!(vga.misc, 0xE3, "480 lines: negative vertical sync, positive horizontal sync");
    assert_eq!(vga.seq, MODE_12H.seq);
    assert_eq!(vga.crtc, MODE_12H.crtc);
    assert_eq!(vga.gc, MODE_12H.gc);
    assert_eq!(vga.ac, MODE_12H.attr);
    assert_eq!(vga.refused_crtc_writes, 0);
    assert_eq!(vga.crtc[0x12], 0xDF, "vertical display end: 479");
    assert_eq!(vga.seq[2], 0x0F, "all four planes enabled");
    assert_eq!(
        &vga.ac[..16],
        &(0..16).collect::<Vec<u8>>()[..],
        "colour n selects DAC entry n"
    );
}

#[test]
fn the_register_image_reads_back_equal() {
    for table in [&MODE_13H, &MODE_12H] {
        let (bus, _vga) = bus();
        regs::set_mode_registers(&bus, table);
        assert_eq!(&regs::read_mode_registers(&bus), table);
    }
}

#[test]
fn the_model_refuses_protected_crtc_writes() {
    // Why the driver unlocks first: with bit 7 of register 0x11 set, registers 0 to 7 ignore writes.
    let (bus, vga) = bus();
    bus.outb(0x3D4, 0x11);
    bus.outb(0x3D5, 0x8C);
    bus.outb(0x3D4, 0x00);
    bus.outb(0x3D5, 0x77);
    assert_eq!(vga.borrow().crtc[0], 0);
    assert_eq!(vga.borrow().refused_crtc_writes, 1);
}

#[test]
fn the_attribute_flip_flop_is_reset_by_reading_status() {
    let (bus, vga) = bus();
    bus.outb(0x3C0, 0x03); // index 3
    assert!(vga.borrow().ac_expects_data());
    let _ = bus.inb(0x3DA);
    assert!(!vga.borrow().ac_expects_data());
}

#[test]
fn the_identity_attribute_palette_is_programmed_without_touching_anything_else() {
    let (bus, vga) = bus();
    regs::set_mode_12h(&bus);
    // As the BIOS leaves EGA-compatible modes: colour 6 selects DAC entry 0x14, colours 8 to 15 entries 0x38 to 0x3F.
    for (i, v) in [0, 1, 2, 3, 4, 5, 0x14, 7, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F]
        .iter()
        .enumerate()
    {
        vga.borrow_mut().ac[i] = *v;
    }
    bus.take_log();
    regs::attribute_palette_identity(&bus);
    let w = bus.writes();
    assert_eq!(w.len(), 33);
    assert_eq!(&w[..4], &[(0x3C0, 0), (0x3C0, 0), (0x3C0, 1), (0x3C0, 1)]);
    assert_eq!(w[w.len() - 1], (0x3C0, 0x20));
    assert_eq!(&vga.borrow().ac[..16], &(0..16).collect::<Vec<u8>>()[..]);
    assert_eq!(vga.borrow().ac[0x10], 0x01, "mode control untouched");
    // The status reads that reset the flip-flop are the only reads.
    assert!(bus
        .log()
        .iter()
        .all(|a| matches!(a, Access::In { port: 0x3DA, .. } | Access::Out { .. })));
}
