// VBE 2.0 over the `Bios` trait: the controller information block (4F00h), mode information blocks (4F01h), the mode set (4F02h),
// the current mode (4F03h) and the bank window (4F05h).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The reference is VESA BIOS Extension 2.0 (VBE Core Function Specification 2.0, 1994). The structures are little-endian and
// packed; offsets below are from that document. Pointers in the information block are real-mode far pointers (segment:offset,
// offset in the low word): the BIOS may leave the strings and the mode list inside the buffer it was given or in its own ROM, so
// they are read with `Bios::read_low`, not assumed to be in the buffer.

use desk_hal::{Bios, BiosRegs, HalError, ModeInfo, PixelLayout};

/// Size of the VBE 2.0 controller information block.
pub const INFO_SIZE: usize = 512;
/// Size of a mode information block.
pub const MODE_INFO_SIZE: usize = 256;

/// OR into the mode number in 4F02h to ask for the linear frame buffer.
pub const MODE_LINEAR: u16 = 0x4000;
/// OR into the mode number in 4F02h to keep the display memory as it is.
pub const MODE_KEEP_MEMORY: u16 = 0x8000;
/// Mode attribute bit: the mode has a linear frame buffer.
pub const ATTR_LINEAR: u16 = 1 << 7;
/// Mode attribute bit: the mode can be set on this hardware.
pub const ATTR_SUPPORTED: u16 = 1 << 0;
/// Mode attribute bit: a graphics mode (otherwise text).
pub const ATTR_GRAPHICS: u16 = 1 << 4;
/// Mode attribute bit: the windowed (banked) memory model is not available.
pub const ATTR_NO_BANKED: u16 = 1 << 6;

/// Memory models (the byte at offset 27 of the mode information block).
pub mod model {
    pub const TEXT: u8 = 0;
    pub const PLANAR: u8 = 3;
    pub const PACKED_PIXEL: u8 = 4;
    pub const DIRECT_COLOUR: u8 = 6;
}

/// The most modes the mode list reader keeps: a BIOS offers a few dozen, and the list is cut here rather than allocating.
pub const MAX_MODES: usize = 128;

/// A real-mode far pointer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FarPtr {
    pub segment: u16,
    pub offset: u16,
}

impl FarPtr {
    fn from_u32(v: u32) -> FarPtr {
        FarPtr {
            segment: (v >> 16) as u16,
            offset: v as u16,
        }
    }
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The controller information block (4F00h), the fields the Desk uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VbeInfo {
    /// BCD: 0x0200 is VBE 2.0.
    pub version: u16,
    pub oem_string: FarPtr,
    /// Bit 0: the DAC width is switchable to 8 bits; bit 1: not VGA compatible; bit 2: the DAC needs blank bit when programmed.
    pub capabilities: u32,
    pub mode_list: FarPtr,
    /// Video memory in 64 KiB units.
    pub memory_64k: u16,
}

impl VbeInfo {
    /// Parses the block. `NoDevice` if it does not start with "VESA" (a BIOS without VBE leaves the buffer as it was, so the
    /// "VBE2" request we wrote is still there and does not match).
    pub fn parse(block: &[u8]) -> Result<VbeInfo, HalError> {
        if block.len() < 0x14 {
            return Err(HalError::InvalidArgument);
        }
        if &block[0..4] != b"VESA" {
            return Err(HalError::NoDevice);
        }
        Ok(VbeInfo {
            version: u16_at(block, 4),
            oem_string: FarPtr::from_u32(u32_at(block, 6)),
            capabilities: u32_at(block, 10),
            mode_list: FarPtr::from_u32(u32_at(block, 14)),
            memory_64k: u16_at(block, 18),
        })
    }
}

/// A mode information block (4F01h), the fields the Desk uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VbeMode {
    pub id: u16,
    pub attributes: u16,
    /// Window granularity and size in KiB, and the segment of window A.
    pub win_granularity_kb: u16,
    pub win_size_kb: u16,
    pub win_a_segment: u16,
    /// Bytes per scan line.
    pub pitch: u16,
    pub width: u16,
    pub height: u16,
    pub planes: u8,
    pub bits_per_pixel: u8,
    pub memory_model: u8,
    /// Physical address of the linear frame buffer (VBE 2.0; 0 if there is none).
    pub phys_base: u32,
}

