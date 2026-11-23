// The first window in the kernel: the shell's scripted scenario drawn on the real display, one rendezvous per frame.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Test modes `shell` and `shell-corrupt` (`docs/FIRST-WINDOW.md`, "Native: the kernel's `shell` test mode in QEMU") and `shell-live`
// (`docs/LIVE-INPUT.md`): the first window on the PS/2 keyboard and mouse, until Escape. The shell crates
// are the same ones the host tests and the hosted program run; what is specific here is where the pixels go. With the linear frame
// buffer the loader set (`--video vbe`) the shell runs at depth 8 through `LinearPresent`; without one it programs mode 12h by
// registers and runs at depth 4 through `PlanarPresent`. At each `frame` step it prints the frame's digests and the rendezvous
// marker, then waits for a key while the run script (`tools/frames.py inject`) takes a QEMU screendump; the scenario's own input is
// scripted, so the keyboard is used only for that acknowledgement. `shell-corrupt` changes one pixel of the screen (not of the
// shell's back buffer) after the `moved` frame, so the transcript stays honest and only the screendump can notice.

use crate::gfx::{self, lfb_region, rendezvous, vga_window};
use crate::hal::PORTS;
use crate::modwin::{self, Instance};
use crate::video::Video;
use crate::{input, pit, println};
use desk_font::Font;
use desk_gfx::Present;
use desk_hal::{InputSource, Rgb6};
use desk_i8042::Ps2Input;
use desk_present::{LinearPresent, PlanarPresent};
use desk_shell::live::{self, Route, Tally};
use desk_shell::scenario::{apply, parse, run as run_scenario, RunError, Step};
use desk_shell::{Event, Shell};
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

/// The shortest time between two presents in a live session, in PIT ticks (10 ms at 100 Hz): a burst of mouse packets inside one tick
/// is one present, and an idle desktop halts between ticks.
const MIN_FRAME_TICKS: u32 = 1;

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
    let new = (0..16u8)
        .find(|i| colours[usize::from(*i)] != colours[usize::from(old)])
        .expect("a differing colour");
    group[at] = new;
    out.begin().expect("begin the corrupting write");
    out.span(x0, y, &group).expect("the corrupting write");
    out.end().expect("end the corrupting write");
    println!(
        "shell: corrupted pixel {},{} of the screen: index {} became {}",
        x, y, old, new
    );
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
    assert!(
        sc.depths.contains(&depth),
        "shell: the scenario does not list depth {}",
        bits(depth)
    );
    let mut shell = Shell::new(sc.screen.0, sc.screen.1, depth, fonts()).expect("shell: a shell for the scenario's screen");
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
        println!(
            "shell: damage {}: {} pixels repainted, {} presented in {} rectangles",
            name, stats.repainted, stats.presented, stats.rects
        );
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

/// The depth the loader's display gives the shell: 8 over a 640 x 480 linear frame buffer, 4 in mode 12h by registers.
fn depth_for(video: &Video) -> Depth {
    match &video.lfb {
        Some(l) if (l.width, l.height) == (640, 480) => Depth::Bits8,
        Some(l) => panic!(
            "shell: the loader's {}x{} frame buffer is not the scenario's 640x480",
            l.width, l.height
        ),
        None => Depth::Bits4,
    }
}

fn fonts() -> Fonts<'static> {
    Fonts {
        ui: Font::parse(UI_FONT).expect("shell: the UI font"),
        title: Font::parse(TITLE_FONT).expect("shell: the title font"),
    }
}

/// The first window on live input until Escape (`docs/LIVE-INPUT.md`, "The live loop"): the scenario's set-up steps, the first frame
/// (mark 00), then each pass drains the mouse and the keys (Escape ends, F12 prints a mark, everything else is the shell's), presents
/// when the shell is dirty and a tick has passed (or a mark is due), and halts until the next interrupt.
fn live_session<P: Present>(depth: Depth, out: &mut P, ps2: &mut Ps2Input) {
    let sc = parse(SCENARIO).unwrap_or_else(|e| panic!("shell: the scenario does not parse: {}", e));
    assert!(
        sc.depths.contains(&depth),
        "shell: the scenario does not list depth {}",
        bits(depth)
    );
    let mut shell = Shell::new(sc.screen.0, sc.screen.1, depth, fonts()).expect("shell: a shell for the scenario's screen");
    for step in sc.steps.iter().take_while(|s| !matches!(s, Step::Frame(_))) {
        apply(&mut shell, step).unwrap_or_else(|e| panic!("shell: the set-up failed: {}", e));
    }
    live_loop(&mut shell, out, ps2, depth, |_, _, _, _| {});
}

