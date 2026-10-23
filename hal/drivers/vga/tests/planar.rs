// Mode 12h's plane writer: the register sequence of each primitive, and which bytes of the window it touches.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The window here is plain memory, not a model of the four planes, so these tests cannot see what the controller would do with the
// writes. They check what the driver asks of it (the registers, in order) and that exactly the intended bytes are accessed; the
// planes themselves are checked against DOSBox's VGA by spikes/a-vga-vbe.

use desk_hal::{HalError, MmioRegion};
use desk_hal_mock::MockPortBus;
use desk_vga::{Canvas as _, Planar16};

const PITCH: usize = 80;

fn window() -> Vec<u8> {
    vec![0xEE; PITCH * 480]
}

fn region(buf: &mut [u8]) -> MmioRegion {
    // SAFETY: the vector outlives the region in every test and is not otherwise touched while the region is in use.
    unsafe { MmioRegion::new(buf.as_mut_ptr(), buf.len()) }
}

/// The writes `Planar16::new` makes.
fn setup() -> Vec<(u16, u32)> {
    vec![
        (0x3CE, 5),
        (0x3CF, 0),
        (0x3CE, 3),
        (0x3CF, 0),
        (0x3C4, 2),
        (0x3C5, 0x0F),
        (0x3CE, 1),
        (0x3CF, 0x0F),
        (0x3CE, 8),
        (0x3CF, 0xFF),
    ]
}

fn teardown() -> Vec<(u16, u32)> {
    vec![(0x3CE, 1), (0x3CF, 0), (0x3CE, 8), (0x3CF, 0xFF)]
}

fn touched(buf: &[u8]) -> Vec<usize> {
    buf.iter().enumerate().filter(|(_, b)| **b != 0xEE).map(|(i, _)| i).collect()
}

#[test]
fn the_controller_is_set_up_and_put_back() {
    let bus = MockPortBus::new();
    let mut buf = window();
    let mut r = region(&mut buf);
    {
        let _c = Planar16::new(&bus, &mut r, 640, 480, 80).unwrap();
        assert_eq!(bus.writes(), setup());
    }
    let w = bus.writes();
    assert_eq!(&w[setup().len()..], &teardown()[..]);
}

#[test]
fn the_geometry_is_validated_before_any_port_write() {
    let bus = MockPortBus::new();
    let mut buf = vec![0u8; 100];
    let mut r = region(&mut buf);
    assert_eq!(
        Planar16::new(&bus, &mut r, 640, 480, 80).err(),
        Some(HalError::InvalidArgument)
    );
    assert_eq!(Planar16::new(&bus, &mut r, 800, 1, 80).err(), Some(HalError::InvalidArgument));
    assert!(bus.log().is_empty());
}

#[test]
fn a_pixel_reads_to_load_the_latches_then_writes_under_a_one_bit_mask() {
    let bus = MockPortBus::new();
    let mut buf = window();
    let mut r = region(&mut buf);
    {
        let mut c = Planar16::new(&bus, &mut r, 640, 480, 80).unwrap();
        bus.take_log();
        c.pixel(10, 2, 0x0A);
        // Set/reset = colour, bit mask = 0x80 >> (10 % 8) = 0x20.
        assert_eq!(bus.writes(), vec![(0x3CE, 0), (0x3CF, 0x0A), (0x3CE, 8), (0x3CF, 0x20)]);
        c.pixel(-1, 0, 1);
        c.pixel(640, 0, 1);
        c.pixel(0, 480, 1);
        assert_eq!(bus.writes().len(), 4, "nothing is programmed for a pixel outside the screen");
    }
    assert_eq!(touched(&buf), vec![2 * PITCH + 1], "byte 10 / 8 = 1 of row 2");
}

#[test]
fn a_horizontal_line_masks_its_ends_and_fills_the_middle_without_reading() {
    let bus = MockPortBus::new();
    let mut buf = window();
    let mut r = region(&mut buf);
    {
        let mut c = Planar16::new(&bus, &mut r, 640, 480, 80).unwrap();
        bus.take_log();
        // Pixels 3 to 22 of row 1: bytes 0 (bits 3 to 7: 0x1F), 1 (all) and 2 (bits 0 to 6: 0xFE).
        c.hline(3, 1, 20, 5);
        assert_eq!(
            bus.writes(),
            vec![
                (0x3CE, 0),
                (0x3CF, 5),
                (0x3CE, 8),
                (0x3CF, 0x1F),
                (0x3CE, 8),
                (0x3CF, 0xFF),
                (0x3CE, 8),
                (0x3CF, 0xFE)
            ]
        );
    }
    assert_eq!(touched(&buf), vec![PITCH, PITCH + 1, PITCH + 2]);
}

