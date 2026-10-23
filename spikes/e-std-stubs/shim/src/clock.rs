// A monotonic clock from the PIT and the BIOS tick count, and the wall clock from INT 21h.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The BIOS counts IRQ 0 ticks at 0040:006C; the PIT's channel 0 (a square wave of period 65536 clocks at 1.193182 MHz) counts down
// twice within each tick, so the counter alone is ambiguous. The read-back command latches the count together with the status byte,
// whose bit 7 is the output pin: high for the first half of the period and low for the second. That gives a position within the tick
// to 838 ns. Reads are retried when a tick arrives between them, and the result never goes backwards (the BIOS counts a tick a little
// after the PIT wraps). The count restarts at midnight; a fall of more than half a day is taken as that, and a day is added from then on.
use crate::Cell64;
use core::arch::asm;

const CLOCKS_PER_TICK: u64 = 65536;

/// # Safety
/// Port I/O: the caller names a port that is safe to read.
unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack, preserves_flags));
    v
}

/// # Safety
/// Port I/O: the caller names a port that is safe to write.
unsafe fn outb(port: u16, v: u8) {
    asm!("out dx, al", in("dx") port, in("al") v, options(nomem, nostack, preserves_flags));
}

/// The position within the current tick, in PIT clocks.
fn pit_position() -> u64 {
    // SAFETY: ports 0x40 and 0x43 are the PIT; read-back (0xC2) latches channel 0's count and status, and the three reads that follow
    // return the status byte, then the count's low and high bytes.
    unsafe {
        outb(0x43, 0xC2);
        let status = inb(0x40);
        let lo = inb(0x40) as u64;
        let hi = inb(0x40) as u64;
        let count = (hi << 8) | lo;
        // Counting down by two from 65536 (stored as 0) in each half.
        let down = if count == 0 { 65536 } else { count };
        let within_half = (65536 - down) / 2;
        if status & 0x80 != 0 {
            within_half
        } else {
            32768 + within_half
        }
    }
}

static LAST: Cell64 = Cell64::new(0);
/// Whole days added so far because the BIOS count restarted at midnight, in nanoseconds.
static OFFSET: Cell64 = Cell64::new(0);

/// A BIOS day is 0x1800B0 ticks.
const DAY_CLOCKS: u64 = 0x1800B0 * CLOCKS_PER_TICK;

/// PIT clocks to nanoseconds: 1e9 / 1_193_182 = 838.0953 ns per clock.
fn clocks_to_ns(clocks: u64) -> u64 {
    clocks * 838 + clocks * 95 / 1000
}

/// Nanoseconds since the BIOS counted its first tick (midnight) of the first day this was called, never decreasing, and carrying on
/// across midnight.
pub fn now_ns() -> u64 {
    let (ticks, pos) = loop {
        let t1 = rt::timer::bios_ticks();
        let p = pit_position();
        if rt::timer::bios_ticks() == t1 {
            break (t1 as u64, p);
        }
    };
    let day_ns = clocks_to_ns(DAY_CLOCKS);
    let mut ns = clocks_to_ns(ticks * CLOCKS_PER_TICK + pos) + OFFSET.get();
    // The reading fell by more than the retry jitter could explain: the BIOS count restarted at midnight. Without this the clamp
    // below would hold the clock still (and a `sleep` spin would never end) until the same time the next day.
    if LAST.get().saturating_sub(ns) > day_ns / 2 {
        OFFSET.set(OFFSET.get() + day_ns);
        ns += day_ns;
    }
    let ns = ns.max(LAST.get());
    LAST.set(ns);
    ns
}

#[cfg(feature = "desk")]
/// Days from 1970-01-01 to the given civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

#[cfg(feature = "desk")]
/// The DOS date and time (INT 21h, AH=2Ah and AH=2Ch) as nanoseconds since 1970-01-01, taking the local clock for UTC. 0 on failure.
pub fn unix_ns() -> u64 {
    use rt::dpmi::{simulate_int, RealRegs};
    let mut d = RealRegs {
        eax: 0x2A00,
        ..RealRegs::default()
    };
    let mut t = RealRegs {
        eax: 0x2C00,
        ..RealRegs::default()
    };
    if simulate_int(0x21, &mut d).is_err() || simulate_int(0x21, &mut t).is_err() {
        return 0;
    }
    let (year, month, day) = ((d.ecx & 0xFFFF) as i64, ((d.edx >> 8) & 0xFF) as i64, (d.edx & 0xFF) as i64);
    let (h, m, s, cs) = (
        ((t.ecx >> 8) & 0xFF) as u64,
        (t.ecx & 0xFF) as u64,
        ((t.edx >> 8) & 0xFF) as u64,
        (t.edx & 0xFF) as u64,
    );
    let days = days_from_civil(year, month, day).max(0) as u64;
    (days * 86_400 + h * 3600 + m * 60 + s) * 1_000_000_000 + cs * 10_000_000
}
