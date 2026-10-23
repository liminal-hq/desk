// Frame digests: CRC-32 of the indices and of the colours they stand for.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::surface::Surface;
use desk_hal::Rgb6;

const fn make_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}

static TABLE: [u32; 256] = make_table();

/// CRC-32 (IEEE 802.3, as zlib's `crc32`) computed in pieces.
#[derive(Clone, Copy, Debug)]
pub struct Crc32 {
    state: u32,
}

impl Default for Crc32 {
    fn default() -> Self {
        Crc32::new()
    }
}

impl Crc32 {
    pub const fn new() -> Crc32 {
        Crc32 { state: 0xFFFF_FFFF }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        let mut c = self.state;
        for &b in bytes {
            c = TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
        }
        self.state = c;
    }

    pub fn finish(&self) -> u32 {
        self.state ^ 0xFFFF_FFFF
    }
}

/// CRC-32 of `bytes`: the IEEE polynomial, as zlib's `crc32`.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut c = Crc32::new();
    c.update(bytes);
    c.finish()
}

/// The two checksums of a picture and its size.
///
/// `index` is the CRC-32 of the palette indices row by row; `colour` is the CRC-32 of each pixel's `Rgb6` as three bytes `r g b`
/// (each 0 to 63), row by row, an index past the end of the palette counting as black.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameDigest {
    pub width: u32,
    pub height: u32,
    pub index: u32,
    pub colour: u32,
}

impl FrameDigest {
    /// The digests of a picture given row by row: `row(y, buf)` fills `buf` (`width` bytes) with row `y`'s indices.
    pub fn from_rows(width: u32, height: u32, palette: &[Rgb6], mut row: impl FnMut(u32, &mut [u8])) -> FrameDigest {
        // Rows of any width go through a fixed buffer in pieces, so the stack use does not grow with the picture.
        const CHUNK: usize = 64;
        let mut index = Crc32::new();
        let mut colour = Crc32::new();
        let mut line = alloc::vec![0u8; width as usize];
        for y in 0..height {
            row(y, &mut line);
            index.update(&line);
            for piece in line.chunks(CHUNK) {
                let mut rgb = [0u8; CHUNK * 3];
                for (i, &p) in piece.iter().enumerate() {
                    let c = palette.get(p as usize).copied().unwrap_or(Rgb6::BLACK);
                    rgb[i * 3..i * 3 + 3].copy_from_slice(&[c.r, c.g, c.b]);
                }
                colour.update(&rgb[..piece.len() * 3]);
            }
        }
        FrameDigest {
            width,
            height,
            index: index.finish(),
            colour: colour.finish(),
        }
    }

    /// The digests of a surface.
    pub fn of(surface: &Surface, palette: &[Rgb6]) -> FrameDigest {
        FrameDigest::from_rows(surface.width(), surface.height(), palette, |y, buf| {
            buf.copy_from_slice(surface.row(y))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(&[0]), 0xD202_EF8D);
    }

    #[test]
    fn crc32_in_pieces_equals_in_one() {
        let mut c = Crc32::new();
        c.update(b"1234");
        c.update(b"");
        c.update(b"56789");
        assert_eq!(c.finish(), 0xCBF4_3926);
    }

    #[test]
    fn digest_of_a_known_two_by_two_picture() {
        let mut s = Surface::new(2, 2, 0);
        s.fill(crate::Rect::new(1, 0, 1, 1), 1);
        s.fill(crate::Rect::new(0, 1, 1, 1), 2);
        s.fill(crate::Rect::new(1, 1, 1, 1), 3);
        let palette = [
            Rgb6::new(0, 0, 0),
            Rgb6::new(63, 0, 0),
            Rgb6::new(0, 63, 0),
            Rgb6::new(1, 2, 3),
        ];
        let d = FrameDigest::of(&s, &palette);
        assert_eq!((d.width, d.height), (2, 2));
        // Indices 0, 1, 2, 3; colours 000000 3f0000 003f00 010203 as bytes. Both values are zlib's `crc32` of those bytes.
        assert_eq!(d.index, crc32(&[0, 1, 2, 3]));
        assert_eq!(d.index, INDEX_2X2);
        assert_eq!(d.colour, crc32(&[0, 0, 0, 63, 0, 0, 0, 63, 0, 1, 2, 3]));
        assert_eq!(d.colour, COLOUR_2X2);
        // An index past the palette counts as black.
        let short = FrameDigest::of(&s, &palette[..3]);
        assert_eq!(short.colour, crc32(&[0, 0, 0, 63, 0, 0, 0, 63, 0, 0, 0, 0]));
        assert_eq!(short.index, d.index);
    }

    #[test]
    fn digests_cover_rows_wider_than_one_chunk() {
        let mut s = Surface::new(150, 2, 0);
        for x in 0..150 {
            s.fill(crate::Rect::new(x, 0, 1, 1), (x % 4) as u8);
        }
        let palette = [
            Rgb6::new(5, 6, 7),
            Rgb6::new(8, 9, 10),
            Rgb6::new(11, 12, 13),
            Rgb6::new(14, 15, 16),
        ];
        let mut bytes = alloc::vec::Vec::new();
        for &p in s.pixels() {
            let c = palette[p as usize];
            bytes.extend_from_slice(&[c.r, c.g, c.b]);
        }
        let d = FrameDigest::of(&s, &palette);
        assert_eq!(d.colour, crc32(&bytes));
        assert_eq!(d.index, crc32(s.pixels()));
    }

    const INDEX_2X2: u32 = 0x8BB9_8613;
    const COLOUR_2X2: u32 = 0x03D1_BF34;
}
