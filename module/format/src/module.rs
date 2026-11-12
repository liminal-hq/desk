// A parsed file: the layout, the checksum, the entry points and every table entry checked, with the tables readable in place.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::crc::crc32_update;
use crate::error::FormatError;
use crate::header::{at, parse_header, rd32, Header, KIND_APP, MAX_NAME};

/// One import: the name the module asks for and the image offset of the word that receives its address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Import<'a> {
    pub name: &'a str,
    pub slot: u32,
}

/// A file that passed every check that needs no destination.
#[derive(Debug, Clone, Copy)]
pub struct Module<'a> {
    header: Header,
    bytes: &'a [u8],
}

fn to32(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn align4(n: u64) -> u64 {
    (n + 3) & !3
}

impl<'a> Module<'a> {
    /// Checks everything that can be checked without a destination: the header, the layout, the CRC and every table entry.
    /// `core_sdk` is the newest service table the caller provides. No input makes this panic.
    pub fn parse(bytes: &'a [u8], core_sdk: u16) -> Result<Module<'a>, FormatError> {
        let h = parse_header(bytes, core_sdk)?;
        check_layout(&h, bytes)?;
        let computed = checksum(bytes);
        if computed != h.crc {
            return Err(FormatError::BadChecksum { stored: h.crc, computed });
        }
        let m = Module { header: h, bytes };
        m.check_entries()?;
        m.check_relocations()?;
        m.check_imports()?;
        Ok(m)
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The image as stored: `file_size` bytes, without the `.bss` tail.
    pub fn image(&self) -> &'a [u8] {
        let start = self.header.image_offset as usize;
        &self.bytes[start..start + self.header.file_size as usize]
    }

    /// The image offsets of the words to relocate, in ascending order.
    pub fn relocations(&self) -> impl Iterator<Item = u32> + 'a {
        let (bytes, base) = (self.bytes, self.header.reloc_offset as usize);
        (0..self.header.reloc_count as usize).map(move |i| rd32(bytes, base + i * 4))
    }

    /// The imports in file order.
    pub fn imports(&self) -> impl Iterator<Item = Import<'a>> + 'a {
        let m = *self;
        (0..self.header.import_count).map(move |i| {
            let (name_offset, slot) = m.import_record(i);
            Import {
                name: m.name_at(name_offset).unwrap_or(""),
                slot,
            }
        })
    }

    fn reloc(&self, i: u32) -> u32 {
        rd32(self.bytes, self.header.reloc_offset as usize + i as usize * 4)
    }

    fn import_record(&self, i: u32) -> (u32, u32) {
        let o = self.header.import_offset as usize + i as usize * 8;
        (rd32(self.bytes, o), rd32(self.bytes, o + 4))
    }

    fn strings(&self) -> &'a [u8] {
        let start = self.header.strings_offset as usize;
        &self.bytes[start..start + self.header.strings_size as usize]
    }

    /// The NUL-terminated name at `offset` in the string table, if there is one.
    fn name_at(&self, offset: u32) -> Option<&'a str> {
        let rest = self.strings().get(offset as usize..)?;
        let len = rest.iter().position(|&b| b == 0)?;
        core::str::from_utf8(&rest[..len]).ok()
    }

    fn check_entries(&self) -> Result<(), FormatError> {
        let h = &self.header;
        if h.start >= h.ro_size {
            return Err(FormatError::EntryOutOfRange {
                which: "start",
                offset: h.start,
            });
        }
        for (which, offset) in [("event", h.event), ("stop", h.stop)] {
            // Zero means "none". An application has one entry, `start`.
            if offset != 0 && (offset >= h.ro_size || h.kind == KIND_APP) {
                return Err(FormatError::EntryOutOfRange { which, offset });
            }
        }
        Ok(())
    }

    fn check_relocations(&self) -> Result<(), FormatError> {
        let h = &self.header;
        let mut previous: Option<u32> = None;
        for index in 0..h.reloc_count {
            let offset = self.reloc(index);
            if offset as u64 + 4 > h.file_size as u64 {
                return Err(FormatError::RelocOutsideImage { index, offset });
            }
            if let Some(p) = previous {
                if offset < p || offset - p < 4 {
                    return Err(FormatError::RelocsOutOfOrder { index });
                }
            }
            previous = Some(offset);
        }
        Ok(())
    }

    fn check_imports(&self) -> Result<(), FormatError> {
        let h = &self.header;
        let image = self.image();
        let mut cursor = 0u32;
        for index in 0..h.import_count {
            let (name_offset, slot) = self.import_record(index);
            let bad_name = FormatError::BadImportName { index };
            // The names are packed in import order, each followed by a NUL.
            if name_offset != cursor {
                return Err(bad_name);
            }
            let name = self.name_at(name_offset).ok_or(FormatError::BadImportName { index })?;
            let valid = !name.is_empty()
                && name.len() <= MAX_NAME
                && name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
            if !valid {
                return Err(bad_name);
            }
            cursor = name_offset + name.len() as u32 + 1;
            if self.imports_before(index).any(|earlier| earlier.name == name) {
                return Err(FormatError::DuplicateImport { index });
            }
            let bad_slot = FormatError::BadImportSlot { index, slot };
            let slot_end = slot as u64 + 4;
            if !slot.is_multiple_of(4) || slot < h.ro_size || slot_end > h.file_size as u64 {
                return Err(bad_slot);
            }
            if image[slot as usize..slot as usize + 4] != [0, 0, 0, 0] {
                return Err(bad_slot);
            }
            if self.imports_before(index).any(|earlier| earlier.slot == slot) || self.relocation_touches(slot) {
                return Err(bad_slot);
            }
        }
        if cursor != h.strings_size {
            return Err(FormatError::BadStringTable {
                expected: cursor,
                size: h.strings_size,
            });
        }
        Ok(())
    }

    /// The imports before `index`, read without checking (they passed their own checks).
    fn imports_before(&self, index: u32) -> impl Iterator<Item = Import<'a>> + 'a {
        let m = *self;
        (0..index).map(move |i| {
            let (name_offset, slot) = m.import_record(i);
            Import {
                name: m.name_at(name_offset).unwrap_or(""),
                slot,
            }
        })
    }

    /// Whether any relocated word overlaps the four bytes at `slot` (the table is sorted, so this is a binary search).
    fn relocation_touches(&self, slot: u32) -> bool {
        // The first relocation that ends after the slot starts: offset + 4 > slot.
        let (mut lo, mut hi) = (0u32, self.header.reloc_count);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.reloc(mid) as u64 + 4 > slot as u64 {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        lo < self.header.reloc_count && (self.reloc(lo) as u64) < slot as u64 + 4
    }
}