#[test]
fn a_line_inside_one_byte_uses_one_combined_mask() {
    let bus = MockPortBus::new();
    let mut buf = window();
    let mut r = region(&mut buf);
    {
        let mut c = Planar16::new(&bus, &mut r, 640, 480, 80).unwrap();
        bus.take_log();
        c.hline(2, 0, 3, 1); // pixels 2, 3, 4: 0x3F & 0xF8 = 0x38
        assert_eq!(bus.writes(), vec![(0x3CE, 0), (0x3CF, 1), (0x3CE, 8), (0x3CF, 0x38)]);
    }
    assert_eq!(touched(&buf), vec![0]);
}

#[test]
fn an_aligned_span_needs_no_partial_masks() {
    let bus = MockPortBus::new();
    let mut buf = window();
    let mut r = region(&mut buf);
    {
        let mut c = Planar16::new(&bus, &mut r, 640, 480, 80).unwrap();
        bus.take_log();
        c.hline(8, 0, 16, 9);
        assert_eq!(
            bus.writes(),
            vec![(0x3CE, 0), (0x3CF, 9), (0x3CE, 8), (0x3CF, 0xFF), (0x3CE, 8), (0x3CF, 0xFF)]
        );
    }
    assert_eq!(touched(&buf), vec![1, 2]);
}

#[test]
fn a_vertical_line_holds_one_mask_for_every_row() {
    let bus = MockPortBus::new();
    let mut buf = window();
    let mut r = region(&mut buf);
    {
        let mut c = Planar16::new(&bus, &mut r, 640, 480, 80).unwrap();
        bus.take_log();
        c.vline(639, 476, 100, 15); // clipped to rows 476 to 479
        assert_eq!(bus.writes(), vec![(0x3CE, 0), (0x3CF, 15), (0x3CE, 8), (0x3CF, 0x01)]);
    }
    assert_eq!(touched(&buf), (476..480).map(|y| y * PITCH + 79).collect::<Vec<_>>());
}

#[test]
fn a_rectangle_fill_clips_and_touches_only_its_bytes() {
    let bus = MockPortBus::new();
    let mut buf = window();
    let mut r = region(&mut buf);
    {
        let mut c = Planar16::new(&bus, &mut r, 640, 480, 80).unwrap();
        c.fill_rect(-4, 10, 16, 2, 3); // pixels 0 to 11: byte 0 whole, byte 1 left half
    }
    assert_eq!(touched(&buf), vec![10 * PITCH, 10 * PITCH + 1, 11 * PITCH, 11 * PITCH + 1]);
}

#[test]
fn reading_selects_each_plane_in_turn_and_restores_plane_zero() {
    let bus = MockPortBus::new();
    let mut buf = vec![0u8; PITCH * 480];
    buf[3 * PITCH] = 0b1000_0000; // pixel (0, 3): bit set in whatever plane is selected
    let mut r = region(&mut buf);
    let mut c = Planar16::new(&bus, &mut r, 640, 480, 80).unwrap();
    bus.take_log();
    // The window is plain memory, so a set bit reads the same in every plane: colour 15.
    assert_eq!(c.get_pixel(0, 3), Some(15));
    assert_eq!(c.get_pixel(1, 3), Some(0));
    assert_eq!(c.get_pixel(640, 3), None);
    let sel: Vec<u32> = bus.writes().iter().filter(|(p, _)| *p == 0x3CF).map(|(_, v)| *v).collect();
    assert_eq!(sel, vec![0, 1, 2, 3, 0, 0, 1, 2, 3, 0]);

    let mut row = [0u8; 640];
    c.read_row(3, &mut row).unwrap();
    assert_eq!(row[0], 15);
    assert!(row[1..].iter().all(|p| *p == 0));
    assert_eq!(c.read_row(480, &mut row), Err(HalError::InvalidArgument));
}
