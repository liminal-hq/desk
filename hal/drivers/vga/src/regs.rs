// The VGA register tables for mode 13h and mode 12h, and the sequence that programs them through the ports.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The values are the standard ones for these modes (the IBM VGA register set as documented in FreeVGA's register references and
// used by the Linux console, SVGALIB and every mode-setting tutorial since). They are not copied from one source: they were
// checked against what DOSBox Staging's BIOS programs for the same modes (see `docs/A-VGA-VBE.md`), and where this table differs
// from a BIOS (the attribute palette in mode 12h) the difference is deliberate and listed there.
//
// The sequence follows the IBM VGA technical reference: video off, synchronous reset, miscellaneous output, sequencer, unlock the
// CRTC, CRTC, graphics controller, attribute controller with the flip-flop reset first, then video on.

use desk_hal::PortIo;

pub const ATTR_ADDR: u16 = 0x3C0;
pub const ATTR_READ: u16 = 0x3C1;
pub const MISC_OUTPUT: u16 = 0x3C2;
pub const SEQ_ADDR: u16 = 0x3C4;
pub const SEQ_DATA: u16 = 0x3C5;
pub const MISC_READ: u16 = 0x3CC;
pub const GC_ADDR: u16 = 0x3CE;
pub const GC_DATA: u16 = 0x3CF;
pub const CRTC_ADDR: u16 = 0x3D4;
pub const CRTC_DATA: u16 = 0x3D5;
pub const INPUT_STATUS_1: u16 = 0x3DA;

/// Sequencer register numbers.
pub mod seq {
    pub const RESET: u8 = 0;
    pub const CLOCKING: u8 = 1;
    pub const MAP_MASK: u8 = 2;
    pub const CHAR_MAP: u8 = 3;
    pub const MEMORY_MODE: u8 = 4;
}

/// Graphics controller register numbers.
pub mod gc {
    pub const SET_RESET: u8 = 0;
    pub const ENABLE_SET_RESET: u8 = 1;
    pub const COLOUR_COMPARE: u8 = 2;
    pub const DATA_ROTATE: u8 = 3;
    pub const READ_MAP_SELECT: u8 = 4;
    pub const MODE: u8 = 5;
    pub const MISC: u8 = 6;
    pub const COLOUR_DONT_CARE: u8 = 7;
    pub const BIT_MASK: u8 = 8;
}

/// The CRTC register that holds the write-protect bit for registers 0 to 7.
const CRTC_VERTICAL_RETRACE_END: u8 = 0x11;

/// A complete register image for one mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeRegs {
    pub misc: u8,
    pub seq: [u8; 5],
    pub crtc: [u8; 25],
    pub gc: [u8; 9],
    /// Attribute registers 0x00 to 0x14: 16 palette entries, mode control, overscan, colour plane enable, horizontal pan, colour
    /// select.
    pub attr: [u8; 21],
}

/// Mode 13h: 320 × 200, 256 colours, chain-4 (one byte per pixel, linear at A0000h), 70 Hz, 400 lines doubled to 200.
pub const MODE_13H: ModeRegs = ModeRegs {
    misc: 0x63,
    seq: [0x03, 0x01, 0x0F, 0x00, 0x0E],
    crtc: [
        0x5F, 0x4F, 0x50, 0x82, 0x54, 0x80, 0xBF, 0x1F, 0x00, 0x41, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x9C, 0x8E, 0x8F, 0x28,
        0x40, 0x96, 0xB9, 0xA3, 0xFF,
    ],
    gc: [0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x05, 0x0F, 0xFF],
    attr: [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x41, 0x00, 0x0F, 0x00,
        0x00,
    ],
};

/// Mode 12h: 640 × 480, 16 colours, four planes at A0000h, 60 Hz. The attribute palette is the identity (colour n selects DAC
/// entry n), not the BIOS's EGA-compatible one (which sends colour 6 to entry 0x14 and 8 to 15 to entries 0x38 to 0x3F), because
/// the Desk programs DAC entries 0 to 15 itself (docs/PALETTE.md).
pub const MODE_12H: ModeRegs = ModeRegs {
    misc: 0xE3,
    seq: [0x03, 0x01, 0x0F, 0x00, 0x06],
    crtc: [
        0x5F, 0x4F, 0x50, 0x82, 0x54, 0x80, 0x0B, 0x3E, 0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xEA, 0x8C, 0xDF, 0x28,
        0x00, 0xE7, 0x04, 0xE3, 0xFF,
    ],
    gc: [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x0F, 0xFF],
    attr: [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x01, 0x00, 0x0F, 0x00,
        0x00,
    ],
};

fn seq_write<P: PortIo>(ports: &P, index: u8, value: u8) {
    ports.outb(SEQ_ADDR, index);
    ports.outb(SEQ_DATA, value);
}

fn crtc_write<P: PortIo>(ports: &P, index: u8, value: u8) {
    ports.outb(CRTC_ADDR, index);
    ports.outb(CRTC_DATA, value);
}

/// Writes a graphics controller register (index to 0x3CE, value to 0x3CF).
pub fn gc_write<P: PortIo>(ports: &P, index: u8, value: u8) {
    ports.outb(GC_ADDR, index);
    ports.outb(GC_DATA, value);
}

/// Writes a sequencer register.
pub fn sequencer_write<P: PortIo>(ports: &P, index: u8, value: u8) {
    seq_write(ports, index, value);
}

