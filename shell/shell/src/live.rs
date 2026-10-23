// The live loop's pure part: which keys the platform keeps, the counters of a session and the lines it prints.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Both platforms run the same loop (drain the mouse, drain the keys, present when dirty, idle) and print the same lines. What they
// share that is not pixels lives here, so it is tested on the host: there are no clocks and no devices in this module.

use alloc::format;
use alloc::string::String;
use desk_gfx::{FrameDigest, FrameStats};
use desk_hal::{KeyCode, KeyEvent};
use desk_ui::Depth;

/// What the platform's loop does with a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// Hand it to `Shell::handle`.
    Shell,
    /// F12 was pressed: a mark is due.
    Mark,
    /// Escape was pressed: the session ends.
    End,
    /// The release of Escape or F12: the platform keeps it and does nothing.
    Swallow,
}

/// Escape pressed ends the session and F12 pressed asks for a mark. Their releases are swallowed too, so the shell never sees either
/// key. Every other key is the shell's.
pub fn route(e: &KeyEvent) -> Route {
    match (e.code, e.pressed) {
        (KeyCode::Escape, true) => Route::End,
        (KeyCode::F12, true) => Route::Mark,
        (KeyCode::Escape | KeyCode::F12, false) => Route::Swallow,
        _ => Route::Shell,
    }
}

/// The most marks a session prints: their numbers are two digits.
pub const MAX_MARKS: u32 = 100;

/// Counts for the summary line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// Key events read, presses and releases, including the ones the platform kept.
    pub keys: u32,
    /// Pointer reports handled.
    pub pointer: u32,
    /// Frames that presented something.
    pub frames: u32,
    /// Pixels handed to the presenter, summed over the frames.
    pub presented: u32,
    /// Marks printed.
    pub marks: u32,
}

impl Tally {
    /// Counts one frame's stats; a frame that presented nothing is not a frame.
    pub fn frame(&mut self, s: &FrameStats) {
        if s.presented > 0 {
            self.frames += 1;
            self.presented = self.presented.saturating_add(s.presented);
        }
    }
}

/// `shell: mark NN DEPTH WIDTHxHEIGHT INDEX COLOUR`, the line both platforms print for mark `n` (from 00 to 99, zero-padded so no
/// mark's line starts with another's).
pub fn mark_line(n: u32, depth: Depth, d: &FrameDigest) -> String {
    let bits = match depth {
        Depth::Bits8 => 8,
        Depth::Bits4 => 4,
    };
    format!(
        "shell: mark {n:02} {bits} {}x{} {:08x} {:08x}",
        d.width, d.height, d.index, d.colour
    )
}

/// The line that says a hundredth mark was asked for and not printed.
pub fn marks_full_line() -> String {
    format!("shell: mark refused, a session prints at most {MAX_MARKS}")
}

/// `shell: live ended by Escape: K keys, P pointer reports, F frames, X pixels presented, M marks`.
pub fn summary_line(t: &Tally) -> String {
    format!(
        "shell: live ended by Escape: {} keys, {} pointer reports, {} frames, {} pixels presented, {} marks",
        t.keys, t.pointer, t.frames, t.presented, t.marks
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use desk_hal::Modifiers;

    fn key(code: KeyCode, pressed: bool) -> KeyEvent {
        KeyEvent {
            code,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn escape_ends_and_f12_marks_on_the_press() {
        assert_eq!(route(&key(KeyCode::Escape, true)), Route::End);
        assert_eq!(route(&key(KeyCode::F12, true)), Route::Mark);
    }

    #[test]
    fn the_releases_of_both_are_swallowed() {
        assert_eq!(route(&key(KeyCode::Escape, false)), Route::Swallow);
        assert_eq!(route(&key(KeyCode::F12, false)), Route::Swallow);
    }

    #[test]
    fn every_other_key_is_the_shells() {
        for code in [
            KeyCode::A,
            KeyCode::Tab,
            KeyCode::LeftAlt,
            KeyCode::F4,
            KeyCode::F11,
            KeyCode::Enter,
            KeyCode::Delete,
            KeyCode::Backspace,
        ] {
            assert_eq!(route(&key(code, true)), Route::Shell, "{code:?} down");
            assert_eq!(route(&key(code, false)), Route::Shell, "{code:?} up");
        }
    }

    #[test]
    fn modifiers_do_not_change_the_route() {
        let e = KeyEvent {
            code: KeyCode::Escape,
            pressed: true,
            modifiers: Modifiers::SHIFT.union(Modifiers::ALT),
        };
        assert_eq!(route(&e), Route::End);
    }

    fn digest() -> FrameDigest {
        FrameDigest {
            width: 640,
            height: 480,
            index: 0x920263aa,
            colour: 0xc310363f,
        }
    }

    #[test]
    fn the_mark_line_matches_the_transcript_format() {
        assert_eq!(
            mark_line(0, Depth::Bits8, &digest()),
            "shell: mark 00 8 640x480 920263aa c310363f"
        );
        assert_eq!(
            mark_line(
                7,
                Depth::Bits4,
                &FrameDigest {
                    index: 0x1,
                    colour: 0xab,
                    ..digest()
                }
            ),
            "shell: mark 07 4 640x480 00000001 000000ab"
        );
    }

    #[test]
    fn marks_have_two_digits_so_none_is_a_prefix_of_another() {
        let lines: alloc::vec::Vec<String> = (0..MAX_MARKS).map(|n| mark_line(n, Depth::Bits8, &digest())).collect();
        assert!(lines[9].starts_with("shell: mark 09 "));
        assert!(lines[10].starts_with("shell: mark 10 "));
        assert!(lines[99].starts_with("shell: mark 99 "));
        for (i, a) in lines.iter().enumerate() {
            let head = &a[..14];
            for (j, b) in lines.iter().enumerate() {
                assert!(i == j || !b.starts_with(head), "mark {i} is a prefix of mark {j}");
            }
        }
    }

    #[test]
    fn the_summary_line_matches_the_transcript_format() {
        let t = Tally {
            keys: 14,
            pointer: 9,
            frames: 12,
            presented: 41234,
            marks: 4,
        };
        assert_eq!(
            summary_line(&t),
            "shell: live ended by Escape: 14 keys, 9 pointer reports, 12 frames, 41234 pixels presented, 4 marks"
        );
    }

    #[test]
    fn the_tally_counts_only_frames_that_presented() {
        let mut t = Tally::default();
        t.frame(&FrameStats::default());
        assert_eq!(t, Tally::default());
        t.frame(&FrameStats {
            repainted: 10,
            presented: 456,
            rects: 2,
        });
        t.frame(&FrameStats {
            repainted: 0,
            presented: 100,
            rects: 1,
        });
        assert_eq!((t.frames, t.presented), (2, 556));
    }

    #[test]
    fn the_refusal_line_names_the_limit() {
        assert!(marks_full_line().contains("100"));
    }
}
