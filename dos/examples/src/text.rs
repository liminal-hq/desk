// The text screen: 80x25 text mode entered, a test screen written and read back cell by cell, the bounds checks, the keyboard poll and
// the restore of the mode and the cursor shape.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Modes: none (the whole run; a second command, `check`, follows it), `leave-held` (exit with a `Text` still held, so only the
// clean-up list can put the mode back) and `check` (run afterwards in the same DOSBox: compares the mode and the cursor shape with
// the record in TEXT.REC). DOSBox's text buffer behaves like ordinary memory, so the read-back says the code reaches the buffer it
// meant to (a cell the BIOS draws is seen by `get`, and the other way round) and that the bounds checks hold; it does not say how a
// real adapter behaves.
use alloc::vec::Vec;
use rt::dos::{self, File, Mode};
use rt::text::{self, Text, TextError, BYTES, COLUMNS, ROWS};
use rt::{cleanup, env, println};

const REC: &str = "TEXT.REC";

/// Registered before any `Text`, so it runs after the clean-up list's restore: says whether the mode and the cursor came back.
fn report() {
    let rec = read_record();
    println!(
        "text: after the clean-up list the mode and the cursor shape are back as found: {}",
        rec == Some((text::video_mode().unwrap_or(0xFF), text::cursor_shape().unwrap_or(0xFFFF)))
    );
}

fn read_record() -> Option<(u8, u16)> {
    let v = File::open(REC, Mode::Read).and_then(|mut f| f.read_to_vec(0)).ok()?;
    (v.len() == 3).then(|| (v[0], u16::from_le_bytes([v[1], v[2]])))
}

fn write_record() {
    let mode = text::video_mode().expect("mode");
    let cursor = text::cursor_shape().expect("cursor").to_le_bytes();
    File::open(REC, Mode::Create)
        .and_then(|mut f| f.write_all(&[mode, cursor[0], cursor[1]]))
        .expect("write the record");
}

/// FNV-1a over every byte of the cell buffer, read back through `get`.
fn digest(t: &Text) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for y in 0..ROWS {
        for x in 0..COLUMNS {
            let (c, a) = t.get(x, y).expect("in range");
            for b in [c, a] {
                h ^= u32::from(b);
                h = h.wrapping_mul(0x0100_0193);
            }
        }
    }
    h
}

fn record(shadow: &mut [u8], x: usize, y: usize, c: u8, a: u8) {
    shadow[(y * COLUMNS + x) * 2] = c;
    shadow[(y * COLUMNS + x) * 2 + 1] = a;
}

fn put(t: &mut Text, shadow: &mut [u8], x: usize, y: usize, c: u8, a: u8) {
    t.put(x, y, c, a).expect("in range");
    record(shadow, x, y, c, a);
}

/// The test screen, written through `put` and `write_str`; the same cells are recorded in `shadow`.
fn draw(t: &mut Text, shadow: &mut [u8]) {
    for y in 0..ROWS {
        for x in 0..COLUMNS {
            put(t, shadow, x, y, b' ', 0x07);
        }
    }
    // A border of CP437 box-drawing characters.
    for x in 1..COLUMNS - 1 {
        put(t, shadow, x, 0, 0xCD, 0x1F);
        put(t, shadow, x, ROWS - 1, 0xCD, 0x1F);
    }
    for y in 1..ROWS - 1 {
        put(t, shadow, 0, y, 0xBA, 0x1F);
        put(t, shadow, COLUMNS - 1, y, 0xBA, 0x1F);
    }
    put(t, shadow, 0, 0, 0xC9, 0x1F);
    put(t, shadow, COLUMNS - 1, 0, 0xBB, 0x1F);
    put(t, shadow, 0, ROWS - 1, 0xC8, 0x1F);
    put(t, shadow, COLUMNS - 1, ROWS - 1, 0xBC, 0x1F);
    // Every attribute, 0x00 to 0xFF, in sixteen rows of sixteen pairs of cells.
    for a in 0..=255usize {
        put(t, shadow, 4 + (a % 16) * 2, 3 + a / 16, b'A' + (a % 26) as u8, a as u8);
    }
    for (x, y, s, a) in [
        (30usize, 1usize, "Desk text screen", 0x1Eu8),
        (40, 5, "Bytes as given: \u{1}\u{7f}", 0x0A),
        (40, 8, "All 256 attributes at left", 0x0B),
        (40, 12, "(c) Liminal HQ", 0x70),
    ] {
        t.write_str(x, y, s, a).expect("in range");
        for (i, b) in s.bytes().enumerate() {
            record(shadow, x + i, y, b, a);
        }
    }
}

fn mismatches(t: &Text, shadow: &[u8]) -> usize {
    let mut bad = 0;
    for y in 0..ROWS {
        for x in 0..COLUMNS {
            let (c, a) = t.get(x, y).expect("in range");
            let at = (y * COLUMNS + x) * 2;
            if c != shadow[at] || a != shadow[at + 1] {
                bad += 1;
            }
        }
    }
    bad
}

