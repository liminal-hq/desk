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

    /// Copies `src` into the region at `offset`, for frame buffers only. `write_bytes` is the byte-exact copy; this is the fast one.
    ///
    /// The contract: each destination byte is written exactly once, in ascending order, by aligned stores up to 32 bits wide (single
    /// bytes up to the next 4-byte boundary of the destination address, then whole 32-bit words, then single bytes for the tail),
    /// and nothing is read from the region. A device that only cares what lands in its memory (a frame buffer) cannot tell it from
    /// `write_bytes`; a register window, where the width of an access matters, must use `write_bytes` or `write8`/`write32`.
    /// `InvalidArgument` if `offset + src.len()` is past the end of the region (nothing is written); an empty copy at the very end
    /// is accepted.
    ///
    /// On x86 the word part is one `rep movsd`. That relies on ES equalling DS and the direction flag being clear. The DOS runtime
    /// satisfies both (`_start` executes `cld` and its interrupt stub sets ES and executes `cld`; `dpmi::set_limit` reloads DS and ES
    /// with the same selector), and so does the kernel (`_start` and both interrupt stubs execute `cld`; `gdt::init` loads DS and ES
    /// with the same flat selector). Other targets use volatile 32-bit stores in the same order.
    pub fn copy_from(&mut self, offset: usize, src: &[u8]) -> Result<(), HalError> {
        match offset.checked_add(src.len()) {
            Some(end) if end <= self.len => {}
            _ => return Err(HalError::InvalidArgument),
        }
        let end = offset + src.len();
        let mut i = offset;
        let mut j = 0;
        while i < end && !(self.base as usize).wrapping_add(i).is_multiple_of(4) {
            // SAFETY: `i` is inside the region (checked above).
            unsafe { self.base.add(i).write_volatile(src[j]) };
            i += 1;
            j += 1;
        }
        let words = (end - i) / 4;
        if words > 0 {
            #[cfg(target_arch = "x86")]
            // SAFETY: `words * 4` bytes are inside the region at `base + i` (aligned to 4) and inside `src` at `j` (both checked
            // above); the direction flag is clear and ES equals DS (see above). ESI is saved by hand because the compiler may not
            // be given it. No `pure` or `nomem`, so the stores are never removed or merged.
            unsafe {
                core::arch::asm!(
                    "push esi", "mov esi, {s}", "rep movsd", "pop esi",
                    s = in(reg) src.as_ptr().add(j),
                    inout("edi") self.base.add(i) => _,
                    inout("ecx") words => _,
                    options(preserves_flags),
                );
            }
            #[cfg(not(target_arch = "x86"))]
            for w in 0..words {
                let b = &src[j + w * 4..j + w * 4 + 4];
                let word = u32::from_ne_bytes([b[0], b[1], b[2], b[3]]);
                // SAFETY: inside the region (checked above) and at an address that is a multiple of 4 (the loop above stopped on
                // one, and each step is 4).
                unsafe { (self.base.add(i + w * 4) as *mut u32).write_volatile(word) };
            }
            i += words * 4;
            j += words * 4;
        }
        while i < end {
            // SAFETY: as the first loop.
            unsafe { self.base.add(i).write_volatile(src[j]) };
            i += 1;
            j += 1;
        }
        Ok(())
    }

    /// Sets `len` bytes from `offset` to `value`, with one bounds check for the whole run. The bytes before the first 4-byte-aligned
    /// address and after the last whole word are written one at a time and the aligned middle a word at a time, so a fill costs a
    /// quarter of the bus writes of `len` single-byte stores. On x86 the word part is one `rep stosd` (the same assumptions as `copy_from`); elsewhere every store is volatile.
    pub fn fill8(&mut self, offset: usize, len: usize, value: u8) -> Result<(), HalError> {
        match offset.checked_add(len) {
            Some(end) if end <= self.len => {}
            _ => return Err(HalError::InvalidArgument),
        }
        let end = offset + len;
        let mut i = offset;
        let word = u32::from_ne_bytes([value; 4]);
        while i < end && !(self.base as usize).wrapping_add(i).is_multiple_of(4) {
            // SAFETY: `i` is inside the region (checked above).
            unsafe { self.base.add(i).write_volatile(value) };
            i += 1;
        }
        #[cfg(target_arch = "x86")]
        {
            let words = (end - i) / 4;
            if words > 0 {
                // SAFETY: `words * 4` bytes are inside the region at `base + i`, which is a multiple of 4 (checked above); the
                // direction flag is clear and ES equals DS (see `copy_from`). No `pure` or `nomem`, so the stores are kept.
                unsafe {
                    core::arch::asm!(
                        "rep stosd",
                        inout("edi") self.base.add(i) => _,
                        inout("ecx") words => _,
                        in("eax") word,
                        options(preserves_flags),
                    );
                }
                i += words * 4;
            }
        }
        #[cfg(not(target_arch = "x86"))]
        while end - i >= 4 {
            // SAFETY: inside the region (checked above) and at an address that is a multiple of 4 (the loop above stopped on one,
            // and each step is 4).
            unsafe { (self.base.add(i) as *mut u32).write_volatile(word) };
            i += 4;
        }
        while i < end {
            // SAFETY: as the first loop.
            unsafe { self.base.add(i).write_volatile(value) };
            i += 1;
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

    #[test]
    fn fill_covers_unaligned_heads_and_tails_and_stays_inside() {
        let mut backing = [0u32; 8];
        for offset in 0..9 {
            for len in 0..20 {
                backing.fill(0x1111_1111);
                {
                    // SAFETY: the array outlives the region and is not otherwise used while it exists.
                    let mut r = unsafe { MmioRegion::new(backing.as_mut_ptr() as *mut u8, 32) };
                    r.fill8(offset, len, 0xAB).unwrap();
                }
                let bytes: Vec<u8> = backing.iter().flat_map(|w| w.to_ne_bytes()).collect();
                for (i, b) in bytes.iter().enumerate() {
                    let want = if (offset..offset + len).contains(&i) { 0xAB } else { 0x11 };
                    assert_eq!(*b, want, "offset {offset}, len {len}, byte {i}");
                }
            }
        }
        // SAFETY: as above.
        let mut r = unsafe { MmioRegion::new(backing.as_mut_ptr() as *mut u8, 32) };
        assert_eq!(r.fill8(30, 3, 0), Err(HalError::InvalidArgument));
        assert_eq!(r.fill8(usize::MAX, 2, 0), Err(HalError::InvalidArgument));
        assert_eq!(r.fill8(32, 0, 0), Ok(()));
    }

    /// Copies `len` bytes of a known pattern at `offset` into a 64-byte region that starts `shift` bytes into the backing, with 16
    /// guard bytes on each side, and checks every byte of the backing.
    fn check_copy(shift: usize, offset: usize, len: usize) {
        let src: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(7).wrapping_add(1)).collect();
        let mut backing = [0x5A5A_5A5Au32; 24];
        {
            // SAFETY: the array outlives the region and is not otherwise used while it exists.
            let mut r = unsafe { MmioRegion::new((backing.as_mut_ptr() as *mut u8).add(16 + shift), 64) };
            r.copy_from(offset, &src).unwrap();
        }
        let bytes: Vec<u8> = backing.iter().flat_map(|w| w.to_ne_bytes()).collect();
        for (i, b) in bytes.iter().enumerate() {
            let at = i as isize - (16 + shift + offset) as isize;
            let want = if (0..len as isize).contains(&at) {
                src[at as usize]
            } else {
                0x5A
            };
            assert_eq!(*b, want, "shift {shift}, offset {offset}, len {len}, byte {i}");
        }
    }

    #[test]
    fn copy_from_matches_a_byte_loop_and_stays_inside() {
        for shift in 0..4 {
            for offset in 0..8 {
                for len in (0..40).chain([47, 56]) {
                    check_copy(shift, offset, len);
                }
            }
        }
        check_copy(0, 0, 64);
        check_copy(3, 0, 64);
    }

    #[test]
    fn copy_from_refuses_out_of_bounds_and_writes_nothing() {
        let mut backing = [0x5A5A_5A5Au32; 8];
        {
            // SAFETY: as above.
            let mut r = unsafe { MmioRegion::new(backing.as_mut_ptr() as *mut u8, 32) };
            assert_eq!(r.copy_from(30, &[1, 2, 3]), Err(HalError::InvalidArgument));
            assert_eq!(r.copy_from(0, &[1; 33]), Err(HalError::InvalidArgument));
            assert_eq!(r.copy_from(usize::MAX, &[1, 2]), Err(HalError::InvalidArgument));
            assert_eq!(r.copy_from(33, &[]), Err(HalError::InvalidArgument));
            // An empty copy at the very end, and a copy that exactly fills the region, are accepted.
            assert_eq!(r.copy_from(32, &[]), Ok(()));
            assert_eq!(r.copy_from(29, &[9, 9, 9]), Ok(()));
        }
        let bytes: Vec<u8> = backing.iter().flat_map(|w| w.to_ne_bytes()).collect();
        assert!(bytes[..29].iter().all(|b| *b == 0x5A));
        assert_eq!(&bytes[29..], &[9, 9, 9]);
    }
}
