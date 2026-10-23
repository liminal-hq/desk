// Desk's hardware abstraction layer: the small set of traits that drivers are written against and platforms implement.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Three kinds of code meet here (the design is `docs/HAL.md`):
//
// - a platform implements these traits for one environment: the hosted DOS runtime over DPMI, the native kernel, or the host
//   mock that tests run against;
// - a driver (the 16550, the PIT, the PIC, later the keyboard, VGA, OPL) is written once against the traits;
// - the Desk core uses drivers and the traits, never DOS, DJGPP or kernel internals.
//
// One module per surface:
//
// - `port`     port I/O (`inb` to `outl`) and the I/O delay
// - `input`    key and mouse events, and the trait the core drains them through
// - `irq`      interrupt lines, the interrupt controller, handler registration as a function pointer and a context pointer
// - `time`     the monotonic tick source and the calibrated delay
// - `ring`     the single-producer, single-consumer queue that hands data from an interrupt handler to the core
// - `mmio`     a bounds-checked, volatile view of a memory-mapped region (the VGA window, a linear frame buffer)
// - `error`    the shared error type
// - `video`    DAC colours, pixel layouts, modes, the frame buffer and the `Display` trait
// - `bios`     a real-mode software interrupt, for VBE
// - `x86`      the real port instructions, the one audited `unsafe` boundary for port I/O (x86 targets only)
//
// Atomics: only load, store, swap and fetch_add/fetch_sub (`lock xadd`). Compare-and-swap is banned (`clippy.toml`): DOSBox with
// `cputype = 486` rejects `CMPXCHG`, and the 486 target has no 64-bit atomics at all.
#![cfg_attr(not(any(test, feature = "std")), no_std)]

pub mod bios;
pub mod error;
pub mod input;
pub mod irq;
pub mod mmio;
pub mod port;
pub mod ring;
pub mod time;
pub mod video;
#[cfg(target_arch = "x86")]
pub mod x86;

pub use bios::{Bios, BiosRegs};
pub use error::HalError;
pub use input::{InputSource, KeyCode, KeyEvent, Modifiers, MouseEvent, Scancode};
pub use irq::{InterruptController, Irq, IrqHandler, IrqRegistry, IrqService, SharedIrqService};
pub use mmio::MmioRegion;
pub use port::PortIo;
pub use ring::SpscRing;
pub use time::{Clock, Delay, Ticks};
pub use video::{Display, FrameBuffer, ModeInfo, PixelLayout, Rgb6};

/// A sink for debug text: the serial port, the VGA text screen, or a test's buffer. Must not allocate, so a panic handler can use it.
pub trait DebugOut {
    /// Writes raw bytes. Never fails: a missing device drops the bytes.
    fn write_bytes(&self, bytes: &[u8]);
}
