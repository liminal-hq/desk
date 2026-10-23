// A memory-mapped region: a bounds-checked, volatile view of device memory such as the VGA window or a linear frame buffer.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The platform makes the region (natively by identity mapping, under DPMI from the first megabyte or with function 0800h for a
// linear frame buffer) and that is the `unsafe` step. Everything a driver does with it afterwards is checked against its length.

use crate::HalError;

/// A window of device memory. Accesses are volatile: the compiler neither removes nor merges them.
#[derive(Debug)]
pub struct MmioRegion {
    base: *mut u8,
    len: usize,
}

// SAFETY: the region is a range of device memory, not Rust data; it may move to the context that owns the device.
unsafe impl Send for MmioRegion {}

impl MmioRegion {
    /// # Safety
    ///
    /// `base..base + len` must be mapped, readable and writable device or ordinary memory for as long as the region lives, and no
    /// Rust reference may cover it while it does.
    pub const unsafe fn new(base: *mut u8, len: usize) -> MmioRegion {
        MmioRegion { base, len }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn check(&self, offset: usize, size: usize) -> Result<(), HalError> {
        match offset.checked_add(size) {
            Some(end) if end <= self.len && (self.base as usize).wrapping_add(offset).is_multiple_of(size) => Ok(()),
            _ => Err(HalError::InvalidArgument),
        }
    }

    pub fn read8(&self, offset: usize) -> Result<u8, HalError> {
        self.check(offset, 1)?;
        // SAFETY: inside the region (checked), which `new`'s contract makes valid.
        Ok(unsafe { self.base.add(offset).read_volatile() })
    }

    pub fn write8(&mut self, offset: usize, value: u8) -> Result<(), HalError> {
        self.check(offset, 1)?;
        // SAFETY: as `read8`.
        unsafe { self.base.add(offset).write_volatile(value) };
        Ok(())
    }

    /// A 32-bit access; the address must be a multiple of 4.
    pub fn read32(&self, offset: usize) -> Result<u32, HalError> {
        self.check(offset, 4)?;
        // SAFETY: inside the region and at an address that is a multiple of 4 (both checked).
        Ok(unsafe { (self.base.add(offset) as *const u32).read_volatile() })
    }

    pub fn write32(&mut self, offset: usize, value: u32) -> Result<(), HalError> {
        self.check(offset, 4)?;
        // SAFETY: as `read32`.
        unsafe { (self.base.add(offset) as *mut u32).write_volatile(value) };
        Ok(())
    }

    /// Copies `bytes` in at `offset`, for blits. One volatile byte store each: correct for any device memory, and the reference
    /// to beat when a platform adds a faster `rep movsd` path for frame buffers.
    pub fn write_bytes(&mut self, offset: usize, bytes: &[u8]) -> Result<(), HalError> {
        match offset.checked_add(bytes.len()) {
            Some(end) if end <= self.len => {}
            _ => return Err(HalError::InvalidArgument),
        }
        for (i, b) in bytes.iter().enumerate() {
            // SAFETY: `offset + i` is inside the region (checked above).
            unsafe { self.base.add(offset + i).write_volatile(*b) };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accesses_are_bounded_and_aligned() {
        let mut backing = [0u32; 4];
        {
            // SAFETY: the array outlives the region and is not otherwise used while it exists.
            let mut r = unsafe { MmioRegion::new(backing.as_mut_ptr() as *mut u8, 16) };
            r.write32(12, 0xA0B0_C0D0).unwrap();
            r.write8(0, 0x13).unwrap();
            r.write_bytes(4, &[1, 2, 3]).unwrap();
            assert_eq!(r.read32(12), Ok(0xA0B0_C0D0));
            assert_eq!(r.read8(5), Ok(2));
            assert_eq!(r.read32(16), Err(HalError::InvalidArgument));
            assert_eq!(r.read32(2), Err(HalError::InvalidArgument));
            assert_eq!(r.write_bytes(14, &[0; 3]), Err(HalError::InvalidArgument));
            assert_eq!(r.read8(usize::MAX), Err(HalError::InvalidArgument));
        }
        assert_eq!(backing[0] & 0xFF, 0x13);
    }
}
