// Graphics from the kernel: VGA modes by registers, a VBE linear frame buffer, a palette fade, a software cursor and a first window.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Everything that draws is the shared `desk-vga` driver (`Linear8`, `Planar16`, the DAC, the register tables). What is specific to
// the kernel is where the memory is (the VGA window at A0000h, or the linear frame buffer the loader set up and `video.rs` checked
// against the memory map), the pacing, and the way back to text mode. `docs/D-NATIVE-VGA.md` describes the tour this file runs
// when the loader image carries the `gfx` test mode: it prints a `gfx: ready NAME` line at each stable picture and waits for a
// key, so the QEMU run script can take a screenshot there (`tools/native-test.sh`).

use crate::bios;
use crate::hal::PORTS;
use crate::scene::{self, Depth};
use crate::video::{Lfb, Video, FONT_BYTES};
use crate::{input, pit, println, vga::Vga};
use alloc::string::String;
use alloc::vec;
use core::arch::asm;
use desk_hal::{InputSource, KeyCode, MmioRegion, PortIo, Rgb6};
use desk_i8042::Ps2Input;
use desk_vga::dac::{self, ENTRIES};
use desk_vga::{checksum, regs, text, Canvas, Linear8, Planar16};

/// Checksums of the same pictures drawn by spike A's program in DOSBox (`spikes/a-vga-vbe/expected/all.txt`): the kernel must
/// produce the same bytes from the same driver and the same scene.
const HOSTED_13H: u32 = 0x5c0f_1f16;
const HOSTED_12H: u32 = 0xfaf8_b758;
pub(crate) const HOSTED_VBE_640X480: u32 = 0x5bc6_5936;

/// A tick is 10 ms (`pit::HZ`).
const SECOND: u32 = pit::HZ;
const ACK_TIMEOUT: u32 = 20 * SECOND;
const INTERACTIVE_TIMEOUT: u32 = 60 * SECOND;

fn vga_window() -> MmioRegion {
    // SAFETY: A0000h to AFFFFh is the VGA's graphics window, ordinary physical memory in the first megabyte that the kernel owns
    // (the frame allocator never hands it out) and that the flat segments make addressable; nothing else holds a reference to it.
    unsafe { MmioRegion::new(0xA0000 as *mut u8, 0x10000) }
}

pub(crate) fn lfb_region(l: &Lfb) -> MmioRegion {
    // SAFETY: `video::init` checked that the frame buffer overlaps no RAM, the first megabyte or the kernel and reserved its
    // frames; the loader's mode set maps `pitch * height` bytes of it, and with no paging the physical address is the pointer.
    unsafe { MmioRegion::new(l.phys as *mut u8, l.bytes() as usize) }
}

// ---- Drawing the same picture in memory, for the reference checksum

pub(crate) fn reference(w: u32, h: u32, depth: Depth) -> u32 {
    let mut buf = vec![0u8; (w * h) as usize];
    // SAFETY: the Vec outlives the region and nothing else touches it while the region is in use.
    let mut region = unsafe { MmioRegion::new(buf.as_mut_ptr(), buf.len()) };
    let mut c = Linear8::new(&mut region, w, h, w).expect("reference surface");
    scene::draw(&mut c, depth);
    checksum(&mut c).expect("reference checksum")
}

/// Draws the test picture, reads the surface back and compares it with the picture drawn in memory and, where there is one, with
/// spike A's checksum from DOSBox. A difference is a failed check, not a message.
fn show_picture<C: Canvas>(c: &mut C, depth: Depth, label: &str, hosted: Option<u32>) {
    scene::draw(c, depth);
    let sum = checksum(c).expect("checksum");
    let want = reference(c.width(), c.height(), depth);
    println!("gfx: {}: frame buffer checksum {:#010x}, picture in memory {:#010x}", label, sum, want);
    assert_eq!(sum, want, "{}: the frame buffer does not hold the picture", label);
    if let Some(h) = hosted {
        println!("gfx: {}: spike A's DOSBox checksum {:#010x}, equal: {}", label, h, sum == h);
        assert_eq!(sum, h, "{}: differs from the hosted run", label);
    }
}