impl VbeMode {
    pub fn parse(id: u16, block: &[u8]) -> Result<VbeMode, HalError> {
        if block.len() < 44 {
            return Err(HalError::InvalidArgument);
        }
        Ok(VbeMode {
            id,
            attributes: u16_at(block, 0),
            win_granularity_kb: u16_at(block, 4),
            win_size_kb: u16_at(block, 6),
            win_a_segment: u16_at(block, 8),
            pitch: u16_at(block, 16),
            width: u16_at(block, 18),
            height: u16_at(block, 20),
            planes: block[24],
            bits_per_pixel: block[25],
            memory_model: block[27],
            phys_base: u32_at(block, 40),
        })
    }

    pub fn supported(&self) -> bool {
        self.attributes & ATTR_SUPPORTED != 0
    }

    pub fn graphics(&self) -> bool {
        self.attributes & ATTR_GRAPHICS != 0
    }

    /// Whether the mode can be set with a linear frame buffer: the attribute bit and an address to map.
    pub fn linear(&self) -> bool {
        self.attributes & ATTR_LINEAR != 0 && self.phys_base != 0
    }

    /// The HAL's description of the mode, or `None` for one the Desk does not use (text, or a layout it has no code for).
    pub fn mode_info(&self) -> Option<ModeInfo> {
        if !self.supported() || !self.graphics() {
            return None;
        }
        let layout = match (self.memory_model, self.bits_per_pixel) {
            (model::PACKED_PIXEL, 8) => PixelLayout::Indexed8,
            (model::PLANAR, 4) => PixelLayout::Planar4,
            (model::DIRECT_COLOUR, 16) => PixelLayout::Rgb565,
            (model::DIRECT_COLOUR, 32) => PixelLayout::Xrgb8888,
            _ => return None,
        };
        Some(ModeInfo {
            id: self.id,
            width: self.width,
            height: self.height,
            layout,
            linear: self.linear(),
        })
    }

    /// Bytes the linear frame buffer occupies.
    pub fn frame_bytes(&self) -> u32 {
        u32::from(self.pitch) * u32::from(self.height)
    }
}

/// Turns the AX a VBE function returns into a result: AL = 4Fh means the function exists, AH = 0 means it worked. `Unsupported`
/// if the function is not there at all, otherwise `Platform` with the AH status (1 failed, 2 not supported by this hardware, 3
/// invalid in the current video mode).
pub fn status(ax: u32) -> Result<(), HalError> {
    if ax & 0xFF != 0x4F {
        return Err(HalError::Unsupported);
    }
    match (ax >> 8) & 0xFF {
        0 => Ok(()),
        ah => Err(HalError::Platform(ah as u16)),
    }
}

/// The VBE calls, over a `Bios`.
pub struct Vbe<'b, B: Bios> {
    bios: &'b B,
}

