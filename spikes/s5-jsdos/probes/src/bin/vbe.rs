// A VBE 2.0 probe: reads the controller information and the mode list, sets a 640x480x8 mode with the linear frame buffer (or, with
// the argument `banked`, through the bank window), maps the buffer with DPMI 0800h, draws a pattern, reads it back and holds the
// picture for a key so a harness can take a screenshot.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Arguments: `nowait` to skip the key wait at the end, an optional hexadecimal mode number to try first (default: the first 640x480x8 mode the controller lists), and the
// word `banked` to use the bank window at A0000h instead of the linear frame buffer. VBE.TXT holds the information and the mode
// list; VBEGFX.TXT is written once the picture is drawn (or the attempt has failed) and is what a harness waits for.
#![no_std]
#![no_main]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use probes::{adler, load_palette, map_physical, say, wait_key, Report};
use rt::dpmi::{self, RealRegs};
use rt::{env, vga};

rt::entry!(main);

const W: usize = 640;
const H: usize = 480;

/// The test pattern: coloured bars over a diagonal XOR texture.
fn pattern(x: usize, y: usize) -> u8 {
    if y < H / 2 {
        (x / 40 * 16 + y / 15) as u8
    } else {
        (x ^ y) as u8
    }
}

/// The DOS-memory block the VBE structures live in: a real-mode segment and the linear address of its first byte.
struct Buf {
    seg: u16,
    lin: u32,
}

impl Buf {
    fn u8(&self, off: u32) -> u8 {
        peek8(self.lin + off)
    }
    fn u16(&self, off: u32) -> u16 {
        self.u8(off) as u16 | (self.u8(off + 1) as u16) << 8
    }
    fn u32(&self, off: u32) -> u32 {
        self.u16(off) as u32 | (self.u16(off + 2) as u32) << 16
    }
    fn clear(&self) {
        for i in 0..1024 {
            // SAFETY: the 1024-byte block was allocated from DOS memory and is reachable below 1 MiB.
            unsafe { dpmi::linear_to_ptr::<u8>(self.lin + i).write_volatile(0) };
        }
    }
    fn poke(&self, off: u32, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            // SAFETY: as `clear`; callers stay inside the block.
            unsafe { dpmi::linear_to_ptr::<u8>(self.lin + off + i as u32).write_volatile(*b) };
        }
    }
}

fn peek8(linear: u32) -> u8 {
    // SAFETY: linear addresses below 1 MiB (DOS memory, the BIOS) are mapped at the same address by the host and reachable
    // through the 4 GiB data segment; reads have no side effects.
    unsafe { dpmi::linear_to_ptr::<u8>(linear).read_volatile() }
}

fn far_to_linear(p: u32) -> u32 {
    ((p >> 16) << 4) + (p & 0xFFFF)
}

fn cstr(linear: u32) -> String {
    let mut s = String::new();
    for i in 0..80 {
        let c = peek8(linear + i);
        if c == 0 {
            break;
        }
        s.push(if (32..127).contains(&c) { c as char } else { '?' });
    }
    s
}

/// INT 10h through the host's real-mode call; returns AX.
fn int10(r: &mut RealRegs) -> u16 {
    dpmi::simulate_int(0x10, r).expect("DPMI 0300h");
    r.eax as u16
}

struct Mode {
    number: u16,
    attrs: u16,
    pitch: u16,
    w: u16,
    h: u16,
    bpp: u8,
    model: u8,
    win_a: u8,
    gran: u16,
    win_size: u16,
    seg_a: u16,
    phys: u32,
}

fn mode_info(b: &Buf, number: u16) -> Option<Mode> {
    b.clear();
    let mut r = RealRegs {
        eax: 0x4F01,
        ecx: number as u32,
        es: b.seg,
        edi: 0,
        ..RealRegs::default()
    };
    if int10(&mut r) != 0x004F {
        return None;
    }
    Some(Mode {
        number,
        attrs: b.u16(0),
        win_a: b.u8(2),
        gran: b.u16(4),
        win_size: b.u16(6),
        seg_a: b.u16(8),
        pitch: b.u16(16),
        w: b.u16(18),
        h: b.u16(20),
        bpp: b.u8(25),
        model: b.u8(27),
        phys: b.u32(40),
    })
}

fn finish(rep: &Report, code: i32) -> i32 {
    rep.save("VBEGFX.TXT");
    code
}

