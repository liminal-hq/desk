// A strict ELF32 reader for linked Desk modules: the image from the program headers, the relocations, imports and entry points from the
// section headers and the symbol table, and a refusal, with the index or offset it concerns, for everything else.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The image comes from the program headers alone (the segments the linker laid out), and the section headers are read only for the
// relocation sections, the symbol table and the names of sections, so a linker that groups sections into segments differently from
// the test generator does not change what is read. Relocation sections are `REL`: the addend of an `R_386_32` is in the word, so the
// reader never adds a symbol's value; a loader adds the load delta to the word once. Every length and offset is checked against the
// file before it is used, and no check trusts an earlier one it did not make itself.
use std::collections::HashSet;
use std::fmt;

use crate::{KIND_APP, KIND_MODULE, MAX_IMPORTS, MAX_MEMORY, MAX_NAME};

const EHDR_SIZE: usize = 52;
const PHDR_SIZE: usize = 32;
const SHDR_SIZE: usize = 40;
const SYM_SIZE: usize = 16;
const REL_SIZE: usize = 8;

const PT_LOAD: u32 = 1;
const PT_TLS: u32 = 7;
const PF_W: u32 = 2;

const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const SHT_RELA: u32 = 4;
const SHT_NOBITS: u32 = 8;
const SHT_REL: u32 = 9;
const SHF_ALLOC: u32 = 2;
const SHF_TLS: u32 = 0x400;

const SHN_UNDEF: u16 = 0;
const SHN_ABS: u16 = 0xfff1;
const SHN_LORESERVE: u16 = 0xff00;
const STT_OBJECT: u8 = 1;

const R_386_NONE: u32 = 0;
const R_386_32: u32 = 1;
const R_386_PC32: u32 = 2;

const IMPORT_PREFIX: &str = "__desk_imp_";
const IMPORT_SECTION: &str = ".desk.imp";

/// Everything a `.DMD` or `.DKX` needs from the linked ELF file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extract {
    pub link_base: u32,
    /// The file-backed bytes of the loadable segments at their offsets from the link base, gaps filled with zeros.
    pub image: Vec<u8>,
    /// The image plus the `.bss` tail, from the link base.
    pub mem_size: u32,
    /// The extent of the read-only segments from the link base.
    pub ro_size: u32,
    /// Image offsets of the `R_386_32` words, ascending and unique, at least four apart.
    pub relocs: Vec<u32>,
    /// The `__desk_imp_<name>` slots in `.desk.imp` as (name, image offset of the slot), in ascending slot order.
    pub imports: Vec<(String, u32)>,
    /// Image offset of `app_start` (a module) or `desk_main` (an application).
    pub start: u32,
    /// Image offset of `app_event`, for a module that has one.
    pub event: Option<u32>,
    /// Image offset of `app_stop`, for a module that has one.
    pub stop: Option<u32>,
}