/// The live loop over a shell that is already set up: the first frame (mark 00), then each pass drains the mouse and the keys,
/// presents when dirty and a tick has passed, and halts. When Escape ends the session `at_end` runs before the summary line (the
/// module-live mode stops and unloads its module there); `shell-live` passes a hook that does nothing.
fn live_loop<P: Present>(
    shell: &mut Shell<'static>,
    out: &mut P,
    ps2: &mut Ps2Input,
    depth: Depth,
    at_end: impl FnOnce(&mut Shell<'static>, &mut P, &mut Ps2Input, &mut Tally),
) {
    let mut tally = Tally::default();
    tally.frame(&shell.frame(out).expect("shell: the first frame"));
    println!("{}", live::mark_line(0, depth, &shell.digest()));
    tally.marks = 1;
    let mut last = pit::ticks();
    loop {
        let (mut mark_due, mut end) = (false, false);
        while let Some(m) = ps2.poll_mouse() {
            tally.pointer += 1;
            shell.handle(Event::Pointer(m));
        }
        while let Some(k) = ps2.poll_key() {
            tally.keys += 1;
            match live::route(&k) {
                Route::Shell => shell.handle(Event::Key(k)),
                Route::Mark => mark_due = true,
                Route::End => {
                    // What was typed after Escape is not part of the session: it is neither handled nor counted.
                    end = true;
                    break;
                }
                Route::Swallow => {}
            }
        }
        if mark_due {
            // Motion that arrived while the keys were handled belongs before the mark.
            while let Some(m) = ps2.poll_mouse() {
                tally.pointer += 1;
                shell.handle(Event::Pointer(m));
            }
        }
        let now = pit::ticks();
        if mark_due || (shell.is_dirty() && now.wrapping_sub(last) >= MIN_FRAME_TICKS) {
            let stats = shell.frame(out).expect("shell: a live frame");
            if stats.presented > 0 {
                last = now;
            }
            tally.frame(&stats);
        }
        if mark_due {
            if tally.marks < live::MAX_MARKS {
                println!("{}", live::mark_line(tally.marks, depth, &shell.digest()));
                tally.marks += 1;
            } else {
                println!("{}", live::marks_full_line());
            }
        }
        if end {
            at_end(shell, out, ps2, &mut tally);
            println!("{}", live::summary_line(&tally));
            return;
        }
        input::idle(ps2);
    }
}

/// The first window on live input on the display the loader left, then restores text mode (`shell-live`).
pub fn live(video: &Video, mut ps2: Ps2Input) {
    let font = video.font;
    let depth = depth_for(video);
    println!("shell: live, depth {}", bits(depth));
    let colours = palette(depth);
    match &video.lfb {
        Some(l) => {
            println!(
                "shell: linear frame buffer, mode {:#06x}, {}x{}, pitch {}",
                l.mode, l.width, l.height, l.pitch
            );
            dac::set_palette(&PORTS, 0, &colours).expect("shell: the palette");
            let mut region = lfb_region(l);
            let mut out = LinearPresent::new(&mut region, l.width, l.height, l.pitch).expect("shell: the linear presenter");
            live_session(depth, &mut out, &mut ps2);
            gfx::leave_vbe();
        }
        None => {
            println!("shell: no linear frame buffer from the loader, mode 12h by registers");
            regs::set_mode_12h(&PORTS);
            dac::set_palette(&PORTS, 0, &colours).expect("shell: the palette");
            let mut region = vga_window();
            let mut out = PlanarPresent::new(&PORTS, &mut region, 640, 480, desk_vga::planar::PITCH_640)
                .expect("shell: the planar presenter");
            live_session(depth, &mut out, &mut ps2);
        }
    }
    println!("shell: done");
    gfx::restore_text(font);
}

/// Runs the first-window scenario on the display the loader left, then restores text mode. `corrupt_pixel` is the `shell-corrupt`
/// test mode.
pub fn run(video: &Video, mut ps2: Ps2Input, corrupt_pixel: bool) {
    let font = video.font;
    let sc_depth = depth_for(video);
    println!("shell: depth {}", bits(sc_depth));
    let colours = palette(sc_depth);
    match (&video.lfb, sc_depth) {
        (Some(l), _) => {
            println!(
                "shell: linear frame buffer, mode {:#06x}, {}x{}, pitch {}",
                l.mode, l.width, l.height, l.pitch
            );
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
            let mut out = PlanarPresent::new(&PORTS, &mut region, 640, 480, desk_vga::planar::PITCH_640)
                .expect("shell: the planar presenter");
            let mut second = vga_window();
            let mut again = |s: &Shell, c: &[Rgb6; dac::ENTRIES]| {
                if corrupt_pixel {
                    let mut p = PlanarPresent::new(&PORTS, &mut second, 640, 480, desk_vga::planar::PITCH_640)
                        .expect("shell: a second planar presenter");
                    corrupt(&mut p, s, c);
                }
            };
            play(sc_depth, &mut out, &mut ps2, &colours, &mut again);
        }
    }
    println!("shell: done");
    gfx::restore_text(font);
}

// The visible-module spike (`docs/LOADER.md`, "Visible-module spike"): the scenario of tests/shell/module-window.scn with the
// `hellowin` window drawn by HELLOWIN.DMD, loaded through the real loader, instead of by the scenario's own steps.

static MODULE_SCENARIO: &str = include_str!("../../tests/shell/module-window.scn");

/// The scenario's name for the window the module draws.
const MODULE_WINDOW: &str = "hellowin";

/// How long the live module run holds its closed desktop after Escape, in PIT ticks (2 s at 100 Hz), unless a key ends it: the run
/// script takes its last screendump then and presses one.
const HOLD_TICKS: u32 = 200;

/// The window a step names, if it names one.
fn window_of(step: &Step) -> Option<&str> {
    match step {
        Step::Open { name: w, .. }
        | Step::Label { window: w, .. }
        | Step::Text { window: w, .. }
        | Step::Fill { window: w, .. }
        | Step::Field { window: w, .. }
        | Step::Close(w)
        | Step::Focus(w) => Some(w),
        _ => None,
    }
}

/// Where the module is in a run: not yet loaded, running, or unloaded.
enum Module {
    Waiting,
    Running(Instance),
    Gone,
}

impl Module {
    /// Applies one step. Steps that do not name the module's window are the shell's; the first one that does loads the module and
    /// starts it (its draws replace the scenario's `open`, `fill` and `text` steps for that window), and `close` stops and unloads it.
    fn step(&mut self, shell: &mut Shell<'static>, step: &Step, screen: (u32, u32)) {
        if window_of(step) != Some(MODULE_WINDOW) {
            apply(shell, step).unwrap_or_else(|e| panic!("shell: the scenario failed: {}", e));
            return;
        }
        match core::mem::replace(self, Module::Gone) {
            Module::Waiting => {
                let mut instance = Instance::load(screen);
                instance.start(shell);
                *self = Module::Running(instance);
            }
            Module::Running(instance) if matches!(step, Step::Close(_)) => instance.unload(shell),
            other => *self = other,
        }
    }

    /// Stops and unloads a module still running (the live run's end).
    fn finish(&mut self, shell: &mut Shell<'static>) {
        if let Module::Running(instance) = core::mem::replace(self, Module::Gone) {
            instance.unload(shell);
        }
    }
}

/// Waits for a key press, or `HOLD_TICKS`, whichever is first; mouse events are dropped.
fn hold(ps2: &mut Ps2Input) {
    let start = pit::ticks();
    loop {
        while ps2.poll_mouse().is_some() {}
        while let Some(k) = ps2.poll_key() {
            if k.pressed {
                return;
            }
        }
        if pit::ticks().wrapping_sub(start) > HOLD_TICKS {
            return;
        }
        input::idle(ps2);
    }
}

/// What runs on the display once it is set: the scripted run (`module-window`) or the live one (`module-live`).
struct ModuleRun {
    ps2: Ps2Input,
    live: bool,
}

impl ModuleRun {
    fn run<P: Present>(&mut self, depth: Depth, out: &mut P) {
        let sc = parse(MODULE_SCENARIO).unwrap_or_else(|e| panic!("shell: the scenario does not parse: {}", e));
        assert!(
            sc.depths.contains(&depth),
            "shell: the scenario does not list depth {}",
            bits(depth)
        );
        let mut shell = Shell::new(sc.screen.0, sc.screen.1, depth, fonts()).expect("shell: a shell for the scenario's screen");
        let mut module = Module::Waiting;
        if self.live {
            for step in sc.steps.iter().take_while(|s| !matches!(s, Step::Frame(_))) {
                module.step(&mut shell, step, sc.screen);
            }
            live_loop(&mut shell, out, &mut self.ps2, depth, |shell, out, ps2, tally| {
                // Escape: the module stops, its window closes and its frames are freed, the desktop is presented, and that is the
                // last mark; the run script's screendump of it happens in the pause.
                module.finish(shell);
                tally.frame(&shell.frame(out).expect("shell: the closing frame"));
                println!("{}", live::mark_line(tally.marks, depth, &shell.digest()));
                tally.marks += 1;
                hold(ps2);
            });
            return;
        }
        for step in &sc.steps {
            match step {
                Step::Frame(name) => {
                    let stats = shell.frame(out).expect("shell: a frame");
                    let d = shell.digest();
                    println!(
                        "shell: frame {} {} {}x{} {:08x} {:08x}",
                        name,
                        bits(depth),
                        d.width,
                        d.height,
                        d.index,
                        d.colour
                    );
                    println!(
                        "shell: damage {}: {} pixels repainted, {} presented in {} rectangles",
                        name, stats.repainted, stats.presented, stats.rects
                    );
                    rendezvous(&mut self.ps2, "shell", name.as_str());
                }
                Step::ExpectDamage { .. } => {}
                other => module.step(&mut shell, other, sc.screen),
            }
        }
    }
}

/// Runs a body on the display the loader left, then restores text mode.
fn on_display(video: &Video, depth: Depth, body: &mut ModuleRun) {
    let font = video.font;
    let colours = palette(depth);
    match &video.lfb {
        Some(l) => {
            println!(
                "shell: linear frame buffer, mode {:#06x}, {}x{}, pitch {}",
                l.mode, l.width, l.height, l.pitch
            );
            dac::set_palette(&PORTS, 0, &colours).expect("shell: the palette");
            let mut region = lfb_region(l);
            let mut out = LinearPresent::new(&mut region, l.width, l.height, l.pitch).expect("shell: the linear presenter");
            body.run(depth, &mut out);
            gfx::leave_vbe();
        }
        None => {
            println!("shell: no linear frame buffer from the loader, mode 12h by registers");
            regs::set_mode_12h(&PORTS);
            dac::set_palette(&PORTS, 0, &colours).expect("shell: the palette");
            let mut region = vga_window();
            let mut out = PlanarPresent::new(&PORTS, &mut region, 640, 480, desk_vga::planar::PITCH_640)
                .expect("shell: the planar presenter");
            body.run(depth, &mut out);
        }
    }
    println!("shell: done");
    gfx::restore_text(font);
}

/// The test modes `module-window` (scripted: one rendezvous per frame) and `module-live` (the same set-up, then the live loop until
/// Escape): HELLOWIN.DMD loaded by the real loader draws a window in the shell. First the negative: outside a shell mode the module
/// cannot bind its services, so the loader refuses it.
pub fn module(video: &Video, ps2: Ps2Input, live: bool) {
    modwin::refuse_outside_shell();
    let depth = depth_for(video);
    println!("shell: {}depth {}", if live { "live, " } else { "" }, bits(depth));
    on_display(video, depth, &mut ModuleRun { ps2, live });
}
