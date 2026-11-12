// CRC-32 (the IEEE polynomial, the same as `zlib.crc32` and `desk-gfx`'s `crc32`), over a file with its own field taken as zero.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const fn make_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}

static TABLE: [u32; 256] = make_table();

/// The CRC-32 of `bytes`.
pub fn crc32(bytes: &[u8]) -> u32 {
    crc32_update(0, bytes)
}

/// Continues a CRC-32: `crc32_update(crc32(a), b)` is the CRC of `a` followed by `b`.
pub(crate) fn crc32_update(crc: u32, bytes: &[u8]) -> u32 {
    let mut c = !crc;
    for &b in bytes {
        c = TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}