/// One refusal per case in docs/LOADER.md, "Building a module", each with the index (a program header, a section, a symbol or a
/// relocation) or the offset or address it concerns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElfError {
    UnsupportedKind { kind: u16 },
    NotElf,
    HeaderTruncated { len: usize },
    Not32Bit { class: u8 },
    NotLittleEndian { data: u8 },
    BadVersion { version: u32 },
    NotExecutable { e_type: u16 },
    NotI386 { machine: u16 },
    BadProgramHeaderSize { size: u16 },
    ProgramHeadersOutsideFile { offset: u32, count: u16 },
    BadSectionHeaderSize { size: u16 },
    SectionHeadersOutsideFile { offset: u32, count: u16 },
    NoLoadSegment,
    SegmentOutsideFile { index: usize, offset: u32, size: u32 },
    SegmentFileLargerThanMemory { index: usize, file_size: u32, mem_size: u32 },
    SegmentWraps { index: usize, vaddr: u32, mem_size: u32 },
    SegmentOutOfOrder { index: usize, vaddr: u32, previous: u32 },
    SegmentOverlap { index: usize, vaddr: u32, previous_end: u32 },
    BssNotAtEnd { index: usize },
    WritableBeforeReadOnly { index: usize },
    WrongLinkBase { found: u32, wanted: u32 },
    TooLarge { mem_size: u64 },
    ThreadLocalSegment { index: usize },
    ThreadLocalSection { index: usize },
    ThreadLocalRelocation { section: usize, index: usize, r_type: u32 },
    SectionOutsideFile { index: usize, offset: u32, size: u32 },
    BadSectionLink { index: usize },
    BadEntrySize { index: usize, size: u32 },
    BadString { index: usize, offset: u32 },
    RelaSection { index: usize },
    BadRelocationType { section: usize, index: usize, r_type: u32 },
    BadSymbolIndex { section: usize, index: usize, symbol: u32 },
    RelocationAgainstUndefined { section: usize, index: usize, symbol: u32 },
    RelocationAgainstAbsolute { section: usize, index: usize, symbol: u32 },
    RelocationAgainstReserved { section: usize, index: usize, symbol: u32 },
    RelocationOutsideImage { section: usize, index: usize, address: u32 },
    RelocationsOverlap { offset: u32 },
    RelocationAtImportSlot { offset: u32 },
    ImportNotObject { symbol: usize },
    ImportBadSize { name: String, size: u32 },
    ImportWrongSection { name: String, section: String },
    ImportBadName { symbol: usize },
    ImportDuplicate { name: String },
    ImportMisaligned { name: String, address: u32 },
    ImportOutsideImage { name: String, address: u32 },
    ImportInReadOnly { name: String, offset: u32 },
    ImportNotZero { name: String },
    ImportSlotShared { name: String, other: String },
    TooManyImports { count: usize },
    MissingEntry { name: &'static str },
    EntryOutsideReadOnly { name: &'static str, offset: u32 },
    ApplicationHasModuleEntry { name: &'static str },
}

impl fmt::Display for ElfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ElfError::*;
        match self {
            UnsupportedKind { kind } => write!(f, "kind {kind} cannot be packed (1 is a module, 2 is an application)"),
            NotElf => write!(f, "not an ELF file"),
            HeaderTruncated { len } => write!(f, "the file is {len} bytes, shorter than an ELF32 header"),
            Not32Bit { class } => write!(f, "not a 32-bit ELF file (class {class})"),
            NotLittleEndian { data } => write!(f, "not a little-endian ELF file (data encoding {data})"),
            BadVersion { version } => write!(f, "unsupported ELF version {version}"),
            NotExecutable { e_type } => write!(
                f,
                "not a fixed-address executable (ET_EXEC), type {e_type}; link with -static and no PIE"
            ),
            NotI386 { machine } => write!(f, "not an Intel 80386 ELF file (machine {machine})"),
            BadProgramHeaderSize { size } => write!(f, "program header entries are {size} bytes, not {PHDR_SIZE}"),
            ProgramHeadersOutsideFile { offset, count } => {
                write!(f, "{count} program headers at offset {offset:#x} lie outside the file")
            }
            BadSectionHeaderSize { size } => write!(f, "section header entries are {size} bytes, not {SHDR_SIZE}"),
            SectionHeadersOutsideFile { offset, count } => {
                write!(f, "{count} section headers at offset {offset:#x} lie outside the file")
            }
            NoLoadSegment => write!(f, "no loadable segment"),
            SegmentOutsideFile { index, offset, size } => {
                write!(
                    f,
                    "segment {index}: {size:#x} file bytes at offset {offset:#x} run past the end of the file"
                )
            }
            SegmentFileLargerThanMemory {
                index,
                file_size,
                mem_size,
            } => {
                write!(
                    f,
                    "segment {index}: file size {file_size:#x} exceeds memory size {mem_size:#x}"
                )
            }
            SegmentWraps { index, vaddr, mem_size } => write!(
                f,
                "segment {index}: {mem_size:#x} bytes at {vaddr:#x} wrap past the address space"
            ),
            SegmentOutOfOrder { index, vaddr, previous } => {
                write!(
                    f,
                    "segment {index} at {vaddr:#x} comes before the previous segment's start {previous:#x}"
                )
            }
            SegmentOverlap {
                index,
                vaddr,
                previous_end,
            } => {
                write!(
                    f,
                    "segment {index} at {vaddr:#x} overlaps the previous segment, which ends at {previous_end:#x}"
                )
            }
            BssNotAtEnd { index } => write!(f, "segment {index} has a zero-filled tail but is not the last segment"),
            WritableBeforeReadOnly { index } => write!(f, "segment {index} is read-only but follows a writable segment"),
            WrongLinkBase { found, wanted } => write!(f, "linked at {found:#x}, not at {wanted:#x}"),
            TooLarge { mem_size } => write!(f, "the image needs {mem_size:#x} bytes of memory, above the limit"),
            ThreadLocalSegment { index } => write!(f, "segment {index} is thread-local storage, which version 1 does not have"),
            ThreadLocalSection { index } => write!(f, "section {index} is thread-local data, which version 1 does not have"),
            ThreadLocalRelocation { section, index, r_type } => {
                write!(f, "relocation {index} of section {section} is a thread-local type ({r_type})")
            }
            SectionOutsideFile { index, offset, size } => write!(
                f,
                "section {index}: {size:#x} bytes at offset {offset:#x} lie outside the file"
            ),
            BadSectionLink { index } => write!(
                f,
                "section {index} links to a section that does not exist or is the wrong kind"
            ),
            BadEntrySize { index, size } => write!(f, "section {index}: entry size {size} is wrong"),
            BadString { index, offset } => write!(f, "section {index}: no string ends at offset {offset:#x}"),
            RelaSection { index } => write!(
                f,
                "section {index} holds relocations with explicit addends (RELA); only REL is read"
            ),
            BadRelocationType { section, index, r_type } => {
                write!(f, "relocation {index} of section {section} has unsupported type {r_type} (only R_386_32, R_386_PC32 and R_386_NONE)")
            }
            BadSymbolIndex { section, index, symbol } => {
                write!(
                    f,
                    "relocation {index} of section {section} names symbol {symbol}, which does not exist"
                )
            }
            RelocationAgainstUndefined { section, index, symbol } => {
                write!(
                    f,
                    "relocation {index} of section {section}: R_386_32 against undefined symbol {symbol}"
                )
            }
            RelocationAgainstAbsolute { section, index, symbol } => {
                write!(
                    f,
                    "relocation {index} of section {section}: R_386_32 against absolute symbol {symbol}"
                )
            }
            RelocationAgainstReserved { section, index, symbol } => {
                write!(
                    f,
                    "relocation {index} of section {section}: R_386_32 against symbol {symbol} in a reserved section"
                )
            }
            RelocationOutsideImage { section, index, address } => {
                write!(
                    f,
                    "relocation {index} of section {section}: the word at {address:#x} is not in the image"
                )
            }
            RelocationsOverlap { offset } => {
                write!(f, "the relocated word at image offset {offset:#x} overlaps the one before it")
            }
            RelocationAtImportSlot { offset } => {
                write!(f, "image offset {offset:#x} is both an import slot and a relocated word")
            }
            ImportNotObject { symbol } => write!(f, "import symbol {symbol} is not a data object"),
            ImportBadSize { name, size } => write!(f, "import {name} is {size} bytes, not 4"),
            ImportWrongSection { name, section } => write!(f, "import {name} is in {section}, not {IMPORT_SECTION}"),
            ImportBadName { symbol } => {
                write!(
                    f,
                    "import symbol {symbol}: the name is not 1 to {MAX_NAME} bytes of a to z, 0 to 9 and underscore"
                )
            }
            ImportDuplicate { name } => write!(f, "import {name} is defined twice"),
            ImportMisaligned { name, address } => write!(f, "import {name} is at {address:#x}, not 4-aligned"),
            ImportOutsideImage { name, address } => write!(f, "import {name} at {address:#x} is not in the file-backed image"),
            ImportInReadOnly { name, offset } => write!(f, "import {name} at image offset {offset:#x} is in the read-only part"),
            ImportNotZero { name } => write!(f, "import {name}: the slot is not zero in the file"),
            ImportSlotShared { name, other } => write!(f, "imports {name} and {other} share a slot"),
            TooManyImports { count } => write!(f, "{count} imports, above the limit of {MAX_IMPORTS}"),
            MissingEntry { name } => write!(f, "no {name} entry point"),
            EntryOutsideReadOnly { name, offset } => {
                write!(f, "{name} at image offset {offset:#x} is outside the read-only part")
            }
            ApplicationHasModuleEntry { name } => write!(f, "an application must not define {name}"),
        }
    }
}