/// Programs the VGA registers for `mode` through the ports, with no BIOS call. Video is off while it runs and on at the end. The
/// DAC palette and the frame buffer are not touched: the caller loads a palette and clears the screen.
///
/// The exact order of writes (asserted by the host tests):
///
/// 1. the attribute controller's flip-flop is reset (read 0x3DA) and video is turned off (0x00 to 0x3C0);
/// 2. the sequencer is held in reset (register 0 = 1), the miscellaneous output register is written, registers 1 to 4 follow, and
///    the reset is released (register 0 = 3);
/// 3. CRTC register 0x11 is written with its protect bit clear, then registers 0 to 24 in order;
/// 4. graphics controller registers 0 to 8;
/// 5. the flip-flop is reset again and the 21 attribute registers are written as an index byte then a data byte, and 0x20 to 0x3C0
///    turns video on;
/// 6. the DAC mask is set to 0xFF.
pub fn set_mode_registers<P: PortIo>(ports: &P, mode: &ModeRegs) {
    // 1. Video off: the attribute controller's palette-address-source bit clear blanks the screen.
    let _ = ports.inb(INPUT_STATUS_1);
    ports.outb(ATTR_ADDR, 0x00);

    // 2. Sequencer: synchronous reset while the clock source (miscellaneous output) changes.
    seq_write(ports, seq::RESET, 0x01);
    ports.outb(MISC_OUTPUT, mode.misc);
    for (i, v) in mode.seq.iter().enumerate().skip(1) {
        seq_write(ports, i as u8, *v);
    }
    seq_write(ports, seq::RESET, mode.seq[0]);

    // 3. CRTC: registers 0 to 7 ignore writes while bit 7 of register 0x11 is set, so clear it first.
    crtc_write(
        ports,
        CRTC_VERTICAL_RETRACE_END,
        mode.crtc[usize::from(CRTC_VERTICAL_RETRACE_END)] & 0x7F,
    );
    for (i, v) in mode.crtc.iter().enumerate() {
        crtc_write(ports, i as u8, *v);
    }

    // 4. Graphics controller.
    for (i, v) in mode.gc.iter().enumerate() {
        gc_write(ports, i as u8, *v);
    }

    // 5. Attribute controller: the address and data registers share port 0x3C0 and a flip-flop that a read of 0x3DA resets. The
    // palette address source bit (0x20) is left clear while registers are written, which keeps video off.
    let _ = ports.inb(INPUT_STATUS_1);
    for (i, v) in mode.attr.iter().enumerate() {
        ports.outb(ATTR_ADDR, i as u8);
        ports.outb(ATTR_ADDR, *v);
    }
    ports.outb(ATTR_ADDR, 0x20);

    // 6. Every DAC entry is reachable.
    ports.outb(crate::dac::PEL_MASK, 0xFF);
}

/// Programs mode 13h (320 × 200 × 256).
pub fn set_mode_13h<P: PortIo>(ports: &P) {
    set_mode_registers(ports, &MODE_13H);
}

/// Programs mode 12h (640 × 480 × 16, planar).
pub fn set_mode_12h<P: PortIo>(ports: &P) {
    set_mode_registers(ports, &MODE_12H);
}

/// Makes attribute palette entry n select DAC entry n for n in 0 to 15 (what the BIOS does not do in the EGA-compatible modes),
/// so that the 16 colours the Desk loads into DAC entries 0 to 15 are the 16 colours on the screen.
pub fn attribute_palette_identity<P: PortIo>(ports: &P) {
    let _ = ports.inb(INPUT_STATUS_1);
    for i in 0..16u8 {
        ports.outb(ATTR_ADDR, i);
        ports.outb(ATTR_ADDR, i);
    }
    ports.outb(ATTR_ADDR, 0x20);
}

/// The registers as read back from the hardware, in the same layout as a table.
pub fn read_mode_registers<P: PortIo>(ports: &P) -> ModeRegs {
    let mut r = ModeRegs {
        misc: ports.inb(MISC_READ),
        seq: [0; 5],
        crtc: [0; 25],
        gc: [0; 9],
        attr: [0; 21],
    };
    for (i, v) in r.seq.iter_mut().enumerate() {
        ports.outb(SEQ_ADDR, i as u8);
        *v = ports.inb(SEQ_DATA);
    }
    for (i, v) in r.crtc.iter_mut().enumerate() {
        ports.outb(CRTC_ADDR, i as u8);
        *v = ports.inb(CRTC_DATA);
    }
    for (i, v) in r.gc.iter_mut().enumerate() {
        ports.outb(GC_ADDR, i as u8);
        *v = ports.inb(GC_DATA);
    }
    for (i, v) in r.attr.iter_mut().enumerate() {
        // Reading 0x3C1 does not move the flip-flop but the index write does, so it is reset before every index. The index keeps
        // video on (0x20) so reading the registers does not blank the screen.
        let _ = ports.inb(INPUT_STATUS_1);
        ports.outb(ATTR_ADDR, i as u8 | 0x20);
        *v = ports.inb(ATTR_READ);
    }
    // Leave the flip-flop expecting an index, with video on.
    let _ = ports.inb(INPUT_STATUS_1);
    ports.outb(ATTR_ADDR, 0x20);
    r
}
