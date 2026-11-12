// The errors of the format: one variant per check, each saying which number it concerns.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

/// A file that is not a valid image, found by `Module::parse` or refused by `Builder::write`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// Fewer bytes than a header.
    TooShort {
        len: usize,
    },
    /// Not a Desk image.
    BadMagic,
    /// `DMD` followed by another digit: a layout this reader does not know.
    UnsupportedVersion(u8),
    /// A header size under 96 or not a multiple of 4.
    BadHeaderSize(u16),
    /// A flag bit this reader does not know.
    UnknownFlags(u16),
    /// A reserved field that is not zero.
    ReservedNotZero {
        offset: u32,
    },
    UnsupportedAbi(u16),
    /// The file needs a newer service table than the core has.
    SdkTooNew {
        needs: u16,
        have: u16,
    },
    /// A kind that is neither 1, 2 nor 3.
    UnknownKind(u16),
    /// A kind this version reserves (a library).
    KindNotSupported(u16),
    /// A non-empty export table, which version 1 reserves.
    ExportsNotSupported {
        count: u32,
    },
    /// An export table that is not where the import table ends.
    ExportTableMisplaced {
        offset: u32,
    },
    /// An alignment that is not a power of two from 16 to 4096.
    BadAlignment(u32),
    /// A link base that is zero, not a multiple of 4096, or leaves no room for the memory size below 4 GiB.
    BadLinkBase(u32),
    /// A table (or the image) that does not fit in the file.
    TableOutsideFile {
        table: &'static str,
        offset: u32,
        len: u64,
    },
    /// A table that starts before the previous one ends.
    TablesOverlap {
        table: &'static str,
        offset: u32,
    },
    /// A table that starts later than the next multiple of 4 after the previous one's end.
    TableGap {
        table: &'static str,
        offset: u32,
        expected: u32,
    },
    /// A padding byte between two tables that is not zero.
    NonZeroPadding {
        at: u32,
    },
    /// Bytes after the end of the string table.
    TrailingBytes {
        at: u32,
    },
    /// File size above memory size, or read-only size above file size.
    SizesOutOfOrder,
    /// A memory size above the limit.
    TooLarge(u32),
    BadChecksum {
        stored: u32,
        computed: u32,
    },
    /// An entry point outside the read-only part, or one an application must not have.
    EntryOutOfRange {
        which: &'static str,
        offset: u32,
    },
    /// A relocation word that is not entirely inside the file-backed part of the image.
    RelocOutsideImage {
        index: u32,
        offset: u32,
    },
    /// A relocation that is not at least 4 above the one before it.
    RelocsOutOfOrder {
        index: u32,
    },
    /// A name that is empty, too long, uses other characters than `a` to `z`, `0` to `9` and `_`, or is not where the string table says.
    BadImportName {
        index: u32,
    },
    DuplicateImport {
        index: u32,
    },
    /// A slot that is unaligned, in the read-only part, outside the file, not zero, shared, or also a relocation.
    BadImportSlot {
        index: u32,
        slot: u32,
    },
    TooManyImports(u32),
    /// A string table with bytes that no import name uses.
    BadStringTable {
        expected: u32,
        size: u32,
    },
}