// ---- Modes

/// Bochs/QEMU's VBE_DISPI registers: an index port and a data port. Writing 0 to the enable register (index 4) turns the VBE mode
/// off so the ordinary VGA registers take effect again. Real VBE cards have their own extended registers for the same job; a card
/// without these is left through the BIOS (`bios.rs`), which knows its own chip.
const DISPI_INDEX: u16 = 0x1CE;
const DISPI_DATA: u16 = 0x1CF;

fn leave_vbe() {
    PORTS.outw(DISPI_INDEX, 0);
    let id = PORTS.inw(DISPI_DATA);
    if (0xB0C0..=0xB0CF).contains(&id) {
        PORTS.outw(DISPI_INDEX, 4);
        PORTS.outw(DISPI_DATA, 0);
        println!("gfx: leaving VBE: Bochs VBE registers (id {:#06x}) switched off", id);
        return;
    }
    // Not a Bochs card (QEMU's Cirrus, a real S3, Trident or ET4000): the register programming of the text and graphics modes does
    // not undo its extended registers, so the BIOS does it, by setting text mode 3 through the real-mode trampoline.
    println!("gfx: leaving VBE: no Bochs VBE registers (read {:#06x}), asking the BIOS: INT 10h AX=0003h", id);
    let mut regs = bios::RealModeRegs::with_eax(0x0003);
    match bios::call(0x10, &mut regs) {
        Ok(()) => println!("gfx: leaving VBE: the BIOS set text mode 3"),
        Err(e) => println!("gfx: leaving VBE: the BIOS call failed: {}", e),
    }
}

pub(crate) fn palette_of(colours: &[Rgb6]) -> [Rgb6; ENTRIES] {
    let mut p = [Rgb6::BLACK; ENTRIES];
    p[..colours.len()].copy_from_slice(colours);
    p
}

/// The Afterglow cursor colours in the reserved slots 240 and 241 (docs/PALETTE.md): the void for the outline, the text colour for
/// the fill.
fn cursor_slots(p: &mut [Rgb6; ENTRIES]) {
    p[240] = scene::DESK16[0];
    p[241] = scene::DESK16[7];
}

// ---- Fades and speed

/// Ticks between two steps of the paced fade: 20 ms, so 16 steps take about a third of a second.
const FADE_STEP_TICKS: u32 = 2;

/// Waits until `ticks` PIT ticks have passed since `start`, sleeping between interrupts.
fn wait_ticks(start: u32, ticks: u32) {
    while pit::ticks().wrapping_sub(start) < ticks {
        // SAFETY: interrupts are on (the timer demo enabled them), so `hlt` returns on the next tick.
        unsafe { asm!("hlt") };
    }
}

/// The Desk's fade (docs/PALETTE.md): 16 steps through the DAC, never through the frame buffer. Each step is `FADE_STEP_TICKS`
/// of the kernel's timer, so the pacing does not depend on the retrace bit, which an emulator may fake. `pause` names a rendezvous
/// at the half-way step (the run script takes a screenshot of the half-dimmed picture there).
fn fade_paced(ps2: &mut Ps2Input, base: &[Rgb6; ENTRIES], out: bool, pause: Option<&str>) {
    let mut start = pit::ticks();
    for i in 1..=dac::FADE_STEPS {
        wait_ticks(start, i * FADE_STEP_TICKS);
        let num = if out { dac::FADE_STEPS - i } else { i };
        dac::set_palette_scaled(&PORTS, base, num, dac::FADE_STEPS);
        if i == dac::FADE_STEPS / 2 {
            if let Some(name) = pause {
                rendezvous(ps2, name);
                // The pause is not part of the fade: the remaining steps keep their spacing.
                start = pit::ticks().wrapping_sub(i * FADE_STEP_TICKS);
            }
        }
    }
}

