// The symbols a borrowed or patched std platform layer expects, supplied over the d0 runtime.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Two interfaces, one feature each:
//
// - `trusty`  with `os` set to "trusty" in the target specification, unmodified std selects the platform layer written for Google's
//             Trusty OS. It is mostly the "unsupported" layer plus three reaches into a C library: the allocator (malloc and friends),
//             standard output (`writev`) and the random seed (`trusty_rng_secure_rand`).
// - `desk`    with `os` set to "desk" or "dos", the patched std (patch/) selects a platform layer that calls the `desk_*` functions.
//
// Standard error is written to standard output in both: DOS standard error is the console and cannot be redirected with `>`, which is
// all the headless harness can capture.
#![no_std]

extern crate alloc;

mod clock;

/// A `u64` in a `static`. A 486 has no 64-bit atomics (`max-atomic-width` is 32), and the program is single-threaded.
pub(crate) struct Cell64(core::cell::Cell<u64>);

impl Cell64 {
    pub(crate) const fn new(v: u64) -> Self {
        Cell64(core::cell::Cell::new(v))
    }
    pub(crate) fn get(&self) -> u64 {
        self.0.get()
    }
    pub(crate) fn set(&self, v: u64) {
        self.0.set(v)
    }
}

// SAFETY: single-threaded; interrupt handlers never call into this crate.
unsafe impl Sync for Cell64 {}

#[cfg(feature = "desk")]
mod desk;
#[cfg(feature = "trusty")]
mod trusty;

/// The random seed: the BIOS tick count mixed with the PIT counter and a running counter, through a 64-bit mixer. Not secure; a 486
/// has no cycle counter, and the point is only that `HashMap` gets different keys on different runs.
pub fn random_bytes(buf: &mut [u8]) {
    static STATE: Cell64 = Cell64::new(0x9E37_79B9_7F4A_7C15);
    let mut x = STATE.get() ^ clock::now_ns() ^ ((rt::timer::bios_ticks() as u64) << 32);
    for b in buf.iter_mut() {
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        *b = (z ^ (z >> 31)) as u8;
    }
    STATE.set(x);
}
