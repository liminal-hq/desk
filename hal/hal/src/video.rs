// The video types: DAC colours, pixel layouts, mode descriptions, the frame buffer and the `Display` trait.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The core sees a `Display` and a `FrameBuffer`; it never sees INT 10h, a VBE information block or a DPMI selector. A platform
// implements `Display` on top of `desk-vga` (the registers, the DAC, the drawing primitives) and its own way of setting a mode.

use crate::{HalError, MmioRegion};

/// A colour as the VGA DAC holds it: six bits per channel, 0 to 63.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb6 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb6 {
    pub const BLACK: Rgb6 = Rgb6 { r: 0, g: 0, b: 0 };

    /// The channels are masked to six bits.
    pub const fn new(r: u8, g: u8, b: u8) -> Rgb6 {
        Rgb6 {
            r: r & 63,
            g: g & 63,
            b: b & 63,
        }
    }

    /// The nearest DAC colour to an 8-bit-per-channel one: `(v * 63 + 127) / 255` on each channel (255 gives 63, 128 gives 32).
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Rgb6 {
        const fn q(v: u8) -> u8 {
            ((v as u32 * 63 + 127) / 255) as u8
        }
        Rgb6 {
            r: q(r),
            g: q(g),
            b: q(b),
        }
    }

    /// The colour as 8 bits per channel, the top two bits repeated into the bottom two (63 gives 255), as a PNG or a BMP wants it.
    pub const fn to_rgb8(self) -> (u8, u8, u8) {
        const fn e(v: u8) -> u8 {
            (v << 2) | (v >> 4)
        }
        (e(self.r), e(self.g), e(self.b))
    }

    /// The colour scaled by `num / den`, rounded down, for fades. `num` above `den` is clamped to `den`.
    pub const fn scaled(self, num: u32, den: u32) -> Rgb6 {
        const fn s(v: u8, num: u32, den: u32) -> u8 {
            if den == 0 || num >= den {
                v
            } else {
                (v as u32 * num / den) as u8
            }
        }
        Rgb6 {
            r: s(self.r, num, den),
            g: s(self.g, num, den),
            b: s(self.b, num, den),
        }
    }
}

/// How a frame buffer stores a pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelLayout {
    /// One byte per pixel, an index into the DAC palette.
    Indexed8,
    /// Four bit planes at the same addresses (VGA mode 12h); a pixel is one bit of each plane.
    Planar4,
    Rgb565,
    Xrgb8888,
}

/// What a platform can set: the identifier is the platform's (a BIOS mode number, with VBE modes from 0x100), the rest is what the
/// core needs to choose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeInfo {
    pub id: u16,
    pub width: u16,
    pub height: u16,
    pub layout: PixelLayout,
    /// Whether the frame buffer is one linear region (a VBE linear frame buffer, or mode 13h) rather than banked.
    pub linear: bool,
}

/// A mode that has been set, and its pixels. For `Planar4` the region is the 64 KiB VGA window and `pitch` is bytes per row of one
/// plane (80 at 640 pixels); for `Indexed8` it is bytes per row.
#[derive(Debug)]
pub struct FrameBuffer {
    pub info: ModeInfo,
    pub pitch: u32,
    pub region: MmioRegion,
    /// The physical address of the region (0xA0000 for the VGA window).
    pub phys: u32,
}

/// The core's view of the screen.
pub trait Display {
    fn modes(&self) -> &[ModeInfo];
    /// Sets the mode and returns its frame buffer. The platform registers the return to text mode with its clean-up list.
    fn set_mode(&mut self, id: u16) -> Result<FrameBuffer, HalError>;
    fn restore_text_mode(&mut self);
    /// Loads `colours` into the DAC from entry `first`. Fails with `InvalidArgument` if they do not fit in 256 entries.
    fn set_palette(&mut self, first: u8, colours: &[Rgb6]) -> Result<(), HalError>;
    fn palette(&self, first: u8, out: &mut [Rgb6]) -> Result<(), HalError>;
    /// Waits, boundedly, for the start of vertical retrace.
    fn wait_vretrace(&self) -> Result<(), HalError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb8_converts_both_ways() {
        assert_eq!(Rgb6::from_rgb8(255, 255, 255), Rgb6::new(63, 63, 63));
        assert_eq!(Rgb6::from_rgb8(0, 0, 0), Rgb6::BLACK);
        assert_eq!(Rgb6::from_rgb8(0xff, 0xaa, 0x40), Rgb6::new(63, 42, 16));
        assert_eq!(Rgb6::new(63, 0, 32).to_rgb8(), (255, 0, 130));
        assert_eq!(Rgb6::new(200, 64, 65), Rgb6::new(8, 0, 1), "channels are masked to six bits");
    }

    #[test]
    fn scaling_rounds_down_and_clamps() {
        let c = Rgb6::new(63, 32, 1);
        assert_eq!(c.scaled(8, 16), Rgb6::new(31, 16, 0));
        assert_eq!(c.scaled(16, 16), c);
        assert_eq!(c.scaled(99, 16), c);
        assert_eq!(c.scaled(0, 16), Rgb6::BLACK);
        assert_eq!(c.scaled(1, 0), c);
    }
}
