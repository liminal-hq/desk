// The VGA and VBE driver: mode 13h and 12h by register programming, the DAC, drawing primitives, and the VBE 2.0 layer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Written once against the HAL (docs/HAL.md, "Contract for spike A"): no unsafe, no BIOS call except through the `Bios` trait the
// VBE layer is given, no DOS. The hosted platform sets VBE modes through the DPMI real-mode call; the native kernel uses
// `regs::set_mode_13h` and `set_mode_12h` (no BIOS), and VBE through whatever real-mode mechanism it grows.
//
// - `regs`    the register tables for mode 13h and 12h and the sequence that programs them through the ports
// - `dac`     palette load and read, the bounded wait for vertical retrace, fades
// - `canvas`  the `Canvas` drawing trait, the Adler-32 checksum used by the checks
// - `linear`  `Linear8`: pixels, lines, rectangles and blits on an 8-bit linear frame buffer (mode 13h, VBE)
// - `planar`  `Planar16`: the same on mode 12h's four planes, through the map mask, set/reset and bit mask
// - `text`    leaving graphics: the text palette and the font for mode 03h, which have to be reloaded without a BIOS
// - `vbe`     VBE 2.0: the information and mode blocks, the calls 4F00h to 4F05h over `Bios`
#![cfg_attr(not(any(test, feature = "std")), no_std)]

pub mod canvas;
pub mod dac;
pub mod linear;
pub mod planar;
pub mod regs;
pub mod text;
pub mod vbe;

pub use canvas::{checksum, Adler32, Canvas};
pub use dac::{fade_in, fade_out, read_palette, set_palette, wait_vretrace, FadeReport};
pub use linear::Linear8;
pub use planar::Planar16;
pub use regs::{set_mode_03h, set_mode_12h, set_mode_13h};
