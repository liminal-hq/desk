// Spike B: the keyboard (IRQ 1) and the mouse (INT 33h) on the pure-Rust DOS runtime, through the shared HAL drivers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: INPUT.EXE [text] [13] [chain] [nomouse] [timeout=N] [stall=N] [dump] [panic] [leave]
//
// The program takes IRQ 1 from the BIOS, finds the mouse driver, sets VBE 640 x 480 x 8 (mode 13h when VBE is not offered, or with
// `13`; no graphics at all with `text`), prints every decoded event as one line, draws a software cursor that follows the mouse
// events and the state of the buttons, and ends on Escape. Every exit path puts the video mode, the interrupt vector, the BIOS
// keyboard state and the mouse driver back.
//
// - `text`      no graphics mode (the keyboard checks do not need one)
// - `13`        mode 13h even if VBE is offered
// - `chain`     IRQ 1 chains to the BIOS handler (`Chain::Previous`) instead of replacing it; the BIOS then fills its type-ahead buffer
// - `nomouse`   do not look for a mouse driver
// - `timeout=N` give up after N seconds without Escape (default 30)
// - `stall=N`  do not read any input for N seconds, so the keys typed meanwhile can overflow the ring
// - `dump`     at the end write the screen as SCREEN.RAW and its palette as SCREEN.PAL, for tools/raw2png.py
// - `panic`     install everything, then panic: the clean-up list must put it all back first
// - `leave`     install everything, then exit at once
//
// Exit status: 0 Escape, 2 the requested video mode was not available, 3 a panic (the runtime's), 4 the timeout. Lines that start with
// `key ` or `mouse ` are events, in the form `desk_int33::transcript` defines and the host tests also produce; lines that start with
// `input:` are the program talking about itself.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use desk_hal::irq::{Chain, Irq, IrqHandler, IrqRegistry};
use desk_hal::ring::SpscRing;
use desk_hal::x86::RawPorts;
use desk_hal::{Bios, BiosRegs, Display, InputSource, KeyCode, MouseEvent, Rgb6};
use desk_i8042::{IrqStats, KbdIrq};
use desk_int33::transcript::{write_key, write_mouse};
use desk_int33::{DosInput, Error as MouseError, Mouse};
use desk_vga::vbe::Vbe;
use desk_vga::{Canvas, Linear8, SoftCursor};
use rt::bios_kbd;
use rt::hal::{DosBios, DosDisplay, MODE_13H};
use rt::irq::{DosIrq, Slot};
use rt::{cleanup, dos, println, timer, vga};

rt::entry!(main);

/// Raw scancodes from the IRQ 1 handler to the core: the shared lock-free ring, 64 bytes.
static KEYS: SpscRing<u8, 64> = SpscRing::new();
/// The handler's counters.
static STATS: IrqStats = IrqStats::new();
/// The handler's state lives in the locked program image, not on the heap.
static KBD_SERVICE: Slot<KbdIrq<RawPorts>> = Slot::new();

/// The protected-mode handler IRQ 1 had before ours, as (selector, offset), for the report that checks it came back.
static ORIGINAL_SELECTOR: AtomicU32 = AtomicU32::new(0);
static ORIGINAL_OFFSET: AtomicU32 = AtomicU32::new(0);
static ORIGINAL_FLAGS: AtomicU32 = AtomicU32::new(0);
static MOUSE_IN_USE: AtomicBool = AtomicBool::new(false);

/// Registered first, so it runs last: says what is true after the rest of the clean-up list has run.
fn report() {
    let now = DosIrq::current(Irq::KEYBOARD).unwrap_or((0, 0));
    let was = (
        ORIGINAL_SELECTOR.load(Ordering::Relaxed) as u16,
        ORIGINAL_OFFSET.load(Ordering::Relaxed),
    );
    println!(
        "input: after the clean-up list: IRQ 1 handler is the original one: {}, ours installed: {}, video mode {:#04x}, BIOS keyboard flags {:#04x} (were {:#04x}), BIOS type-ahead {} keys",
        now == was,
        DosIrq::is_installed(Irq::KEYBOARD),
        vga::get_mode(),
        bios_kbd::flags(),
        ORIGINAL_FLAGS.load(Ordering::Relaxed),
        bios_kbd::buffered()
    );
}