impl std::error::Error for ElfError {}

fn rd16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn rd32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Whether `size` bytes at `offset` lie inside a file of `len` bytes.
fn inside(len: usize, offset: u32, size: u64) -> bool {
    u64::from(offset) + size <= len as u64
}

struct Load {
    index: usize,
    vaddr: u32,
    offset: u32,
    file_size: u32,
    mem_size: u32,
    writable: bool,
}

struct Section {
    name: String,
    sh_type: u32,
    flags: u32,
    offset: u32,
    size: u32,
    link: u32,
    info: u32,
    entsize: u32,
}

struct Symbol {
    name: String,
    value: u32,
    size: u32,
    info: u8,
    shndx: u16,
}

/// A NUL-terminated string at `offset` in the string table `table`, or `None` if it is out of range, unterminated or not UTF-8.
fn string_at(table: &[u8], offset: u32) -> Option<&str> {
    let rest = table.get(offset as usize..)?;
    let end = rest.iter().position(|&c| c == 0)?;
    std::str::from_utf8(&rest[..end]).ok()
}

fn valid_import_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

fn is_tls_relocation(r_type: u32) -> bool {
    matches!(r_type, 14..=19 | 24..=37 | 39..=41)
}

/// Reads the linked ELF32 file `bytes` for an image of `kind` (`KIND_MODULE` or `KIND_APP`) linked at `link_base`.
pub fn read(bytes: &[u8], link_base: u32, kind: u16) -> Result<Extract, ElfError> {
    if kind != KIND_MODULE && kind != KIND_APP {
        return Err(ElfError::UnsupportedKind { kind });
    }
    let len = bytes.len();
    if len < 16 || bytes[..4] != *b"\x7fELF" {
        return Err(ElfError::NotElf);
    }
    if bytes[4] != 1 {
        return Err(ElfError::Not32Bit { class: bytes[4] });
    }
    if bytes[5] != 1 {
        return Err(ElfError::NotLittleEndian { data: bytes[5] });
    }
    if len < EHDR_SIZE {
        return Err(ElfError::HeaderTruncated { len });
    }
    let version = rd32(bytes, 20);
    if bytes[6] != 1 || version != 1 {
        return Err(ElfError::BadVersion {
            version: if bytes[6] != 1 { u32::from(bytes[6]) } else { version },
        });
    }
    let e_type = rd16(bytes, 16);
    if e_type != 2 {
        return Err(ElfError::NotExecutable { e_type });
    }
    let machine = rd16(bytes, 18);
    if machine != 3 {
        return Err(ElfError::NotI386 { machine });
    }
    let phoff = rd32(bytes, 28);
    let shoff = rd32(bytes, 32);
    let phentsize = rd16(bytes, 42);
    let phnum = rd16(bytes, 44);
    let shentsize = rd16(bytes, 46);
    let shnum = rd16(bytes, 48);
    let shstrndx = rd16(bytes, 50);

    if phnum > 0 {
        if usize::from(phentsize) != PHDR_SIZE {
            return Err(ElfError::BadProgramHeaderSize { size: phentsize });
        }
        if !inside(len, phoff, u64::from(phnum) * PHDR_SIZE as u64) {
            return Err(ElfError::ProgramHeadersOutsideFile {
                offset: phoff,
                count: phnum,
            });
        }
    }
    if shnum > 0 {
        if usize::from(shentsize) != SHDR_SIZE {
            return Err(ElfError::BadSectionHeaderSize { size: shentsize });
        }
        if !inside(len, shoff, u64::from(shnum) * SHDR_SIZE as u64) {
            return Err(ElfError::SectionHeadersOutsideFile {
                offset: shoff,
                count: shnum,
            });
        }
    }

    let loads = read_loads(bytes, phoff, usize::from(phnum))?;
    let first = loads.first().ok_or(ElfError::NoLoadSegment)?;
    if first.vaddr != link_base {
        return Err(ElfError::WrongLinkBase {
            found: first.vaddr,
            wanted: link_base,
        });
    }

    // The image: every file-backed byte at its offset from the link base. Segments are ascending and disjoint here.
    let end_of = |l: &Load, size: u32| u64::from(l.vaddr) + u64::from(size);
    let file_end = loads.iter().map(|l| end_of(l, l.file_size)).max().unwrap_or(0);
    let mem_end = loads.iter().map(|l| end_of(l, l.mem_size)).max().unwrap_or(0);
    let mem_size = mem_end - u64::from(link_base);
    if mem_size > u64::from(MAX_MEMORY) {
        return Err(ElfError::TooLarge { mem_size });
    }
    let ro_end = loads
        .iter()
        .filter(|l| !l.writable)
        .map(|l| end_of(l, l.mem_size))
        .max()
        .unwrap_or(u64::from(link_base));
    let mut image = vec![0u8; (file_end - u64::from(link_base)) as usize];
    for l in &loads {
        let at = (l.vaddr - link_base) as usize;
        let from = l.offset as usize;
        image[at..at + l.file_size as usize].copy_from_slice(&bytes[from..from + l.file_size as usize]);
    }
    let ro_size = (ro_end - u64::from(link_base)) as u32;
    let file_backed = |address: u32, size: u32| {
        loads
            .iter()
            .any(|l| u64::from(address) >= u64::from(l.vaddr) && u64::from(address) + u64::from(size) <= end_of(l, l.file_size))
    };

    let sections = read_sections(bytes, shoff, usize::from(shnum), usize::from(shstrndx))?;

    // Symbols: the imports and the entry points.
    let mut symbols: Vec<Symbol> = Vec::new();
    let mut symtab_index = None;
    if let Some(si) = sections.iter().position(|s| s.sh_type == SHT_SYMTAB) {
        symtab_index = Some(si);
        symbols = read_symbols(bytes, &sections, si)?;
    }

    let mut imports: Vec<(String, u32)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut slots: Vec<(u32, String)> = Vec::new();
    for (i, sym) in symbols.iter().enumerate() {
        let Some(name) = sym.name.strip_prefix(IMPORT_PREFIX) else {
            continue;
        };
        if sym.info & 0xf != STT_OBJECT {
            return Err(ElfError::ImportNotObject { symbol: i });
        }
        if !valid_import_name(name) {
            return Err(ElfError::ImportBadName { symbol: i });
        }
        if sym.size != 4 {
            return Err(ElfError::ImportBadSize {
                name: name.into(),
                size: sym.size,
            });
        }
        let section = sections
            .get(usize::from(sym.shndx))
            .filter(|_| sym.shndx != SHN_UNDEF && sym.shndx < SHN_LORESERVE);
        match section {
            Some(s) if s.name == IMPORT_SECTION => {}
            Some(s) => {
                return Err(ElfError::ImportWrongSection {
                    name: name.into(),
                    section: s.name.clone(),
                })
            }
            None => {
                return Err(ElfError::ImportWrongSection {
                    name: name.into(),
                    section: "(none)".into(),
                })
            }
        }
        if !seen.insert(name.to_string()) {
            return Err(ElfError::ImportDuplicate { name: name.into() });
        }
        if sym.value % 4 != 0 {
            return Err(ElfError::ImportMisaligned {
                name: name.into(),
                address: sym.value,
            });
        }
        if !file_backed(sym.value, 4) {
            return Err(ElfError::ImportOutsideImage {
                name: name.into(),
                address: sym.value,
            });
        }
        let offset = sym.value - link_base;
        if offset < ro_size {
            return Err(ElfError::ImportInReadOnly {
                name: name.into(),
                offset,
            });
        }
        if image[offset as usize..offset as usize + 4] != [0; 4] {
            return Err(ElfError::ImportNotZero { name: name.into() });
        }
        if let Some((_, other)) = slots.iter().find(|(o, _)| *o == offset) {
            return Err(ElfError::ImportSlotShared {
                name: name.into(),
                other: other.clone(),
            });
        }
        slots.push((offset, name.to_string()));
        imports.push((name.to_string(), offset));
    }
    if imports.len() > MAX_IMPORTS {
        return Err(ElfError::TooManyImports { count: imports.len() });
    }
    imports.sort_by_key(|(_, slot)| *slot);

    let entry = |name: &'static str| -> Result<Option<u32>, ElfError> {
        let Some(sym) = symbols
            .iter()
            .find(|s| s.name == name && s.shndx != SHN_UNDEF && s.shndx < SHN_LORESERVE)
        else {
            return Ok(None);
        };
        let offset = sym.value.wrapping_sub(link_base);
        if sym.value < link_base || offset >= ro_size {
            return Err(ElfError::EntryOutsideReadOnly { name, offset });
        }
        Ok(Some(offset))
    };
    let (start, event, stop) = if kind == KIND_MODULE {
        (
            entry("app_start")?.ok_or(ElfError::MissingEntry { name: "app_start" })?,
            entry("app_event")?,
            entry("app_stop")?,
        )
    } else {
        for name in ["app_event", "app_stop"] {
            if entry(name)?.is_some() {
                return Err(ElfError::ApplicationHasModuleEntry { name });
            }
        }
        (
            entry("desk_main")?.ok_or(ElfError::MissingEntry { name: "desk_main" })?,
            None,
            None,
        )
    };

    // Relocations: the words the loader adjusts.
    let mut relocs: Vec<u32> = Vec::new();
    for (si, sec) in sections.iter().enumerate() {
        if sec.sh_type != SHT_REL && sec.sh_type != SHT_RELA {
            continue;
        }
        let Some(target) = sections.get(sec.info as usize) else {
            return Err(ElfError::BadSectionLink { index: si });
        };
        if target.flags & SHF_ALLOC == 0 {
            // Debug information and the like: the loader never sees these sections.
            continue;
        }
        if sec.sh_type == SHT_RELA {
            return Err(ElfError::RelaSection { index: si });
        }
        if symtab_index != Some(sec.link as usize) {
            return Err(ElfError::BadSectionLink { index: si });
        }
        if sec.entsize as usize != REL_SIZE || !(sec.size as usize).is_multiple_of(REL_SIZE) {
            return Err(ElfError::BadEntrySize {
                index: si,
                size: sec.entsize,
            });
        }
        if !inside(len, sec.offset, u64::from(sec.size)) {
            return Err(ElfError::SectionOutsideFile {
                index: si,
                offset: sec.offset,
                size: sec.size,
            });
        }
        let data = &bytes[sec.offset as usize..(sec.offset + sec.size) as usize];
        for (ri, rel) in data.as_chunks::<REL_SIZE>().0.iter().enumerate() {
            let address = rd32(rel, 0);
            let info = rd32(rel, 4);
            let (r_type, symbol) = (info & 0xff, info >> 8);
            match r_type {
                R_386_NONE | R_386_PC32 => continue,
                R_386_32 => {}
                t if is_tls_relocation(t) => {
                    return Err(ElfError::ThreadLocalRelocation {
                        section: si,
                        index: ri,
                        r_type,
                    })
                }
                _ => {
                    return Err(ElfError::BadRelocationType {
                        section: si,
                        index: ri,
                        r_type,
                    })
                }
            }
            if symbol == 0 {
                return Err(ElfError::RelocationAgainstUndefined {
                    section: si,
                    index: ri,
                    symbol,
                });
            }
            let Some(sym) = symbols.get(symbol as usize) else {
                return Err(ElfError::BadSymbolIndex {
                    section: si,
                    index: ri,
                    symbol,
                });
            };
            match sym.shndx {
                SHN_UNDEF => {
                    return Err(ElfError::RelocationAgainstUndefined {
                        section: si,
                        index: ri,
                        symbol,
                    })
                }
                SHN_ABS => {
                    return Err(ElfError::RelocationAgainstAbsolute {
                        section: si,
                        index: ri,
                        symbol,
                    })
                }
                s if s >= SHN_LORESERVE => {
                    return Err(ElfError::RelocationAgainstReserved {
                        section: si,
                        index: ri,
                        symbol,
                    })
                }
                _ => {}
            }
            if !file_backed(address, 4) {
                return Err(ElfError::RelocationOutsideImage {
                    section: si,
                    index: ri,
                    address,
                });
            }
            relocs.push(address - link_base);
        }
    }
    relocs.sort_unstable();
    relocs.dedup();
    if let Some(w) = relocs.windows(2).find(|w| w[1] - w[0] < 4) {
        return Err(ElfError::RelocationsOverlap { offset: w[1] });
    }
    if let Some(r) = relocs.iter().find(|r| imports.iter().any(|(_, slot)| slot == *r)) {
        return Err(ElfError::RelocationAtImportSlot { offset: *r });
    }

    Ok(Extract {
        link_base,
        image,
        mem_size: mem_size as u32,
        ro_size,
        relocs,
        imports,
        start,
        event,
        stop,
    })
}

