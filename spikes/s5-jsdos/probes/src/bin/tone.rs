// A sound probe: detects the AdLib (OPL2) and the Sound Blaster, then plays an A4-ish tone three ways with a second of silence
// between them: an OPL2 sine at 440 Hz, a Sound Blaster single-cycle DMA square wave at 400 Hz (20000 Hz, 50 samples a period)
// and the PC speaker at 440 Hz. A harness that can read the emulator's audio output measures the pitches; the program reports
// what the hardware said (detection, DSP version, DMA terminal count, pending IRQ) in TONE.TXT.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

extern crate alloc;

use probes::{delay_ms, in8, out8, say, Report};
use rt::dpmi;

rt::entry!(main);

const SB: u16 = 0x220;

/// One OPL2 register write with the settling reads the chip needs.
fn opl(reg: u8, val: u8) {
    out8(0x388, reg);
    for _ in 0..6 {
        in8(0x388);
    }
    out8(0x389, val);
    for _ in 0..35 {
        in8(0x388);
    }
}

/// The AdLib detection every DOS program uses: timer 1 must overflow and set the status bits.
fn opl_present() -> bool {
    opl(4, 0x60);
    opl(4, 0x80);
    let first = in8(0x388);
    opl(2, 0xFF);
    opl(4, 0x21);
    for _ in 0..2000 {
        in8(0x388);
    }
    let second = in8(0x388);
    opl(4, 0x60);
    opl(4, 0x80);
    (first & 0xE0) == 0 && (second & 0xE0) == 0xC0
}

fn dsp_write(v: u8) {
    for _ in 0..100_000 {
        if in8(SB + 0xC) & 0x80 == 0 {
            break;
        }
    }
    out8(SB + 0xC, v);
}

fn dsp_read() -> Option<u8> {
    for _ in 0..100_000 {
        if in8(SB + 0xE) & 0x80 != 0 {
            return Some(in8(SB + 0xA));
        }
    }
    None
}

fn main() -> i32 {
    let mut rep = Report::default();
    say!(rep, "S5-TONE start");

    // AdLib.
    let adlib = opl_present();
    say!(rep, "opl: detected {}", adlib);

    // Sound Blaster: reset, then ask for the version.
    out8(SB + 6, 1);
    delay_ms(10);
    out8(SB + 6, 0);
    let reset = dsp_read();
    say!(rep, "sb: reset answer {:?} (Some(170) is 0xAA, a DSP is there)", reset);
    let version = if reset == Some(0xAA) {
        dsp_write(0xE1);
        let (maj, min) = (dsp_read(), dsp_read());
        say!(rep, "sb: DSP version {:?}.{:?}", maj, min);
        maj
    } else {
        None
    };

    // 1. The OPL2 tone: a sustained sine on one carrier, F-number 580 in block 4, 580 * 49716 / 2^16 = 440 Hz.
    if adlib {
        opl(0x01, 0x20);
        opl(0x20, 0x21);
        opl(0x40, 0x3F);
        opl(0x60, 0xF0);
        opl(0x80, 0x0F);
        opl(0xE0, 0x00);
        opl(0x23, 0x21);
        opl(0x43, 0x00);
        opl(0x63, 0xF0);
        opl(0x83, 0x0F);
        opl(0xE3, 0x00);
        opl(0xC0, 0x01);
        opl(0xA0, (580 & 255) as u8);
        opl(0xB0, 0x20 | (4 << 2) | (580 >> 8) as u8);
        say!(rep, "tone 1: OPL2, 440 Hz, 2 s");
        delay_ms(2000);
        opl(0xB0, 0x00);
    }
    delay_ms(1000);

    // 2. The Sound Blaster: eight-bit single-cycle DMA on channel 1 from a buffer in conventional memory that does not cross a
    // 64 KiB physical page.
    if version.is_some() {
        const N: usize = 20_000;
        let (seg, _sel) = dpmi::alloc_dos_memory(0x1100).expect("DOS memory");
        let mut linear = (seg as u32) << 4;
        if (linear & 0xFFFF) as usize + N > 0x10000 {
            linear = (linear + 0xFFFF) & !0xFFFF;
        }
        for i in 0..N {
            let v = if (i / 25) % 2 == 0 { 0xC0u8 } else { 0x40 };
            // SAFETY: inside the block just allocated (68 KiB, N bytes used after alignment), below 1 MiB.
            unsafe { dpmi::linear_to_ptr::<u8>(linear + i as u32).write_volatile(v) };
        }
        let mask = in8(0x21);
        out8(0x21, mask | 0x80); // IRQ 7 stays pending in the PIC instead of reaching a handler that is not there
        out8(0x0A, 0x05); // mask DMA channel 1
        out8(0x0C, 0); // clear the byte flip-flop
        out8(0x0B, 0x49); // channel 1, single cycle, memory to device
        out8(0x02, linear as u8);
        out8(0x02, (linear >> 8) as u8);
        out8(0x83, (linear >> 16) as u8);
        out8(0x03, ((N - 1) & 0xFF) as u8);
        out8(0x03, ((N - 1) >> 8) as u8);
        out8(0x0A, 0x01); // unmask channel 1
        dsp_write(0xD1); // speaker on
        dsp_write(0x40); // time constant: 256 - 1000000 / 20000 = 206
        dsp_write(206);
        dsp_write(0x14); // 8-bit single-cycle DMA output
        dsp_write(((N - 1) & 0xFF) as u8);
        dsp_write(((N - 1) >> 8) as u8);
        say!(rep, "tone 2: Sound Blaster DMA, 400 Hz, 1 s");
        delay_ms(1300);
        out8(0x20, 0x0A); // read the PIC's request register
        let irr = in8(0x20);
        out8(0x0C, 0);
        let count = in8(0x03) as u16 | (in8(0x03) as u16) << 8;
        in8(SB + 0xE); // acknowledge the DSP's interrupt
        out8(0x21, mask);
        say!(
            rep,
            "sb: DMA count register {:#06x} (0xffff is terminal count), IRQ 7 pending {}",
            count,
            irr & 0x80 != 0
        );
    }
    delay_ms(1000);

    // 3. The PC speaker: PIT channel 2 as a square wave, gate and speaker bits on port 61h.
    let divisor = 1_193_182u32 / 440;
    out8(0x43, 0xB6);
    out8(0x42, divisor as u8);
    out8(0x42, (divisor >> 8) as u8);
    let gate = in8(0x61);
    out8(0x61, gate | 3);
    say!(rep, "tone 3: PC speaker, 440 Hz, 2 s");
    delay_ms(2000);
    out8(0x61, gate & !3);
    delay_ms(500);

    rep.save("TONE.TXT");
    say!(rep, "S5-TONE done");
    0
}
