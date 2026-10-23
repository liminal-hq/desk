// The VBE 2.0 layer: parsing the information and mode blocks from byte blobs, and the exact registers each call passes to the BIOS.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_hal::{HalError, ModeInfo, PixelLayout};
use desk_hal_mock::video::BiosReply;
use desk_hal_mock::MockBios;
use desk_vga::vbe::{self, FarPtr, Vbe, VbeInfo, VbeMode};

/// A VBE 2.0 controller information block as a BIOS returns it: the strings and the mode list are in its ROM at C000h.
fn info_block() -> Vec<u8> {
    let mut b = vec![0u8; 512];
    b[0..4].copy_from_slice(b"VESA");
    b[4..6].copy_from_slice(&0x0200u16.to_le_bytes());
    b[6..10].copy_from_slice(&[0x40, 0x01, 0x00, 0xC0]); // OEM string at C000:0140
    b[10..14].copy_from_slice(&1u32.to_le_bytes()); // DAC width switchable
    b[14..18].copy_from_slice(&[0x00, 0x01, 0x00, 0xC0]); // mode list at C000:0100
    b[18..20].copy_from_slice(&64u16.to_le_bytes()); // 4 MiB
    b
}

/// A mode information block: attributes, pitch, width, height, planes, bits, model and the frame buffer address.
fn mode_block(attrs: u16, pitch: u16, w: u16, h: u16, bpp: u8, model: u8, phys: u32) -> Vec<u8> {
    let mut b = vec![0u8; 256];
    b[0..2].copy_from_slice(&attrs.to_le_bytes());
    b[4..6].copy_from_slice(&64u16.to_le_bytes());
    b[6..8].copy_from_slice(&64u16.to_le_bytes());
    b[8..10].copy_from_slice(&0xA000u16.to_le_bytes());
    b[16..18].copy_from_slice(&pitch.to_le_bytes());
    b[18..20].copy_from_slice(&w.to_le_bytes());
    b[20..22].copy_from_slice(&h.to_le_bytes());
    b[24] = 1;
    b[25] = bpp;
    b[27] = model;
    b[40..44].copy_from_slice(&phys.to_le_bytes());
    b
}

const GRAPHICS_LFB: u16 = 0x009B; // supported, extended info, colour, graphics, linear frame buffer
const GRAPHICS_BANKED: u16 = 0x001B;

fn bios_with_modes() -> MockBios {
    let bios = MockBios::new();
    bios.put_low(0xC000, 0x0100, &[0x01, 0x01, 0x03, 0x01, 0x41, 0x01, 0x12, 0x01, 0xFF, 0xFF]);
    bios.put_low(0xC000, 0x0140, b"Mock OEM, Desk test\0");
    bios
}

#[test]
fn the_information_block_parses() {
    let info = VbeInfo::parse(&info_block()).unwrap();
    assert_eq!(info.version, 0x0200);
    assert_eq!(
        info.oem_string,
        FarPtr {
            segment: 0xC000,
            offset: 0x0140
        }
    );
    assert_eq!(
        info.mode_list,
        FarPtr {
            segment: 0xC000,
            offset: 0x0100
        }
    );
    assert_eq!(info.capabilities, 1);
    assert_eq!(info.memory_64k, 64);
}

#[test]
fn a_block_without_the_signature_is_no_vbe() {
    // A BIOS with no VBE returns without writing the buffer, so "VBE2" is still there.
    let mut b = vec![0u8; 512];
    b[0..4].copy_from_slice(b"VBE2");
    assert_eq!(VbeInfo::parse(&b), Err(HalError::NoDevice));
    assert_eq!(VbeInfo::parse(&b[..10]), Err(HalError::InvalidArgument));
}

#[test]
fn the_mode_block_parses_and_describes_a_linear_indexed_mode() {
    let m = VbeMode::parse(
        0x101,
        &mode_block(GRAPHICS_LFB, 640, 640, 480, 8, vbe::model::PACKED_PIXEL, 0xE000_0000),
    )
    .unwrap();
    assert_eq!((m.width, m.height, m.pitch, m.bits_per_pixel), (640, 480, 640, 8));
    assert_eq!(m.phys_base, 0xE000_0000);
    assert!(m.supported() && m.graphics() && m.linear());
    assert_eq!(m.frame_bytes(), 307_200);
    assert_eq!((m.win_granularity_kb, m.win_size_kb, m.win_a_segment), (64, 64, 0xA000));
    assert_eq!(
        m.mode_info(),
        Some(ModeInfo {
            id: 0x101,
            width: 640,
            height: 480,
            layout: PixelLayout::Indexed8,
            linear: true
        })
    );
}

