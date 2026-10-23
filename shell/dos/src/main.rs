// The first window on DOS: the shell on the pure-Rust DOS runtime, at 640 x 480 with 256 or 16 colours.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: SHELL.EXE [depth8 | depth4] [scenario] [dump] [fontfile] [corrupt] [timeout=N]
//
// - `depth8`    VBE 640 x 480 x 8 with a linear frame buffer (the default). With no such mode it says so and runs at depth 4.
// - `depth4`    VGA mode 12h, 16 colours on four planes.
// - `scenario`  runs tests/shell/first-window.scn (embedded) with scripted input. At each frame it prints
//               `shell: frame NAME DEPTH WIDTHxHEIGHT INDEX COLOUR`, the shell's own digests.
// - `dump`      with `scenario`: reads the frame buffer and the DAC back after each frame, prints their digests as
//               `shell: readback ...` (which must equal the frame's) and writes NAME.RAW and NAME.PAL for tools/frames.py.
//               Without it, writes the last screen as SCREEN.RAW and SCREEN.PAL when the run ends.
// - `fontfile`  loads the faces from UI.DFN and TITLE.DFN beside the program instead of using the embedded copies.
// - `corrupt`   flips one pixel of the frame buffer after the `moved` frame is presented, so the read-back must differ (the negative
//               check); it implies the read-back.
// - `timeout=N` interactive mode only: gives up after N seconds without Escape.
//
// Without `scenario` the shell runs on the keyboard (IRQ 1) and the mouse (INT 33h) until Escape. Exit status: 0 success, 1 a
// read-back differed from the shell's frame or a step failed, 2 the requested video mode was not available, 3 a panic (the
// runtime's), 4 the interactive timeout, 5 an argument it does not know.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use desk_font::Font;
use desk_gfx::{FrameDigest, Present};
use desk_hal::irq::{Chain, Irq, IrqHandler, IrqRegistry};
use desk_hal::ring::SpscRing;
use desk_hal::x86::RawPorts;
use desk_hal::{Bios, BiosRegs, Display, FrameBuffer, HalError, InputSource, KeyCode, PixelLayout, Rgb6};
use desk_i8042::{IrqStats, KbdIrq};
use desk_int33::{DosInput, Mouse};
use desk_present::{read_back_linear, read_back_planar, LinearPresent, PlanarPresent};
use desk_shell::scenario::{apply, parse, Step};
use desk_shell::{Event, Shell};
use desk_ui::{palette, Depth};
use desk_wm::Fonts;
use rt::hal::{DosBios, DosDisplay, MODE_12H};
use rt::irq::{DosIrq, Slot};
use rt::{bios_kbd, cleanup, dos, println, timer, vga};

rt::entry!(main);

static UI_DFN: &[u8] = include_bytes!("../../../fonts/generated/ui.dfn");
static TITLE_DFN: &[u8] = include_bytes!("../../../fonts/generated/title.dfn");
static SCENARIO: &str = include_str!("../../../tests/shell/first-window.scn");

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;

/// Raw scancodes from the IRQ 1 handler to the core: the shared lock-free ring.
static KEYS: SpscRing<u8, 64> = SpscRing::new();
static STATS: IrqStats = IrqStats::new();
/// The handler's state lives in the program image, not on the heap.
static KBD_SERVICE: Slot<KbdIrq<RawPorts>> = Slot::new();
static ORIGINAL_FLAGS: AtomicU32 = AtomicU32::new(0);
static MOUSE_IN_USE: AtomicBool = AtomicBool::new(false);

/// BIOS ticks since `start`. The counter at 0040:006C restarts from 0 at midnight (after 0x1800B0 ticks), not at 2^32.
fn elapsed_ticks(start: u32) -> u32 {
    const PER_DAY: u32 = 0x18_00B0;
    let now = timer::bios_ticks();
    if now >= start {
        now - start
    } else {
        now + PER_DAY - start
    }
}

fn bits(depth: Depth) -> u32 {
    if depth == Depth::Bits8 {
        8
    } else {
        4
    }
}

/// Registered first, so it runs last: says what the video mode is after the rest of the clean-up list has run.
fn report() {
    println!("shell: after the clean-up list the video mode is {:#04x}", vga::get_mode());
}

/// The BIOS keyboard state goes back as it was found.
fn restore_bios_keyboard() {
    bios_kbd::restore(bios_kbd::Saved::from_flags(ORIGINAL_FLAGS.load(Ordering::Relaxed) as u8));
}

