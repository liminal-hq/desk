// A small writer of ELF32 files for the packer's tests: header, program headers, sections, a symbol table and relocation sections.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `good()` describes a valid module linked the way `rust-lld` does it (three segments, `.bss` in the last, `REL` sections made by
// `--emit-relocs`, import slots in `.desk.imp`); a test changes the description, or patches the bytes of the finished file with the
// offsets below, and expects one exact refusal. The layout is fixed: the ELF header, then the program headers (at 52), then the
// segments' bytes, then the sections' bytes, then the section header table. Allocated sections hold no bytes of their own: their
// contents are in the segments, and the reader never looks at them.
#![allow(dead_code)]

pub const BASE: u32 = 0x0040_0000;

// Offsets in the ELF header.
pub const E_CLASS: usize = 4;
pub const E_DATA: usize = 5;
pub const E_IDENT_VERSION: usize = 6;
pub const E_TYPE: usize = 16;
pub const E_MACHINE: usize = 18;
pub const E_VERSION: usize = 20;
pub const E_PHOFF: usize = 28;
pub const E_SHOFF: usize = 32;
pub const E_PHENTSIZE: usize = 42;
pub const E_PHNUM: usize = 44;
pub const E_SHENTSIZE: usize = 46;
pub const E_SHNUM: usize = 48;
pub const E_SHSTRNDX: usize = 50;

// Offsets in a program header.
pub const P_TYPE: usize = 0;
pub const P_OFFSET: usize = 4;
pub const P_VADDR: usize = 8;
pub const P_FILESZ: usize = 16;
pub const P_MEMSZ: usize = 20;
pub const P_FLAGS: usize = 24;

// Offsets in a section header.
pub const SH_NAME: usize = 0;
pub const SH_TYPE: usize = 4;
pub const SH_FLAGS: usize = 8;
pub const SH_OFFSET: usize = 16;
pub const SH_SIZE: usize = 20;
pub const SH_LINK: usize = 24;
pub const SH_INFO: usize = 28;
pub const SH_ENTSIZE: usize = 36;

pub const PT_LOAD: u32 = 1;
pub const PT_DYNAMIC: u32 = 2;
pub const PT_INTERP: u32 = 3;
pub const PT_TLS: u32 = 7;
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

pub const SHT_PROGBITS: u32 = 1;
pub const SHT_RELA: u32 = 4;
pub const SHF_WRITE: u32 = 1;
pub const SHF_ALLOC: u32 = 2;
pub const SHF_EXEC: u32 = 4;

pub const R_386_NONE: u32 = 0;
pub const R_386_32: u32 = 1;
pub const R_386_PC32: u32 = 2;
pub const R_386_GOT32: u32 = 3;
pub const R_386_TLS_TPOFF32: u32 = 37;

pub const STT_NOTYPE: u8 = 0;
pub const STT_OBJECT: u8 = 1;
pub const STT_FUNC: u8 = 2;
pub const STT_SECTION: u8 = 3;

pub fn put16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

pub fn put32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

pub fn get32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The file offset of program header `i`.
pub fn phdr(i: usize) -> usize {
    52 + 32 * i
}

/// The file offset of section header `i` in a finished file.
pub fn shdr(b: &[u8], i: usize) -> usize {
    get32(b, E_SHOFF) as usize + 40 * i
}

pub struct Segment {
    pub p_type: u32,
    pub vaddr: u32,
    pub flags: u32,
    pub data: Vec<u8>,
    /// Zero-filled bytes after `data` in memory only.
    pub bss: u32,
}

pub enum Body {
    /// An allocated section whose bytes are in a segment: only its size is recorded.
    Alloc(u32),
    /// A section with bytes in the file.
    Bytes(Vec<u8>),
    /// A section with no bytes in the file (`.bss`).
    NoBits(u32),
    Symtab,
    Strtab,
    Shstrtab,
    /// A relocation section for section number `target`, with `REL` entries (`RELA` ones with an addend of zero if `rela`).
    Rel {
        target: usize,
        rels: Vec<Rel>,
        rela: bool,
    },
}