#[test]
fn modes_the_desk_does_not_use_are_not_described() {
    let text = VbeMode::parse(0x108, &mode_block(0x000B, 160, 80, 60, 4, vbe::model::TEXT, 0)).unwrap();
    assert_eq!(text.mode_info(), None, "text mode");
    let unsupported = VbeMode::parse(
        0x105,
        &mode_block(0x0090 & !1, 1024, 1024, 768, 8, vbe::model::PACKED_PIXEL, 0xE000_0000),
    )
    .unwrap();
    assert_eq!(unsupported.mode_info(), None, "not supported on this hardware");
    let odd = VbeMode::parse(
        0x110,
        &mode_block(GRAPHICS_LFB, 1920, 640, 480, 15, vbe::model::DIRECT_COLOUR, 0xE000_0000),
    )
    .unwrap();
    assert_eq!(odd.mode_info(), None, "15-bit colour has no layout");
    let rgb565 = VbeMode::parse(
        0x111,
        &mode_block(GRAPHICS_LFB, 1280, 640, 480, 16, vbe::model::DIRECT_COLOUR, 0xE000_0000),
    )
    .unwrap();
    assert_eq!(rgb565.mode_info().map(|i| i.layout), Some(PixelLayout::Rgb565));
    let banked = VbeMode::parse(
        0x101,
        &mode_block(GRAPHICS_BANKED, 640, 640, 480, 8, vbe::model::PACKED_PIXEL, 0),
    )
    .unwrap();
    assert!(!banked.linear());
    assert_eq!(banked.mode_info().map(|i| i.linear), Some(false));
    // The bit says linear but there is no address: not usable as one.
    let no_addr = VbeMode::parse(
        0x101,
        &mode_block(GRAPHICS_LFB, 640, 640, 480, 8, vbe::model::PACKED_PIXEL, 0),
    )
    .unwrap();
    assert!(!no_addr.linear());
}

#[test]
fn info_asks_for_vbe_2_and_reads_the_strings_and_the_mode_list_from_the_bios_memory() {
    let bios = bios_with_modes();
    bios.push_reply(BiosReply::ok().with_buffer(info_block()));
    let vbe = Vbe::new(&bios);
    let info = vbe.info().unwrap();

    let calls = bios.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].vector, 0x10);
    assert_eq!(calls[0].regs.eax, 0x4F00);
    let sent = calls[0].buffer_in.as_ref().unwrap();
    assert_eq!(sent.len(), 512);
    assert_eq!(&sent[..4], b"VBE2");

    let mut ids = [0u16; 16];
    let n = vbe.mode_list(&info, &mut ids).unwrap();
    assert_eq!(&ids[..n], &[0x101, 0x103, 0x141, 0x112]);
    let mut oem = [0u8; 64];
    let n = vbe.oem_string(&info, &mut oem).unwrap();
    assert_eq!(&oem[..n], b"Mock OEM, Desk test");
    let mut short = [0u8; 4];
    assert_eq!(vbe.oem_string(&info, &mut short), Ok(4));
}

#[test]
fn the_mode_list_is_bounded_by_the_output_and_by_the_segment() {
    let bios = MockBios::new();
    bios.put_low(0xC000, 0x0100, &[0x01, 0x01, 0x03, 0x01, 0x05, 0x01, 0xFF, 0xFF]);
    let vbe = Vbe::new(&bios);
    let info = VbeInfo {
        version: 0x200,
        oem_string: FarPtr::default(),
        capabilities: 0,
        mode_list: FarPtr {
            segment: 0xC000,
            offset: 0x0100,
        },
        memory_64k: 1,
    };
    let mut two = [0u16; 2];
    assert_eq!(vbe.mode_list(&info, &mut two), Ok(2));
    assert_eq!(two, [0x101, 0x103]);
    // A list that never ends runs off the segment and stops with an error rather than looping.
    let runaway = VbeInfo {
        mode_list: FarPtr {
            segment: 0xC000,
            offset: 0xFFFE,
        },
        ..info
    };
    let mut many = [0u16; 8];
    let r = vbe.mode_list(&runaway, &mut many);
    assert!(r.is_ok() || r == Err(HalError::InvalidArgument));
    assert_eq!(
        vbe.oem_string(
            &VbeInfo {
                oem_string: FarPtr::default(),
                ..info
            },
            &mut many.map(|_| 0u8)[..]
        ),
        Ok(0)
    );
}