/// The CRC of the file with the checksum field taken as zero.
fn checksum(bytes: &[u8]) -> u32 {
    let (before, rest) = bytes.split_at(at::CRC);
    let after = rest.get(4..).unwrap_or(&[]);
    let c = crc32_update(0, before);
    let c = crc32_update(c, &[0, 0, 0, 0]);
    crc32_update(c, after)
}

/// Checks that the image and the tables follow each other in the order of the specification, each at the next multiple of 4, with
/// zero padding between them, and that the file ends where the string table does.
fn check_layout(h: &Header, bytes: &[u8]) -> Result<(), FormatError> {
    let tables: [(&'static str, u32, u64); 5] = [
        ("image", h.image_offset, h.file_size as u64),
        ("relocations", h.reloc_offset, h.reloc_count as u64 * 4),
        ("imports", h.import_offset, h.import_count as u64 * 8),
        ("exports", h.export_offset, 0),
        ("strings", h.strings_offset, h.strings_size as u64),
    ];
    // The end of the previous table, and where the next one must start.
    let mut end = h.header_size as u64;
    for (table, offset, len) in tables {
        if offset as u64 + len > bytes.len() as u64 {
            return Err(FormatError::TableOutsideFile { table, offset, len });
        }
        if (offset as u64) < end {
            return Err(FormatError::TablesOverlap { table, offset });
        }
        let expected = align4(end);
        if offset as u64 != expected {
            return Err(FormatError::TableGap {
                table,
                offset,
                expected: to32(expected),
            });
        }
        if let Some(i) = bytes[end as usize..offset as usize].iter().position(|&b| b != 0) {
            return Err(FormatError::NonZeroPadding {
                at: to32(end + i as u64),
            });
        }
        end = offset as u64 + len;
    }
    if end < bytes.len() as u64 {
        return Err(FormatError::TrailingBytes { at: to32(end) });
    }
    Ok(())
}
