// hellowin: the visible-module spike's module. `app_start` opens a window and draws in it through three services, then makes one bad
// call of each kind the kernel refuses; `app_stop` fills the window past the cap on items.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The draws are the ones of tests/shell/module-window.scn for `hellowin`, in the same order, so the native run gives the digests the
// host scenario gives. The numbers of the colour roles are the positions in `desk_ui::Role::ALL`: a throwaway numbering until R79
// names the roles in the SDK (docs/LOADER.md, "Visible-module spike").
#![no_std]
#![no_main]
#![deny(clippy::float_arithmetic)]

use core::fmt::Write;

use desk_guest::{desk_app, import, log, Lines};

import!(
    /// Opens a window whose content area is `w` by `h`; returns its handle, or 0 when the core refuses.
    fn desk_window_new(app: u32, title: *const u8, len: u32, w: i32, h: i32, flags: u32) -> u32
);
import!(
    /// Fills a rectangle of the window's content area (content coordinates) in a colour role.
    fn desk_fill(win: u32, x: i32, y: i32, w: i32, h: i32, role: u32)
);
import!(
    /// Draws a line of text at a position of the window's content area in a face and a colour role.
    fn desk_text(win: u32, x: i32, y: i32, s: *const u8, len: u32, font: u32, role: u32)
);

// Positions in `desk_ui::Role::ALL`.
const RAISED: u32 = 3;
const LINE: u32 = 4;
const BORDER: u32 = 5;
const TEXT_MUTED: u32 = 6;
const TEXT_SECONDARY: u32 = 7;
const TEXT: u32 = 8;
const ACCENT: u32 = 9;
const HALO2: u32 = 10;
const SELECTION: u32 = 12;
/// One past the last role: refused.
const NO_ROLE: u32 = 15;

/// The only face so far.
const UI_FONT: u32 = 0;

fn fill(win: u32, x: i32, y: i32, w: i32, h: i32, role: u32) {
    // SAFETY: the module is loaded, so the slot is bound; `desk_fill` takes plain numbers.
    unsafe { desk_fill(win, x, y, w, h, role) }
}

fn text(win: u32, x: i32, y: i32, s: &str, role: u32) {
    // SAFETY: as above; `s` is readable for its length and the core only reads it.
    unsafe { desk_text(win, x, y, s.as_ptr(), s.len() as u32, UI_FONT, role) }
}

fn start(app: u32, _args: &[u8]) -> i32 {
    let title = "HELLOWIN.DMD";
    // SAFETY: the slot is bound; `title` is readable for its length.
    let win = unsafe { desk_window_new(app, title.as_ptr(), title.len() as u32, 320, 160, 0) };
    if win == 0 {
        log("hellowin: no window");
        return 1;
    }

    // The overhang at the top-left is drawn first, so the text and swatches paint over what they share with it.
    fill(win, -10, -10, 30, 20, HALO2);
    text(win, 8, 8, "Drawn by HELLOWIN.DMD", TEXT);
    text(win, 8, 28, "a loaded module, through three services", TEXT_SECONDARY);
    text(win, 8, 48, "desk_window_new, desk_fill, desk_text", TEXT_MUTED);
    let swatches = [RAISED, LINE, BORDER, TEXT_MUTED, TEXT_SECONDARY, TEXT, ACCENT, SELECTION];
    for (i, role) in swatches.into_iter().enumerate() {
        fill(win, 8 + 36 * i as i32, 76, 28, 20, role);
    }
    // The overhang at the bottom-right.
    fill(win, 250, 110, 200, 200, ACCENT);

    bad_calls(app, win);
    let _ = writeln!(Lines::new(), "hellowin: window {win}");
    0
}

/// One call of each kind the core refuses; none of them draws anything, and the core prints one line for each.
fn bad_calls(app: u32, win: u32) {
    let title = "bad";
    let one = "x";
    let not_utf8: [u8; 2] = [0xC3, 0x28];
    // SAFETY (each call below): the slots are bound, and each pointer is readable for its length or is one the core must refuse
    // without reading it.
    unsafe {
        desk_window_new(app + 1, title.as_ptr(), 3, 100, 100, 0); // not the running instance
        desk_window_new(app, title.as_ptr(), 3, 100, 100, 1); // flags the core does not define
        desk_window_new(app, not_utf8.as_ptr(), 2, 100, 100, 0); // a title that is not UTF-8
        desk_window_new(app, title.as_ptr(), 3, 0, 100, 0); // no width
    }
    fill(0, 0, 0, 4, 4, TEXT); // handle 0
    fill(9, 0, 0, 4, 4, TEXT); // a handle that is not a slot
    fill(win, 0, 0, 0, 10, TEXT); // an empty rectangle
    fill(win, 0, 0, 4, 4, NO_ROLE); // a role that does not exist

    // SAFETY: as above.
    unsafe {
        desk_text(win, 0, 0, not_utf8.as_ptr(), 2, UI_FONT, TEXT); // not UTF-8
        desk_text(win, 0, 0, one.as_ptr(), 1, 1, TEXT); // a face that does not exist
        desk_text(win, 0, 0, core::ptr::null(), 4, UI_FONT, TEXT); // a null pointer with a length
        desk_text(win, 0, 0, 0xFFFF_FFF0usize as *const u8, 32, UI_FONT, TEXT); // a pointer and length that wrap
    }
}

/// The fills `app_stop` makes: the window holds 13 items and the cap is 32, so 19 fit and the 20th is refused. `desk_fill` returns
/// nothing, as docs/SDK.md has it, so the module cannot see a refusal; it asks for one fill more than fits and the core's transcript
/// shows the line.
const STOP_FILLS: u32 = 20;

fn stop(app: u32) {
    for i in 0..STOP_FILLS as i32 {
        fill(1, i, 150, 1, 1, TEXT);
    }
    let _ = writeln!(Lines::new(), "hellowin: stop app={app}, {STOP_FILLS} fills asked for");
}

desk_app!(start = start, stop = stop);