#[test]
fn mode_asks_4f01_with_the_mode_in_cx() {
    let bios = MockBios::new();
    bios.push_reply(BiosReply::ok().with_buffer(mode_block(
        GRAPHICS_LFB,
        640,
        640,
        480,
        8,
        vbe::model::PACKED_PIXEL,
        0xE000_0000,
    )));
    let m = Vbe::new(&bios).mode(0x101).unwrap();
    assert_eq!(m.id, 0x101);
    let c = &bios.calls()[0];
    assert_eq!((c.regs.eax, c.regs.ecx), (0x4F01, 0x101));
    assert_eq!(c.buffer_in.as_ref().unwrap().len(), 256);
}

#[test]
fn find_picks_the_first_matching_linear_mode_and_skips_failures() {
    let bios = MockBios::new();
    bios.push_reply(BiosReply::ok().with_buffer(mode_block(GRAPHICS_BANKED, 640, 640, 480, 8, vbe::model::PACKED_PIXEL, 0))); // 0x100: banked only
    bios.push_reply(BiosReply::ok().with_ax(0x024F)); // 0x101: the call fails
    bios.push_reply(BiosReply::ok().with_buffer(mode_block(
        GRAPHICS_LFB,
        800,
        800,
        600,
        8,
        vbe::model::PACKED_PIXEL,
        0xE000_0000,
    ))); // wrong size
    bios.push_reply(BiosReply::ok().with_buffer(mode_block(
        GRAPHICS_LFB,
        640,
        640,
        480,
        8,
        vbe::model::PACKED_PIXEL,
        0xE000_0000,
    ))); // 0x103
    let vbe = Vbe::new(&bios);
    let m = vbe.find(&[0x100, 0x101, 0x102, 0x103, 0x104], 640, 480, 8, true).unwrap();
    assert_eq!(m.id, 0x103);
    assert_eq!(bios.calls().len(), 4, "stops at the match");

    let bios = MockBios::new();
    bios.push_reply(BiosReply::ok().with_buffer(mode_block(GRAPHICS_BANKED, 640, 640, 480, 8, vbe::model::PACKED_PIXEL, 0)));
    assert!(
        Vbe::new(&bios).find(&[0x100], 640, 480, 8, true).is_none(),
        "banked only, but a linear mode was required"
    );
    let bios = MockBios::new();
    bios.push_reply(BiosReply::ok().with_buffer(mode_block(GRAPHICS_BANKED, 640, 640, 480, 8, vbe::model::PACKED_PIXEL, 0)));
    assert_eq!(Vbe::new(&bios).find(&[0x100], 640, 480, 8, false).map(|m| m.id), Some(0x100));
}

#[test]
fn set_mode_adds_the_linear_bit() {
    let bios = MockBios::new();
    bios.push_reply(BiosReply::ok());
    bios.push_reply(BiosReply::ok());
    let vbe = Vbe::new(&bios);
    vbe.set_mode(0x101, true).unwrap();
    vbe.set_mode(0x101, false).unwrap();
    let calls = bios.calls();
    assert_eq!((calls[0].regs.eax, calls[0].regs.ebx), (0x4F02, 0x4101));
    assert_eq!((calls[1].regs.eax, calls[1].regs.ebx), (0x4F02, 0x0101));
    assert!(calls[0].buffer_in.is_none());
}

#[test]
fn current_mode_and_window_calls() {
    let bios = MockBios::new();
    bios.push_reply(BiosReply {
        eax: 0x004F,
        ebx: 0x4101,
        ..BiosReply::default()
    });
    bios.push_reply(BiosReply::ok());
    let vbe = Vbe::new(&bios);
    assert_eq!(vbe.current_mode(), Ok(0x4101));
    vbe.set_window(0, 3).unwrap();
    let c = &bios.calls();
    assert_eq!(c[0].regs.eax, 0x4F03);
    assert_eq!((c[1].regs.eax, c[1].regs.ebx, c[1].regs.edx), (0x4F05, 0, 3));
}

#[test]
fn failures_are_told_apart() {
    assert_eq!(vbe::status(0x004F), Ok(()));
    assert_eq!(vbe::status(0x014F), Err(HalError::Platform(1)), "function failed");
    assert_eq!(
        vbe::status(0x024F),
        Err(HalError::Platform(2)),
        "not supported by the hardware"
    );
    assert_eq!(vbe::status(0x034F), Err(HalError::Platform(3)), "invalid in the current mode");
    assert_eq!(
        vbe::status(0x0000),
        Err(HalError::Unsupported),
        "AL is not 4Fh: no VBE at all"
    );
    assert_eq!(vbe::status(0x1003), Err(HalError::Unsupported));
}

#[test]
fn a_bios_that_cannot_make_the_call_is_an_error_not_a_panic() {
    let bios = MockBios::new();
    assert_eq!(Vbe::new(&bios).info(), Err(HalError::Unsupported));
    assert_eq!(Vbe::new(&bios).set_mode(0x101, true), Err(HalError::Unsupported));
}