/// The loadable segments in program-header order, each checked on its own and against the one before it.
fn read_loads(bytes: &[u8], phoff: u32, phnum: usize) -> Result<Vec<Load>, ElfError> {
    let mut loads: Vec<Load> = Vec::new();
    for index in 0..phnum {
        let at = phoff as usize + index * PHDR_SIZE;
        let p_type = rd32(bytes, at);
        if p_type == PT_TLS {
            return Err(ElfError::ThreadLocalSegment { index });
        }
        if p_type != PT_LOAD {
            continue;
        }
        let (offset, vaddr, file_size, mem_size, flags) = (
            rd32(bytes, at + 4),
            rd32(bytes, at + 8),
            rd32(bytes, at + 16),
            rd32(bytes, at + 20),
            rd32(bytes, at + 24),
        );
        if file_size > mem_size {
            return Err(ElfError::SegmentFileLargerThanMemory {
                index,
                file_size,
                mem_size,
            });
        }
        if !inside(bytes.len(), offset, u64::from(file_size)) {
            return Err(ElfError::SegmentOutsideFile {
                index,
                offset,
                size: file_size,
            });
        }
        if u64::from(vaddr) + u64::from(mem_size) > u64::from(u32::MAX) {
            return Err(ElfError::SegmentWraps { index, vaddr, mem_size });
        }
        if mem_size == 0 {
            continue;
        }
        let load = Load {
            index,
            vaddr,
            offset,
            file_size,
            mem_size,
            writable: flags & PF_W != 0,
        };
        if let Some(prev) = loads.last() {
            if load.vaddr < prev.vaddr {
                return Err(ElfError::SegmentOutOfOrder {
                    index,
                    vaddr,
                    previous: prev.vaddr,
                });
            }
            let prev_end = prev.vaddr + prev.mem_size;
            if load.vaddr < prev_end {
                return Err(ElfError::SegmentOverlap {
                    index,
                    vaddr,
                    previous_end: prev_end,
                });
            }
            if prev.file_size < prev.mem_size {
                return Err(ElfError::BssNotAtEnd { index: prev.index });
            }
            if prev.writable && !load.writable {
                return Err(ElfError::WritableBeforeReadOnly { index });
            }
        }
        loads.push(load);
    }
    Ok(loads)
}

