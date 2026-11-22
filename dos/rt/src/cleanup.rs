// The clean-up list: what must be undone before the program ends, whether it exits or panics.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A hooked interrupt vector left pointing into a program that has gone is a crash waiting for the next tick, and a screen left
// in a graphics mode is a user waiting at a prompt they cannot read. Whatever installs such a thing registers a function here;
// `exit` and the panic handler run them in strict reverse order of registration, once each.
//
// Plain loads and stores, no compare-and-swap: the program is single-threaded, and DOSBox's `cputype = 486` refuses CMPXCHG (a
// real 486 from the B step on has it), so nothing in the runtime may use it.
use core::sync::atomic::{AtomicUsize, Ordering};

/// How many functions the list holds.
pub const SLOTS: usize = 16;

/// The list is full: nothing was registered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Full;

// The entries, bottom first: LIST[0] was registered first and runs last. Slots at COUNT and above are zero.
static LIST: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static COUNT: AtomicUsize = AtomicUsize::new(0);

/// Registers `f` to run at exit, above everything registered so far (so it runs first). If `f` is already on the list it moves to the
/// top, so the order is always the order of the last registration. Returns false, and changes nothing, if the list is full.
pub fn register(f: fn()) -> bool {
    let present = position(f as usize);
    let count = COUNT.load(Ordering::SeqCst);
    if present.is_none() && count == SLOTS {
        return false;
    }
    forget(f);
    let count = COUNT.load(Ordering::SeqCst);
    LIST[count].store(f as usize, Ordering::SeqCst);
    COUNT.store(count + 1, Ordering::SeqCst);
    true
}

/// Registers `f`, or says the list is full. The form callers use to roll back what they just acquired.
pub fn try_register(f: fn()) -> Result<(), Full> {
    if register(f) {
        Ok(())
    } else {
        Err(Full)
    }
}

/// Removes `f` from the list without running it (the owner has already undone its work). The entries above it move down, in order.
pub fn forget(f: fn()) {
    let Some(at) = position(f as usize) else { return };
    let count = COUNT.load(Ordering::SeqCst);
    for i in at..count - 1 {
        LIST[i].store(LIST[i + 1].load(Ordering::SeqCst), Ordering::SeqCst);
    }
    LIST[count - 1].store(0, Ordering::SeqCst);
    COUNT.store(count - 1, Ordering::SeqCst);
}

/// How many functions are registered.
pub fn len() -> usize {
    COUNT.load(Ordering::SeqCst)
}

fn position(f: usize) -> Option<usize> {
    (0..COUNT.load(Ordering::SeqCst)).find(|&i| LIST[i].load(Ordering::SeqCst) == f)
}

/// Runs every registered function, last registered first, and empties the list. A function may `forget` or `register` while it runs.
pub fn run_all() {
    loop {
        let count = COUNT.load(Ordering::SeqCst);
        if count == 0 {
            return;
        }
        let f = LIST[count - 1].swap(0, Ordering::SeqCst);
        COUNT.store(count - 1, Ordering::SeqCst);
        if f != 0 {
            // SAFETY: the value was stored from a `fn()` by `register`.
            let f: fn() = unsafe { core::mem::transmute::<usize, fn()>(f) };
            f();
        }
    }
}
