// Spike A: VGA mode 13h and 12h and VBE 2.0 from Rust on the pure-Rust DOS runtime, through `desk-vga`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: VGAVBE.EXE [ports] [13] [bios13] [12] [bios12] [vbe] [dump] [corrupt] [panic] [leave]
//
// With no stage named it runs every stage in order. Exit status: 0 all checks held, 1 a checksum or read-back differed, 2 VBE was
// not available or refused (the screen is back in text mode first), 3 a panic (the runtime's).
//
// - `ports`   reads all 256 DAC entries through the ports (the first use of port I/O from ring 3), writes a test palette, reads it
//             back, restores the original
// - `13`      mode 13h by direct register programming, the picture, the checksum against the same picture in memory, a fade
// - `bios13`  mode 13h by INT 10h through `DosDisplay`, the same picture and checksum, and how the BIOS's registers compare
// - `12`      mode 12h by direct register programming, planar writes, read-back through the read map select
// - `bios12`  mode 12h by INT 10h, the same
// - `vbe`     the VBE information, the 640 x 480 x 8 mode with a linear frame buffer, mapped with DPMI 0800h, the picture, the checksum
// - `dump`    also writes each picture's pixels and palette to files (M13.RAW/.PAL, VBE.RAW/.PAL, M12.RAW/.PAL) for the PNG converter
// - `corrupt` flips one pixel after drawing, so the checksum must differ (the negative check)
// - `panic`   enters mode 13h and panics; the clean-up list must put text mode back
// - `leave`   enters mode 13h and exits; the clean-up list must put text mode back
#![no_std]
#![no_main]

extern crate alloc;

mod scene;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};
use desk_hal::x86::RawPorts;
use desk_hal::{Display, HalError, MmioRegion, Rgb6};
use desk_vga::regs::{self, MODE_12H, MODE_13H};
use desk_vga::vbe::Vbe;
use desk_vga::{checksum, dac, Canvas, Linear8, Planar16};
use rt::hal::{DosBios, DosDisplay};
use rt::{cleanup, dos, println, vga};
use scene::Depth;

rt::entry!(main);

/// Set when any check fails; becomes exit status 1.
static FAILED: AtomicBool = AtomicBool::new(false);

fn check(what: &str, ok: bool) {
    println!("avga: {what}: {ok}");
    if !ok {
        FAILED.store(true, Ordering::Relaxed);
    }
}

/// Registered first, so it runs last: says what the video mode is after the rest of the clean-up list has run.
fn report() {
    println!("avga: after the clean-up list the video mode is {:#04x}", vga::get_mode());
}

/// Back to text mode after a mode that was set by programming the registers. The BIOS never learned of that mode, so the video mode
/// it reports is still 3 and `vga::leave`, which only acts when the reported mode is not 3, would leave the screen in graphics.
fn leave_direct() {
    vga::set_mode(3);
    cleanup::forget(leave_direct);
}

/// Graphics controller register 6 in text mode: alphanumeric, 32 KiB at B8000h. Graphics modes have 05h.
const TEXT_GC_MISC: u8 = 0x0E;

fn window(linear: u32, len: usize) -> MmioRegion {
    // SAFETY: the caller passes video memory that is mapped for as long as the region is used (A0000h in the first megabyte).
    unsafe { MmioRegion::new(rt::dpmi::linear_to_ptr::<u8>(linear), len) }
}

fn ram(buf: &mut [u8]) -> MmioRegion {
    // SAFETY: the slice outlives the region and nothing else touches it while the region is in use.
    unsafe { MmioRegion::new(buf.as_mut_ptr(), buf.len()) }
}

/// The checksum of the picture drawn in memory: what the frame buffer must hold.
fn reference(w: u32, h: u32, depth: Depth) -> u32 {
    let mut buf = vec![0u8; (w * h) as usize];
    let mut region = ram(&mut buf);
    let mut c = Linear8::new(&mut region, w, h, w).expect("reference surface");
    scene::draw(&mut c, depth);
    checksum(&mut c).expect("reference checksum")
}

fn dump(name: &str, pixels: &[u8], palette: &[Rgb6]) {
    let mut raw = Vec::new();
    for c in palette {
        raw.extend_from_slice(&[c.r, c.g, c.b]);
    }
    let write = |ext: &str, bytes: &[u8]| {
        let mut path = String::from(name);
        path.push_str(ext);
        dos::File::open(&path, dos::Mode::Create)
            .and_then(|mut f| f.write_all(bytes))
            .expect("dump file");
    };
    write(".PAL", &raw);
    write(".RAW", pixels);
    println!("avga: wrote {name}.RAW ({} bytes) and {name}.PAL", pixels.len());
}

