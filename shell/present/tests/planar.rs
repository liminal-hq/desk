// Host tests for the planar presenter: `planarise`, the four-plane model, the register writes on the mock port bus.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_gfx::{MemoryPresent, Present};
use desk_hal::{HalError, MmioRegion};
use desk_hal_mock::vga::{GC_DATA, GC_INDEX, SEQ_DATA, SEQ_INDEX};
use desk_hal_mock::{MockPortBus, VgaModel};
use desk_present::{planarise, PlanarPresent, PlaneSink, MAX_PLANE_BYTES};
use desk_vga::Planar16;

fn region(buf: &mut [u8]) -> MmioRegion {
    // SAFETY: the buffer outlives the region in every test and is not otherwise touched while the region is in use.
    unsafe { MmioRegion::new(buf.as_mut_ptr(), buf.len()) }
}

fn next(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

fn blank() -> [[u8; MAX_PLANE_BYTES]; 4] {
    [[0; MAX_PLANE_BYTES]; 4]
}

/// The obvious per-pixel definition of the plane layout.
fn reference(pixels: &[u8]) -> Vec<[u8; 4]> {
    let mut out = vec![[0u8; 4]; pixels.len() / 8];
    for (i, &px) in pixels.iter().enumerate() {
        for (p, plane) in out[i / 8].iter_mut().enumerate() {
            if px >> p & 1 == 1 {
                *plane |= 0x80 >> (i % 8);
            }
        }
    }
    out
}

#[test]
fn every_colour_at_every_bit_position() {
    for colour in 0..16u8 {
        for pos in 0..8 {
            let mut group = [0u8; 8];
            group[pos] = colour;
            let mut planes = blank();
            assert_eq!(planarise(&group, &mut planes), 1);
            for (p, plane) in planes.iter().enumerate() {
                let want = if colour >> p & 1 == 1 { 0x80 >> pos } else { 0 };
                assert_eq!(plane[0], want, "colour {colour}, position {pos}, plane {p}");
            }
        }
    }
    // Only the low four bits are planes.
    let mut planes = blank();
    planarise(&[0xF5; 8], &mut planes);
    assert_eq!(planes.map(|p| p[0]), [0xFF, 0, 0xFF, 0]);
}

#[test]
fn a_random_row_matches_the_per_pixel_reference() {
    let mut seed = 0xC0FF_EE11;
    let row: Vec<u8> = (0..1280).map(|_| next(&mut seed) as u8).collect();
    let mut planes = blank();
    assert_eq!(planarise(&row, &mut planes), 160);
    for (b, want) in reference(&row).iter().enumerate() {
        assert_eq!([planes[0][b], planes[1][b], planes[2][b], planes[3][b]], *want, "byte {b}");
    }
    // A partial group is ignored.
    assert_eq!(planarise(&row[..13], &mut planes), 1);
    assert_eq!(planarise(&row[..7], &mut planes), 0);
}

/// Four planes in memory, one byte per plane byte.
struct Planes {
    planes: [Vec<u8>; 4],
    selected: u8,
}

impl Planes {
    fn new(bytes: usize) -> Planes {
        Planes {
            planes: std::array::from_fn(|_| vec![0; bytes]),
            selected: 0,
        }
    }

    /// Recombines the planes into indices, `width` pixels per row.
    fn indices(&self, width: usize, height: usize, pitch: usize) -> Vec<u8> {
        let mut out = vec![0u8; width * height];
        for y in 0..height {
            for x in 0..width {
                for (p, plane) in self.planes.iter().enumerate() {
                    out[y * width + x] |= (plane[y * pitch + x / 8] >> (7 - x % 8) & 1) << p;
                }
            }
        }
        out
    }
}

impl PlaneSink for Planes {
    fn select(&mut self, plane: u8) -> Result<(), HalError> {
        self.selected = plane;
        Ok(())
    }

    fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), HalError> {
        self.planes[self.selected as usize][offset..offset + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }
}

#[test]
fn recombined_planes_equal_memory_present() {
    for (w, h, pitch) in [(64u32, 9u32, 8u32), (64, 9, 11), (640, 6, 80)] {
        let mut mem = MemoryPresent::new(w, h, 8);
        let mut p = PlanarPresent::with_sink(Planes::new((pitch * h) as usize), w, h, pitch).unwrap();
        assert_eq!((p.width(), p.height(), p.align()), (w, h, 8));
        let mut seed = 0xBEEF_0042;
        p.begin().unwrap();
        mem.begin().unwrap();
        for _ in 0..300 {
            let y = next(&mut seed) % h;
            let x = next(&mut seed) % (w / 8) * 8;
            let len = next(&mut seed) % ((w - x) / 8 + 1) * 8;
            let row: Vec<u8> = (0..len).map(|_| next(&mut seed) as u8 & 0x0F).collect();
            p.span(x, y, &row).unwrap();
            mem.span(x, y, &row).unwrap();
        }
        p.end().unwrap();
        assert_eq!(p.sink().indices(w as usize, h as usize, pitch as usize), mem.pixels);
    }
}

#[test]
fn unaligned_or_off_screen_spans_are_refused() {
    let mut p = PlanarPresent::with_sink(Planes::new(8 * 4), 64, 4, 8).unwrap();
    assert_eq!(p.span(4, 0, &[1; 8]), Err(HalError::InvalidArgument), "unaligned start");
    assert_eq!(p.span(0, 0, &[1; 12]), Err(HalError::InvalidArgument), "unaligned length");
    assert_eq!(p.span(0, 0, &[1; 5]), Err(HalError::InvalidArgument));
    assert_eq!(p.span(56, 0, &[1; 16]), Err(HalError::InvalidArgument), "past the right edge");
    assert_eq!(p.span(0, 4, &[1; 8]), Err(HalError::InvalidArgument), "past the last row");
    assert!(p.sink().planes.iter().all(|pl| pl.iter().all(|&b| b == 0)), "nothing written");
    p.span(56, 3, &[0x0F; 8]).unwrap();
    assert_eq!(p.sink().planes[3][3 * 8 + 7], 0xFF);
}

#[test]
fn refused_geometry() {
    assert_eq!(
        PlanarPresent::with_sink(Planes::new(0), 66, 1, 9).err(),
        Some(HalError::InvalidArgument),
        "width not a multiple of 8"
    );
    assert_eq!(
        PlanarPresent::with_sink(Planes::new(0), 64, 1, 7).err(),
        Some(HalError::InvalidArgument),
        "pitch too small"
    );
    assert_eq!(
        PlanarPresent::with_sink(Planes::new(0), 1288, 1, 161).err(),
        Some(HalError::InvalidArgument),
        "wider than the row buffer"
    );
    let ports = MockPortBus::new();
    let mut buf = vec![0u8; 80 * 10];
    let mut r = region(&mut buf);
    assert!(PlanarPresent::new(&ports, &mut r, 640, 10, 80).is_ok());
    assert_eq!(
        PlanarPresent::new(&ports, &mut r, 640, 11, 80).err(),
        Some(HalError::InvalidArgument),
        "region smaller than a plane"
    );
}

fn writes_of(bus: &MockPortBus) -> Vec<(u16, u32)> {
    bus.writes()
}

#[test]
fn begin_span_end_write_the_expected_registers() {
    let bus = MockPortBus::new();
    let mut buf = vec![0u8; 80 * 2];
    let mut r = region(&mut buf);
    let mut p = PlanarPresent::new(&bus, &mut r, 640, 2, 80).unwrap();

    p.begin().unwrap();
    assert_eq!(
        writes_of(&bus),
        [
            (GC_INDEX, 5),
            (GC_DATA, 0x00),
            (GC_INDEX, 3),
            (GC_DATA, 0x00),
            (GC_INDEX, 1),
            (GC_DATA, 0x00),
            (GC_INDEX, 8),
            (GC_DATA, 0xFF),
        ]
    );
    bus.take_log();

    p.span(0, 0, &[0x0F; 16]).unwrap();
    p.span(8, 1, &[0x01; 8]).unwrap();
    let want: Vec<(u16, u32)> = [1u32, 2, 4, 8, 1, 2, 4, 8]
        .iter()
        .flat_map(|&m| [(SEQ_INDEX, 2), (SEQ_DATA, m)])
        .collect();
    assert_eq!(
        writes_of(&bus),
        want,
        "the map mask four times per span, in plane order, and nothing else"
    );
    bus.take_log();

    p.end().unwrap();
    assert_eq!(writes_of(&bus), [(SEQ_INDEX, 2), (SEQ_DATA, 0x0F)]);
    // The mock region is plain memory, so the last plane written wins: plane 3 of colour 0Fh is FFh, of colour 01h is 00h.
    assert_eq!(&buf[..2], &[0xFF, 0xFF]);
    assert_eq!(buf[80 + 1], 0x00);
}

#[test]
fn begin_after_a_leaked_planar16_leaves_set_reset_off() {
    let bus = MockPortBus::new();
    let model = bus.attach(0x3C0, 0x3DF, VgaModel::new());
    let mut buf = vec![0u8; 80 * 4];
    {
        let mut r = region(&mut buf);
        let canvas = Planar16::new(&bus, &mut r, 640, 4, 80).unwrap();
        core::mem::forget(canvas);
    }
    assert_eq!(model.borrow().gc[1], 0x0F, "the leaked driver left set/reset on");
    let mut r = region(&mut buf);
    let mut p = PlanarPresent::new(&bus, &mut r, 640, 4, 80).unwrap();
    p.begin().unwrap();
    {
        let m = model.borrow();
        assert_eq!(m.gc[1], 0x00, "enable set/reset");
        assert_eq!(m.gc[5], 0x00, "write mode 0");
        assert_eq!(m.gc[3], 0x00, "no rotate, replace");
        assert_eq!(m.gc[8], 0xFF, "bit mask");
    }
    p.span(0, 0, &[3; 8]).unwrap();
    p.end().unwrap();
    assert_eq!(model.borrow().seq[2], 0x0F);
}
