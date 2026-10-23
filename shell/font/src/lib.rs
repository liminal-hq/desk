// Reads the .DFN bitmap faces: metrics, glyph rows and text advances, with no drawing code.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The layout is `docs/FORMATS.md` (".DFN: a bitmap font"): `DFN1`, height, baseline, glyph count (2 bytes), then the glyphs in
// ascending code-point order, each a code point (4 bytes), a width (1 byte, the advance) and `height` rows of `ceil(width / 8)`
// bytes with the leftmost pixel in the highest bit. Numbers are little-endian. `parse` checks every glyph once, so the lookups
// after it cannot fail on a malformed file.
#![no_std]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

extern crate alloc;

use alloc::vec::Vec;

const MAGIC: [u8; 4] = *b"DFN1";
const HEADER_LEN: usize = 8;

/// Why a `.DFN` file was refused. Offsets are byte offsets from the start of the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontError {
    /// The file does not start with `DFN1`.
    BadMagic,
    /// The file ends inside the header (offset 0), or inside the glyph that starts at `offset`.
    Truncated { offset: usize },
    /// The height is zero.
    ZeroHeight,
    /// The baseline is larger than the height.
    BaselineOutOfCell,
    /// The glyph at this index has a code point that is not above the previous one.
    OutOfOrder { glyph: u16 },
    /// The file continues past the last glyph, starting at `offset`.
    TrailingBytes { offset: usize },
}

/// A parsed face, borrowing the file's bytes.
#[derive(Clone, Debug)]
pub struct Font<'a> {
    bytes: &'a [u8],
    height: u8,
    baseline: u8,
    // Code point and the offset of the glyph's record (its code point field), in code-point order.
    index: Vec<(u32, u32)>,
}

/// One glyph: its advance (width including the gap after it) and its rows, `height` of them, `stride` bytes each.
#[derive(Clone, Copy, Debug)]
pub struct Glyph<'a> {
    pub code: u32,
    pub advance: u8,
    pub height: u8,
    pub stride: u8,
    pub rows: &'a [u8],
}

impl Glyph<'_> {
    /// Whether the pixel at column `x`, row `y` is set. Outside the glyph's advance and height it is clear.
    pub fn bit(&self, x: u32, y: u32) -> bool {
        if x >= u32::from(self.advance) || y >= u32::from(self.height) {
            return false;
        }
        let at = (y * u32::from(self.stride) + x / 8) as usize;
        match self.rows.get(at) {
            Some(byte) => byte & (0x80 >> (x % 8)) != 0,
            None => false,
        }
    }
}

impl<'a> Font<'a> {
    /// Parses and validates a whole `.DFN` file, which must be consumed exactly.
    pub fn parse(bytes: &'a [u8]) -> Result<Font<'a>, FontError> {
        let seen = bytes.len().min(MAGIC.len());
        if bytes[..seen] != MAGIC[..seen] {
            return Err(FontError::BadMagic);
        }
        if bytes.len() < HEADER_LEN {
            return Err(FontError::Truncated { offset: 0 });
        }
        let height = bytes[4];
        let baseline = bytes[5];
        let count = u16::from_le_bytes([bytes[6], bytes[7]]);
        if height == 0 {
            return Err(FontError::ZeroHeight);
        }
        if baseline > height {
            return Err(FontError::BaselineOutOfCell);
        }
        let mut index = Vec::with_capacity(usize::from(count).min((bytes.len() - HEADER_LEN) / 5));
        let mut at = HEADER_LEN;
        let mut previous: Option<u32> = None;
        for glyph in 0..count {
            let truncated = FontError::Truncated { offset: at };
            let head = bytes.get(at..at + 5).ok_or(truncated)?;
            let code = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
            let width = usize::from(head[4]);
            let size = usize::from(height) * width.div_ceil(8);
            if bytes.len() - at - 5 < size {
                return Err(truncated);
            }
            if previous.is_some_and(|p| code <= p) {
                return Err(FontError::OutOfOrder { glyph });
            }
            previous = Some(code);
            index.push((code, at as u32));
            at += 5 + size;
        }
        if at != bytes.len() {
            return Err(FontError::TrailingBytes { offset: at });
        }
        Ok(Font {
            bytes,
            height,
            baseline,
            index,
        })
    }

    /// The rows in every glyph: the whole cell.
    pub fn height(&self) -> u32 {
        u32::from(self.height)
    }

    /// The rows above the baseline.
    pub fn baseline(&self) -> u32 {
        u32::from(self.baseline)
    }

    pub fn glyph_count(&self) -> usize {
        self.index.len()
    }

    /// The glyph for `c`, by binary search of the index.
    pub fn glyph(&self, c: char) -> Option<Glyph<'a>> {
        let code = c as u32;
        let slot = self.index.binary_search_by_key(&code, |&(code, _)| code).ok()?;
        let at = self.index[slot].1 as usize;
        let advance = self.bytes[at + 4];
        let stride = advance.div_ceil(8);
        let start = at + 5;
        let rows = &self.bytes[start..start + usize::from(self.height) * usize::from(stride)];
        Some(Glyph {
            code,
            advance,
            height: self.height,
            stride,
            rows,
        })
    }

    /// The glyph for `c`, or the one for `?` when the face lacks `c`. A face with neither gives an empty glyph of advance 0.
    pub fn glyph_or_fallback(&self, c: char) -> Glyph<'a> {
        self.glyph(c).or_else(|| self.glyph('?')).unwrap_or(Glyph {
            code: '?' as u32,
            advance: 0,
            height: self.height,
            stride: 0,
            rows: &[],
        })
    }

    /// The width of `s` in pixels: the sum of its glyphs' advances, fallbacks included.
    pub fn advance(&self, s: &str) -> u32 {
        s.chars().map(|c| u32::from(self.glyph_or_fallback(c).advance)).sum()
    }
}