fn read_all<C: Canvas>(c: &mut C) -> Vec<u8> {
    let (w, h) = (c.width() as usize, c.height());
    let mut out = vec![0u8; w * h as usize];
    for y in 0..h {
        c.read_row(y, &mut out[y as usize * w..(y as usize + 1) * w])
            .expect("read row");
    }
    out
}

/// The registers of `got` that differ from `want`, as "group[index]=got/want", or "none".
fn diff(got: &regs::ModeRegs, want: &regs::ModeRegs) -> String {
    let mut out = String::new();
    let mut note = |group: &str, a: &[u8], b: &[u8]| {
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            if x != y {
                out.push_str(&alloc::format!(" {group}[{i:#x}]={x:#04x}/{y:#04x}"));
            }
        }
    };
    note("misc", &[got.misc], &[want.misc]);
    note("seq", &got.seq, &want.seq);
    note("crtc", &got.crtc, &want.crtc);
    note("gc", &got.gc, &want.gc);
    note("attr", &got.attr, &want.attr);
    if out.is_empty() {
        out.push_str(" none");
    }
    out
}

/// Stage 1: the DAC through the ports, before any mode is set.
fn stage_ports(ports: &RawPorts) {
    let mut original = [Rgb6::BLACK; 256];
    check(
        "read 256 DAC entries through 3C7/3C9",
        dac::read_palette(ports, 0, &mut original).is_ok(),
    );
    let mut sum = desk_vga::Adler32::new();
    for c in &original {
        sum.update(&[c.r, c.g, c.b]);
    }
    println!("avga: power-up palette checksum {:#010x}", sum.finish());
    let test = scene::palette256();
    check("wrote 256 entries through 3C8/3C9", dac::set_palette(ports, 0, &test).is_ok());
    let mut back = [Rgb6::BLACK; 256];
    dac::read_palette(ports, 0, &mut back).expect("read back");
    check("read back equals written", back == test);
    dac::set_palette(ports, 0, &original).expect("restore");
    dac::read_palette(ports, 0, &mut back).expect("read back");
    check("original palette restored", back == original);
    check(
        "a range past entry 255 is refused",
        dac::set_palette(ports, 250, &[Rgb6::BLACK; 7]) == Err(HalError::InvalidArgument),
    );
}

/// The fade: 16 frames to black, 16 back, ending exactly where it began.
fn fade_check(ports: &RawPorts, base: &[Rgb6; 256], label: &str) {
    dac::set_palette(ports, 0, base).expect("palette");
    let out = dac::fade_out(ports, base, dac::FADE_STEPS);
    let mut now = [Rgb6::BLACK; 256];
    dac::read_palette(ports, 0, &mut now).expect("read");
    check(
        &alloc::format!("{label}: fade out ends black"),
        now.iter().all(|c| *c == Rgb6::BLACK),
    );
    let inn = dac::fade_in(ports, base, dac::FADE_STEPS);
    dac::read_palette(ports, 0, &mut now).expect("read");
    check(&alloc::format!("{label}: fade in restores the palette"), now == *base);
    check(
        &alloc::format!("{label}: 32 frames, every one synchronised to retrace"),
        out.synced + inn.synced == 32 && out.timed_out + inn.timed_out == 0,
    );
}

/// Draws the picture on an 8-bit linear surface, reads it back and compares.
fn linear_picture(c: &mut Linear8, depth: Depth, label: &str, corrupt: bool) {
    scene::draw(c, depth);
    if corrupt {
        let v = c.get_pixel(0, 0).unwrap_or(0);
        c.pixel(0, 0, v ^ 0x55);
    }
    let sum = checksum(c).expect("checksum");
    let want = reference(c.width(), c.height(), depth);
    println!("avga: {label}: frame buffer checksum {sum:#010x}, picture in memory {want:#010x}");
    check(&alloc::format!("{label}: checksums match"), sum == want);
}

fn stage_13_direct(ports: &RawPorts, corrupt: bool, dump_files: bool) {
    println!("avga: mode 13h by registers, from video mode {:#04x}", vga::get_mode());
    cleanup::register(leave_direct);
    regs::set_mode_13h(ports);
    let got = regs::read_mode_registers(ports);
    check("registers read back equal the table", got == MODE_13H);
    let palette = scene::palette256();
    dac::set_palette(ports, 0, &palette).expect("palette");
    let mut region = window(0xA0000, 0x10000);
    let mut c = Linear8::new(&mut region, 320, 200, 320).expect("surface");
    c.clear(0);
    linear_picture(&mut c, Depth::Colours256, "13h direct", corrupt);
    if dump_files {
        dump("M13", &read_all(&mut c), &palette);
    }
    fade_check(ports, &palette, "13h direct");
    leave_direct();
    check(
        "13h direct: the text mode registers are back",
        regs::read_mode_registers(ports).gc[6] == TEXT_GC_MISC,
    );
    println!("avga: back in video mode {:#04x}", vga::get_mode());
}