/// Function 0000h again: the mouse driver's defaults, where a DOS program expects it.
fn reset_mouse() {
    if MOUSE_IN_USE.swap(false, Ordering::Relaxed) {
        let mut regs = BiosRegs::default();
        let _ = DosBios.int(desk_int33::VECTOR, &mut regs, None);
    }
}

/// A presenter behind a pointer, so one closure serves both depths (`Shell::frame` wants a sized `Present`).
struct Dyn<'a>(&'a mut dyn Present);

impl Present for Dyn<'_> {
    fn width(&self) -> u32 {
        self.0.width()
    }
    fn height(&self) -> u32 {
        self.0.height()
    }
    fn align(&self) -> u32 {
        self.0.align()
    }
    fn begin(&mut self) -> Result<(), HalError> {
        self.0.begin()
    }
    fn span(&mut self, x: u32, y: u32, pixels: &[u8]) -> Result<(), HalError> {
        self.0.span(x, y, pixels)
    }
    fn end(&mut self) -> Result<(), HalError> {
        self.0.end()
    }
}

/// The video mode and presenter of one depth, and what is needed to read the frame back.
struct Screen {
    depth: Depth,
    fb: FrameBuffer,
}

impl Screen {
    /// Runs `f` with the presenter for the screen's depth over the frame buffer.
    fn present<R>(&mut self, ports: &RawPorts, f: impl FnOnce(&mut Dyn) -> R) -> Result<R, HalError> {
        let pitch = self.fb.pitch;
        match self.depth {
            Depth::Bits8 => Ok(f(&mut Dyn(&mut LinearPresent::new(
                &mut self.fb.region,
                WIDTH,
                HEIGHT,
                pitch,
            )?))),
            Depth::Bits4 => Ok(f(&mut Dyn(&mut PlanarPresent::new(
                ports,
                &mut self.fb.region,
                WIDTH,
                HEIGHT,
                pitch,
            )?))),
        }
    }

    /// The indices on screen, through the read-back of the depth.
    fn read(&mut self, ports: &RawPorts) -> Result<Vec<u8>, HalError> {
        let mut px = vec![0u8; (WIDTH * HEIGHT) as usize];
        let pitch = self.fb.pitch;
        match self.depth {
            Depth::Bits8 => read_back_linear(&mut self.fb.region, WIDTH, HEIGHT, pitch, &mut px)?,
            Depth::Bits4 => read_back_planar(ports, &mut self.fb.region, WIDTH, HEIGHT, pitch, &mut px)?,
        }
        Ok(px)
    }
}

/// Sets the video mode for `want` and loads the palette. Without a 640 x 480 x 8 linear mode, depth 8 says so and gives depth 4.
/// `None` when no mode could be set (the screen is in text mode).
fn start_screen(display: &mut DosDisplay, want: Depth) -> Option<Screen> {
    println!(
        "shell: display offers {} modes, VBE {}",
        display.modes().len(),
        if display.has_vbe() { "present" } else { "absent" }
    );
    let mut depth = want;
    let mut id = MODE_12H;
    if want == Depth::Bits8 {
        let found = display.modes().iter().find(|m| {
            m.layout == PixelLayout::Indexed8 && m.linear && u32::from(m.width) == WIDTH && u32::from(m.height) == HEIGHT
        });
        match found {
            Some(m) => id = m.id,
            None => {
                println!("shell: no 640x480x8 mode with a linear frame buffer; falling back to depth 4");
                depth = Depth::Bits4;
            }
        }
    }
    let fb = match display.set_mode(id) {
        Ok(fb) => fb,
        Err(e) => {
            println!("shell: mode {id:#05x} failed: {e}");
            return None;
        }
    };
    println!("shell: mode {id:#05x}, depth {}, pitch {}", bits(depth), fb.pitch);
    if let Err(e) = display.set_palette(0, &palette(depth)) {
        println!("shell: setting the palette failed: {e}");
        display.restore_text_mode();
        return None;
    }
    Some(Screen { depth, fb })
}

fn write_file(name: &str, bytes: &[u8]) {
    dos::File::open(name, dos::Mode::Create)
        .and_then(|mut f| f.write_all(bytes))
        .expect("capture file");
}

fn load(name: &str) -> Vec<u8> {
    dos::File::open(name, dos::Mode::Read)
        .and_then(|mut f| f.read_to_vec(4096))
        .unwrap_or_else(|e| panic!("cannot read {name}: {e:?}"))
}