/// A fade out and back in, checked by reading the palette back, then the driver's own retrace-synchronised fade for comparison.
fn fade_demo(ps2: &mut Ps2Input, label: &str, base: &[Rgb6; ENTRIES], pause: Option<&str>) {
    let mut now = [Rgb6::BLACK; ENTRIES];
    let t0 = pit::ticks();
    fade_paced(ps2, base, true, pause);
    let t1 = pit::ticks();
    dac::read_palette(&PORTS, 0, &mut now).expect("read the palette");
    let black = now.iter().all(|c| *c == Rgb6::BLACK);
    fade_paced(ps2, base, false, None);
    let t2 = pit::ticks();
    dac::read_palette(&PORTS, 0, &mut now).expect("read the palette");
    let restored = now == *base;
    println!("gfx: {}: paced fade out, {} steps of {} ticks, took {} ticks (including any screenshot pause), ends black: {}", label, dac::FADE_STEPS, FADE_STEP_TICKS, t1 - t0, black);
    println!("gfx: {}: paced fade in, {} steps, took {} ticks, palette restored: {}", label, dac::FADE_STEPS, t2 - t1, restored);
    assert!(black && restored, "{}: the fade did not end where it should", label);
    // The driver's fade waits for vertical retrace before each step. A real VGA takes 16 frames (about 230 ms at 70 Hz) for it;
    // QEMU's VGA toggles the retrace bit on every read, so it "synchronises" at once and the fade takes no time at all.
    let t3 = pit::ticks();
    let out = dac::fade_out(&PORTS, base, dac::FADE_STEPS);
    let inn = dac::fade_in(&PORTS, base, dac::FADE_STEPS);
    let t4 = pit::ticks();
    println!(
        "gfx: {}: the driver's retrace-synchronised fade took {} ticks for {} + {} frames ({} + {} synchronised)",
        label,
        t4 - t3,
        dac::FADE_STEPS,
        dac::FADE_STEPS,
        out.synced,
        inn.synced
    );
}

/// Runs `f` until a second of ticks has passed (at least once) and returns how many times it ran and the ticks it took.
fn for_a_second(mut f: impl FnMut(u32)) -> (u32, u32) {
    let start = pit::ticks();
    let mut n = 0;
    loop {
        f(n);
        n += 1;
        let elapsed = pit::ticks().wrapping_sub(start);
        if elapsed >= SECOND {
            return (n, elapsed);
        }
    }
}

fn rate(n: u32, ticks: u32) -> (u32, u32) {
    let x10 = u64::from(n) * u64::from(SECOND) * 10 / u64::from(ticks.max(1));
    ((x10 / 10) as u32, (x10 % 10) as u32)
}

fn perf<C: Canvas>(c: &mut C, label: &str, cursor_colours: (u8, u8)) {
    let (w, h) = (c.width(), c.height());
    let (n, t) = for_a_second(|i| c.fill_rect(0, 0, w, h, (i % 15 + 1) as u8));
    let (a, b) = rate(n, t);
    println!("gfx: perf: {}: {} full-screen fills in {} ticks, {}.{} per second (emulated speed, not real hardware)", label, n, t, a, b);
    let mut cursor = Cursor::new(cursor_colours);
    cursor.show(c);
    let (n, t) = for_a_second(|i| {
        cursor.hide(c);
        cursor.x = (i * 3 % (w - 20)) as i32;
        cursor.y = (i * 2 % (h - 30)) as i32;
        cursor.show(c);
    });
    let (a, b) = rate(n, t);
    println!("gfx: perf: {}: {} cursor moves in {} ticks, {}.{} per second (emulated speed, not real hardware)", label, n, t, a, b);
}

/// The full-screen fill as one `pixel` call per pixel, the slowest way to use the driver and what a fill cost before `fill8`.
fn perf_pixels<C: Canvas>(c: &mut C, label: &str) {
    let (w, h) = (c.width() as i32, c.height() as i32);
    let (n, t) = for_a_second(|i| {
        for y in 0..h {
            for x in 0..w {
                c.pixel(x, y, (i % 15 + 1) as u8);
            }
        }
    });
    let (a, b) = rate(n, t);
    println!("gfx: perf: {}: {} full-screen fills, one `pixel` call per pixel, in {} ticks, {}.{} per second (emulated speed, not real hardware)", label, n, t, a, b);
}

