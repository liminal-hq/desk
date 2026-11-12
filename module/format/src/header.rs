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

pub(crate) fn rd16(b: &[u8], o: usize) -> u16 {
    let mut a = [0u8; 2];
    if let Some(s) = o.checked_add(2).and_then(|e| b.get(o..e)) {
        a.copy_from_slice(s);
    }
    u16::from_le_bytes(a)
}

pub(crate) fn rd32(b: &[u8], o: usize) -> u32 {
    let mut a = [0u8; 4];
    if let Some(s) = o.checked_add(4).and_then(|e| b.get(o..e)) {
        a.copy_from_slice(s);
    }
    u32::from_le_bytes(a)
}

/// Checks every rule that needs only the header's own fields, in the order of the specification, and returns the fields.
/// `core_sdk` is the newest service table the caller provides.
pub fn parse_header(bytes: &[u8], core_sdk: u16) -> Result<Header, FormatError> {
    if bytes.len() < HEADER_SIZE as usize {
        return Err(FormatError::TooShort { len: bytes.len() });
    }
    if bytes[0..3] == MAGIC[0..3] && bytes[3] != MAGIC[3] && bytes[3].is_ascii_digit() {
        return Err(FormatError::UnsupportedVersion(bytes[3] - b'0'));
    }
    if bytes[0..4] != MAGIC {
        return Err(FormatError::BadMagic);
    }
    let h = Header {
        header_size: rd16(bytes, at::HEADER_SIZE),
        flags: rd16(bytes, at::FLAGS),
        abi: rd16(bytes, at::ABI),
        sdk: rd16(bytes, at::SDK),
        kind: rd16(bytes, at::KIND),
        link_base: rd32(bytes, at::LINK_BASE),
        align: rd32(bytes, at::ALIGN),
        image_offset: rd32(bytes, at::IMAGE_OFFSET),
        file_size: rd32(bytes, at::FILE_SIZE),
        mem_size: rd32(bytes, at::MEM_SIZE),
        ro_size: rd32(bytes, at::RO_SIZE),
        start: rd32(bytes, at::START),
        event: rd32(bytes, at::EVENT),
        stop: rd32(bytes, at::STOP),
        reloc_offset: rd32(bytes, at::RELOC_OFFSET),
        reloc_count: rd32(bytes, at::RELOC_COUNT),
        import_offset: rd32(bytes, at::IMPORT_OFFSET),
        import_count: rd32(bytes, at::IMPORT_COUNT),
        export_offset: rd32(bytes, at::EXPORT_OFFSET),
        export_count: rd32(bytes, at::EXPORT_COUNT),
        strings_offset: rd32(bytes, at::STRINGS_OFFSET),
        strings_size: rd32(bytes, at::STRINGS_SIZE),
        crc: rd32(bytes, at::CRC),
    };
    if h.header_size < HEADER_SIZE || !h.header_size.is_multiple_of(4) {
        return Err(FormatError::BadHeaderSize(h.header_size));
    }
    if h.flags != 0 {
        return Err(FormatError::UnknownFlags(h.flags));
    }
    if rd16(bytes, at::RESERVED_A) != 0 {
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
