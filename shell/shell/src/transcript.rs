// The transcript: the lines a hosted run prints, held in a fixed buffer until the screen can show them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// On DOS the console draws text with the BIOS, and while a VBE graphics mode is on that text goes straight onto the frame buffer, over
// the window. The hosted program therefore makes no console call while it draws: it files each line here, and prints them all once the
// clean-up list has put the text mode back (docs/LIVE-INPUT.md). The checks read the same lines in the same order.
//
// The buffer is a `static` that the clean-up list's functions reach too, so it is built of atomics used with plain loads and stores (the
// program is single-threaded, and a compare-and-swap is not allowed: shell/clippy.toml) and needs no `unsafe`.
//
// When the buffer is full the transcript keeps its first lines whole and drops the rest: a line is stored in full or not at all, and
// once one has been dropped every later one is too, so what was kept is an unbroken beginning. `flush` ends such a transcript with
// `shell: transcript truncated, N lines dropped`.

use alloc::string::String;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

/// A fixed buffer of `N` bytes for transcript lines; each line takes its length and one byte for the separator.
pub struct Transcript<const N: usize> {
    bytes: [AtomicU8; N],
    len: AtomicUsize,
    dropped: AtomicUsize,
}

impl<const N: usize> Transcript<N> {
    pub const fn new() -> Self {
        Transcript {
            bytes: [const { AtomicU8::new(0) }; N],
            len: AtomicUsize::new(0),
            dropped: AtomicUsize::new(0),
        }
    }

    /// Files one line (without its line end), or counts it as dropped if it does not fit or an earlier line was dropped.
    pub fn line(&self, args: fmt::Arguments) {
        let mut text = String::new();
        let _ = text.write_fmt(args);
        let start = self.len.load(Ordering::Relaxed);
        let end = start + text.len() + 1;
        if end > N || self.dropped.load(Ordering::Relaxed) > 0 {
            self.dropped
                .store(self.dropped.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
            return;
        }
        for (slot, b) in self.bytes[start..end - 1].iter().zip(text.bytes()) {
            slot.store(b, Ordering::Relaxed);
        }
        self.bytes[end - 1].store(b'\n', Ordering::Relaxed);
        self.len.store(end, Ordering::Relaxed);
    }

    /// Hands every filed line to `out` in order, then the truncation line if any were dropped, and empties the buffer.
    pub fn flush(&self, mut out: impl FnMut(&str)) {
        let len = self.len.swap(0, Ordering::Relaxed);
        let dropped = self.dropped.swap(0, Ordering::Relaxed);
        let bytes: alloc::vec::Vec<u8> = self.bytes[..len].iter().map(|b| b.load(Ordering::Relaxed)).collect();
        for line in String::from_utf8_lossy(&bytes).lines() {
            out(line);
        }
        if dropped > 0 {
            out(&alloc::format!("shell: transcript truncated, {dropped} lines dropped"));
        }
    }

    /// The bytes filed so far, for the host tests.
    #[cfg(test)]
    fn used(&self) -> usize {
        self.len.load(Ordering::Relaxed)
    }
}

impl<const N: usize> Default for Transcript<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn flushed<const N: usize>(t: &Transcript<N>) -> Vec<String> {
        let mut v = Vec::new();
        t.flush(|l| v.push(String::from(l)));
        v
    }

    #[test]
    fn nothing_is_handed_out_until_the_flush() {
        let t = Transcript::<64>::new();
        let mut seen = 0;
        t.line(format_args!("shell: mark {:02} 8 640x480", 0));
        t.line(format_args!("shell: live, depth {}", 8));
        assert_eq!(seen, 0);
        t.flush(|_| seen += 1);
        assert_eq!(seen, 2);
    }

    #[test]
    fn lines_come_back_in_order_and_the_buffer_is_empty_after() {
        let t = Transcript::<64>::new();
        t.line(format_args!("one"));
        t.line(format_args!(""));
        t.line(format_args!("three {}", 3));
        assert_eq!(flushed(&t), ["one", "", "three 3"]);
        assert_eq!(t.used(), 0);
        assert!(flushed(&t).is_empty());
    }

    #[test]
    fn a_line_that_fills_the_buffer_exactly_is_kept() {
        let t = Transcript::<8>::new();
        t.line(format_args!("1234567"));
        assert_eq!(flushed(&t), ["1234567"]);
    }

    #[test]
    fn overflow_keeps_the_beginning_whole_and_says_how_many_were_dropped() {
        let t = Transcript::<16>::new();
        t.line(format_args!("aaaa"));
        t.line(format_args!("bbbbbbbbbbbbbbbb"));
        // Short enough to fit in what is left, but an earlier line was dropped, so it goes too.
        t.line(format_args!("c"));
        t.line(format_args!("d"));
        assert_eq!(flushed(&t), ["aaaa", "shell: transcript truncated, 3 lines dropped"]);
        // The flush starts a fresh transcript.
        t.line(format_args!("e"));
        assert_eq!(flushed(&t), ["e"]);
    }
}