fn main() -> i32 {
    let args = env::args();
    let banked = args.iter().any(|a| a == "banked");
    let forced = args.iter().find_map(|a| u16::from_str_radix(a, 16).ok());
    let mut rep = Report::default();
    say!(
        rep,
        "S5-VBE start (access: {})",
        if banked { "banked" } else { "linear frame buffer" }
    );

    let (seg, sel) = dpmi::alloc_dos_memory(64).expect("DOS memory");
    let _ = sel;
    let b = Buf {
        seg,
        lin: (seg as u32) << 4,
    };

    // 4F00h with "VBE2" asks for the VBE 2.0 block, 512 bytes.
    b.clear();
    b.poke(0, b"VBE2");
    let mut r = RealRegs {
        eax: 0x4F00,
        es: b.seg,
        edi: 0,
        ..RealRegs::default()
    };
    let ax = int10(&mut r);
    say!(rep, "info: AX={:#06x} (0x004f is supported and successful)", ax);
    if ax != 0x004F {
        say!(rep, "result: no VBE");
        return finish(&rep, 2);
    }
    let sig = [b.u8(0), b.u8(1), b.u8(2), b.u8(3)];
    let version = b.u16(4);
    let oem = cstr(far_to_linear(b.u32(6)));
    let caps = b.u32(10);
    let list = far_to_linear(b.u32(14));
    let mem64k = b.u16(18);
    say!(
        rep,
        "info: signature {:?} version {}.{} oem \"{}\"",
        core::str::from_utf8(&sig).unwrap_or("?"),
        version >> 8,
        version & 255,
        oem
    );
    say!(
        rep,
        "info: capabilities {:#010x}, video memory {} KiB, mode list at {:#07x}",
        caps,
        mem64k as u32 * 64,
        list
    );
    if version >= 0x0200 {
        say!(
            rep,
            "info: software revision {:#06x}, vendor \"{}\", product \"{}\"",
            b.u16(20),
            cstr(far_to_linear(b.u32(22))),
            cstr(far_to_linear(b.u32(26)))
        );
    }

    // The mode list, copied out before the buffer is reused.
    let mut numbers: Vec<u16> = Vec::new();
    for i in 0..256u32 {
        let n = peek8(list + i * 2) as u16 | (peek8(list + i * 2 + 1) as u16) << 8;
        if n == 0xFFFF {
            break;
        }
        numbers.push(n);
    }
    say!(rep, "modes: {} listed", numbers.len());
    let mut chosen: Option<Mode> = None;
    for &n in &numbers {
        let Some(m) = mode_info(&b, n) else {
            say!(rep, "mode {:#05x}: 4F01h failed", n);
            continue;
        };
        say!(
            rep,
            "mode {:#05x}: {}x{}x{} model {} attrs {:#06x} lfb {} pitch {} phys {:#010x}",
            m.number,
            m.w,
            m.h,
            m.bpp,
            m.model,
            m.attrs,
            (m.attrs & 0x80) != 0,
            m.pitch,
            m.phys
        );
        let fits = m.w as usize == W && m.h as usize == H && m.bpp == 8 && m.model == 4 && (m.attrs & 0x01) != 0;
        let usable = if banked { true } else { (m.attrs & 0x80) != 0 };
        if fits && usable && (forced == Some(n) || (forced.is_none() && chosen.is_none())) {
            chosen = Some(m);
        }
    }
    rep.save("VBE.TXT");

    let Some(m) = chosen else {
        say!(
            rep,
            "result: no 640x480x8 packed-pixel mode with {} offered",
            if banked { "support" } else { "a linear frame buffer" }
        );
        return finish(&rep, 3);
    };

    // 4F02h: set the mode, bit 14 asks for the linear frame buffer.
    let arg = m.number | if banked { 0 } else { 0x4000 };
    let mut r = RealRegs {
        eax: 0x4F02,
        ebx: arg as u32,
        ..RealRegs::default()
    };
    let ax = int10(&mut r);
    let mut r2 = RealRegs {
        eax: 0x4F03,
        ..RealRegs::default()
    };
    let ax3 = int10(&mut r2);
    let current = r2.ebx as u16;
    let bios_mode = vga::get_mode();
    if ax != 0x004F {
        vga::set_mode(3);
        say!(rep, "set mode {:#05x} (BX={:#06x}): AX={:#06x}, failed", m.number, arg, ax);
        return finish(&rep, 4);
    }
    load_palette();

    let pitch = m.pitch as usize;
    let mut file = Report::default();
    file.0.push_str(&alloc::format!(
        "mode={:#05x} bx={:#06x} set_ax={:#06x} get_ax={:#06x} current={:#06x} int10_0f_mode={:#04x}\n",
        m.number,
        arg,
        ax,
        ax3,
        current,
        bios_mode
    ));
    let (sum, want);
    if banked {
        // Bank window A at its segment (A000h), 4F05h BH=0 BL=0 DX=granularity units.
        let win = (m.gran as usize).max(1) * 1024;
        let size = pitch * H;
        let base = (m.seg_a as u32) << 4;
        let mut bank_calls = 0;
        let mut pos = 0usize;
        let mut bad = 0;
        let mut set_bank = |n: usize| {
            let mut r = RealRegs {
                eax: 0x4F05,
                ebx: 0,
                edx: n as u32,
                ..RealRegs::default()
            };
            if int10(&mut r) != 0x004F {
                bad += 1;
            }
        };
        let mut acc = alloc::vec![0u8; size];
        while pos < size {
            set_bank(pos / win);
            bank_calls += 1;
            let n = win.min(size - pos);
            for i in 0..n {
                let o = pos + i;
                let (x, y) = (o % pitch, o / pitch);
                let v = if x < W { pattern(x, y) } else { 0 };
                // SAFETY: the bank window at A0000h, `n <= win` bytes of it.
                unsafe { dpmi::linear_to_ptr::<u8>(base + i as u32).write_volatile(v) };
            }
            pos += n;
        }
        pos = 0;
        while pos < size {
            set_bank(pos / win);
            let n = win.min(size - pos);
            for i in 0..n {
                // SAFETY: as above.
                acc[pos + i] = unsafe { dpmi::linear_to_ptr::<u8>(base + i as u32).read_volatile() };
            }
            pos += n;
        }
        sum = adler(
            (0..H)
                .flat_map(|y| (0..W).map(move |x| (x, y)))
                .map(|(x, y)| acc[y * pitch + x]),
        );
        want = adler((0..H).flat_map(|y| (0..W).map(move |x| pattern(x, y))));
        file.0.push_str(&alloc::format!(
            "access=banked window_a_attrs={:#04x} granularity_kib={} window_kib={} bank_calls={} bank_failures={}\n",
            m.win_a,
            m.gran,
            m.win_size,
            bank_calls,
            bad
        ));
    } else {
        // DPMI 0800h maps the physical frame buffer; the size is what the mode needs, rounded to a page.
        let len = (pitch * H) as u32;
        let lin = match map_physical(m.phys, len) {
            Ok(l) => l,
            Err(e) => {
                vga::set_mode(3);
                say!(rep, "DPMI 0800h for {:#010x}: error {:#06x}", m.phys, e);
                return finish(&rep, 5);
            }
        };
        let fb = dpmi::linear_to_ptr::<u8>(lin);
        for y in 0..H {
            for x in 0..W {
                // SAFETY: inside the pitch * H bytes the host just mapped, through the 4 GiB data segment.
                unsafe { fb.add(y * pitch + x).write_volatile(pattern(x, y)) };
            }
        }
        // SAFETY: as above.
        sum = adler(
            (0..H)
                .flat_map(|y| (0..W).map(move |x| (x, y)))
                .map(|(x, y)| unsafe { fb.add(y * pitch + x).read_volatile() }),
        );
        want = adler((0..H).flat_map(|y| (0..W).map(move |x| pattern(x, y))));
        file.0.push_str(&alloc::format!(
            "access=lfb phys={:#010x} mapped_linear={:#010x} length={}\n",
            m.phys,
            lin,
            len
        ));
    }
    file.0.push_str(&alloc::format!(
        "checksum={:#010x} expected={:#010x} match={}\n",
        sum,
        want,
        sum == want
    ));
    file.save("VBEGFX.TXT");
    if !args.iter().any(|a| a == "nowait") {
        wait_key();
    }
    vga::set_mode(3);
    say!(rep, "{}", file.0.trim_end().replace('\n', "; "));
    say!(rep, "S5-VBE done");
    if sum == want {
        0
    } else {
        1
    }
}
