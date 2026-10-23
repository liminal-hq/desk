// Host tests for desk-font: the four committed faces, the metrics they hold today, the text-art sample and the malformed inputs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_font::{Font, FontError};

static UI: &[u8] = include_bytes!("../../../fonts/generated/ui.dfn");
static TITLE: &[u8] = include_bytes!("../../../fonts/generated/title.dfn");
static FIXED: &[u8] = include_bytes!("../../../fonts/generated/fixed.dfn");
static SMALL: &[u8] = include_bytes!("../../../fonts/generated/small.dfn");

fn faces() -> [(&'static str, &'static [u8], u32, u32, usize); 4] {
    [
        ("ui", UI, 14, 11, 289),
        ("title", TITLE, 14, 12, 289),
        ("fixed", FIXED, 16, 13, 332),
        ("small", SMALL, 8, 7, 289),
    ]
}

#[test]
fn the_four_faces_parse_with_the_metrics_the_files_hold() {
    for (name, bytes, height, baseline, count) in faces() {
        let font = Font::parse(bytes).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(font.height(), height, "{name} height");
        assert_eq!(font.baseline(), baseline, "{name} baseline");
        assert_eq!(font.glyph_count(), count, "{name} glyph count");
    }
}

// Walks the file by hand (not through the parser) and compares with what the parser reports for every glyph.
#[test]
fn every_glyphs_rows_are_height_times_stride_bytes_and_the_parse_consumes_the_file() {
    for (name, bytes, height, _, count) in faces() {
        let font = Font::parse(bytes).unwrap();
        let mut at = 8;
        let mut seen = 0;
        while at < bytes.len() {
            let code = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            let width = bytes[at + 4] as usize;
            let stride = width.div_ceil(8);
            let glyph = font
                .glyph(char::from_u32(code).unwrap())
                .unwrap_or_else(|| panic!("{name}: U+{code:04X}"));
            assert_eq!(glyph.code, code);
            assert_eq!(glyph.advance as usize, width);
            assert_eq!(glyph.stride as usize, stride);
            assert_eq!(glyph.height as u32, height);
            assert_eq!(glyph.rows.len(), height as usize * stride, "{name} U+{code:04X}");
            assert_eq!(glyph.rows, &bytes[at + 5..at + 5 + height as usize * stride]);
            at += 5 + height as usize * stride;
            seen += 1;
        }
        assert_eq!(at, bytes.len(), "{name} consumed exactly");
        assert_eq!(seen, count);
    }
}

#[test]
fn desk_fixed_advances_8_and_desk_small_6_for_every_glyph() {
    for (bytes, advance) in [(FIXED, 8), (SMALL, 6)] {
        let font = Font::parse(bytes).unwrap();
        let mut checked = 0;
        for code in 0..=0x10FFFFu32 {
            if let Some(glyph) = char::from_u32(code).and_then(|c| font.glyph(c)) {
                assert_eq!(glyph.advance, advance, "U+{code:04X}");
                checked += 1;
            }
        }
        assert_eq!(checked, font.glyph_count());
    }
}

#[test]
fn advance_of_a_string_is_the_sum_of_its_glyphs() {
    for (name, bytes, ..) in faces() {
        let font = Font::parse(bytes).unwrap();
        let sum: u32 = "Desk".chars().map(|c| font.glyph(c).unwrap().advance as u32).sum();
        assert!(sum > 0);
        assert_eq!(font.advance("Desk"), sum, "{name}");
        assert_eq!(font.advance(""), 0);
    }
    assert_eq!(Font::parse(FIXED).unwrap().advance("Desk"), 32);
    assert_eq!(Font::parse(SMALL).unwrap().advance("Desk"), 24);
}

#[test]
fn a_missing_character_falls_back_to_the_question_mark() {
    for (name, bytes, ..) in faces() {
        let font = Font::parse(bytes).unwrap();
        assert!(font.glyph('\u{FFFD}').is_none(), "{name} has no U+FFFD");
        let question = font.glyph('?').unwrap();
        let fallback = font.glyph_or_fallback('\u{FFFD}');
        assert_eq!(fallback.code, '?' as u32);
        assert_eq!(fallback.advance, question.advance);
        assert_eq!(fallback.rows, question.rows);
        assert_eq!(font.advance("\u{FFFD}"), question.advance as u32);
        assert_eq!(font.advance("a\u{FFFD}b"), font.advance("a?b"));
        for c in ['?', '\u{2026}', '\u{D7}'] {
            assert!(font.glyph(c).is_some(), "{name} has {c:?}");
        }
    }
}

#[test]
fn glyph_bit_reads_the_leftmost_pixel_from_the_highest_bit_and_is_clear_outside() {
    let font = Font::parse(UI).unwrap();
    let glyph = font.glyph('l').unwrap();
    let mut set = 0;
    for y in 0..font.height() {
        for x in 0..glyph.advance as u32 {
            let expected = glyph.rows[(y * glyph.stride as u32 + x / 8) as usize] & (0x80 >> (x % 8)) != 0;
            assert_eq!(glyph.bit(x, y), expected);
            set += expected as u32;
        }
    }
    assert!(set > 0);
    assert!(!glyph.bit(glyph.advance as u32, 0));
    assert!(!glyph.bit(0, font.height()));
    assert!(!glyph.bit(u32::MAX, u32::MAX));
}

