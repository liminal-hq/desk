// Shared by the format tests: a valid module to damage, byte patching with the checksum refreshed, and a fixed-seed generator.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![allow(dead_code)]

use desk_module::{crc32, Builder, FormatError, Module, KIND_MODULE, LINK_BASE};

/// The newest service table the tests' core provides.
pub const CORE_SDK: u16 = 1;

/// Header offsets, written out here rather than imported so a moved field fails the tests.
pub mod off {
    pub const HEADER_SIZE: usize = 4;
    pub const FLAGS: usize = 6;
    pub const ABI: usize = 8;
    pub const SDK: usize = 10;
    pub const KIND: usize = 12;
    pub const LINK_BASE: usize = 16;
    pub const ALIGN: usize = 20;
    pub const IMAGE_OFFSET: usize = 24;
    pub const FILE_SIZE: usize = 28;
    pub const MEM_SIZE: usize = 32;
    pub const RO_SIZE: usize = 36;
    pub const START: usize = 40;
    pub const EVENT: usize = 44;
    pub const STOP: usize = 48;
    pub const RELOC_OFFSET: usize = 52;
    pub const RELOC_COUNT: usize = 56;
    pub const IMPORT_OFFSET: usize = 60;
    pub const IMPORT_COUNT: usize = 64;
    pub const EXPORT_OFFSET: usize = 68;
    pub const EXPORT_COUNT: usize = 72;
    pub const STRINGS_OFFSET: usize = 76;
    pub const STRINGS_SIZE: usize = 80;
    pub const CRC: usize = 84;
}

pub const FILE_SIZE: u32 = 0x80;
pub const MEM_SIZE: u32 = 0x100;
pub const RO_SIZE: u32 = 0x40;
pub const SLOT_LOG: u32 = 0x60;
pub const SLOT_ALLOC: u32 = 0x64;
pub const RELOCS: [u32; 5] = [0x44, 0x4C, 0x5C, 0x68, 0x70];

/// A valid module: code to 0x40, then data with five relocated words and two import slots, then a 0x80-byte `.bss` tail.
pub fn sample() -> Builder {
    let mut image: Vec<u8> = (0..FILE_SIZE)
        .map(|i| (i.wrapping_mul(7).wrapping_add(3) as u8) | 1)
        .collect();
    for slot in [SLOT_LOG, SLOT_ALLOC] {
        image[slot as usize..slot as usize + 4].fill(0);
    }
    // Pointers into the image: its start, the middle, the end of the file part, the end of the tail (one past), and the start again.
    let targets = [0x10, 0x80, FILE_SIZE, MEM_SIZE, 0];
    for (offset, target) in RELOCS.iter().zip(targets) {
        let o = *offset as usize;
        image[o..o + 4].copy_from_slice(&(LINK_BASE + target).to_le_bytes());
    }
    Builder {
        sdk: 1,
        kind: KIND_MODULE,
        align: 16,
        link_base: LINK_BASE,
        image,
        mem_size: MEM_SIZE,
        ro_size: RO_SIZE,
        start: 0x10,
        event: 0x20,
        stop: 0x30,
        relocs: RELOCS.to_vec(),
        imports: vec![("desk_log".to_string(), SLOT_LOG), ("desk_alloc".to_string(), SLOT_ALLOC)],
    }
}

/// A valid module with an image of 0x41 bytes (so three bytes of padding follow it), no relocations and no imports.
pub fn sample_odd() -> Builder {
    Builder {
        image: vec![0x90; 0x41],
        mem_size: 0x41,
        ro_size: 0x41,
        start: 0,
        event: 0,
        stop: 0,
        relocs: vec![],
        imports: vec![],
        ..sample()
    }
}

pub fn sample_bytes() -> Vec<u8> {
    sample().write().unwrap()
}

pub fn get32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

pub fn set16(b: &mut [u8], o: usize, v: u16) {
    b[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

pub fn set32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// Recomputes the checksum field, so a test about another check is not stopped by this one.
pub fn refresh_crc(b: &mut [u8]) {
    if b.len() >= off::CRC + 4 {
        b[off::CRC..off::CRC + 4].fill(0);
        let crc = crc32(b);
        set32(b, off::CRC, crc);
    }
}

/// The sample's bytes after `f`, with the checksum refreshed.
pub fn damaged(f: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut b = sample_bytes();
    f(&mut b);
    refresh_crc(&mut b);
    b
}

pub fn parse(bytes: &[u8]) -> Result<Module<'_>, FormatError> {
    Module::parse(bytes, CORE_SDK)
}

/// The error `bytes` is refused with; panics if it parses.
pub fn refusal(bytes: &[u8]) -> FormatError {
    match parse(bytes) {
        Ok(_) => panic!("expected a refusal, but the file parsed"),
        Err(e) => e,
    }
}

/// A fixed-seed xorshift generator: the tests have no randomness but this.
pub struct Rng(pub u32);

impl Rng {
    pub fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    pub fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}
