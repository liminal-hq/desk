// Packing: a linked ELF file becomes a `.DMD` or `.DKX` through the shared format crate, `info` describes a file, and `cross` compares
// two links of one module at different bases.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use std::fmt;

use desk_module::{load_into, Builder, FormatError, Header, Module, ABI_I486_FLAT, KIND_APP, KIND_LIBRARY, KIND_MODULE};

use crate::elf::{read, ElfError};

/// The alignment every packed image asks for: enough for any scalar the compiler lays out, and every platform's allocator gives it.
pub const ALIGN: u32 = 16;
/// The SDK version a packed image asks for when `--sdk` does not say.
pub const DEFAULT_SDK: u16 = 1;

/// Why a file was not packed.
#[derive(Debug, PartialEq, Eq)]
pub enum PackError {
    /// The ELF reader refused the linker's output.
    Elf(ElfError),
    /// The container refused what the reader extracted (the packed file is parsed again before it is returned).
    Format(FormatError),
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackError::Elf(e) => write!(f, "{e}"),
            PackError::Format(e) => write!(f, "the packed file is not a valid image: {e}"),
        }
    }
}

impl From<ElfError> for PackError {
    fn from(e: ElfError) -> PackError {
        PackError::Elf(e)
    }
}

impl From<FormatError> for PackError {
    fn from(e: FormatError) -> PackError {
        PackError::Format(e)
    }
}

/// Packs a linked ELF file: reads it strictly, lays the container out and parses the result as a loader would.
pub fn pack(elf: &[u8], kind: u16, link_base: u32, sdk: u16) -> Result<Vec<u8>, PackError> {
    let x = read(elf, link_base, kind)?;
    let file = Builder {
        sdk,
        kind,
        align: ALIGN,
        link_base: x.link_base,
        image: x.image,
        mem_size: x.mem_size,
        ro_size: x.ro_size,
        start: x.start,
        event: x.event.unwrap_or(0),
        stop: x.stop.unwrap_or(0),
        relocs: x.relocs,
        imports: x.imports,
    }
    .write()?;
    Module::parse(&file, sdk)?;
    Ok(file)
}

/// The name of a kind as `info` and the magic file print it.
pub fn kind_name(kind: u16) -> String {
    match kind {
        KIND_MODULE => "module (.DMD)".into(),
        KIND_APP => "application (.DKX)".into(),
        KIND_LIBRARY => "library (reserved)".into(),
        other => format!("unknown kind {other}"),
    }
}

/// One line per header field, then the import names: what `dmdpack info` prints. The file is parsed with every check but the SDK
/// version, so a file that a loader would refuse is described only if it is well formed.
pub fn info(bytes: &[u8]) -> Result<String, FormatError> {
    let m = Module::parse(bytes, u16::MAX)?;
    let h: &Header = m.header();
    let mut out = String::new();
    let mut line = |name: &str, value: String| out.push_str(&format!("{name:<14}{value}\n"));
    line("kind", kind_name(h.kind));
    line(
        "abi",
        if h.abi == ABI_I486_FLAT {
            "i486 flat".into()
        } else {
            format!("unknown {}", h.abi)
        },
    );
    line("sdk", h.sdk.to_string());
    line("header size", h.header_size.to_string());
    line("flags", format!("{:#x}", h.flags));
    line("link base", format!("{:#010x}", h.link_base));
    line("alignment", h.align.to_string());
    line("file size", bytes.len().to_string());
    line("image bytes", h.file_size.to_string());
    line("memory bytes", h.mem_size.to_string());
    line("read-only", h.ro_size.to_string());
    line("start", format!("{:#x}", h.start));
    line("event", format!("{:#x}", h.event));
    line("stop", format!("{:#x}", h.stop));
    line("relocations", h.reloc_count.to_string());
    line("imports", h.import_count.to_string());
    line("exports", h.export_count.to_string());
    line("strings", h.strings_size.to_string());
    line("crc", format!("{:#010x}", h.crc));
    for i in m.imports() {
        out.push_str(&format!("  import {} at {:#x}\n", i.name, i.slot));
    }
    Ok(out)
}