/// The section headers with their names. A section's contents are not read here.
fn read_sections(bytes: &[u8], shoff: u32, shnum: usize, shstrndx: usize) -> Result<Vec<Section>, ElfError> {
    let mut raw: Vec<(u32, Section)> = Vec::new();
    for index in 0..shnum {
        let at = shoff as usize + index * SHDR_SIZE;
        raw.push((
            rd32(bytes, at),
            Section {
                name: String::new(),
                sh_type: rd32(bytes, at + 4),
                flags: rd32(bytes, at + 8),
                offset: rd32(bytes, at + 16),
                size: rd32(bytes, at + 20),
                link: rd32(bytes, at + 24),
                info: rd32(bytes, at + 28),
                entsize: rd32(bytes, at + 36),
            },
        ));
    }
    if shnum == 0 {
        return Ok(Vec::new());
    }
    let table = raw
        .get(shstrndx)
        .map(|(_, s)| s)
        .filter(|s| s.sh_type == SHT_STRTAB)
        .ok_or(ElfError::BadSectionLink { index: shstrndx })?;
    if !inside(bytes.len(), table.offset, u64::from(table.size)) {
        return Err(ElfError::SectionOutsideFile {
            index: shstrndx,
            offset: table.offset,
            size: table.size,
        });
    }
    let names = &bytes[table.offset as usize..(table.offset + table.size) as usize];
    let mut sections = Vec::with_capacity(shnum);
    for (index, (name_offset, mut sec)) in raw.into_iter().enumerate() {
        sec.name = string_at(names, name_offset)
            .ok_or(ElfError::BadString {
                index,
                offset: name_offset,
            })?
            .to_string();
        if sec.name == ".tdata" || sec.name == ".tbss" || sec.flags & SHF_TLS != 0 {
            return Err(ElfError::ThreadLocalSection { index });
        }
        sections.push(sec);
    }
    Ok(sections)
}