pub struct Section {
    pub name: String,
    pub flags: u32,
    pub addr: u32,
    pub body: Body,
}

#[derive(Clone, Copy)]
pub struct Rel {
    pub offset: u32,
    pub r_type: u32,
    /// The symbol table index (the null symbol is 0, `Elf::symbols[0]` is 1).
    pub symbol: u32,
}

#[derive(Clone)]
pub enum Shndx {
    Section(String),
    Undef,
    Abs,
    Raw(u16),
}

pub struct Symbol {
    pub name: String,
    pub value: u32,
    pub size: u32,
    pub kind: u8,
    pub shndx: Shndx,
}

pub struct Elf {
    pub segments: Vec<Segment>,
    pub sections: Vec<Section>,
    pub symbols: Vec<Symbol>,
}

/// Zero-fills `v` up to a multiple of `align`.
fn pad(v: &mut Vec<u8>, align: usize) {
    let to = v.len().div_ceil(align) * align;
    v.resize(to, 0);
}

/// Appends `s` and a NUL to a string table and returns its offset.
fn intern(table: &mut Vec<u8>, s: &str) -> u32 {
    if s.is_empty() {
        return 0;
    }
    let at = table.len() as u32;
    table.extend_from_slice(s.as_bytes());
    table.push(0);
    at
}

impl Elf {
    pub fn section_index(&self, name: &str) -> usize {
        self.sections
            .iter()
            .position(|s| s.name == name)
            .unwrap_or_else(|| panic!("no section {name}"))
            + 1
    }

    pub fn section(&mut self, name: &str) -> &mut Section {
        let i = self.section_index(name) - 1;
        &mut self.sections[i]
    }

    pub fn symbol_index(&self, name: &str) -> u32 {
        self.symbols
            .iter()
            .position(|s| s.name == name)
            .unwrap_or_else(|| panic!("no symbol {name}")) as u32
            + 1
    }

    pub fn symbol(&mut self, name: &str) -> &mut Symbol {
        let i = self.symbol_index(name) as usize - 1;
        &mut self.symbols[i]
    }

    /// Appends a relocation to the relocation section named `section`.
    pub fn add_rel(&mut self, section: &str, rel: Rel) {
        match &mut self.section(section).body {
            Body::Rel { rels, .. } => rels.push(rel),
            _ => panic!("{section} is not a relocation section"),
        }
    }

    /// Replaces the relocations of the relocation section named `section`.
    pub fn set_rels(&mut self, section: &str, new: Vec<Rel>) {
        match &mut self.section(section).body {
            Body::Rel { rels, .. } => *rels = new,
            _ => panic!("{section} is not a relocation section"),
        }
    }