fn stage_13_bios(display: &mut DosDisplay, corrupt: bool) {
    let mut fb = display.set_mode(rt::hal::MODE_13H).expect("mode 13h");
    check("13h by INT 10h: BIOS reports mode 13h", vga::get_mode() == 0x13);
    println!(
        "avga: 13h by INT 10h: frame buffer {} x {} pitch {} at {:#x}",
        fb.info.width, fb.info.height, fb.pitch, fb.phys
    );
    let got = regs::read_mode_registers(display.ports());
    println!(
        "avga: 13h by INT 10h: registers that differ from the table (BIOS/table):{}",
        diff(&got, &MODE_13H)
    );
    display.set_palette(0, &scene::palette256()).expect("palette");
    let mut c = Linear8::new(&mut fb.region, 320, 200, fb.pitch).expect("surface");
    linear_picture(&mut c, Depth::Colours256, "13h INT 10h", corrupt);
    display.restore_text_mode();
    println!("avga: back in video mode {:#04x}", vga::get_mode());
}

/// Draws the picture on mode 12h's planes and compares the read-back with the picture in memory.
fn planar_picture<P: desk_hal::PortIo>(ports: &P, region: &mut MmioRegion, label: &str, corrupt: bool, dump_files: bool) {
    let want = reference(640, 480, Depth::Colours16);
    let mut c = Planar16::new(ports, region, 640, 480, 80).expect("planar surface");
    scene::draw(&mut c, Depth::Colours16);
    if corrupt {
        let v = c.get_pixel(0, 0).unwrap_or(0);
        c.pixel(0, 0, v ^ 5);
    }
    let sum = checksum(&mut c).expect("checksum");
    println!("avga: {label}: planes read back checksum {sum:#010x}, picture in memory {want:#010x}");
    check(&alloc::format!("{label}: checksums match"), sum == want);
    check(
        &alloc::format!("{label}: pixel probes"),
        c.get_pixel(3, 3) == Some(10) && c.get_pixel(639, 479) == Some(0) && c.get_pixel(640, 0).is_none(),
    );
    if dump_files {
        dump("M12", &read_all(&mut c), &scene::DESK16);
    }
}

fn stage_12_direct(ports: &RawPorts, corrupt: bool, dump_files: bool) {
    println!("avga: mode 12h by registers, from video mode {:#04x}", vga::get_mode());
    cleanup::register(leave_direct);
    regs::set_mode_12h(ports);
    check(
        "registers read back equal the table",
        regs::read_mode_registers(ports) == MODE_12H,
    );
    dac::set_palette(ports, 0, &scene::DESK16).expect("palette");
    let mut region = window(0xA0000, 0x10000);
    // Clear all four planes first: direct programming leaves whatever text mode had in them.
    {
        let mut c = Planar16::new(ports, &mut region, 640, 480, 80).expect("planar surface");
        c.clear(0);
    }
    planar_picture(ports, &mut region, "12h direct", corrupt, dump_files);
    leave_direct();
    check(
        "12h direct: the text mode registers are back",
        regs::read_mode_registers(ports).gc[6] == TEXT_GC_MISC,
    );
    println!("avga: back in video mode {:#04x}", vga::get_mode());
}

fn stage_12_bios(display: &mut DosDisplay, corrupt: bool) {
    let mut fb = display.set_mode(rt::hal::MODE_12H).expect("mode 12h");
    check("12h by INT 10h: BIOS reports mode 12h", vga::get_mode() == 0x12);
    println!(
        "avga: 12h by INT 10h: {} x {} pitch {} per plane",
        fb.info.width, fb.info.height, fb.pitch
    );
    let got = regs::read_mode_registers(display.ports());
    println!(
        "avga: 12h by INT 10h: registers that differ from the table (BIOS/table):{}",
        diff(&got, &MODE_12H)
    );
    display.set_palette(0, &scene::DESK16).expect("palette");
    let ports = *display.ports();
    planar_picture(&ports, &mut fb.region, "12h INT 10h", corrupt, false);
    display.restore_text_mode();
    println!("avga: back in video mode {:#04x}", vga::get_mode());
}