/// The BIOS keyboard state goes back as it was found.
fn restore_bios_keyboard() {
    bios_kbd::restore(bios_kbd::Saved::from_flags(ORIGINAL_FLAGS.load(Ordering::Relaxed) as u8));
}

/// Function 0000h again: the driver's defaults (cursor hidden, full range, centred), which is where a DOS program expects it.
fn reset_mouse() {
    if MOUSE_IN_USE.swap(false, Ordering::Relaxed) {
        let mut regs = BiosRegs::default();
        let _ = DosBios.int(desk_int33::VECTOR, &mut regs, None);
    }
}

const BG: u8 = 0;
const OUTLINE: u8 = 1;
const FILL: u8 = 2;
const LIT: u8 = 3;
const DIM: u8 = 4;
const DOWN: u8 = 5;
const UP: u8 = 6;

fn palette() -> [Rgb6; 7] {
    [
        Rgb6::from_rgb8(0x1b, 0x1e, 0x28),
        Rgb6::from_rgb8(0x05, 0x05, 0x07),
        Rgb6::from_rgb8(0xe0, 0xe0, 0xe0),
        Rgb6::from_rgb8(0xff, 0xaa, 0x40),
        Rgb6::from_rgb8(0x33, 0x38, 0x47),
        Rgb6::from_rgb8(0x2e, 0xc6, 0x6a),
        Rgb6::from_rgb8(0x7c, 0x87, 0x96),
    ]
}

/// The screen: the three button boxes, the key log and the cursor.
struct Scene<'a> {
    canvas: Linear8<'a>,
    cursor: SoftCursor,
    log: i32,
    buttons: u8,
}

impl Scene<'_> {
    fn paint_background(&mut self) {
        self.canvas.clear(BG);
        for x in 0..3 {
            self.button(x, false);
        }
    }

    fn button(&mut self, which: i32, on: bool) {
        self.canvas.fill_rect(8 + which * 32, 8, 24, 12, if on { LIT } else { DIM });
        self.canvas.rect(8 + which * 32, 8, 24, 12, FILL);
    }

    /// Everything under the cursor is drawn with the cursor lifted.
    fn buttons(&mut self, state: u8) {
        self.cursor.hide(&mut self.canvas);
        for (i, bit) in [MouseEvent::LEFT, MouseEvent::MIDDLE, MouseEvent::RIGHT]
            .into_iter()
            .enumerate()
        {
            self.button(i as i32, state & bit != 0);
        }
        self.buttons = state;
    }

    fn key(&mut self, pressed: bool) {
        self.cursor.hide(&mut self.canvas);
        let (w, h) = (self.canvas.width() as i32, self.canvas.height() as i32);
        let (x, y) = (8 + self.log * 8 % (w - 16), 28 + self.log * 8 / (w - 16) * 8);
        if y + 6 < h {
            self.canvas.fill_rect(x, y, 6, 6, if pressed { DOWN } else { UP });
        }
        self.log += 1;
    }

    fn cursor_to(&mut self, x: i32, y: i32) {
        self.cursor.show(&mut self.canvas, x, y, OUTLINE, FILL);
    }
}

/// Picks the display's mode: VBE 640 x 480 x 8 with a linear frame buffer if it is offered (and not forced off), else mode 13h.
fn pick_mode(display: &DosDisplay, force_13h: bool) -> (u16, u32, u32) {
    if !force_13h && display.has_vbe() {
        let vbe = Vbe::new(&DosBios);
        if let Ok(info) = vbe.info() {
            let mut ids = [0u16; desk_vga::vbe::MAX_MODES];
            if let Ok(n) = vbe.mode_list(&info, &mut ids) {
                if let Some(mode) = vbe.find(&ids[..n], 640, 480, 8, true) {
                    return (mode.id, 640, 480);
                }
            }
        }
    }
    (MODE_13H, 320, 200)
}