/// Writes `STEM.RAW` (the indices) and `STEM.PAL` (the DAC's six-bit `r g b` triples) for `tools/frames.py`.
fn write_capture(stem: &str, pixels: &[u8], dac: &[Rgb6; 256]) {
    let pal: Vec<u8> = dac.iter().flat_map(|c| [c.r, c.g, c.b]).collect();
    write_file(&alloc::format!("{stem}.PAL"), &pal);
    write_file(&alloc::format!("{stem}.RAW"), pixels);
    println!("shell: wrote {stem}.RAW ({} bytes) and {stem}.PAL", pixels.len());
}

/// The scenario: each frame presented, then (with `verify`) the frame buffer and the DAC read back and held to the shell's digests.
/// Returns whether every read-back matched.
fn scenario(display: &DosDisplay, screen: &mut Screen, shell: &mut Shell, dump: bool, corrupt: bool) -> Result<bool, String> {
    let sc = parse(SCENARIO).map_err(|e| alloc::format!("the scenario does not parse: {e}"))?;
    if sc.screen != (WIDTH, HEIGHT) || !sc.depths.contains(&screen.depth) {
        return Err(String::from("the scenario is not for this screen"));
    }
    let verify = dump || corrupt;
    let ports = display.ports();
    let (mut ok, mut frames) = (true, 0);
    for step in &sc.steps {
        let Step::Frame(name) = step else {
            apply(shell, step).map_err(|e| alloc::format!("{e}"))?;
            continue;
        };
        screen
            .present(ports, |p| shell.frame(p))
            .and_then(|r| r)
            .map_err(|e| alloc::format!("frame {}: {e}", name.as_str()))?;
        let d = shell.digest();
        println!(
            "shell: frame {} {} {}x{} {:08x} {:08x}",
            name.as_str(),
            bits(screen.depth),
            d.width,
            d.height,
            d.index,
            d.colour
        );
        frames += 1;
        if corrupt && name.as_str() == "moved" {
            // One pixel of the corner, written through the presenter's own span (whole bytes on the planes), differing from what it held.
            let mut row = screen.read(ports).map_err(|e| alloc::format!("{e}"))?[..8].to_vec();
            row[0] ^= if screen.depth == Depth::Bits8 { 0x55 } else { 0x05 };
            screen
                .present(ports, |p| {
                    p.begin()?;
                    p.span(0, 0, &row)?;
                    p.end()
                })
                .and_then(|r| r)
                .map_err(|e| alloc::format!("{e}"))?;
            println!("shell: corrupted one pixel after {}", name.as_str());
        }
        if !verify {
            continue;
        }
        let px = screen.read(ports).map_err(|e| alloc::format!("read-back: {e}"))?;
        let mut dac = [Rgb6::BLACK; 256];
        display
            .palette(0, &mut dac)
            .map_err(|e| alloc::format!("palette read-back: {e}"))?;
        let back = FrameDigest::from_rows(WIDTH, HEIGHT, &dac, |y, buf| {
            let start = y as usize * WIDTH as usize;
            buf.copy_from_slice(&px[start..start + WIDTH as usize]);
        });
        let same = back == d;
        ok &= same;
        println!(
            "shell: readback {} {} {}x{} {:08x} {:08x} {}",
            name.as_str(),
            bits(screen.depth),
            back.width,
            back.height,
            back.index,
            back.colour,
            if same { "equal" } else { "DIFFERENT" }
        );
        if dump {
            write_capture(&String::from(name.as_str()).to_ascii_uppercase(), &px, &dac);
        }
    }
    println!("shell: scenario done, {frames} frames");
    Ok(ok)
}