impl<'b, B: Bios> Vbe<'b, B> {
    pub fn new(bios: &'b B) -> Vbe<'b, B> {
        Vbe { bios }
    }

    fn call(&self, regs: &mut BiosRegs, buffer: Option<&mut [u8]>) -> Result<(), HalError> {
        self.bios.int(0x10, regs, buffer)?;
        status(regs.eax)
    }

    /// 4F00h: the controller information. Asks for the VBE 2.0 layout by putting "VBE2" in the buffer first.
    pub fn info(&self) -> Result<VbeInfo, HalError> {
        let mut block = [0u8; INFO_SIZE];
        block[0..4].copy_from_slice(b"VBE2");
        let mut regs = BiosRegs {
            eax: 0x4F00,
            ..BiosRegs::default()
        };
        self.call(&mut regs, Some(&mut block))?;
        VbeInfo::parse(&block)
    }

    /// Reads the mode list `info` points at: 16-bit mode numbers up to 0xFFFF, at most `out.len()` of them. Returns how many.
    pub fn mode_list(&self, info: &VbeInfo, out: &mut [u16]) -> Result<usize, HalError> {
        let mut n = 0;
        let mut at = info.mode_list;
        while n < out.len() {
            let mut word = [0u8; 2];
            self.bios.read_low(at.segment, at.offset, &mut word)?;
            let id = u16::from_le_bytes(word);
            if id == 0xFFFF {
                break;
            }
            out[n] = id;
            n += 1;
            at.offset = at.offset.checked_add(2).ok_or(HalError::InvalidArgument)?;
        }
        Ok(n)
    }

    /// Copies the OEM string (up to `out.len()` bytes, stopping at the terminator) and returns its length.
    pub fn oem_string(&self, info: &VbeInfo, out: &mut [u8]) -> Result<usize, HalError> {
        let mut n = 0;
        let mut at = info.oem_string;
        if at.segment == 0 && at.offset == 0 {
            return Ok(0);
        }
        while n < out.len() {
            let mut b = [0u8; 1];
            self.bios.read_low(at.segment, at.offset, &mut b)?;
            if b[0] == 0 {
                break;
            }
            out[n] = b[0];
            n += 1;
            at.offset = at.offset.checked_add(1).ok_or(HalError::InvalidArgument)?;
        }
        Ok(n)
    }

    /// 4F01h: information about one mode.
    pub fn mode(&self, id: u16) -> Result<VbeMode, HalError> {
        let mut block = [0u8; MODE_INFO_SIZE];
        let mut regs = BiosRegs {
            eax: 0x4F01,
            ecx: id.into(),
            ..BiosRegs::default()
        };
        self.call(&mut regs, Some(&mut block))?;
        VbeMode::parse(id, &block)
    }

    /// The first mode in `ids` that is a supported graphics mode of exactly `width` × `height` at `bits` bits per pixel in the
    /// packed-pixel or direct-colour model, with a linear frame buffer if `linear` is set. Modes whose 4F01h call fails are skipped.
    pub fn find(&self, ids: &[u16], width: u16, height: u16, bits: u8, linear: bool) -> Option<VbeMode> {
        ids.iter().filter_map(|id| self.mode(*id).ok()).find(|m| {
            m.supported()
                && m.graphics()
                && m.width == width
                && m.height == height
                && m.bits_per_pixel == bits
                && matches!(m.memory_model, model::PACKED_PIXEL | model::DIRECT_COLOUR)
                && (!linear || m.linear())
        })
    }

    /// 4F02h: sets a mode. With `linear` the linear frame buffer bit is added, and the display memory is cleared.
    pub fn set_mode(&self, id: u16, linear: bool) -> Result<(), HalError> {
        let bx = if linear { id | MODE_LINEAR } else { id };
        let mut regs = BiosRegs {
            eax: 0x4F02,
            ebx: bx.into(),
            ..BiosRegs::default()
        };
        self.call(&mut regs, None)
    }

    /// 4F03h: the current mode, including the flag bits the mode was set with.
    pub fn current_mode(&self) -> Result<u16, HalError> {
        let mut regs = BiosRegs {
            eax: 0x4F03,
            ..BiosRegs::default()
        };
        self.call(&mut regs, None)?;
        Ok(regs.ebx as u16)
    }

    /// 4F05h: positions bank window `window` (0 for A, 1 for B) at `position`, in units of the window granularity. This is the
    /// banked fallback; with a linear frame buffer it is never called.
    pub fn set_window(&self, window: u8, position: u16) -> Result<(), HalError> {
        let mut regs = BiosRegs {
            eax: 0x4F05,
            ebx: window.into(),
            edx: position.into(),
            ..BiosRegs::default()
        };
        self.call(&mut regs, None)
    }
}