/// A parsed module that cannot be placed where it was asked to go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError<'a> {
    /// The file is of a kind the caller does not load (an application given to a module loader, or the reverse).
    KindNotLoadable {
        kind: u16,
        wanted: u16,
    },
    DestTooSmall {
        need: u32,
        have: usize,
    },
    DestMisaligned {
        addr: u32,
        align: u32,
    },
    /// The destination would run past the top of the 32-bit address space.
    DestWraps {
        addr: u32,
        need: u32,
    },
    /// A relocation word whose stored value lies outside the image.
    RelocTargetOutOfRange {
        index: u32,
        offset: u32,
        value: u32,
    },
    MissingImport {
        index: u32,
        name: &'a str,
    },
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::TooShort { len } => write!(f, "too short: {len} bytes is less than a header"),
            FormatError::BadMagic => write!(f, "not a Desk module: bad magic"),
            FormatError::UnsupportedVersion(v) => write!(f, "unsupported format version {v}"),
            FormatError::BadHeaderSize(n) => write!(f, "bad header size {n}"),
            FormatError::UnknownFlags(n) => write!(f, "unknown flags {n:#06x}"),
            FormatError::ReservedNotZero { offset } => write!(f, "reserved field at header offset {offset} is not zero"),
            FormatError::UnsupportedAbi(n) => write!(f, "unsupported ABI {n}"),
            FormatError::SdkTooNew { needs, have } => write!(f, "needs SDK {needs}, the core has {have}"),
            FormatError::UnknownKind(n) => write!(f, "unknown kind {n}"),
            FormatError::KindNotSupported(n) => write!(f, "kind {n} is not supported in version 1"),
            FormatError::ExportsNotSupported { count } => write!(f, "exports are not supported in version 1 ({count} listed)"),
            FormatError::ExportTableMisplaced { offset } => write!(f, "export table at {offset} is not where the imports end"),
            FormatError::BadAlignment(n) => write!(f, "bad alignment {n}"),
            FormatError::BadLinkBase(n) => write!(f, "bad link base {n:#010x}"),
            FormatError::TableOutsideFile { table, offset, len } => {
                write!(f, "the {table} ({len} bytes at {offset}) do not fit in the file")
            }
            FormatError::TablesOverlap { table, offset } => {
                write!(f, "the {table} at {offset} start before the previous table ends")
            }
            FormatError::TableGap { table, offset, expected } => {
                write!(f, "the {table} start at {offset}, expected {expected}")
            }
            FormatError::NonZeroPadding { at } => write!(f, "padding byte at {at} is not zero"),
            FormatError::TrailingBytes { at } => write!(f, "trailing bytes after offset {at}"),
            FormatError::SizesOutOfOrder => write!(f, "sizes out of order"),
            FormatError::TooLarge(n) => write!(f, "too large: {n} bytes"),
            FormatError::BadChecksum { stored, computed } => {
                write!(f, "bad checksum: stored {stored:#010x}, computed {computed:#010x}")
            }
            FormatError::EntryOutOfRange { which, offset } => write!(f, "the {which} entry {offset:#x} is out of range"),
            FormatError::RelocOutsideImage { index, offset } => {
                write!(f, "relocation {index} at {offset:#x} is outside the image")
            }
            FormatError::RelocsOutOfOrder { index } => write!(f, "relocation {index} is out of order or overlaps the one before"),
            FormatError::BadImportName { index } => write!(f, "import {index} has a bad name"),
            FormatError::DuplicateImport { index } => write!(f, "import {index} repeats an earlier name"),
            FormatError::BadImportSlot { index, slot } => write!(f, "import {index} has a bad slot {slot:#x}"),
            FormatError::TooManyImports(n) => write!(f, "too many imports: {n}"),
            FormatError::BadStringTable { expected, size } => {
                write!(f, "string table is {size} bytes, the names fill {expected}")
            }
        }
    }
}

impl fmt::Display for LoadError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::KindNotLoadable { kind, wanted } => {
                write!(f, "kind {kind} is not loadable here (this loader takes kind {wanted})")
            }
            LoadError::DestTooSmall { need, have } => write!(f, "destination too small: needs {need} bytes, has {have}"),
            LoadError::DestMisaligned { addr, align } => write!(f, "destination {addr:#x} is not a multiple of {align}"),
            LoadError::DestWraps { addr, need } => write!(f, "destination {addr:#x} plus {need} bytes passes the top of memory"),
            LoadError::RelocTargetOutOfRange { index, offset, value } => {
                write!(f, "relocation {index} at {offset:#x} holds {value:#x}, outside the image")
            }
            LoadError::MissingImport { index, name } => write!(f, "missing import {index}: {name}"),
        }
    }
}

impl core::error::Error for FormatError {}
impl core::error::Error for LoadError<'_> {}
