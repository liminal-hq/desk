// The 96-byte header: its constants, its fields and the checks that need nothing but the header (docs/LOADER.md).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::error::FormatError;

pub const MAGIC: [u8; 4] = *b"DMD1";
pub const HEADER_SIZE: u16 = 96;
/// The address every shipped file is linked for.
pub const LINK_BASE: u32 = 0x0040_0000;
/// i486, flat 32-bit near pointers, cdecl, x87 present.
pub const ABI_I486_FLAT: u16 = 1;
/// Loaded into the caller's address space (`.DMD`).
pub const KIND_MODULE: u16 = 1;
/// Becomes a new task (`.DKX`).
pub const KIND_APP: u16 = 2;
/// Reserved: refused in version 1.
pub const KIND_LIBRARY: u16 = 3;
pub const MAX_MEMORY: u32 = 16 << 20;
pub const MAX_IMPORTS: u32 = 256;
/// The longest import name, in bytes.
pub const MAX_NAME: usize = 31;

/// Byte offsets of the header fields, shared by the reader and the writer.
pub(crate) mod at {
    pub const MAGIC: usize = 0;
    pub const HEADER_SIZE: usize = 4;
    pub const FLAGS: usize = 6;
    pub const ABI: usize = 8;
    pub const SDK: usize = 10;
    pub const KIND: usize = 12;
    pub const RESERVED_A: usize = 14;
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
    pub const RESERVED_B: usize = 88;
}

/// The fields of the header, as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub header_size: u16,
    pub flags: u16,
    pub abi: u16,
    pub sdk: u16,
    pub kind: u16,
    pub link_base: u32,
    pub align: u32,
    pub image_offset: u32,
    pub file_size: u32,
    pub mem_size: u32,
    pub ro_size: u32,
    pub start: u32,
    pub event: u32,
    pub stop: u32,
    pub reloc_offset: u32,
    pub reloc_count: u32,
    pub import_offset: u32,
    pub import_count: u32,
    /// Reserved: an empty table in version 1.
    pub export_offset: u32,
    pub export_count: u32,
    pub strings_offset: u32,
    pub strings_size: u32,
    pub crc: u32,
}