/// The same full-screen fill with `write_bytes`, which the compiler turns into the library's memset: the obvious next step beyond
/// one volatile store per pixel. Only for the speed comparison; it writes the frame buffer's memory without the region's checks.
fn perf_raw_fill(l: &Lfb) {
    let base = l.phys as *mut u8;
    let bytes = l.bytes() as usize;
    let (n, t) = for_a_second(|i| {
        // SAFETY: `lfb_region` explains why `bytes` bytes at the physical address are the frame buffer; no region is in use here.
        unsafe { core::ptr::write_bytes(base, (i % 15 + 1) as u8, bytes) };
    });
    let (a, b) = rate(n, t);
    println!("gfx: perf: vbe memset fill: {} full-screen fills in {} ticks, {}.{} per second (emulated speed, not real hardware)", n, t, a, b);
}

// ---- The cursor

const CURSOR_W: usize = 12;
const CURSOR_H: usize = 19;
/// `X` is the outline, `.` the fill, a space is transparent.
const ARROW: [&[u8; CURSOR_W]; CURSOR_H] = [
    b"X           ",
    b"XX          ",
    b"X.X         ",
    b"X..X        ",
    b"X...X       ",
    b"X....X      ",
    b"X.....X     ",
    b"X......X    ",
    b"X.......X   ",
    b"X........X  ",
    b"X.....XXXXX ",
    b"X..X..X     ",
    b"X.X X..X    ",
    b"XX  X..X    ",
    b"X    X..X   ",
    b"     X..X   ",
    b"      X..X  ",
    b"      X..X  ",
    b"       XX   ",
];

/// A software cursor: the arrow is drawn over the picture after the pixels under it are saved, and the saved pixels are put back
/// before it moves. A VGA has no hardware cursor, and the Desk draws its own (docs/SPEC.md).
struct Cursor {
    x: i32,
    y: i32,
    shown: bool,
    saved: [u8; CURSOR_W * CURSOR_H],
    outline: u8,
    fill: u8,
}

impl Cursor {
    const fn new(colours: (u8, u8)) -> Cursor {
        Cursor { x: 0, y: 0, shown: false, saved: [0; CURSOR_W * CURSOR_H], outline: colours.0, fill: colours.1 }
    }

    /// Saves what is under the arrow and draws it. Does nothing if it is already shown.
    fn show<C: Canvas>(&mut self, c: &mut C) {
        if self.shown {
            return;
        }
        for (row, line) in ARROW.iter().enumerate() {
            for (col, px) in line.iter().enumerate() {
                if *px == b' ' {
                    continue;
                }
                let (x, y) = (self.x + col as i32, self.y + row as i32);
                self.saved[row * CURSOR_W + col] = c.get_pixel(x, y).unwrap_or(0);
                c.pixel(x, y, if *px == b'X' { self.outline } else { self.fill });
            }
        }
        self.shown = true;
    }

    /// Puts the saved pixels back.
    fn hide<C: Canvas>(&mut self, c: &mut C) {
        if !self.shown {
            return;
        }
        for (row, line) in ARROW.iter().enumerate() {
            for (col, px) in line.iter().enumerate() {
                if *px != b' ' {
                    c.pixel(self.x + col as i32, self.y + row as i32, self.saved[row * CURSOR_W + col]);
                }
            }
        }
        self.shown = false;
    }
}

// ---- The desktop and the first window

struct Looks {
    bands: &'static [u8],
    cursor: (u8, u8),
}

impl Looks {
    const CURSOR_256: (u8, u8) = (240, 241);
    const COLOURS_256: Looks = Looks { bands: &[16, 17, 18, 19, 20, 21, 22], cursor: Looks::CURSOR_256 };
    /// Slots 10, 9 and 8 are the horizon's three bands in 16 colours; the cursor uses the void and the text colour.
    const COLOURS_16: Looks = Looks { bands: &[10, 9, 8], cursor: (0, 7) };
}