fn render(font: &Font, text: &str) -> String {
    let mut out = String::new();
    for y in 0..font.height() {
        for c in text.chars() {
            let glyph = font.glyph_or_fallback(c);
            for x in 0..glyph.advance as u32 {
                out.push(if glyph.bit(x, y) { '#' } else { '.' });
            }
        }
        out.push('\n');
    }
    out
}

#[test]
fn the_text_art_sample_matches_the_committed_one() {
    let ui = Font::parse(UI).unwrap();
    let title = Font::parse(TITLE).unwrap();
    let sample = format!(
        "Desk UI: Files and Folders, 10 items\n{}\nDesk Title: Welcome to Desk\n{}",
        render(&ui, "Files and Folders, 10 items"),
        render(&title, "Welcome to Desk"),
    );
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/expected/ui-sample.txt");
    if std::env::var_os("DESK_FONT_RECORD").is_some() {
        std::fs::write(path, &sample).unwrap();
    }
    let expected = std::fs::read_to_string(path).expect("run once with DESK_FONT_RECORD=1");
    assert_eq!(sample, expected);
}

// A one-glyph file: code point, width, and `height` rows of `ceil(width / 8)` bytes.
fn file(height: u8, baseline: u8, glyphs: &[(u32, u8)]) -> Vec<u8> {
    let mut v = b"DFN1".to_vec();
    v.extend([height, baseline]);
    v.extend((glyphs.len() as u16).to_le_bytes());
    for &(code, width) in glyphs {
        v.extend(code.to_le_bytes());
        v.push(width);
        v.extend(std::iter::repeat_n(0xA5, height as usize * (width as usize).div_ceil(8)));
    }
    v
}

#[test]
fn a_well_formed_small_file_parses() {
    let bytes = file(2, 1, &[(0x41, 8), (0x42, 9)]);
    let font = Font::parse(&bytes).unwrap();
    assert_eq!(font.glyph_count(), 2);
    assert_eq!(font.glyph('B').unwrap().rows.len(), 4);
    assert!(font.glyph('A').unwrap().bit(0, 0));
    assert!(font.glyph('A').unwrap().bit(2, 0));
    assert!(!font.glyph('A').unwrap().bit(1, 0));
    assert_eq!(font.advance("AB?"), 17);
}

#[test]
fn bad_magic() {
    let mut bytes = file(2, 1, &[(0x41, 8)]);
    bytes[3] = b'2';
    assert_eq!(Font::parse(&bytes).unwrap_err(), FontError::BadMagic);
    assert_eq!(Font::parse(b"").unwrap_err(), FontError::Truncated { offset: 0 });
    assert_eq!(Font::parse(b"XFN").unwrap_err(), FontError::BadMagic);
}

#[test]
fn a_header_cut_short() {
    let bytes = file(2, 1, &[(0x41, 8)]);
    for len in [4, 5, 7] {
        assert_eq!(
            Font::parse(&bytes[..len]).unwrap_err(),
            FontError::Truncated { offset: 0 },
            "{len}"
        );
    }
}

#[test]
fn a_glyph_cut_short_names_where_it_starts() {
    let bytes = file(2, 1, &[(0x41, 8), (0x42, 8)]);
    // The first glyph is bytes 8..15 (5 + 2 rows); the second starts at 15.
    assert_eq!(bytes.len(), 22);
    for len in [15, 16, 19, 20, 21] {
        assert_eq!(
            Font::parse(&bytes[..len]).unwrap_err(),
            FontError::Truncated { offset: 15 },
            "{len}"
        );
    }
    assert_eq!(Font::parse(&bytes[..10]).unwrap_err(), FontError::Truncated { offset: 8 });
    // The header promises two glyphs but the file holds one whole one.
    assert_eq!(Font::parse(&bytes[..15]).unwrap_err(), FontError::Truncated { offset: 15 });
}

#[test]
fn zero_height() {
    assert_eq!(Font::parse(&file(0, 0, &[(0x41, 8)])).unwrap_err(), FontError::ZeroHeight);
}

#[test]
fn a_baseline_past_the_cell() {
    assert_eq!(
        Font::parse(&file(2, 3, &[(0x41, 8)])).unwrap_err(),
        FontError::BaselineOutOfCell
    );
    assert!(Font::parse(&file(2, 2, &[(0x41, 8)])).is_ok());
}

#[test]
fn code_points_out_of_order() {
    assert_eq!(
        Font::parse(&file(2, 1, &[(0x42, 8), (0x41, 8)])).unwrap_err(),
        FontError::OutOfOrder { glyph: 1 }
    );
    assert_eq!(
        Font::parse(&file(2, 1, &[(0x41, 8), (0x42, 8), (0x42, 8)])).unwrap_err(),
        FontError::OutOfOrder { glyph: 2 }
    );
}

#[test]
fn bytes_after_the_last_glyph() {
    let mut bytes = file(2, 1, &[(0x41, 8)]);
    bytes.push(0);
    assert_eq!(Font::parse(&bytes).unwrap_err(), FontError::TrailingBytes { offset: 15 });
}
