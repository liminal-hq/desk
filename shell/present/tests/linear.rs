// Host tests for the linear presenter and the linear read-back, over a Vec-backed MmioRegion.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_gfx::{MemoryPresent, Present};
use desk_hal::{HalError, MmioRegion};
use desk_present::{read_back_linear, LinearPresent};

fn region(buf: &mut [u8]) -> MmioRegion {
    // SAFETY: the buffer outlives the region in every test and is not otherwise touched while the region is in use.
    unsafe { MmioRegion::new(buf.as_mut_ptr(), buf.len()) }
}

/// A fixed-seed xorshift, so the tests are the same on every run.
fn next(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

#[test]
fn the_same_spans_give_the_same_bytes_as_memory_present() {
    for (w, h, pitch) in [(20u32, 7u32, 20u32), (20, 7, 32), (640, 4, 704)] {
        let mut buf = vec![0xEEu8; pitch as usize * h as usize];
        let mut mem = MemoryPresent::new(w, h, 1);
        mem.pixels.fill(0xEE);
        let mut seed = 0x1234_5678;
        {
            let mut r = region(&mut buf);
            let mut p = LinearPresent::new(&mut r, w, h, pitch).unwrap();
            assert_eq!((p.width(), p.height(), p.align()), (w, h, 1));
            p.begin().unwrap();
            mem.begin().unwrap();
            for _ in 0..200 {
                let y = next(&mut seed) % h;
                let x = next(&mut seed) % w;
                let len = next(&mut seed) % (w - x + 1);
                let row: Vec<u8> = (0..len).map(|_| next(&mut seed) as u8).collect();
                p.span(x, y, &row).unwrap();
                mem.span(x, y, &row).unwrap();
            }
            p.end().unwrap();
        }
        for y in 0..h as usize {
            let got = &buf[y * pitch as usize..y * pitch as usize + w as usize];
            assert_eq!(got, &mem.pixels[y * w as usize..(y + 1) * w as usize], "row {y}");
            // The bytes between the visible width and the pitch are never touched.
            assert!(buf[y * pitch as usize + w as usize..(y + 1) * pitch as usize]
                .iter()
                .all(|&b| b == 0xEE));
        }
    }
}

#[test]
fn spans_at_the_last_column_and_row_land_exactly() {
    let (w, h, pitch) = (10u32, 4u32, 16u32);
    let mut buf = vec![0u8; (pitch * h) as usize];
    {
        let mut r = region(&mut buf);
        let mut p = LinearPresent::new(&mut r, w, h, pitch).unwrap();
        p.span(9, 3, &[7]).unwrap();
        p.span(0, 3, &[1, 2]).unwrap();
        p.span(8, 0, &[5, 6]).unwrap();
        p.span(10, 0, &[]).unwrap();
    }
    assert_eq!(buf[3 * 16 + 9], 7);
    assert_eq!(&buf[3 * 16..3 * 16 + 2], &[1, 2]);
    assert_eq!(&buf[8..10], &[5, 6]);
    assert_eq!(buf.iter().filter(|&&b| b != 0).count(), 5);
}

#[test]
fn spans_off_the_screen_are_refused_and_write_nothing() {
    let (w, h, pitch) = (10u32, 4u32, 16u32);
    let mut buf = vec![0u8; (pitch * h) as usize];
    {
        let mut r = region(&mut buf);
        let mut p = LinearPresent::new(&mut r, w, h, pitch).unwrap();
        assert_eq!(
            p.span(9, 0, &[1, 1]),
            Err(HalError::InvalidArgument),
            "past the last column, into the pitch"
        );
        assert_eq!(p.span(0, 0, &[1; 11]), Err(HalError::InvalidArgument));
        assert_eq!(p.span(0, 4, &[1]), Err(HalError::InvalidArgument), "past the last row");
        assert_eq!(p.span(11, 0, &[]), Err(HalError::InvalidArgument));
        assert_eq!(p.span(u32::MAX, 0, &[1, 1]), Err(HalError::InvalidArgument));
    }
    assert!(buf.iter().all(|&b| b == 0));
}

#[test]
fn refused_geometry() {
    let mut buf = vec![0u8; 100];
    let mut r = region(&mut buf);
    assert!(LinearPresent::new(&mut r, 10, 10, 10).is_ok());
    assert_eq!(
        LinearPresent::new(&mut r, 10, 11, 10).err(),
        Some(HalError::InvalidArgument),
        "region too small"
    );
    assert_eq!(
        LinearPresent::new(&mut r, 10, 5, 9).err(),
        Some(HalError::InvalidArgument),
        "pitch below width"
    );
    assert_eq!(
        LinearPresent::new(&mut r, u32::MAX, u32::MAX, u32::MAX).err(),
        Some(HalError::InvalidArgument)
    );
}

#[test]
fn a_known_picture_reads_back() {
    let (w, h, pitch) = (13u32, 5u32, 20u32);
    let picture: Vec<u8> = (0..w * h).map(|i| (i * 7 + 3) as u8).collect();
    let mut buf = vec![0xAAu8; (pitch * h) as usize];
    {
        let mut r = region(&mut buf);
        let mut p = LinearPresent::new(&mut r, w, h, pitch).unwrap();
        for y in 0..h {
            p.span(0, y, &picture[(y * w) as usize..((y + 1) * w) as usize]).unwrap();
        }
    }
    let mut r = region(&mut buf);
    let mut out = vec![0u8; (w * h) as usize];
    read_back_linear(&mut r, w, h, pitch, &mut out).unwrap();
    assert_eq!(out, picture);
    let mut short = vec![0u8; (w * h) as usize - 1];
    assert_eq!(
        read_back_linear(&mut r, w, h, pitch, &mut short),
        Err(HalError::InvalidArgument)
    );
    assert_eq!(
        read_back_linear(&mut r, w, h + 9, pitch, &mut out),
        Err(HalError::InvalidArgument)
    );
}
