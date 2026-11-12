// Writing a file: the layout of the specification, with the checksum filled in. Used by the packer and by the tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::string::String;
use alloc::vec::Vec;

use crate::crc::crc32;
use crate::error::FormatError;
use crate::header::{at, ABI_I486_FLAT, HEADER_SIZE, KIND_MODULE, LINK_BASE, MAGIC};

/// The parts of a file, before layout. Fields are the header's, offsets are image offsets, and `relocs` are given sorted.
///
/// `write` lays the pieces out and checksums them but does not judge them: a builder holding a bad name, an unsorted table or a
/// slot in the read-only part writes that file, so the tests can make damaged files from it. `Module::parse` is the judge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Builder {
    pub sdk: u16,
    pub kind: u16,
    pub align: u32,
    pub link_base: u32,
    /// The bytes stored in the file; its length is the file size.
    pub image: Vec<u8>,
    pub mem_size: u32,
    pub ro_size: u32,
    pub start: u32,
    pub event: u32,
    pub stop: u32,
    pub relocs: Vec<u32>,
    /// Name and image offset of the slot, in file order.
    pub imports: Vec<(String, u32)>,
}

impl Default for Builder {
    /// An empty module for SDK 1, linked at `LINK_BASE`, aligned to 16.
    fn default() -> Builder {
        Builder {
            sdk: 1,
            kind: KIND_MODULE,
            align: 16,
            link_base: LINK_BASE,
            image: Vec::new(),
            mem_size: 0,
            ro_size: 0,
            start: 0,
            event: 0,
            stop: 0,
            relocs: Vec::new(),
            imports: Vec::new(),
        }
    }
}

fn put16(out: &mut [u8], o: usize, v: u16) {
    out[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

fn put32(out: &mut [u8], o: usize, v: u32) {
    out[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

fn pad4(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn len32(n: usize) -> Result<u32, FormatError> {
    u32::try_from(n).map_err(|_| FormatError::TooLarge(u32::MAX))
}

impl Builder {
    /// Lays the file out in the order of the specification, sorts nothing (the caller gives sorted relocations), fills in the CRC.
    /// Fails only when the parts cannot be represented: a size past 32 bits, or a name holding a NUL.
    pub fn write(&self) -> Result<Vec<u8>, FormatError> {
        let file_size = len32(self.image.len())?;
        let reloc_count = len32(self.relocs.len())?;
        let import_count = len32(self.imports.len())?;
        let mut names: Vec<u8> = Vec::new();
        let mut name_offsets: Vec<u32> = Vec::new();
        for (i, (name, _)) in self.imports.iter().enumerate() {
            if name.as_bytes().contains(&0) {
                return Err(FormatError::BadImportName { index: i as u32 });
            }
            name_offsets.push(len32(names.len())?);
            names.extend_from_slice(name.as_bytes());
            names.push(0);
        }

        let mut out = alloc::vec![0u8; HEADER_SIZE as usize];
        let image_offset = len32(out.len())?;
        out.extend_from_slice(&self.image);
        pad4(&mut out);
        let reloc_offset = len32(out.len())?;
        for r in &self.relocs {
            out.extend_from_slice(&r.to_le_bytes());
        }
        let import_offset = len32(out.len())?;
        for ((_, slot), name_offset) in self.imports.iter().zip(&name_offsets) {
            out.extend_from_slice(&name_offset.to_le_bytes());
            out.extend_from_slice(&slot.to_le_bytes());
        }
        let export_offset = len32(out.len())?;
        let strings_offset = export_offset;
        out.extend_from_slice(&names);
        len32(out.len())?;

        out[at::MAGIC..at::MAGIC + 4].copy_from_slice(&MAGIC);
        put16(&mut out, at::HEADER_SIZE, HEADER_SIZE);
        put16(&mut out, at::ABI, ABI_I486_FLAT);
        put16(&mut out, at::SDK, self.sdk);
        put16(&mut out, at::KIND, self.kind);
        put32(&mut out, at::LINK_BASE, self.link_base);
        put32(&mut out, at::ALIGN, self.align);
        put32(&mut out, at::IMAGE_OFFSET, image_offset);
        put32(&mut out, at::FILE_SIZE, file_size);
        put32(&mut out, at::MEM_SIZE, self.mem_size);
        put32(&mut out, at::RO_SIZE, self.ro_size);
        put32(&mut out, at::START, self.start);
        put32(&mut out, at::EVENT, self.event);
        put32(&mut out, at::STOP, self.stop);
        put32(&mut out, at::RELOC_OFFSET, reloc_offset);
        put32(&mut out, at::RELOC_COUNT, reloc_count);
        put32(&mut out, at::IMPORT_OFFSET, import_offset);
        put32(&mut out, at::IMPORT_COUNT, import_count);
        put32(&mut out, at::EXPORT_OFFSET, export_offset);
        put32(&mut out, at::EXPORT_COUNT, 0);
        put32(&mut out, at::STRINGS_OFFSET, strings_offset);
        put32(&mut out, at::STRINGS_SIZE, len32(names.len())?);
        // The checksum field is zero while the CRC is taken.
        let crc = crc32(&out);
        put32(&mut out, at::CRC, crc);
        Ok(out)
    }
}