/// The symbol table at section `si` with every name resolved through its string table.
fn read_symbols(bytes: &[u8], sections: &[Section], si: usize) -> Result<Vec<Symbol>, ElfError> {
    let sec = &sections[si];
    if sec.entsize as usize != SYM_SIZE || !(sec.size as usize).is_multiple_of(SYM_SIZE) {
        return Err(ElfError::BadEntrySize {
            index: si,
            size: sec.entsize,
        });
    }
    let strtab_index = sec.link as usize;
    let strtab = sections
        .get(strtab_index)
        .filter(|s| s.sh_type == SHT_STRTAB)
        .ok_or(ElfError::BadSectionLink { index: si })?;
    for (index, s) in [(si, sec), (strtab_index, strtab)] {
        if s.sh_type != SHT_NOBITS && !inside(bytes.len(), s.offset, u64::from(s.size)) {
            return Err(ElfError::SectionOutsideFile {
                index,
                offset: s.offset,
                size: s.size,
            });
        }
    }
    let names = &bytes[strtab.offset as usize..(strtab.offset + strtab.size) as usize];
    let data = &bytes[sec.offset as usize..(sec.offset + sec.size) as usize];
    let mut symbols = Vec::with_capacity(data.len() / SYM_SIZE);
    for raw in data.as_chunks::<SYM_SIZE>().0 {
        let name_offset = rd32(raw, 0);
        let name = string_at(names, name_offset).ok_or(ElfError::BadString {
            index: strtab_index,
            offset: name_offset,
        })?;
        symbols.push(Symbol {
            name: name.to_string(),
            value: rd32(raw, 4),
            size: rd32(raw, 8),
            info: raw[12],
            shndx: rd16(raw, 14),
        });
    }
    Ok(symbols)
}