fn leave_held() -> i32 {
    assert!(cleanup::register(report), "clean-up list full");
    // Start from mode 1 (40x25 text), not the 3 that `Text` sets, so that a restore that did nothing would show.
    rt::vga::set_mode(1);
    text::set_cursor_shape(0x0A0B).expect("cursor");
    write_record();
    println!("text: leave-held: found mode {}", text::video_mode().expect("mode"));
    let mut t = Text::enter().expect("enter");
    t.write_str(2, 2, "held at exit", 0x07).expect("write");
    println!("text: leave-held: mode now {}", text::video_mode().expect("mode"));
    println!("text: leave-held: exiting with the text screen held");
    core::mem::forget(t);
    0
}

fn check() -> i32 {
    let rec = read_record();
    let now = (text::video_mode().ok(), text::cursor_shape().ok());
    println!(
        "text: check: the mode and the cursor shape are as before the text screen: {}",
        rec.is_some() && rec.map(|(m, c)| (Some(m), Some(c))) == Some(now)
    );
    let _ = dos::remove(REC);
    0
}

pub fn main() -> i32 {
    match env::args().first().map(alloc::string::String::as_str) {
        Some("leave-held") => return leave_held(),
        Some("check") => return check(),
        _ => {}
    }
    assert!(cleanup::register(report), "clean-up list full");
    // Not the BIOS default shape: a restore that put back the default would then differ from what was found.
    text::set_cursor_shape(0x0A0B).expect("cursor");
    write_record();
    let (m0, c0) = (text::video_mode().expect("mode"), text::cursor_shape().expect("cursor"));
    println!("text: found mode {}, cursor shape {:#06x}", m0, c0);

    let mut t = Text::enter().expect("enter");
    println!(
        "text: entered mode {} at segment {:#06x} ({} class {:?})",
        text::video_mode().expect("mode"),
        t.segment(),
        if t.is_mono() { "monochrome" } else { "colour" },
        rt::probe::video().class
    );
    println!("text: cursor shape now {:#06x}", text::cursor_shape().expect("cursor"));
    println!("text: a second enter while one is active: {:?}", Text::enter().err());
    println!(
        "text: mode after the refused second enter {}",
        text::video_mode().expect("mode")
    );

    let mut shadow: Vec<u8> = alloc::vec![0; BYTES];
    draw(&mut t, &mut shadow);
    println!("text: test screen written, {} bytes of cells", BYTES);
    println!("text: digest {:#010x}", digest(&t));
    println!("text: mismatches against what was written: {}", mismatches(&t, &shadow));

    // The BIOS and `get`/`put` agree on where the buffer is: a cell the BIOS draws is seen by `get`, and a cell `put` writes is read by
    // the BIOS. Nothing here uses this module's own pointer on both sides.
    text::bios_write_cell(10, 10, b'Z', 0x4E).expect("bios write");
    println!(
        "text: a cell the BIOS wrote is seen by get: {}",
        t.get(10, 10) == Ok((b'Z', 0x4E))
    );
    t.put(11, 10, b'Y', 0x2F).expect("put");
    println!(
        "text: a cell put wrote is read by the BIOS: {}",
        text::bios_read_cell(11, 10) == Ok((b'Y', 0x2F))
    );

    let before = digest(&t);
    let refused = [
        t.put(COLUMNS, 0, b'X', 0x4F),
        t.put(0, ROWS, b'X', 0x4F),
        t.put(usize::MAX, usize::MAX, b'X', 0x4F),
        t.write_str(COLUMNS - 3, 5, "abcd", 0x4F),
        t.write_str(0, ROWS, "a", 0x4F),
        t.get(COLUMNS, 0).map(|_| ()),
        t.get(0, ROWS).map(|_| ()),
    ];
    println!(
        "text: out-of-range calls refused: {}",
        refused.iter().all(|r| *r == Err(TextError::OutOfRange))
    );
    println!("text: digest unchanged by the refused writes: {}", digest(&t) == before);

    t.fill(b'.', 0x2E).expect("fill");
    println!("text: after fill the cell (79, 24) is {:?}", t.get(79, 24).expect("get"));
    t.clear().expect("clear");
    println!("text: after clear the cell (0, 0) is {:?}", t.get(0, 0).expect("get"));
    println!("text: poll_key with no key pending: {:?}", t.poll_key());
    println!(
        "text: a key stuffed into the type-ahead buffer: {}",
        text::stuff_key(b'k', 0x25)
    );
    println!("text: poll_key with a key pending: {:?}", t.poll_key());
    println!("text: poll_key after taking it: {:?}", t.poll_key());

    drop(t);
    println!(
        "text: after leaving, mode {} (found {}), cursor shape {:#06x} (found {:#06x})",
        text::video_mode().expect("mode"),
        m0,
        text::cursor_shape().expect("cursor"),
        c0
    );
    println!("text: exiting");
    0
}