// Slots of the 16-colour set that are the same in both palettes (docs/PALETTE.md).
const VOID: u8 = 0;
const CHROME: u8 = 1;
const CONTENT: u8 = 2;
const LINE: u8 = 4;
const MUTED: u8 = 5;
const TEXT: u8 = 7;
const ACCENT: u8 = 8;

/// One 8 × 16 glyph from the BIOS font, pixel by pixel.
fn glyph<C: Canvas>(c: &mut C, font: &[u8; FONT_BYTES], x: i32, y: i32, ch: u8, colour: u8) {
    for row in 0..16 {
        let bits = font[ch as usize * 16 + row];
        for col in 0..8 {
            if bits & (0x80 >> col) != 0 {
                c.pixel(x + col, y + row as i32, colour);
            }
        }
    }
}

fn label<C: Canvas>(c: &mut C, font: Option<&[u8; FONT_BYTES]>, x: i32, y: i32, s: &str, colour: u8) {
    let Some(font) = font else { return };
    for (i, b) in s.bytes().enumerate() {
        glyph(c, font, x + i as i32 * 8, y, if (0x20..0x7F).contains(&b) { b } else { b'?' }, colour);
    }
}

/// The window's geometry, as fractions of the screen so it fits 640 × 480 and any other size.
struct Window {
    x: i32,
    y: i32,
    w: u32,
    h: u32,
}

const TITLE_H: u32 = 22;

impl Window {
    fn new(sw: u32, sh: u32) -> Window {
        Window { x: (sw * 3 / 20) as i32, y: (sh / 6) as i32, w: sw * 7 / 10, h: sh * 11 / 20 }
    }

    fn body_y(&self) -> i32 {
        self.y + TITLE_H as i32
    }
}

fn draw_desktop<C: Canvas>(c: &mut C, looks: &Looks, win: &Window, font: Option<&[u8; FONT_BYTES]>) {
    let (w, h) = (c.width(), c.height());
    c.clear(VOID);
    // The horizon: equal bands of one slot each, never dithered.
    let top = h * 45 / 100;
    let n = looks.bands.len() as u32;
    for (i, slot) in looks.bands.iter().enumerate() {
        let (y0, y1) = (top * i as u32 / n, top * (i as u32 + 1) / n);
        c.fill_rect(0, y0 as i32, w, y1 - y0, *slot);
    }
    // The window: a 1-pixel border, an Afterglow orange title bar and a plain body.
    c.fill_rect(win.x, win.y, win.w, TITLE_H, ACCENT);
    c.fill_rect(win.x, win.body_y(), win.w, win.h - TITLE_H, CONTENT);
    c.rect(win.x, win.y, win.w, win.h, LINE);
    c.hline(win.x, win.body_y() - 1, win.w, LINE);
    label(c, font, win.x + 10, win.y + 3, "Desk: first window", VOID);
    c.rect(win.x + win.w as i32 - 20, win.y + 5, 12, 12, VOID);
    label(c, font, win.x + 12, win.body_y() + 12, "Move the mouse. Type here. Esc leaves.", MUTED);
    draw_echo(c, font, win, "");
}

/// The typed text and a caret, on a cleared line of the body.
fn draw_echo<C: Canvas>(c: &mut C, font: Option<&[u8; FONT_BYTES]>, win: &Window, typed: &str) {
    let (x, y) = (win.x + 12, win.body_y() + 52);
    c.fill_rect(win.x + 1, y - 4, win.w - 2, 24, CHROME);
    label(c, font, x, y, "> ", MUTED);
    label(c, font, x + 16, y, typed, TEXT);
    c.fill_rect(x + 16 + typed.len() as i32 * 8, y, 8, 16, ACCENT);
}

// ---- Waiting for input

