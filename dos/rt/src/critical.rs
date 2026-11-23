// A guard that makes a failing disk fail the call instead of waiting on "Abort, Retry, Fail?".
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// DOS raises INT 24h when a disk call hits a critical error (no disk in the drive, a write-protected disk, a drive not ready) and
// by default prints a prompt and waits for a key. A program with a graphics screen up has nobody to read it. `FailCritical` points
// INT 24h at a three-byte real-mode stub, `mov al,3` and `iret` (AL=3 is "fail the call"), for as long as it is held, so the file
// call returns a DOS error instead.
//
// The stub is real-mode code, so it lives in conventional memory from DPMI 0100h, one paragraph. The vector is read and set
// through DOS (AH=35h and AH=25h, the real-mode vector). The vector goes in only once the clean-up registration has succeeded, so
// an exit or a panic always finds `restore` on the list; the registration is refused when the list is full, and then nothing has
// been touched. `restore` is idempotent: the guard's drop and the clean-up list may both run it.
//
// Nesting: a second guard while one is held is allowed and shares the first one's stub. A depth count says when the last one is
// gone; only then is the vector put back. `restore` from the clean-up list ignores the depth and puts everything back at once, so a
// guard dropped after that (which only happens after exit has begun) finds nothing to do.
//
// DOS also restores INT 24h from the PSP when the program ends, so a vector checked after the program has gone cannot tell this
// module's restore from DOS's. A function registered before the guard, which runs after `restore`, can.
//
// Not verifiable headless: a real critical error. DOSBox gives DOS error 5 (access denied) for a read-only mount, which DOS does
// not raise INT 24h for, so the stub is only proved by calling it through a real-mode interrupt simulation.
use crate::{cleanup, dos, dpmi};
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// Not a DOS code: the clean-up list is full, so the guard was not installed.
pub const ERR_CLEANUP_FULL: u16 = 0x8001;

/// `mov al, 3` then `iret`.
pub const STUB: [u8; 3] = [0xB0, 0x03, 0xCF];

static DEPTH: AtomicUsize = AtomicUsize::new(0);
// The segment and selector of the stub's paragraph, and the vector found when the first guard went in (segment << 16 | offset).
static STUB_SEGMENT: AtomicU32 = AtomicU32::new(0);
static STUB_SELECTOR: AtomicU32 = AtomicU32::new(0);
static SAVED: AtomicU32 = AtomicU32::new(0);

/// Holds INT 24h at the failing stub until dropped.
pub struct FailCritical(());

impl FailCritical {
    /// Installs the stub (or joins the guard already held). Fails, with nothing changed, if the clean-up list is full
    /// (`ERR_CLEANUP_FULL`), if no conventional memory is left, or if DOS refuses the vector.
    pub fn new() -> Result<FailCritical, dos::DosError> {
        let depth = DEPTH.load(Ordering::SeqCst);
        if depth > 0 {
            DEPTH.store(depth + 1, Ordering::SeqCst);
            return Ok(FailCritical(()));
        }
        let (segment, selector) = dpmi::alloc_dos_memory(1).map_err(|e| dos::DosError(e.0))?;
        let at = dpmi::linear_to_ptr::<u8>((segment as u32) << 4);
        // SAFETY: the paragraph was just allocated and has 16 bytes, 3 of which are written.
        unsafe { core::ptr::copy_nonoverlapping(STUB.as_ptr(), at, STUB.len()) };
        if !cleanup::register(restore) {
            let _ = dpmi::free_dos_memory(selector);
            return Err(dos::DosError(ERR_CLEANUP_FULL));
        }
        let installed = dos::get_vector(0x24).and_then(|(s, o)| {
            SAVED.store(((s as u32) << 16) | o as u32, Ordering::SeqCst);
            dos::set_vector(0x24, segment, 0)
        });
        if let Err(e) = installed {
            cleanup::forget(restore);
            let _ = dpmi::free_dos_memory(selector);
            return Err(e);
        }
        STUB_SEGMENT.store(segment as u32, Ordering::SeqCst);
        STUB_SELECTOR.store(selector as u32, Ordering::SeqCst);
        DEPTH.store(1, Ordering::SeqCst);
        Ok(FailCritical(()))
    }

    /// The real-mode address of the stub while a guard is held, for a test that calls it through INT 24h.
    pub fn stub_address() -> Option<(u16, u16)> {
        if DEPTH.load(Ordering::SeqCst) == 0 {
            None
        } else {
            Some((STUB_SEGMENT.load(Ordering::SeqCst) as u16, 0))
        }
    }

    /// How many guards are held.
    pub fn depth() -> usize {
        DEPTH.load(Ordering::SeqCst)
    }
}

impl Drop for FailCritical {
    fn drop(&mut self) {
        let depth = DEPTH.load(Ordering::SeqCst);
        if depth > 1 {
            DEPTH.store(depth - 1, Ordering::SeqCst);
        } else {
            restore();
        }
    }
}

/// Puts INT 24h back as it was found, frees the stub and leaves the clean-up list. Does nothing when no guard is held.
pub fn restore() {
    if DEPTH.load(Ordering::SeqCst) == 0 {
        return;
    }
    DEPTH.store(0, Ordering::SeqCst);
    let saved = SAVED.load(Ordering::SeqCst);
    let _ = dos::set_vector(0x24, (saved >> 16) as u16, saved as u16);
    let _ = dpmi::free_dos_memory(STUB_SELECTOR.load(Ordering::SeqCst) as u16);
    STUB_SEGMENT.store(0, Ordering::SeqCst);
    cleanup::forget(restore);
}