    pub fn build(&self) -> Vec<u8> {
        let mut out = vec![0u8; 52 + 32 * self.segments.len()];
        out[..4].copy_from_slice(b"\x7fELF");
        out[E_CLASS] = 1;
        out[E_DATA] = 1;
        out[E_IDENT_VERSION] = 1;
        put16(&mut out, E_TYPE, 2);
        put16(&mut out, E_MACHINE, 3);
        put32(&mut out, E_VERSION, 1);
        put32(&mut out, 24, self.symbol_value_or("app_start"));
        put32(&mut out, E_PHOFF, 52);
        put16(&mut out, 40, 52);
        put16(&mut out, E_PHENTSIZE, 32);
        put16(&mut out, E_PHNUM, self.segments.len() as u16);
        put16(&mut out, E_SHENTSIZE, 40);

        // Segments.
        for (i, seg) in self.segments.iter().enumerate() {
            pad(&mut out, 16);
            let at = phdr(i);
            put32(&mut out, at + P_TYPE, seg.p_type);
            let here = out.len() as u32;
            put32(&mut out, at + P_OFFSET, here);
            put32(&mut out, at + P_VADDR, seg.vaddr);
            put32(&mut out, at + 12, seg.vaddr);
            put32(&mut out, at + P_FILESZ, seg.data.len() as u32);
            put32(&mut out, at + P_MEMSZ, seg.data.len() as u32 + seg.bss);
            put32(&mut out, at + P_FLAGS, seg.flags);
            put32(&mut out, at + 28, 0x1000);
            out.extend_from_slice(&seg.data);
        }

        // The string tables: section names (with the null section first) and symbol names.
        let mut shstr = vec![0u8];
        let mut strtab = vec![0u8];
        let names: Vec<u32> = self.sections.iter().map(|s| intern(&mut shstr, &s.name)).collect();
        let symtab_index = self
            .sections
            .iter()
            .position(|s| matches!(s.body, Body::Symtab))
            .map(|i| i as u32 + 1);
        let strtab_index = self
            .sections
            .iter()
            .position(|s| matches!(s.body, Body::Strtab))
            .map(|i| i as u32 + 1);
        let shstr_index = self
            .sections
            .iter()
            .position(|s| matches!(s.body, Body::Shstrtab))
            .map(|i| i as u32 + 1);

        let mut symtab = vec![0u8; 16];
        for sym in &self.symbols {
            let name = intern(&mut strtab, &sym.name);
            let shndx = match &sym.shndx {
                Shndx::Section(n) => self.section_index(n) as u16,
                Shndx::Undef => 0,
                Shndx::Abs => 0xfff1,
                Shndx::Raw(v) => *v,
            };
            let mut e = [0u8; 16];
            e[0..4].copy_from_slice(&name.to_le_bytes());
            e[4..8].copy_from_slice(&sym.value.to_le_bytes());
            e[8..12].copy_from_slice(&sym.size.to_le_bytes());
            let bind = if sym.kind == STT_SECTION { 0 } else { 1 };
            e[12] = (bind << 4) | sym.kind;
            e[14..16].copy_from_slice(&shndx.to_le_bytes());
            symtab.extend_from_slice(&e);
        }

        // The sections' bytes.
        struct Placed {
            sh_type: u32,
            offset: u32,
            size: u32,
            link: u32,
            info: u32,
            entsize: u32,
        }
        let mut placed = Vec::new();
        for sec in &self.sections {
            pad(&mut out, 4);
            let offset = out.len() as u32;
            let mut p = Placed {
                sh_type: SHT_PROGBITS,
                offset: 0,
                size: 0,
                link: 0,
                info: 0,
                entsize: 0,
            };
            match &sec.body {
                Body::Alloc(size) => p.size = *size,
                Body::Bytes(b) => {
                    p.offset = offset;
                    p.size = b.len() as u32;
                    out.extend_from_slice(b);
                }
                Body::NoBits(size) => {
                    p.sh_type = 8;
                    p.size = *size;
                }
                Body::Symtab => {
                    p.sh_type = 2;
                    p.offset = offset;
                    p.size = symtab.len() as u32;
                    p.link = strtab_index.unwrap_or(0);
                    p.info = 1;
                    p.entsize = 16;
                    out.extend_from_slice(&symtab);
                }
                Body::Strtab => {
                    p.sh_type = 3;
                    p.offset = offset;
                    p.size = strtab.len() as u32;
                    out.extend_from_slice(&strtab);
                }
                Body::Shstrtab => {
                    p.sh_type = 3;
                    p.offset = offset;
                    p.size = shstr.len() as u32;
                    out.extend_from_slice(&shstr);
                }
                Body::Rel { target, rels, rela } => {
                    p.sh_type = if *rela { SHT_RELA } else { 9 };
                    p.offset = offset;
                    p.link = symtab_index.unwrap_or(0);
                    p.info = *target as u32;
                    p.entsize = if *rela { 12 } else { 8 };
                    for r in rels {
                        out.extend_from_slice(&r.offset.to_le_bytes());
                        out.extend_from_slice(&((r.symbol << 8) | r.r_type).to_le_bytes());
                        if *rela {
                            out.extend_from_slice(&0u32.to_le_bytes());
                        }
                    }
                    p.size = out.len() as u32 - offset;
                }
            }
            placed.push(p);
        }

        // The section header table: the null section, then one per section.
        pad(&mut out, 4);
        let shoff = out.len();
        out.extend_from_slice(&[0u8; 40]);
        for (i, sec) in self.sections.iter().enumerate() {
            let p = &placed[i];
            let mut h = [0u8; 40];
            h[0..4].copy_from_slice(&names[i].to_le_bytes());
            h[4..8].copy_from_slice(&p.sh_type.to_le_bytes());
            h[8..12].copy_from_slice(&sec.flags.to_le_bytes());
            h[12..16].copy_from_slice(&sec.addr.to_le_bytes());
            h[16..20].copy_from_slice(&p.offset.to_le_bytes());
            h[20..24].copy_from_slice(&p.size.to_le_bytes());
            h[24..28].copy_from_slice(&p.link.to_le_bytes());
            h[28..32].copy_from_slice(&p.info.to_le_bytes());
            h[32..36].copy_from_slice(&4u32.to_le_bytes());
            h[36..40].copy_from_slice(&p.entsize.to_le_bytes());
            out.extend_from_slice(&h);
        }
        put32(&mut out, E_SHOFF, shoff as u32);
        put16(&mut out, E_SHNUM, self.sections.len() as u16 + 1);
        put16(&mut out, E_SHSTRNDX, shstr_index.unwrap_or(0) as u16);
        out
    }