/// Returns false (with the screen already back in text mode) when VBE is missing or refuses.
fn stage_vbe(display: &mut DosDisplay, corrupt: bool, dump_files: bool) -> bool {
    let vbe = Vbe::new(&DosBios);
    let info = match vbe.info() {
        Ok(i) => i,
        Err(e) => {
            println!("avga: vbe: no VBE information ({e}); not available");
            return false;
        }
    };
    let mut oem = [0u8; 64];
    let n = vbe.oem_string(&info, &mut oem).unwrap_or(0);
    println!(
        "avga: vbe: version {:x}.{:x}, {} KiB video memory, OEM \"{}\"",
        info.version >> 8,
        info.version & 0xFF,
        u32::from(info.memory_64k) * 64,
        core::str::from_utf8(&oem[..n]).unwrap_or("?")
    );
    let mut ids = [0u16; desk_vga::vbe::MAX_MODES];
    let count = vbe.mode_list(&info, &mut ids).unwrap_or(0);
    println!("avga: vbe: {count} modes listed");
    for id in &ids[..count] {
        if let Ok(m) = vbe.mode(*id) {
            if m.supported() && m.graphics() && m.bits_per_pixel <= 8 {
                println!(
                    "avga: vbe:   {:#05x} {}x{}x{} model {} {}",
                    m.id,
                    m.width,
                    m.height,
                    m.bits_per_pixel,
                    m.memory_model,
                    if m.linear() { "linear" } else { "banked" }
                );
            }
        }
    }
    let refused = vbe.set_mode(0x1FF, true);
    println!("avga: vbe: setting the unknown mode 0x1ff gives {refused:?}");
    check(
        "an unknown VBE mode is refused and text mode stays",
        refused.is_err() && vga::get_mode() == 0x03,
    );
    let Some(mode) = vbe.find(&ids[..count], 640, 480, 8, true) else {
        println!("avga: vbe: no 640x480x8 mode with a linear frame buffer");
        return false;
    };
    println!(
        "avga: vbe: using {:#05x}: pitch {}, frame buffer at physical {:#010x}, {} bytes",
        mode.id,
        mode.pitch,
        mode.phys_base,
        mode.frame_bytes()
    );
    let mut fb = match display.set_mode(mode.id) {
        Ok(fb) => fb,
        Err(e) => {
            println!("avga: vbe: set mode failed: {e}");
            check("a failed VBE mode set leaves text mode", vga::get_mode() == 0x03);
            return false;
        }
    };
    check(
        "vbe: BIOS reports the VBE mode with the linear bit",
        vbe.current_mode() == Ok(mode.id | 0x4000),
    );
    let palette = scene::palette256();
    display.set_palette(0, &palette).expect("palette");
    let mut c = Linear8::new(&mut fb.region, 640, 480, fb.pitch).expect("surface");
    c.clear(0);
    linear_picture(&mut c, Depth::Colours256, "vbe 640x480x8", corrupt);
    if dump_files {
        dump("VBE", &read_all(&mut c), &palette);
    }
    fade_check(display.ports(), &palette, "vbe 640x480x8");
    display.restore_text_mode();
    println!("avga: back in video mode {:#04x}", vga::get_mode());
    true
}

fn main() -> i32 {
    let args = rt::env::args();
    let has = |a: &str| args.iter().any(|x| x == a);
    let stages = ["ports", "13", "bios13", "12", "bios12", "vbe"];
    let all = !stages.iter().any(|s| has(s));
    let (corrupt, dump_files) = (has("corrupt"), has("dump"));
    cleanup::register(report);

    // SAFETY: a DPMI client under CWSDPMI may use the VGA ports; `stage_ports` is the first evidence, and the construction is the
    // platform's one unsafe step for port I/O (docs/HAL.md).
    let ports = unsafe { RawPorts::new() };

    if has("panic") || has("leave") {
        let mut display = DosDisplay::new();
        let _fb = display.set_mode(rt::hal::MODE_13H).expect("mode 13h");
        println!(
            "avga: in mode {:#04x}, now {}",
            vga::get_mode(),
            if has("panic") { "panicking" } else { "exiting" }
        );
        if has("panic") {
            panic!("panic in graphics mode");
        }
        return 0;
    }

    if all || has("ports") {
        stage_ports(&ports);
    }
    if all || has("13") {
        stage_13_direct(&ports, corrupt, dump_files);
    }
    let mut display = DosDisplay::new();
    println!(
        "avga: display offers {} modes, VBE {}",
        display.modes().len(),
        if display.has_vbe() { "present" } else { "absent" }
    );
    if all || has("bios13") {
        stage_13_bios(&mut display, corrupt);
    }
    if all || has("12") {
        stage_12_direct(&ports, corrupt, dump_files);
    }
    if all || has("bios12") {
        stage_12_bios(&mut display, corrupt);
    }
    let mut status = 0;
    if (all || has("vbe")) && !stage_vbe(&mut display, corrupt, dump_files) {
        status = 2;
    }
    if FAILED.load(Ordering::Relaxed) {
        println!("avga: FAILED");
        return 1;
    }
    if status == 0 {
        println!("avga: all checks held");
    }
    status
}