/// Prints the marker the run script waits for, then waits for a key press (any key) as the acknowledgement that the screenshot is
/// taken. Mouse events that arrive meanwhile are dropped.
fn rendezvous(ps2: &mut Ps2Input, name: &str) {
    println!("gfx: ready {}", name);
    let start = pit::ticks();
    loop {
        while ps2.poll_mouse().is_some() {}
        while let Some(k) = ps2.poll_key() {
            if k.pressed {
                return;
            }
        }
        if pit::ticks().wrapping_sub(start) > ACK_TIMEOUT {
            panic!("gfx: no key to acknowledge {} within {} ticks", name, ACK_TIMEOUT);
        }
        input::idle(ps2);
    }
}

/// The window with the cursor on `c`: draws the desktop, then follows the mouse and echoes the keys until Escape is released.
fn window_session<C: Canvas>(c: &mut C, ps2: &mut Ps2Input, looks: &Looks, font: Option<&[u8; FONT_BYTES]>) {
    let (w, h) = (c.width() as i32, c.height() as i32);
    let win = Window::new(c.width(), c.height());
    draw_desktop(c, looks, &win, font);
    let mut cursor = Cursor::new(looks.cursor);
    (cursor.x, cursor.y) = (w / 2, h / 2);
    cursor.show(c);
    println!("gfx: cursor {},{}", cursor.x, cursor.y);
    let mut typed = String::new();
    let start = pit::ticks();
    loop {
        let (mut moved, mut redraw, mut done) = (false, false, false);
        let mut buttons = 0;
        let (mut nx, mut ny) = (cursor.x, cursor.y);
        while let Some(m) = ps2.poll_mouse() {
            nx = (nx + i32::from(m.dx)).clamp(0, w - 1);
            ny = (ny + i32::from(m.dy)).clamp(0, h - 1);
            buttons = m.buttons;
            moved = true;
        }
        while let Some(k) = ps2.poll_key() {
            input::print_key(k);
            if k.pressed {
                // Backspace has a character of its own (`\u{8}`), so it is tested before `char_for` is asked.
                if k.code == KeyCode::Backspace {
                    if typed.pop().is_some() {
                        redraw = true;
                    }
                } else if let Some(ch) = desk_kbd::char_for(k.code, k.modifiers) {
                    if (' '..='~').contains(&ch) && typed.len() < 40 {
                        typed.push(ch);
                        redraw = true;
                    }
                }
            } else if k.code == KeyCode::Escape {
                done = true;
            }
        }
        if moved || redraw {
            cursor.hide(c);
            if redraw {
                draw_echo(c, font, &win, &typed);
            }
            (cursor.x, cursor.y) = (nx, ny);
            cursor.show(c);
            if moved {
                println!("gfx: cursor {},{} buttons {}", cursor.x, cursor.y, buttons);
            }
            if redraw {
                println!("gfx: echo \"{}\"", typed);
            }
        }
        if done {
            println!("gfx: window closed");
            return;
        }
        if pit::ticks().wrapping_sub(start) > INTERACTIVE_TIMEOUT {
            panic!("gfx: no Escape within {} ticks", INTERACTIVE_TIMEOUT);
        }
        input::idle(ps2);
    }
}

// ---- Back to text

/// Leaves graphics for 80 × 25 text without the BIOS: the text registers, the 16 text colours, the font in plane 2 (the loader kept
/// a copy of the BIOS's), and a cleared screen. Without the font the text would be garbage, so it is then left alone and reported.
fn restore_text(font: Option<&[u8; FONT_BYTES]>) {
    regs::set_mode_03h(&PORTS);
    text::set_text_palette(&PORTS).expect("the text palette");
    match font {
        Some(f) => {
            text::load_font(&PORTS, &mut vga_window(), f).expect("the font");
            Vga::clear();
            println!("gfx: text mode restored: registers, palette and the 8x16 font reloaded");
        }
        None => println!("gfx: text mode registers restored, but the loader gave no font, so the characters are not readable"),
    }
}

// ---- The tour

