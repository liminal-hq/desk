// A Rust runtime for 32-bit DOS programs that run under a DPMI host (CWSDPMI) behind DJGPP's stub: no C library, no crt0.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// One concern per module:
//
// - `start`    the entry point `_start`, the registers the stub hands over, the stack and the stub's information block
// - `dpmi`     a client for the DPMI host: INT 31h, memory blocks, descriptors, locking, interrupt vectors, real-mode calls
// - `dos`      INT 21h wrappers: console and file I/O, the exit call
// - `heap`     the global allocator, over DPMI memory blocks
// - `env`      the command tail and the environment, read from the PSP
// - `fpu`      the x87: initial state and the maths `core` does not have
// - `io`       `print!`, `println!` and a `core::fmt` writer over standard output
// - `mem`      memcpy and friends for the COFF (stage 1) build only; the ELF build gets them from compiler_builtins
// - `panic`    the panic handler: report, run the clean-up list, exit with status 3 (the `panic-handler` feature, on by default)
// - `cleanup`  what must be undone before the program ends, by exit or by panic
// - `timer`    a hook on IRQ 0 that counts ticks (hardware proof)
// - `vga`      mode 13h and the frame buffer (hardware proof)
//
// Single-threaded, with one exception: interrupt handlers, which touch only `static`s and never allocate or call DOS.
#![no_std]

extern crate alloc;

pub mod cleanup;
pub mod dos;
pub mod dpmi;
pub mod env;
pub mod fpu;
pub mod heap;
pub mod io;
#[cfg(target_os = "windows")]
mod mem;
#[cfg(feature = "panic-handler")]
mod panic;
pub mod start;
pub mod timer;
pub mod vga;

pub use dos::exit;

/// Declares the program's entry function, which runs after the runtime is up. It returns the exit status.
///
/// ```ignore
/// rt::entry!(main);
/// fn main() -> i32 { rt::println!("hello"); 0 }
/// ```
#[macro_export]
macro_rules! entry {
    ($f:path) => {
        #[no_mangle]
        pub extern "Rust" fn __rt_main() -> i32 {
            let f: fn() -> i32 = $f;
            f()
        }
    };
}