/// Writes the screen as it is (the cursor drawn) and the DAC as SCREEN.RAW and SCREEN.PAL.
fn dump(display: &DosDisplay, canvas: &mut Linear8) {
    let (w, h) = (canvas.width() as usize, canvas.height());
    let mut pixels = alloc::vec![0u8; w * h as usize];
    for y in 0..h {
        let row = &mut pixels[y as usize * w..(y as usize + 1) * w];
        let _ = canvas.read_row(y, row);
    }
    let mut colours = [Rgb6::BLACK; 256];
    let _ = display.palette(0, &mut colours);
    let mut pal = alloc::vec::Vec::new();
    for c in &colours {
        pal.extend_from_slice(&[c.r, c.g, c.b]);
    }
    for (name, bytes) in [("SCREEN.PAL", &pal[..]), ("SCREEN.RAW", &pixels[..])] {
        match dos::File::open(name, dos::Mode::Create).and_then(|mut f| f.write_all(bytes)) {
            Ok(()) => println!("input: wrote {name} ({} bytes)", bytes.len()),
            Err(e) => println!("input: could not write {name}: {e:?}"),
        }
    }
}

fn main() -> i32 {
    let args = rt::env::args();
    let has = |a: &str| args.iter().any(|x| x == a);
    let timeout_s = args
        .iter()
        .find_map(|a| a.strip_prefix("timeout=").and_then(|n| n.parse::<u32>().ok()))
        .unwrap_or(30);
    cleanup::register(report);

    // The BIOS has the keyboard's state; Desk takes the keyboard over from it (docs/B-DOS-INPUT.md).
    let saved = bios_kbd::save();
    ORIGINAL_FLAGS.store(u32::from(saved.flags()), Ordering::Relaxed);
    let locks = bios_kbd::locks();
    println!(
        "input: BIOS keyboard flags {:#04x}, locks at start {:?}",
        saved.flags(),
        locks
    );

    // The mouse: found through its driver, or absent (the keyboard-only mode).
    let mut mouse = None;
    if !has("nomouse") {
        match Mouse::detect(DosBios) {
            Ok(m) => {
                MOUSE_IN_USE.store(true, Ordering::Relaxed);
                cleanup::register(reset_mouse);
                println!(
                    "input: mouse: driver found, {} buttons, wheel API {}, type {:?}",
                    m.buttons(),
                    if m.has_wheel() { "yes" } else { "no" },
                    m.driver_type().ok().flatten()
                );
                mouse = Some(m);
            }
            Err(MouseError::NoDriver) => println!("input: mouse: no mouse driver; keyboard only"),
            Err(e) => println!("input: mouse: {e}; keyboard only"),
        }
    } else {
        println!("input: mouse: not looked for; keyboard only");
    }

    // The screen.
    let mut display = DosDisplay::new();
    let graphics = !has("text");
    let (mut width, mut height) = (640u32, 480u32);
    let mut framebuffer = None;
    if graphics {
        let (id, w, h) = pick_mode(&display, has("13"));
        match display.set_mode(id) {
            Ok(fb) => {
                println!("input: video: mode {id:#05x}, {w} x {h}, pitch {}", fb.pitch);
                (width, height) = (w, h);
                framebuffer = Some(fb);
            }
            Err(e) => {
                println!("input: video: mode {id:#05x} failed: {e}");
                return 2;
            }
        }
        let _ = display.set_palette(0, &palette());
    }

    // The driver's own position runs in the virtual coordinates of the mode: 640 columns in mode 13h too. Desk integrates the
    // mickeys itself and only keeps the driver's range and position as a cross-check.
    if let Some(m) = &mouse {
        let (rx, ry) = (639, height as i16 - 1);
        let _ = m.set_range((0, rx), (0, ry));
        let _ = m.set_position(320, (height / 2) as i16);
        let centre = m.position().map(|p| (p.x, p.y)).ok();
        let _ = m.set_position(700, height as i16 + 100);
        let clamped = m.position().map(|p| (p.x, p.y)).ok();
        let _ = m.set_position(320, (height / 2) as i16);
        println!("input: mouse: range 0..={rx} x 0..={ry}, centre {centre:?}, an out-of-range position clamps to {clamped:?}");
        let _ = m.motion();
    }

    // The keyboard: ours replaces the BIOS handler unless `chain` was asked for.
    let (tx, rx) = KEYS.split().expect("the ring is split once");
    let service = KBD_SERVICE
        .place(KbdIrq::new(
            // SAFETY: a DPMI client under CWSDPMI may read the keyboard controller's ports; the handler is the only user while installed.
            unsafe { RawPorts::new() },
            tx,
            &STATS,
        ))
        .expect("the slot is used once");
    let original = DosIrq::current(Irq::KEYBOARD).expect("the IRQ 1 vector");
    ORIGINAL_SELECTOR.store(u32::from(original.0), Ordering::Relaxed);
    ORIGINAL_OFFSET.store(original.1, Ordering::Relaxed);
    cleanup::register(restore_bios_keyboard);
    let chain = if has("chain") { Chain::Previous } else { Chain::None };
    DosIrq
        .register(Irq::KEYBOARD, IrqHandler::from_static(service), chain)
        .expect("install the IRQ 1 handler");
    println!(
        "input: keyboard: IRQ 1 handler installed at vector {:#04x} (the vector changed: {}), {}",
        DosIrq::vector(Irq::KEYBOARD),
        DosIrq::current(Irq::KEYBOARD).ok() != Some(original),
        if chain == Chain::Previous {
            "chained to the BIOS handler"
        } else {
            "replacing the BIOS handler"
        }
    );

    let mut input = DosInput::new(rx, locks, mouse);

    if has("panic") {
        println!("input: now panicking");
        panic!("panic with the keyboard and the mouse taken over");
    }
    if has("leave") {
        println!("input: now exiting");
        return 0;
    }

    let mut scene = framebuffer.as_mut().map(|fb| {
        let canvas = Linear8::new(&mut fb.region, width, height, fb.pitch).expect("the frame buffer holds the mode");
        let mut s = Scene {
            canvas,
            cursor: SoftCursor::new(),
            log: 0,
            buttons: 0,
        };
        s.paint_background();
        s
    });
    let (mut x, mut y) = (width as i32 / 2, height as i32 / 2);
    if let Some(s) = scene.as_mut() {
        s.cursor_to(x, y);
    }

    let stall = args
        .iter()
        .find_map(|a| a.strip_prefix("stall=").and_then(|n| n.parse::<u32>().ok()))
        .unwrap_or(0);
    let began = timer::bios_ticks();
    while timer::bios_ticks().wrapping_sub(began) < stall * 19 {}
    if stall > 0 {
        println!(
            "input: stalled {stall} s: {} scancode bytes read by the handler, {} dropped by the full ring",
            IrqStats::get(&STATS.kbd_bytes),
            input.keys_dropped()
        );
    }

    let start = timer::bios_ticks();
    // 18.2065 ticks a second, rounded up.
    let limit = timeout_s * 19;
    let (mut keys, mut mice) = (0u32, 0u32);
    let mut line = String::new();
    let status = loop {
        let mut escape = false;
        while let Some(e) = input.poll_key() {
            line.clear();
            let _ = write_key(&mut line, &e);
            println!("{line}");
            keys += 1;
            if let Some(s) = scene.as_mut() {
                s.key(e.pressed);
            }
            escape |= e.code == KeyCode::Escape && e.pressed;
        }
        let mut moved = false;
        while let Some(m) = input.poll_mouse() {
            line.clear();
            let _ = write_mouse(&mut line, &m);
            println!("{line}");
            mice += 1;
            // The cursor is the sum of the deltas, clamped to the screen: what the Desk core does with a PS/2 report.
            x = (x + i32::from(m.dx)).clamp(0, width as i32 - 1);
            y = (y + i32::from(m.dy)).clamp(0, height as i32 - 1);
            moved = true;
            if let Some(s) = scene.as_mut() {
                if m.buttons != s.buttons {
                    s.buttons(m.buttons);
                }
            }
        }
        if let Some(s) = scene.as_mut() {
            // Redraw the cursor after anything drawn over it, and when it moved.
            if moved || !s.cursor.is_drawn() {
                s.cursor_to(x, y);
            }
        }
        if escape {
            break 0;
        }
        if timer::bios_ticks().wrapping_sub(start) > limit {
            println!("input: timed out after {timeout_s} s without Escape");
            break 4;
        }
    };

    println!(
        "input: {keys} key events, {mice} mouse events, cursor at ({x}, {y}), scancode bytes {} read, {} dropped, {} decoder recoveries, BIOS type-ahead {} keys",
        IrqStats::get(&STATS.kbd_bytes),
        input.keys_dropped(),
        input.recoveries(),
        bios_kbd::buffered()
    );
    if has("dump") {
        if let Some(s) = scene.as_mut() {
            dump(&display, &mut s.canvas);
        }
    }
    if let Some(m) = input.mouse() {
        if let Ok(p) = m.position() {
            println!("input: mouse: the driver's own position is ({}, {})", p.x, p.y);
        }
    }
    status
}
