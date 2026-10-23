// The first window in the kernel: the shell's scripted scenario drawn on the real display, one rendezvous per frame.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Test modes `shell` and `shell-corrupt` (`docs/FIRST-WINDOW.md`, "Native: the kernel's `shell` test mode in QEMU"). The shell crates
// are the same ones the host tests and the hosted program run; what is specific here is where the pixels go. With the linear frame
// buffer the loader set (`--video vbe`) the shell runs at depth 8 through `LinearPresent`; without one it programs mode 12h by
// registers and runs at depth 4 through `PlanarPresent`. At each `frame` step it prints the frame's digests and the rendezvous
// marker, then waits for a key while the run script (`tools/frames.py inject`) takes a QEMU screendump; the scenario's own input is
// scripted, so the keyboard is used only for that acknowledgement. `shell-corrupt` changes one pixel of the screen (not of the
// shell's back buffer) after the `moved` frame, so the transcript stays honest and only the screendump can notice.

use crate::gfx::{self, lfb_region, rendezvous, vga_window};
use crate::hal::PORTS;
use crate::println;
use crate::video::Video;
use desk_font::Font;
use desk_gfx::Present;
use desk_hal::Rgb6;
use desk_i8042::Ps2Input;
use desk_present::{LinearPresent, PlanarPresent};
use desk_shell::scenario::{parse, run as run_scenario, RunError};
use desk_shell::Shell;
use desk_ui::{palette, Depth};
use desk_vga::{dac, regs};
use desk_wm::Fonts;

static UI_FONT: &[u8] = include_bytes!("../../fonts/generated/ui.dfn");
static TITLE_FONT: &[u8] = include_bytes!("../../fonts/generated/title.dfn");
static SCENARIO: &str = include_str!("../../tests/shell/first-window.scn");

/// The frame after which `shell-corrupt` changes the screen, and the pixel it changes (a stretch of the desktop clear of every window
/// and of the cursor in that frame).
const CORRUPT_AFTER: &str = "moved";
const CORRUPT_AT: (u32, u32) = (600, 440);

fn bits(depth: Depth) -> u32 {
    match depth {
        Depth::Bits8 => 8,
        Depth::Bits4 => 4,
    }
}

/// Changes the pixel at `CORRUPT_AT` on the screen to a palette index with a different colour, through the presenter so it works for
/// either depth (the planar presenter writes whole 8-pixel groups).
fn corrupt<P: Present>(out: &mut P, shell: &Shell, colours: &[Rgb6; dac::ENTRIES]) {
    let (x, y) = CORRUPT_AT;
    let x0 = x & !7;
    let mut group = [0u8; 8];
    group.copy_from_slice(&shell.back().row(y)[x0 as usize..x0 as usize + 8]);
    let at = (x - x0) as usize;
    let old = group[at];
    let new = (0..16u8).find(|i| colours[usize::from(*i)] != colours[usize::from(old)]).expect("a differing colour");
    group[at] = new;
    out.begin().expect("begin the corrupting write");
    out.span(x0, y, &group).expect("the corrupting write");
    out.end().expect("end the corrupting write");
    println!("shell: corrupted pixel {},{} of the screen: index {} became {}", x, y, old, new);
}

/// Runs the scenario at `depth` into `out`: a frame, its digests, the rendezvous. `corrupt_with` reaches the same screen by a second
/// presenter for `shell-corrupt`; it is called after `CORRUPT_AFTER` is presented and digested.
fn play<P: Present>(
    depth: Depth,
    out: &mut P,
    ps2: &mut Ps2Input,
    colours: &[Rgb6; dac::ENTRIES],
    corrupt_with: &mut dyn FnMut(&Shell, &[Rgb6; dac::ENTRIES]),
) {
    let sc = parse(SCENARIO).unwrap_or_else(|e| panic!("shell: the scenario does not parse: {}", e));
    assert!(sc.depths.contains(&depth), "shell: the scenario does not list depth {}", bits(depth));
    let fonts = Fonts {
        ui: Font::parse(UI_FONT).expect("shell: the UI font"),
        title: Font::parse(TITLE_FONT).expect("shell: the title font"),
    };
    let mut shell = Shell::new(sc.screen.0, sc.screen.1, depth, fonts).expect("shell: a shell for the scenario's screen");
    let result = run_scenario(&sc, depth, &mut shell, out, |name, s, stats| {
        let d = s.digest();
        println!(
            "shell: frame {} {} {}x{} {:08x} {:08x}",
            name,
            bits(depth),
            d.width,
            d.height,
            d.index,
            d.colour
        );
        println!("shell: damage {}: {} pixels repainted, {} presented in {} rectangles", name, stats.repainted, stats.presented, stats.rects);
        if name.as_str() == CORRUPT_AFTER {
            corrupt_with(s, colours);
        }
        rendezvous(ps2, "shell", name.as_str());
        Ok::<(), RunError>(())
    });
    if let Err(e) = result {
        panic!("shell: the scenario failed: {}", e);
    }
}

/// Runs the first-window scenario on the display the loader left, then restores text mode. `corrupt_pixel` is the `shell-corrupt`
/// test mode.
pub fn run(video: &Video, mut ps2: Ps2Input, corrupt_pixel: bool) {
    let font = video.font;
    let sc_depth = match &video.lfb {
        Some(l) if (l.width, l.height) == (640, 480) => Depth::Bits8,
        Some(l) => panic!("shell: the loader's {}x{} frame buffer is not the scenario's 640x480", l.width, l.height),
        None => Depth::Bits4,
    };
    println!("shell: depth {}", bits(sc_depth));
    let colours = palette(sc_depth);
    match (&video.lfb, sc_depth) {
        (Some(l), _) => {
            println!("shell: linear frame buffer, mode {:#06x}, {}x{}, pitch {}", l.mode, l.width, l.height, l.pitch);
            dac::set_palette(&PORTS, 0, &colours).expect("shell: the palette");
            let mut region = lfb_region(l);
            let mut out = LinearPresent::new(&mut region, l.width, l.height, l.pitch).expect("shell: the linear presenter");
            let mut second = lfb_region(l);
            let (w, h, pitch) = (l.width, l.height, l.pitch);
            let mut again = |s: &Shell, c: &[Rgb6; dac::ENTRIES]| {
                if corrupt_pixel {
                    let mut p = LinearPresent::new(&mut second, w, h, pitch).expect("shell: a second linear presenter");
                    corrupt(&mut p, s, c);
                }
            };
            play(sc_depth, &mut out, &mut ps2, &colours, &mut again);
            gfx::leave_vbe();
        }
        (None, _) => {
            println!("shell: no linear frame buffer from the loader, mode 12h by registers");
            regs::set_mode_12h(&PORTS);
            dac::set_palette(&PORTS, 0, &colours).expect("shell: the palette");
            let mut region = vga_window();
            let mut out = PlanarPresent::new(&PORTS, &mut region, 640, 480, desk_vga::planar::PITCH_640).expect("shell: the planar presenter");
            let mut second = vga_window();
            let mut again = |s: &Shell, c: &[Rgb6; dac::ENTRIES]| {
                if corrupt_pixel {
                    let mut p = PlanarPresent::new(&PORTS, &mut second, 640, 480, desk_vga::planar::PITCH_640).expect("shell: a second planar presenter");
                    corrupt(&mut p, s, c);
                }
            };
            play(sc_depth, &mut out, &mut ps2, &colours, &mut again);
        }
    }
    println!("shell: done");
    gfx::restore_text(font);
}
