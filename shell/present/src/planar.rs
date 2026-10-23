// The planar presenter: VGA mode 12h, four bit planes at A0000h, written whole bytes at a time so nothing is ever read.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_gfx::Present;
use desk_hal::{HalError, MmioRegion, PortIo};
use desk_vga::regs::{gc, gc_write, seq, sequencer_write};

/// Bytes in one plane row at the widest surface the compositor accepts (1280 pixels).
pub const MAX_PLANE_BYTES: usize = 160;

/// Splits groups of eight palette indices into plane bytes: bit `p` of pixel `i` goes to bit `7 - i` of plane `p`'s byte
/// `i / 8`. Returns the number of bytes written to each plane, `pixels.len() / 8` (a trailing partial group and anything beyond
/// 160 bytes per plane is ignored). Pure.
pub fn planarise(pixels: &[u8], planes: &mut [[u8; MAX_PLANE_BYTES]; 4]) -> usize {
    let n = (pixels.len() / 8).min(MAX_PLANE_BYTES);
    for (b, group) in pixels.as_chunks::<8>().0.iter().take(n).enumerate() {
        let mut bytes = [0u8; 4];
        for (i, &px) in group.iter().enumerate() {
            let bit = 7 - i;
            for (p, byte) in bytes.iter_mut().enumerate() {
                *byte |= ((px >> p) & 1) << bit;
            }
        }
        for (p, byte) in bytes.iter().enumerate() {
            planes[p][b] = *byte;
        }
    }
    n
}

/// Where plane bytes go: the VGA (map mask, then the region), or four `Vec`s in a test.
pub trait PlaneSink {
    /// Once per frame, before any span: whatever the planes' writes rely on.
    fn begin(&mut self) -> Result<(), HalError> {
        Ok(())
    }
    /// Directs the writes that follow to one plane (0 to 3).
    fn select(&mut self, plane: u8) -> Result<(), HalError>;
    /// Writes `bytes` at byte `offset` of the selected plane.
    fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), HalError>;
    /// Once per frame, after the last span.
    fn end(&mut self) -> Result<(), HalError> {
        Ok(())
    }
}

/// The VGA's planes at A0000h: the sequencer map mask picks the plane, then a `write_bytes` into the region.
pub struct VgaPlanes<'a, P: PortIo> {
    ports: &'a P,
    region: &'a mut MmioRegion,
}

impl<'a, P: PortIo> VgaPlanes<'a, P> {
    /// `InvalidArgument` if the region is smaller than one plane (`pitch * height` bytes).
    pub fn new(ports: &'a P, region: &'a mut MmioRegion, pitch: u32, height: u32) -> Result<VgaPlanes<'a, P>, HalError> {
        if u64::from(pitch) * u64::from(height) > region.len() as u64 {
            return Err(HalError::InvalidArgument);
        }
        Ok(VgaPlanes { ports, region })
    }
}

impl<P: PortIo> PlaneSink for VgaPlanes<'_, P> {
    /// Sets, every frame, everything the spans rely on and trusts nothing from before: `desk_vga::Planar16` leaves set/reset on
    /// while it lives, so write mode 0, data rotate 0 with replace, enable set/reset 0 (the CPU byte is written) and bit mask
    /// `FFh` are written every time.
    fn begin(&mut self) -> Result<(), HalError> {
        gc_write(self.ports, gc::MODE, 0x00);
        gc_write(self.ports, gc::DATA_ROTATE, 0x00);
        gc_write(self.ports, gc::ENABLE_SET_RESET, 0x00);
        gc_write(self.ports, gc::BIT_MASK, 0xFF);
        Ok(())
    }

    fn select(&mut self, plane: u8) -> Result<(), HalError> {
        if plane > 3 {
            return Err(HalError::InvalidArgument);
        }
        sequencer_write(self.ports, seq::MAP_MASK, 1 << plane);
        Ok(())
    }

    fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), HalError> {
        self.region.write_bytes(offset, bytes)
    }

    /// Restores the map mask to all four planes, the state the BIOS and other drivers expect.
    fn end(&mut self) -> Result<(), HalError> {
        sequencer_write(self.ports, seq::MAP_MASK, 0x0F);
        Ok(())
    }
}

/// VGA mode 12h through any `PlaneSink`. Spans are 8-pixel aligned, so every byte written is whole.
pub struct PlanarPresent<S: PlaneSink> {
    sink: S,
    width: u32,
    height: u32,
    pitch: u32,
    planes: [[u8; MAX_PLANE_BYTES]; 4],
}

impl<'a, P: PortIo> PlanarPresent<VgaPlanes<'a, P>> {
    /// Mode 12h at A0000h. `pitch` is bytes per plane row (80 at 640 pixels). `InvalidArgument` for bad geometry (see
    /// `with_sink`) or a region smaller than one plane.
    pub fn new(
        ports: &'a P,
        region: &'a mut MmioRegion,
        width: u32,
        height: u32,
        pitch: u32,
    ) -> Result<PlanarPresent<VgaPlanes<'a, P>>, HalError> {
        PlanarPresent::with_sink(VgaPlanes::new(ports, region, pitch, height)?, width, height, pitch)
    }
}

impl<S: PlaneSink> PlanarPresent<S> {
    /// `InvalidArgument` unless `width` is a multiple of 8 and at most 1280 and a row of `pitch` bytes holds it.
    pub fn with_sink(sink: S, width: u32, height: u32, pitch: u32) -> Result<PlanarPresent<S>, HalError> {
        if !width.is_multiple_of(8) || width as usize > MAX_PLANE_BYTES * 8 || pitch < width / 8 {
            return Err(HalError::InvalidArgument);
        }
        Ok(PlanarPresent {
            sink,
            width,
            height,
            pitch,
            planes: [[0; MAX_PLANE_BYTES]; 4],
        })
    }

    /// The sink, for a test to look inside.
    pub fn sink(&self) -> &S {
        &self.sink
    }
}

impl<S: PlaneSink> Present for PlanarPresent<S> {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn align(&self) -> u32 {
        8
    }

    fn begin(&mut self) -> Result<(), HalError> {
        self.sink.begin()
    }

    /// Refuses (`InvalidArgument`) a span off the screen or whose start or length is not a multiple of 8.
    fn span(&mut self, x: u32, y: u32, pixels: &[u8]) -> Result<(), HalError> {
        if y >= self.height
            || u64::from(x) + pixels.len() as u64 > u64::from(self.width)
            || !x.is_multiple_of(8)
            || !pixels.len().is_multiple_of(8)
        {
            return Err(HalError::InvalidArgument);
        }
        let n = planarise(pixels, &mut self.planes);
        if n == 0 {
            return Ok(());
        }
        let offset = y as usize * self.pitch as usize + (x / 8) as usize;
        for p in 0..4u8 {
            self.sink.select(p)?;
            self.sink.write(offset, &self.planes[p as usize][..n])?;
        }
        Ok(())
    }

    fn end(&mut self) -> Result<(), HalError> {
        self.sink.end()
    }
}
