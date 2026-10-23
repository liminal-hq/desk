// VGA mode 13h held on screen: draws the d0 test pattern with a known palette, reports the frame buffer checksum to a file and
// waits for a key so a browser harness can take a screenshot. With the argument `nowait` it does not wait, for runs with no
// keyboard (a native headless DOSBox).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

extern crate alloc;

use probes::{adler, load_palette, say, wait_key, Report};
use rt::vga;

rt::entry!(main);

fn main() -> i32 {
    let mut rep = Report::default();
    say!(rep, "S5-VGA13 start");
    let mode0 = vga::get_mode();
    vga::enter_mode13();
    let mode = vga::get_mode();
    load_palette();
    vga::draw_pattern();
    let fb = vga::framebuffer();
    // SAFETY: the 64000 bytes of mode 13h at A0000h, reachable through the 4 GiB data segment.
    let sum = adler((0..vga::WIDTH * vga::HEIGHT).map(|i| unsafe { fb.add(i).read_volatile() }));
    let want = vga::expected_checksum();
    // Nothing is printed while the mode is set: BIOS text output would draw glyphs over the pattern. The report goes to a file
    // that the harness polls for, and is printed after the mode is back.
    let mut file = Report::default();
    file.0.push_str(&alloc::format!(
        "mode_before={:#04x} mode={:#04x}\nchecksum={:#010x} expected={:#010x} match={}\n",
        mode0,
        mode,
        sum,
        want,
        sum == want
    ));
    file.save("VGA13.TXT");
    if !rt::env::args().iter().any(|a| a == "nowait") {
        wait_key();
    }
    vga::leave();
    say!(
        rep,
        "mode {:#04x}, checksum {:#010x}, expected {:#010x}, match {}",
        mode,
        sum,
        want,
        sum == want
    );
    say!(rep, "S5-VGA13 done");
    if sum == want && mode == 0x13 {
        0
    } else {
        1
    }
}
