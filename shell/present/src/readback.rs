// The capture read-back: the frame buffer as palette indices, for the DOS dumps. Nothing else reads video memory.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_hal::{HalError, MmioRegion, PortIo};
use desk_vga::{Canvas, Linear8, Planar16};

/// Reads the visible `width × height` indices of an 8-bit frame buffer into `out` (row-major, `width` per row), through
/// `Linear8::read_row`. `InvalidArgument` for bad geometry or an `out` shorter than `width × height`.
pub fn read_back_linear(region: &mut MmioRegion, width: u32, height: u32, pitch: u32, out: &mut [u8]) -> Result<(), HalError> {
    let mut canvas = Linear8::new(region, width, height, pitch)?;
    read_rows(&mut canvas, width, height, out)
}

/// As `read_back_linear` for mode 12h (`pitch` is bytes per plane row), through `Planar16::read_row`. The controller's
/// set/reset is off again when it returns.
pub fn read_back_planar<P: PortIo>(
    ports: &P,
    region: &mut MmioRegion,
    width: u32,
    height: u32,
    pitch: u32,
    out: &mut [u8],
) -> Result<(), HalError> {
    let mut canvas = Planar16::new(ports, region, width, height, pitch)?;
    read_rows(&mut canvas, width, height, out)
}

fn read_rows<C: Canvas>(canvas: &mut C, width: u32, height: u32, out: &mut [u8]) -> Result<(), HalError> {
    let w = width as usize;
    if (out.len() as u64) < u64::from(width) * u64::from(height) {
        return Err(HalError::InvalidArgument);
    }
    for y in 0..height {
        let start = y as usize * w;
        canvas.read_row(y, &mut out[start..start + w])?;
    }
    Ok(())
}