/// Why two links of one module are not the same module at two bases.
#[derive(Debug, PartialEq, Eq)]
pub enum CrossError {
    /// One of the two files is not a valid image.
    Format { which: char, error: FormatError },
    /// The first file could not be loaded at the second's link base.
    Load(String),
    /// A header field that does not depend on the base differs.
    Layout { field: &'static str, a: u32, b: u32 },
    /// The first file, loaded at the second's link base, differs from the second at this image offset.
    ImageDiffers { offset: u32, a: u8, b: u8 },
    /// The two relocation tables differ at this index (a missing entry is `None`).
    RelocsDiffer { index: usize, a: Option<u32>, b: Option<u32> },
    /// The two import tables differ at this index.
    ImportsDiffer { index: usize },
}

impl fmt::Display for CrossError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CrossError::Format { which, error } => write!(f, "file {which} is not a valid image: {error}"),
            CrossError::Load(e) => write!(f, "the first file cannot be loaded at the second's link base: {e}"),
            CrossError::Layout { field, a, b } => write!(f, "the {field} differs: {a:#x} against {b:#x}"),
            CrossError::ImageDiffers { offset, a, b } => write!(
                f,
                "the first file loaded at the second's link base differs from the second at image offset {offset:#x}: {a:#04x} against {b:#04x}"
            ),
            CrossError::RelocsDiffer { index, a, b } => {
                let show = |v: &Option<u32>| v.map_or("none".to_string(), |v| format!("{v:#x}"));
                write!(f, "the relocation tables differ at entry {index}: {} against {}", show(a), show(b))
            }
            CrossError::ImportsDiffer { index } => write!(f, "the import tables differ at entry {index}"),
        }
    }
}

/// Proves relocation without running anything: `a` and `b` are one module linked at two bases. Loading `a` at `b`'s link base, with
/// every import bound to 0, must reproduce `b`'s image byte for byte (so every word `a` lists moved by the right delta and every
/// other byte was already the same), and the two relocation tables must be equal. A module linked without `--emit-relocs` has no
/// relocation sections, which the packer cannot tell from a module with no absolute words; this is what catches it.
pub fn cross(a: &[u8], b: &[u8]) -> Result<(), CrossError> {
    let ma = Module::parse(a, u16::MAX).map_err(|error| CrossError::Format { which: 'a', error })?;
    let mb = Module::parse(b, u16::MAX).map_err(|error| CrossError::Format { which: 'b', error })?;
    let (ha, hb) = (ma.header(), mb.header());
    let same: [(&'static str, u32, u32); 8] = [
        ("kind", ha.kind.into(), hb.kind.into()),
        ("image size", ha.file_size, hb.file_size),
        ("memory size", ha.mem_size, hb.mem_size),
        ("read-only size", ha.ro_size, hb.ro_size),
        ("start", ha.start, hb.start),
        ("event", ha.event, hb.event),
        ("stop", ha.stop, hb.stop),
        ("import count", ha.import_count, hb.import_count),
    ];
    for (field, x, y) in same {
        if x != y {
            return Err(CrossError::Layout { field, a: x, b: y });
        }
    }
    for (index, (x, y)) in ma.imports().zip(mb.imports()).enumerate() {
        if x != y {
            return Err(CrossError::ImportsDiffer { index });
        }
    }
    let mut dest = vec![0u8; ha.mem_size as usize];
    load_into(&ma, ha.kind, &mut dest, hb.link_base, &|_| Some(0)).map_err(|e| CrossError::Load(e.to_string()))?;
    for (offset, (x, y)) in dest.iter().zip(mb.image()).enumerate() {
        if x != y {
            return Err(CrossError::ImageDiffers {
                offset: offset as u32,
                a: *x,
                b: *y,
            });
        }
    }
    let (ra, rb): (Vec<u32>, Vec<u32>) = (ma.relocations().collect(), mb.relocations().collect());
    for index in 0..ra.len().max(rb.len()) {
        if ra.get(index) != rb.get(index) {
            return Err(CrossError::RelocsDiffer {
                index,
                a: ra.get(index).copied(),
                b: rb.get(index).copied(),
            });
        }
    }
    Ok(())
}
