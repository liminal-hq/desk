// The BIOS surface: a real-mode software interrupt, for the things only firmware knows how to do (VBE).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Under DOS this is the DPMI real-mode call (function 0300h) with a buffer in conventional memory. The native kernel has no BIOS
// once it is in protected mode; it can offer the same trait from a V86 monitor or a real-mode trampoline, or not offer it and
// leave the VBE layer unused (docs/HAL.md, "Open questions").

use crate::HalError;

/// The registers of a real-mode call. Segment registers other than `es` and `ds` are the platform's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BiosRegs {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
    pub es: u16,
    pub ds: u16,
    /// The flags after the call (bit 0 is the carry flag).
    pub flags: u16,
}

/// The size of the buffer a platform guarantees to supply: a VBE information block is 512 bytes.
pub const BIOS_BUFFER_MAX: usize = 512;

pub trait Bios {
    /// Runs software interrupt `vector` in real mode with `regs`, and stores the registers it returns in `regs`.
    ///
    /// If `buffer` is given (at most `BIOS_BUFFER_MAX` bytes; longer is `InvalidArgument`), the platform copies it to a block of
    /// memory below 1 MiB, points `es:di` at it (`regs.es` and `regs.edi` are overwritten), makes the call and copies the block
    /// back, so a function that reads and writes a structure (VBE 4F00h wants "VBE2" in the first four bytes) sees both ways. The
    /// block stays valid, and unchanged by the platform, until the next call: that is what lets `read_low` follow the far pointers
    /// a VBE information block holds into it.
    fn int(&self, vector: u8, regs: &mut BiosRegs, buffer: Option<&mut [u8]>) -> Result<(), HalError>;

    /// Reads `out.len()` bytes from the real-mode address `segment:offset` (the first megabyte), for the strings and mode lists a
    /// BIOS leaves in its own ROM or in the transfer block. `InvalidArgument` if the range leaves the first megabyte.
    fn read_low(&self, segment: u16, offset: u16, out: &mut [u8]) -> Result<(), HalError>;
}