    fn symbol_value_or(&self, name: &str) -> u32 {
        self.symbols.iter().find(|s| s.name == name).map(|s| s.value).unwrap_or(0)
    }
}

fn word(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

/// A valid module. Addresses (from `BASE`, image offsets in brackets):
///
/// Every relocated word holds the address it points at (the linker's value, `S + A`), as the container requires.
///
/// - segment 0, R E: `.text`, 0x40 bytes at 0x400000 [0]. `app_start` [0], `app_event` [0x10], `app_stop` [0x20]. Relocations:
///   `R_386_32` at [0x08] (against `.rodata`) and [0x0c] (against the import `desk_log`), `R_386_PC32` at [0x30], `R_386_NONE` at [0x34].
/// - segment 1, R: `.rodata`, 0x20 bytes at 0x401000 [0x1000]; `R_386_32` at [0x1008] against `.rodata`.
/// - segment 2, RW: `.desk.imp` (8 bytes: the slots of `desk_log` [0x2000] and `desk_alloc` [0x2004]), `.data` (8 bytes at [0x2008],
///   `R_386_32` at [0x2008] against `.data`) and `.bss` (0x30 bytes at [0x2010]); file size 0x10, memory size 0x40.
/// - `.rel.debug_info` with a relocation in a section that is not allocated, and `.debug_info` to go with it.
pub fn good() -> Elf {
    let mut text = vec![0u8; 0x40];
    text[0x08..0x0c].copy_from_slice(&word(BASE + 0x1000));
    text[0x0c..0x10].copy_from_slice(&word(BASE + 0x2000));
    text[0x30..0x34].copy_from_slice(&word(0xffff_ffec));
    let mut rodata = vec![0u8; 0x20];
    rodata[0..8].copy_from_slice(b"hello\n\0\0");
    rodata[0x08..0x0c].copy_from_slice(&word(BASE + 0x1000));
    let mut rw = vec![0u8; 0x10];
    rw[0x08..0x0c].copy_from_slice(&word(BASE + 0x2000 + 0x08));

    let ax = SHF_ALLOC | SHF_EXEC;
    let aw = SHF_ALLOC | SHF_WRITE;
    let sec = |name: &str, flags: u32, addr: u32, body: Body| Section {
        name: name.into(),
        flags,
        addr,
        body,
    };
    let sym = |name: &str, value: u32, size: u32, kind: u8, section: &str| Symbol {
        name: name.into(),
        value,
        size,
        kind,
        shndx: Shndx::Section(section.into()),
    };
    // Section numbers: 1 .text, 2 .rodata, 3 .desk.imp, 4 .data, 5 .bss, 6 .debug_info, then the tables and relocations.
    let sections = vec![
        sec(".text", ax, BASE, Body::Alloc(0x40)),
        sec(".rodata", SHF_ALLOC, BASE + 0x1000, Body::Alloc(0x20)),
        sec(".desk.imp", aw, BASE + 0x2000, Body::Alloc(8)),
        sec(".data", aw, BASE + 0x2008, Body::Alloc(8)),
        sec(".bss", aw, BASE + 0x2010, Body::NoBits(0x30)),
        sec(".debug_info", 0, 0, Body::Bytes(vec![0xaa; 16])),
        sec(".symtab", 0, 0, Body::Symtab),
        sec(".strtab", 0, 0, Body::Strtab),
        sec(".shstrtab", 0, 0, Body::Shstrtab),
        // Symbols (1-based): 1 .rodata, 2 .data, 3 app_start, 4 app_event, 5 app_stop, 6 desk_log slot, 7 desk_alloc slot.
        sec(
            ".rel.text",
            0,
            0,
            Body::Rel {
                target: 1,
                rela: false,
                rels: vec![
                    Rel {
                        offset: BASE + 0x08,
                        r_type: R_386_32,
                        symbol: 1,
                    },
                    Rel {
                        offset: BASE + 0x0c,
                        r_type: R_386_32,
                        symbol: 6,
                    },
                    Rel {
                        offset: BASE + 0x30,
                        r_type: R_386_PC32,
                        symbol: 3,
                    },
                    Rel {
                        offset: BASE + 0x34,
                        r_type: R_386_NONE,
                        symbol: 0,
                    },
                ],
            },
        ),
        sec(
            ".rel.rodata",
            0,
            0,
            Body::Rel {
                target: 2,
                rela: false,
                rels: vec![Rel {
                    offset: BASE + 0x1008,
                    r_type: R_386_32,
                    symbol: 1,
                }],
            },
        ),
        sec(
            ".rel.data",
            0,
            0,
            Body::Rel {
                target: 4,
                rela: false,
                rels: vec![Rel {
                    offset: BASE + 0x2008,
                    r_type: R_386_32,
                    symbol: 2,
                }],
            },
        ),
        // A relocation against an undefined symbol, at an address outside the image, in a section that is not allocated.
        sec(
            ".rel.debug_info",
            0,
            0,
            Body::Rel {
                target: 6,
                rela: false,
                rels: vec![Rel {
                    offset: 0x10,
                    r_type: R_386_32,
                    symbol: 0,
                }],
            },
        ),
    ];
    let symbols = vec![
        sym(".rodata", BASE + 0x1000, 0, STT_SECTION, ".rodata"),
        sym(".data", BASE + 0x2008, 0, STT_SECTION, ".data"),
        sym("app_start", BASE, 0x10, STT_FUNC, ".text"),
        sym("app_event", BASE + 0x10, 0x10, STT_FUNC, ".text"),
        sym("app_stop", BASE + 0x20, 0x10, STT_FUNC, ".text"),
        sym("__desk_imp_desk_log", BASE + 0x2000, 4, STT_OBJECT, ".desk.imp"),
        sym("__desk_imp_desk_alloc", BASE + 0x2004, 4, STT_OBJECT, ".desk.imp"),
    ];
    let seg = |flags: u32, vaddr: u32, data: Vec<u8>, bss: u32| Segment {
        p_type: PT_LOAD,
        vaddr,
        flags,
        data,
        bss,
    };
    Elf {
        segments: vec![
            seg(PF_R | PF_X, BASE, text, 0),
            seg(PF_R, BASE + 0x1000, rodata, 0),
            seg(PF_R | PF_W, BASE + 0x2000, rw, 0x30),
        ],
        sections,
        symbols,
    }
}