/// The 640 × 480 × 8 linear frame buffer the loader set: the picture, the fade, the speed, the window with the cursor.
fn tour_vbe(l: &Lfb, ps2: &mut Ps2Input, font: Option<&[u8; FONT_BYTES]>) {
    println!("gfx: VBE mode {:#06x}, {}x{}, pitch {}, frame buffer at {:#x}", l.mode, l.width, l.height, l.pitch, l.phys);
    let mut region = lfb_region(l);
    let mut c = Linear8::new(&mut region, l.width, l.height, l.pitch).expect("the frame buffer surface");
    let mut palette = palette_of(&scene::palette256());
    cursor_slots(&mut palette);
    dac::set_palette(&PORTS, 0, &palette).expect("palette");
    c.clear(0);
    let hosted = (l.width, l.height) == (640, 480);
    show_picture(&mut c, Depth::Colours256, "vbe picture", hosted.then_some(HOSTED_VBE_640X480));
    rendezvous(ps2, "vbe-picture");
    fade_demo(ps2, "vbe", &palette, Some("vbe-fade-half"));
    perf(&mut c, "vbe", Looks::COLOURS_256.cursor);
    perf_pixels(&mut c, "vbe");
    perf_raw_fill(l);
    window_session(&mut c, ps2, &Looks::COLOURS_256, font);
}

fn tour_13h(ps2: &mut Ps2Input) {
    regs::set_mode_13h(&PORTS);
    let readback = regs::read_mode_registers(&PORTS);
    println!("gfx: mode 13h by registers, no BIOS: registers read back equal the table: {}", readback == regs::MODE_13H);
    assert!(readback == regs::MODE_13H);
    let palette = palette_of(&scene::palette256());
    dac::set_palette(&PORTS, 0, &palette).expect("palette");
    let mut region = vga_window();
    let mut c = Linear8::new(&mut region, 320, 200, 320).expect("mode 13h surface");
    c.clear(0);
    show_picture(&mut c, Depth::Colours256, "13h", Some(HOSTED_13H));
    rendezvous(ps2, "mode-13h");
    fade_demo(ps2, "13h", &palette, None);
}

fn tour_12h(ps2: &mut Ps2Input, font: Option<&[u8; FONT_BYTES]>) {
    regs::set_mode_12h(&PORTS);
    let readback = regs::read_mode_registers(&PORTS);
    println!("gfx: mode 12h by registers, no BIOS: registers read back equal the table: {}", readback == regs::MODE_12H);
    assert!(readback == regs::MODE_12H);
    let palette = palette_of(&scene::DESK16);
    dac::set_palette(&PORTS, 0, &palette).expect("palette");
    let mut region = vga_window();
    let mut c = Planar16::new(&PORTS, &mut region, 640, 480, desk_vga::planar::PITCH_640).expect("mode 12h surface");
    // Direct programming leaves whatever the previous mode had in the planes.
    c.clear(0);
    show_picture(&mut c, Depth::Colours16, "12h", Some(HOSTED_12H));
    rendezvous(ps2, "mode-12h");
    fade_demo(ps2, "12h", &palette, None);
    perf(&mut c, "12h, planar", Looks::COLOURS_16.cursor);
    window_session(&mut c, ps2, &Looks::COLOURS_16, font);
}

/// The whole tour. With a VBE frame buffer from the loader: the picture, the fade, the speed and the window in VBE, then leave VBE
/// and show mode 13h. Without one (the loader did not try, or VBE failed): modes 13h and 12h by registers, and the window in 12h.
/// Either way it ends by restoring text mode.
pub fn tour(video: &Video, mut ps2: Ps2Input) {
    let font = video.font;
    match &video.lfb {
        Some(l) => {
            tour_vbe(l, &mut ps2, font);
            leave_vbe();
        }
        None => println!("gfx: no VBE frame buffer from the loader, using the VGA register modes"),
    }
    tour_13h(&mut ps2);
    if video.lfb.is_none() {
        tour_12h(&mut ps2, font);
    }
    restore_text(font);
    rendezvous(&mut ps2, "text");
    println!("gfx: tour complete");
}