/// The shell on the keyboard and the mouse until Escape (or the timeout): returns the exit status.
fn interactive(display: &DosDisplay, screen: &mut Screen, shell: &mut Shell, timeout_s: Option<u32>) -> i32 {
    let saved = bios_kbd::save();
    ORIGINAL_FLAGS.store(u32::from(saved.flags()), Ordering::Relaxed);
    let locks = bios_kbd::locks();
    let mouse = match Mouse::detect(DosBios) {
        Ok(m) => {
            MOUSE_IN_USE.store(true, Ordering::Relaxed);
            cleanup::register(reset_mouse);
            let _ = m.motion();
            println!("shell: mouse driver found, {} buttons", m.buttons());
            Some(m)
        }
        Err(_) => {
            println!("shell: no mouse driver; keyboard only");
            None
        }
    };
    let (tx, rx) = KEYS.split().expect("the ring is split once");
    let service = KBD_SERVICE
        .place(KbdIrq::new(
            // SAFETY: a DPMI client under CWSDPMI may read the keyboard controller's ports; the handler is the only user while installed.
            unsafe { RawPorts::new() },
            tx,
            &STATS,
        ))
        .expect("the slot is used once");
    cleanup::register(restore_bios_keyboard);
    DosIrq
        .register(Irq::KEYBOARD, IrqHandler::from_static(service), Chain::None)
        .expect("install the IRQ 1 handler");
    let mut input = DosInput::new(rx, locks, mouse);

    // The scenario's opening steps (up to its first frame), so there is something to move and a field to type in.
    if let Ok(sc) = parse(SCENARIO) {
        for step in sc.steps.iter().take_while(|s| !matches!(s, Step::Frame(_))) {
            let _ = apply(shell, step);
        }
    }

    let start = timer::bios_ticks();
    let limit = timeout_s.map(|s| s.saturating_mul(19));
    loop {
        let mut escape = false;
        while let Some(e) = input.poll_key() {
            escape |= e.code == KeyCode::Escape && e.pressed;
            shell.handle(Event::Key(e));
        }
        while let Some(m) = input.poll_mouse() {
            shell.handle(Event::Pointer(m));
        }
        if let Err(e) = screen.present(display.ports(), |p| shell.frame(p)).and_then(|r| r) {
            println!("shell: frame failed: {e}");
            return 1;
        }
        if escape {
            return 0;
        }
        if limit.is_some_and(|l| elapsed_ticks(start) > l) {
            println!("shell: timed out without Escape");
            return 4;
        }
    }
}

fn main() -> i32 {
    let args = rt::env::args();
    let has = |a: &str| args.iter().any(|x| x == a);
    let timeout_s = args
        .iter()
        .find_map(|a| a.strip_prefix("timeout=").and_then(|n| n.parse::<u32>().ok()));
    cleanup::register(report);
    for a in &args {
        let known = ["depth8", "depth4", "scenario", "dump", "fontfile", "corrupt"].contains(&a.as_str())
            || a.strip_prefix("timeout=")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        if !known {
            println!("shell: unknown argument {a:?}; usage: SHELL.EXE [depth8 | depth4] [scenario] [dump] [fontfile] [corrupt] [timeout=N]");
            return 5;
        }
    }

    // Both faces are read before the screen changes, so a missing file reports in text mode.
    let files = has("fontfile").then(|| (load("UI.DFN"), load("TITLE.DFN")));
    let (ui, title) = match &files {
        Some((u, t)) => (&u[..], &t[..]),
        None => (UI_DFN, TITLE_DFN),
    };
    println!(
        "shell: faces from {}",
        if files.is_some() {
            "UI.DFN and TITLE.DFN"
        } else {
            "the program"
        }
    );
    let fonts = Fonts {
        ui: Font::parse(ui).expect("the UI face"),
        title: Font::parse(title).expect("the title face"),
    };

    let mut display = DosDisplay::new();
    let want = if has("depth4") { Depth::Bits4 } else { Depth::Bits8 };
    let Some(mut screen) = start_screen(&mut display, want) else {
        return 2;
    };
    let mut shell = Shell::new(WIDTH, HEIGHT, screen.depth, fonts).expect("a shell for 640x480");

    let status = if has("scenario") {
        match scenario(&display, &mut screen, &mut shell, has("dump"), has("corrupt")) {
            Ok(true) => 0,
            Ok(false) => {
                println!("shell: FAILED, a read-back differs from the shell's frame");
                1
            }
            Err(e) => {
                display.restore_text_mode();
                println!("shell: FAILED, {e}");
                1
            }
        }
    } else {
        interactive(&display, &mut screen, &mut shell, timeout_s)
    };
    // The interactive run's last screen, for a look at what the keys and the pointer did.
    if has("dump") && !has("scenario") {
        let mut dac = [Rgb6::BLACK; 256];
        let read = screen.read(display.ports()).map_err(|e| alloc::format!("{e}"));
        match read.and_then(|px| display.palette(0, &mut dac).map(|()| px).map_err(|e| alloc::format!("{e}"))) {
            Ok(px) => write_capture("SCREEN", &px, &dac),
            Err(e) => println!("shell: could not read the screen back: {e}"),
        }
    }
    display.restore_text_mode();
    status
}