/// The little-endian word at `o`, or `None` when it is not entirely inside `b`.
pub(crate) fn get16(b: &[u8], o: usize) -> Option<u16> {
    let s = b.get(o..o.checked_add(2)?)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

pub(crate) fn get32(b: &[u8], o: usize) -> Option<u32> {
    let s = b.get(o..o.checked_add(4)?)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// `get32` for reads the layout checks have already put in bounds. An out-of-bounds read here is a bug in this crate, not a bad
/// file: it fails a debug build loudly, and a release build reads 0, which the checks behind it (the CRC, the table checks) do not
/// let through as a valid file.
pub(crate) fn rd32(b: &[u8], o: usize) -> u32 {
    get32(b, o).unwrap_or_else(|| {
        debug_assert!(false, "read of 4 bytes at {o} is outside a {}-byte slice", b.len());
        0
    })
}

/// Checks every rule that needs only the header's own fields, in the order of the specification, and returns the fields.
/// `core_sdk` is the newest service table the caller provides.
pub fn parse_header(bytes: &[u8], core_sdk: u16) -> Result<Header, FormatError> {
    // The magic comes first, so a short file that is plainly another version or not a module says so.
    let Some(magic) = bytes.get(0..4) else {
        return Err(FormatError::TooShort { len: bytes.len() });
    };
    if magic[0..3] == MAGIC[0..3] && magic[3] != MAGIC[3] && magic[3].is_ascii_digit() {
        return Err(FormatError::UnsupportedVersion(magic[3] - b'0'));
    }
    if magic != MAGIC {
        return Err(FormatError::BadMagic);
    }
    if bytes.len() < HEADER_SIZE as usize {
        return Err(FormatError::TooShort { len: bytes.len() });
    }
    let too_short = FormatError::TooShort { len: bytes.len() };
    let r16 = |o: usize| get16(bytes, o).ok_or_else(|| too_short.clone());
    let r32 = |o: usize| get32(bytes, o).ok_or_else(|| too_short.clone());
    let h = Header {
        header_size: r16(at::HEADER_SIZE)?,
        flags: r16(at::FLAGS)?,
        abi: r16(at::ABI)?,
        sdk: r16(at::SDK)?,
        kind: r16(at::KIND)?,
        link_base: r32(at::LINK_BASE)?,
        align: r32(at::ALIGN)?,
        image_offset: r32(at::IMAGE_OFFSET)?,
        file_size: r32(at::FILE_SIZE)?,
        mem_size: r32(at::MEM_SIZE)?,
        ro_size: r32(at::RO_SIZE)?,
        start: r32(at::START)?,
        event: r32(at::EVENT)?,
        stop: r32(at::STOP)?,
        reloc_offset: r32(at::RELOC_OFFSET)?,
        reloc_count: r32(at::RELOC_COUNT)?,
        import_offset: r32(at::IMPORT_OFFSET)?,
        import_count: r32(at::IMPORT_COUNT)?,
        export_offset: r32(at::EXPORT_OFFSET)?,
        export_count: r32(at::EXPORT_COUNT)?,
        strings_offset: r32(at::STRINGS_OFFSET)?,
        strings_size: r32(at::STRINGS_SIZE)?,
        crc: r32(at::CRC)?,
    };
    if h.header_size < HEADER_SIZE || !h.header_size.is_multiple_of(4) {
        return Err(FormatError::BadHeaderSize(h.header_size));
    }
    if h.flags != 0 {
        return Err(FormatError::UnknownFlags(h.flags));
    }
    if r16(at::RESERVED_A)? != 0 {
        return Err(FormatError::ReservedNotZero {
            offset: at::RESERVED_A as u32,
        });
    }
    if bytes[at::RESERVED_B..HEADER_SIZE as usize].iter().any(|&b| b != 0) {
        return Err(FormatError::ReservedNotZero {
            offset: at::RESERVED_B as u32,
        });
    }
    if h.abi != ABI_I486_FLAT {
        return Err(FormatError::UnsupportedAbi(h.abi));
    }
    if h.sdk > core_sdk {
        return Err(FormatError::SdkTooNew {
            needs: h.sdk,
            have: core_sdk,
        });
    }
    match h.kind {
        KIND_MODULE | KIND_APP => {}
        KIND_LIBRARY => return Err(FormatError::KindNotSupported(h.kind)),
        other => return Err(FormatError::UnknownKind(other)),
    }
    if h.export_count != 0 {
        return Err(FormatError::ExportsNotSupported { count: h.export_count });
    }
    if h.align < 16 || h.align > 4096 || !h.align.is_power_of_two() {
        return Err(FormatError::BadAlignment(h.align));
    }
    if h.link_base == 0 || !h.link_base.is_multiple_of(4096) {
        return Err(FormatError::BadLinkBase(h.link_base));
    }
    if h.file_size > h.mem_size || h.ro_size > h.file_size {
        return Err(FormatError::SizesOutOfOrder);
    }
    if h.mem_size > MAX_MEMORY {
        return Err(FormatError::TooLarge(h.mem_size));
    }
    // The image, with its `.bss` tail, must fit below 4 GiB at its link base, so the relocated words never wrap.
    if h.link_base as u64 + h.mem_size as u64 > u32::MAX as u64 {
        return Err(FormatError::BadLinkBase(h.link_base));
    }
    if h.import_count > MAX_IMPORTS {
        return Err(FormatError::TooManyImports(h.import_count));
    }
    // The export table is empty and sits at the end of the import table; the import table's end cannot overflow 64 bits.
    let imports_end = h.import_offset as u64 + h.import_count as u64 * 8;
    if h.export_offset as u64 != imports_end {
        return Err(FormatError::ExportTableMisplaced { offset: h.export_offset });
    }
    Ok(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_outside_the_slice_are_none() {
        let b = [1u8, 2, 3, 4, 5];
        assert_eq!(get32(&b, 0), Some(0x0403_0201));
        assert_eq!(get32(&b, 1), Some(0x0504_0302));
        assert_eq!(get32(&b, 2), None);
        assert_eq!(get32(&b, usize::MAX), None);
        assert_eq!(get16(&b, 3), Some(0x0504));
        assert_eq!(get16(&b, 4), None);
        assert_eq!(get16(&b, usize::MAX - 1), None);
    }

    // The failure path of `rd32` is reachable only through a bug in this crate; a debug build says so.
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "outside a 3-byte slice")]
    fn an_unchecked_read_outside_the_slice_fails_a_debug_build() {
        rd32(&[1, 2, 3], 0);
    }
}
