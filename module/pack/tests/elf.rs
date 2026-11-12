// Tests of the packer's ELF reader: one file `elfgen` writes, the exact `Extract` it gives, and one crafted file per refusal.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
mod elfgen;

use dmdpack::{read, ElfError, Extract, KIND_APP, KIND_MODULE};
use elfgen::*;

// Section numbers in `good()`: 1 .text, 2 .rodata, 3 .desk.imp, 4 .data, 5 .bss, 6 .debug_info, 7 .symtab, 8 .strtab, 9 .shstrtab,
// 10 .rel.text, 11 .rel.rodata, 12 .rel.data, 13 .rel.debug_info. Symbol table numbers: 1 .rodata, 2 .data, 3 app_start, 4 app_event,
// 5 app_stop, 6 the slot of desk_log, 7 the slot of desk_alloc.
const REL_TEXT: usize = 10;
const REL_RODATA: usize = 11;
const REL_DATA: usize = 12;
const SHF_TLS: u32 = 0x400;

fn ok(e: &Elf) -> Extract {
    read(&e.build(), BASE, KIND_MODULE).unwrap_or_else(|err| panic!("refused a good file: {err}"))
}

fn refuse(bytes: &[u8], expected: ElfError) {
    assert_eq!(read(bytes, BASE, KIND_MODULE), Err(expected));
}

fn refuse_elf(e: &Elf, expected: ElfError) {
    refuse(&e.build(), expected);
}

fn refuse_patched(patch: impl FnOnce(&mut Vec<u8>), expected: ElfError) {
    let mut b = good().build();
    patch(&mut b);
    refuse(&b, expected);
}

// ---- What a good file gives ----

#[test]
fn a_good_module_gives_exactly_the_stated_extract() {
    let x = ok(&good());
    assert_eq!(x.link_base, BASE);
    assert_eq!(x.mem_size, 0x2040);
    assert_eq!(x.ro_size, 0x1020);
    assert_eq!(x.relocs, vec![0x08, 0x0c, 0x1008, 0x2008]);
    assert_eq!(
        x.imports,
        vec![("desk_log".to_string(), 0x2000), ("desk_alloc".to_string(), 0x2004)]
    );
    assert_eq!((x.start, x.event, x.stop), (0, Some(0x10), Some(0x20)));
    // The image is the three segments at their offsets with zeros between; the bss tail is not in it.
    assert_eq!(x.image.len(), 0x2010);
    let mut expected = vec![0u8; 0x2010];
    let g = good();
    for seg in &g.segments {
        let at = (seg.vaddr - BASE) as usize;
        expected[at..at + seg.data.len()].copy_from_slice(&seg.data);
    }
    assert_eq!(x.image, expected);
    assert_eq!(&x.image[0x1000..0x1006], b"hello\n");
    assert!(x.image[0x40..0x1000].iter().all(|&b| b == 0));
    assert!(x.image[0x1020..0x2000].iter().all(|&b| b == 0));
    // The word of a relocation holds its addend: the reader leaves it as the linker wrote it.
    assert_eq!(get32(&x.image, 0x08), BASE + 0x1000);
    assert_eq!(get32(&x.image, 0x2008), BASE + 0x2008);
}

#[test]
fn pc32_none_and_non_allocated_relocations_produce_nothing() {
    let x = ok(&good());
    assert!(!x.relocs.contains(&0x30), "R_386_PC32 must not be listed");
    assert!(!x.relocs.contains(&0x34), "R_386_NONE must not be listed");
    // A non-allocated section's relocations are ignored whatever they say.
    let mut e = good();
    e.add_rel(
        ".rel.debug_info",
        Rel {
            offset: 0x4,
            r_type: R_386_32,
            symbol: 99,
        },
    );
    e.add_rel(
        ".rel.debug_info",
        Rel {
            offset: 0x8,
            r_type: R_386_GOT32,
            symbol: 1,
        },
    );
    e.add_rel(
        ".rel.debug_info",
        Rel {
            offset: 0xc,
            r_type: R_386_TLS_TPOFF32,
            symbol: 1,
        },
    );
    assert_eq!(ok(&e), ok(&good()));
}

#[test]
fn a_relocation_listed_twice_is_listed_once() {
    let mut e = good();
    e.add_rel(
        ".rel.rodata",
        Rel {
            offset: BASE + 0x1008,
            r_type: R_386_32,
            symbol: 1,
        },
    );
    assert_eq!(ok(&e), ok(&good()));
}

#[test]
fn imports_come_back_in_slot_order_not_symbol_order() {
    let mut e = good();
    e.symbols.swap(5, 6);
    let x = ok(&e);
    assert_eq!(
        x.imports,
        vec![("desk_log".to_string(), 0x2000), ("desk_alloc".to_string(), 0x2004)]
    );
}

#[test]
fn a_file_with_no_section_headers_is_read_up_to_its_missing_entry() {
    refuse_patched(
        |b| {
            put16(b, E_SHNUM, 0);
        },
        ElfError::MissingEntry { name: "app_start" },
    );
}

#[test]
fn the_limit_of_256_imports_is_allowed_and_257_is_not() {
    let many = |n: usize| {
        let mut e = good();
        e.set_rels(".rel.data", vec![]);
        e.segments[2].data = vec![0; n * 4];
        e.symbols.retain(|s| !s.name.starts_with("__desk_imp_"));
        for i in 0..n {
            e.symbols.push(Symbol {
                name: format!("__desk_imp_f{i}"),
                value: BASE + 0x2000 + 4 * i as u32,
                size: 4,
                kind: STT_OBJECT,
                shndx: Shndx::Section(".desk.imp".into()),
            });
        }
        // The first relocation of .rel.text named the slot of desk_log by number; point it at the section symbol instead.
        e.set_rels(
            ".rel.text",
            vec![Rel {
                offset: BASE + 0x08,
                r_type: R_386_32,
                symbol: 1,
            }],
        );
        e
    };
    let x = ok(&many(256));
    assert_eq!(x.imports.len(), 256);
    assert_eq!(x.imports[255], ("f255".to_string(), 0x2000 + 4 * 255));
    refuse_elf(&many(257), ElfError::TooManyImports { count: 257 });
}

// ---- Not a file the reader takes ----

#[test]
fn refuses_what_is_not_elf() {
    refuse(b"", ElfError::NotElf);
    refuse(b"MZ\x90\0not an elf file at all, just text", ElfError::NotElf);
    refuse_patched(|b| b[0] = 0x7e, ElfError::NotElf);
}

#[test]
fn refuses_a_file_shorter_than_the_header() {
    let mut b = good().build();
    b.truncate(40);
    refuse(&b, ElfError::HeaderTruncated { len: 40 });
}

#[test]
fn refuses_64_bit() {
    refuse_patched(|b| b[E_CLASS] = 2, ElfError::Not32Bit { class: 2 });
}

#[test]
fn refuses_big_endian() {
    refuse_patched(|b| b[E_DATA] = 2, ElfError::NotLittleEndian { data: 2 });
}

#[test]
fn refuses_an_unknown_elf_version() {
    refuse_patched(|b| put32(b, E_VERSION, 2), ElfError::BadVersion { version: 2 });
    refuse_patched(|b| b[E_IDENT_VERSION] = 0, ElfError::BadVersion { version: 0 });
}

#[test]
fn refuses_what_is_not_et_exec() {
    refuse_patched(|b| put16(b, E_TYPE, 3), ElfError::NotExecutable { e_type: 3 });
    refuse_patched(|b| put16(b, E_TYPE, 1), ElfError::NotExecutable { e_type: 1 });
}

#[test]
fn refuses_what_is_not_em_386() {
    refuse_patched(|b| put16(b, E_MACHINE, 62), ElfError::NotI386 { machine: 62 });
}

#[test]
fn refuses_program_headers_of_the_wrong_size() {
    refuse_patched(|b| put16(b, E_PHENTSIZE, 56), ElfError::BadProgramHeaderSize { size: 56 });
}

#[test]
fn refuses_program_headers_outside_the_file() {
    let len = good().build().len() as u32;
    refuse_patched(
        |b| put32(b, E_PHOFF, len - 10),
        ElfError::ProgramHeadersOutsideFile {
            offset: len - 10,
            count: 3,
        },
    );
    refuse_patched(
        |b| put16(b, E_PHNUM, 0x2000),
        ElfError::ProgramHeadersOutsideFile {
            offset: 52,
            count: 0x2000,
        },
    );
    refuse_patched(
        |b| put32(b, E_PHOFF, 0xffff_fff0),
        ElfError::ProgramHeadersOutsideFile {
            offset: 0xffff_fff0,
            count: 3,
        },
    );
}

#[test]
fn refuses_section_headers_of_the_wrong_size() {
    refuse_patched(|b| put16(b, E_SHENTSIZE, 64), ElfError::BadSectionHeaderSize { size: 64 });
}

#[test]
fn refuses_a_section_header_table_outside_the_file() {
    let len = good().build().len() as u32;
    refuse_patched(
        |b| put32(b, E_SHOFF, len - 10),
        ElfError::SectionHeadersOutsideFile {
            offset: len - 10,
            count: 14,
        },
    );
    refuse_patched(
        |b| put16(b, E_SHNUM, 0xffff),
        ElfError::SectionHeadersOutsideFile {
            offset: len - 14 * 40,
            count: 0xffff,
        },
    );
    refuse_patched(
        |b| put32(b, E_SHOFF, 0xffff_ffe0),
        ElfError::SectionHeadersOutsideFile {
            offset: 0xffff_ffe0,
            count: 14,
        },
    );
    // A file cut off in the middle of the section header table.
    let mut b = good().build();
    let shoff = get32(&b, E_SHOFF);
    b.truncate(shoff as usize + 100);
    refuse(
        &b,
        ElfError::SectionHeadersOutsideFile {
            offset: shoff,
            count: 14,
        },
    );
}

#[test]
fn refuses_a_file_with_nothing_to_load() {
    refuse_patched(|b| put16(b, E_PHNUM, 0), ElfError::NoLoadSegment);
    refuse_patched(
        |b| {
            for i in 0..3 {
                put32(b, phdr(i) + P_TYPE, 0);
            }
        },
        ElfError::NoLoadSegment,
    );
}

#[test]
fn refuses_a_kind_it_cannot_pack() {
    let b = good().build();
    assert_eq!(read(&b, BASE, 0), Err(ElfError::UnsupportedKind { kind: 0 }));
    assert_eq!(read(&b, BASE, 3), Err(ElfError::UnsupportedKind { kind: 3 }));
}

// ---- Segments ----

#[test]
fn refuses_a_segment_whose_file_bytes_run_past_the_end() {
    // The first of the analysis's two probes: a PT_LOAD whose file part is outside the file.
    let mut b = good().build();
    let offset = get32(&b, phdr(2) + P_OFFSET);
    put32(&mut b, phdr(2) + P_FILESZ, 0x10000);
    put32(&mut b, phdr(2) + P_MEMSZ, 0x10000);
    refuse(
        &b,
        ElfError::SegmentOutsideFile {
            index: 2,
            offset,
            size: 0x10000,
        },
    );
}

#[test]
fn refuses_a_segment_whose_offset_is_past_the_end() {
    let len = good().build().len() as u32;
    refuse_patched(
        |b| put32(b, phdr(0) + P_OFFSET, len + 1),
        ElfError::SegmentOutsideFile {
            index: 0,
            offset: len + 1,
            size: 0x40,
        },
    );
    refuse_patched(
        |b| put32(b, phdr(0) + P_OFFSET, 0xffff_fff8),
        ElfError::SegmentOutsideFile {
            index: 0,
            offset: 0xffff_fff8,
            size: 0x40,
        },
    );
}

#[test]
fn refuses_a_segment_whose_file_size_exceeds_its_memory_size() {
    // The second probe.
    refuse_patched(
        |b| put32(b, phdr(2) + P_MEMSZ, 8),
        ElfError::SegmentFileLargerThanMemory {
            index: 2,
            file_size: 0x10,
            mem_size: 8,
        },
    );
    refuse_patched(
        |b| put32(b, phdr(0) + P_FILESZ, 0xffff_fff0),
        ElfError::SegmentFileLargerThanMemory {
            index: 0,
            file_size: 0xffff_fff0,
            mem_size: 0x40,
        },
    );
}

#[test]
fn refuses_a_segment_that_wraps_the_address_space() {
    refuse_patched(
        |b| put32(b, phdr(1) + P_VADDR, 0xffff_fff0),
        ElfError::SegmentWraps {
            index: 1,
            vaddr: 0xffff_fff0,
            mem_size: 0x20,
        },
    );
}

#[test]
fn refuses_overlapping_segments() {
    refuse_patched(
        |b| put32(b, phdr(1) + P_VADDR, BASE + 0x30),
        ElfError::SegmentOverlap {
            index: 1,
            vaddr: BASE + 0x30,
            previous_end: BASE + 0x40,
        },
    );
    // One byte of overlap, through the memory size of the earlier segment.
    refuse_patched(
        |b| put32(b, phdr(0) + P_MEMSZ, 0x1001),
        ElfError::SegmentOverlap {
            index: 1,
            vaddr: BASE + 0x1000,
            previous_end: BASE + 0x1001,
        },
    );
}

#[test]
fn refuses_segments_out_of_address_order() {
    let mut e = good();
    e.segments.reverse();
    refuse_elf(
        &e,
        ElfError::SegmentOutOfOrder {
            index: 1,
            vaddr: BASE + 0x1000,
            previous: BASE + 0x2000,
        },
    );
}

#[test]
fn refuses_a_bss_that_is_not_at_the_end() {
    let mut e = good();
    e.segments[0].bss = 0x10;
    refuse_elf(&e, ElfError::BssNotAtEnd { index: 0 });
}

#[test]
fn refuses_a_writable_segment_before_a_read_only_one() {
    let mut e = good();
    e.segments[1].flags = PF_R | PF_W;
    e.segments[2].flags = PF_R;
    refuse_elf(&e, ElfError::WritableBeforeReadOnly { index: 2 });
}

#[test]
fn refuses_a_link_base_other_than_the_one_asked_for() {
    let b = good().build();
    assert_eq!(
        read(&b, 0x0080_0000, KIND_MODULE),
        Err(ElfError::WrongLinkBase {
            found: BASE,
            wanted: 0x0080_0000
        })
    );
    assert_eq!(
        read(&b, 0, KIND_MODULE),
        Err(ElfError::WrongLinkBase { found: BASE, wanted: 0 })
    );
    // A file linked at the other base is refused when the default is asked for.
    let mut e = good();
    for s in &mut e.segments {
        s.vaddr += 0x0040_0000;
    }
    assert_eq!(
        read(&e.build(), BASE, KIND_MODULE),
        Err(ElfError::WrongLinkBase {
            found: 0x0080_0000,
            wanted: BASE
        })
    );
}

#[test]
fn refuses_more_memory_than_the_limit() {
    let mut e = good();
    e.segments[2].bss = 17 << 20;
    refuse_elf(
        &e,
        ElfError::TooLarge {
            mem_size: 0x2010 + (17 << 20),
        },
    );
}

#[test]
fn accepts_memory_up_to_the_limit() {
    let mut e = good();
    e.segments[2].bss = (16 << 20) - 0x2010;
    assert_eq!(ok(&e).mem_size, 16 << 20);
}

// ---- Thread-local storage ----

#[test]
fn refuses_a_thread_local_segment() {
    let mut e = good();
    e.segments.push(Segment {
        p_type: PT_TLS,
        vaddr: 0,
        flags: PF_R,
        data: vec![],
        bss: 4,
    });
    refuse_elf(&e, ElfError::ThreadLocalSegment { index: 3 });
}

#[test]
fn refuses_tdata_and_tbss_sections() {
    let mut e = good();
    e.sections.push(Section {
        name: ".tdata".into(),
        flags: SHF_ALLOC | SHF_WRITE,
        addr: BASE + 0x3000,
        body: Body::Alloc(4),
    });
    refuse_elf(&e, ElfError::ThreadLocalSection { index: 14 });
    let mut e = good();
    e.sections.push(Section {
        name: ".tbss".into(),
        flags: SHF_ALLOC | SHF_WRITE,
        addr: BASE + 0x3000,
        body: Body::NoBits(4),
    });
    refuse_elf(&e, ElfError::ThreadLocalSection { index: 14 });
    // A thread-local section under another name is still one.
    let mut e = good();
    e.sections.push(Section {
        name: ".cell".into(),
        flags: SHF_ALLOC | 0x400,
        addr: BASE + 0x3000,
        body: Body::Alloc(4),
    });
    refuse_elf(&e, ElfError::ThreadLocalSection { index: 14 });
}

#[test]
fn refuses_a_thread_local_relocation() {
    for r_type in [14, 15, 16, 17, 18, 19, 24, 32, 33, 34, 35, 36, 37, 39, 40, 41] {
        let mut e = good();
        e.add_rel(
            ".rel.text",
            Rel {
                offset: BASE + 0x38,
                r_type,
                symbol: 1,
            },
        );
        refuse_elf(
            &e,
            ElfError::ThreadLocalRelocation {
                section: REL_TEXT,
                index: 4,
                r_type,
            },
        );
    }
}

// ---- Relocations ----

#[test]
fn refuses_a_relocation_type_other_than_the_three() {
    for r_type in [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 20, 21, 22, 23, 38, 42, 200] {
        let mut e = good();
        e.add_rel(
            ".rel.text",
            Rel {
                offset: BASE + 0x38,
                r_type,
                symbol: 1,
            },
        );
        refuse_elf(
            &e,
            ElfError::BadRelocationType {
                section: REL_TEXT,
                index: 4,
                r_type,
            },
        );
    }
    // R_386_GOT32 by name, in the section after the first.
    let mut e = good();
    e.add_rel(
        ".rel.rodata",
        Rel {
            offset: BASE + 0x1010,
            r_type: R_386_GOT32,
            symbol: 1,
        },
    );
    refuse_elf(
        &e,
        ElfError::BadRelocationType {
            section: REL_RODATA,
            index: 1,
            r_type: R_386_GOT32,
        },
    );
}

#[test]
fn refuses_an_r_386_32_against_an_absolute_symbol() {
    let mut e = good();
    e.symbol(".data").shndx = Shndx::Abs;
    refuse_elf(
        &e,
        ElfError::RelocationAgainstAbsolute {
            section: REL_DATA,
            index: 0,
            symbol: 2,
        },
    );
}

#[test]
fn refuses_an_r_386_32_against_an_undefined_symbol() {
    let mut e = good();
    e.symbol(".data").shndx = Shndx::Undef;
    refuse_elf(
        &e,
        ElfError::RelocationAgainstUndefined {
            section: REL_DATA,
            index: 0,
            symbol: 2,
        },
    );
    // The null symbol is undefined too.
    let mut e = good();
    e.add_rel(
        ".rel.text",
        Rel {
            offset: BASE + 0x38,
            r_type: R_386_32,
            symbol: 0,
        },
    );
    refuse_elf(
        &e,
        ElfError::RelocationAgainstUndefined {
            section: REL_TEXT,
            index: 4,
            symbol: 0,
        },
    );
}

#[test]
fn refuses_an_r_386_32_against_a_symbol_in_a_reserved_section() {
    let mut e = good();
    e.symbol(".data").shndx = Shndx::Raw(0xfff2);
    refuse_elf(
        &e,
        ElfError::RelocationAgainstReserved {
            section: REL_DATA,
            index: 0,
            symbol: 2,
        },
    );
}

#[test]
fn refuses_an_r_386_32_against_a_symbol_that_does_not_exist() {
    let mut e = good();
    e.add_rel(
        ".rel.text",
        Rel {
            offset: BASE + 0x38,
            r_type: R_386_32,
            symbol: 99,
        },
    );
    refuse_elf(
        &e,
        ElfError::BadSymbolIndex {
            section: REL_TEXT,
            index: 4,
            symbol: 99,
        },
    );
}

#[test]
fn refuses_a_relocation_into_a_section_the_image_does_not_contain() {
    // Into the .bss, which has no file bytes.
    let mut e = good();
    e.add_rel(
        ".rel.text",
        Rel {
            offset: BASE + 0x2020,
            r_type: R_386_32,
            symbol: 1,
        },
    );
    refuse_elf(
        &e,
        ElfError::RelocationOutsideImage {
            section: REL_TEXT,
            index: 4,
            address: BASE + 0x2020,
        },
    );
}

#[test]
fn refuses_a_relocation_into_an_allocated_section_that_is_in_no_segment() {
    let mut e = good();
    e.sections.push(Section {
        name: ".extra".into(),
        flags: SHF_ALLOC | SHF_WRITE,
        addr: BASE + 0x3000,
        body: Body::Alloc(8),
    });
    e.sections.push(Section {
        name: ".rel.extra".into(),
        flags: 0,
        addr: 0,
        body: Body::Rel {
            target: 14,
            rela: false,
            rels: vec![Rel {
                offset: BASE + 0x3000,
                r_type: R_386_32,
                symbol: 1,
            }],
        },
    });
    refuse_elf(
        &e,
        ElfError::RelocationOutsideImage {
            section: 15,
            index: 0,
            address: BASE + 0x3000,
        },
    );
}

#[test]
fn refuses_a_relocated_word_that_runs_past_the_file_bytes_of_its_segment() {
    // The file bytes of the last segment end at 0x2010; a word at 0x200e would run into the .bss.
    let mut e = good();
    e.add_rel(
        ".rel.data",
        Rel {
            offset: BASE + 0x200e,
            r_type: R_386_32,
            symbol: 2,
        },
    );
    refuse_elf(
        &e,
        ElfError::RelocationOutsideImage {
            section: REL_DATA,
            index: 1,
            address: BASE + 0x200e,
        },
    );
    // At 0x200c the word is the last four bytes: allowed.
    let mut e = good();
    e.segments[2].data[0xc..0x10].copy_from_slice(&(BASE + 0x2000).to_le_bytes());
    e.add_rel(
        ".rel.data",
        Rel {
            offset: BASE + 0x200c,
            r_type: R_386_32,
            symbol: 2,
        },
    );
    assert_eq!(ok(&e).relocs, vec![0x08, 0x0c, 0x1008, 0x2008, 0x200c]);
}

#[test]
fn refuses_a_relocation_in_the_gap_between_segments() {
    let mut e = good();
    e.add_rel(
        ".rel.text",
        Rel {
            offset: BASE + 0x800,
            r_type: R_386_32,
            symbol: 1,
        },
    );
    refuse_elf(
        &e,
        ElfError::RelocationOutsideImage {
            section: REL_TEXT,
            index: 4,
            address: BASE + 0x800,
        },
    );
}

#[test]
fn refuses_relocations_with_explicit_addends() {
    let mut e = good();
    match &mut e.section(".rel.rodata").body {
        Body::Rel { rela, .. } => *rela = true,
        _ => unreachable!(),
    }
    refuse_elf(&e, ElfError::RelaSection { index: REL_RODATA });
}

#[test]
fn refuses_relocated_words_that_overlap() {
    let mut e = good();
    e.add_rel(
        ".rel.text",
        Rel {
            offset: BASE + 0x0a,
            r_type: R_386_32,
            symbol: 1,
        },
    );
    refuse_elf(&e, ElfError::RelocationsOverlap { offset: 0x0a });
}

#[test]
fn refuses_a_relocated_word_that_is_an_import_slot() {
    let mut e = good();
    e.add_rel(
        ".rel.data",
        Rel {
            offset: BASE + 0x2004,
            r_type: R_386_32,
            symbol: 2,
        },
    );
    refuse_elf(&e, ElfError::RelocationAtImportSlot { offset: 0x2004 });
}

// ---- Imports ----

fn import_error(change: impl FnOnce(&mut Elf), expected: ElfError) {
    let mut e = good();
    change(&mut e);
    refuse_elf(&e, expected);
}

#[test]
fn refuses_an_import_symbol_of_size_2() {
    import_error(
        |e| e.symbol("__desk_imp_desk_log").size = 2,
        ElfError::ImportBadSize {
            name: "desk_log".into(),
            size: 2,
        },
    );
    import_error(
        |e| e.symbol("__desk_imp_desk_alloc").size = 8,
        ElfError::ImportBadSize {
            name: "desk_alloc".into(),
            size: 8,
        },
    );
}

#[test]
fn refuses_an_import_in_data_instead_of_the_import_section() {
    import_error(
        |e| {
            let s = e.symbol("__desk_imp_desk_log");
            s.shndx = Shndx::Section(".data".into());
            s.value = BASE + 0x2008;
        },
        ElfError::ImportWrongSection {
            name: "desk_log".into(),
            section: ".data".into(),
        },
    );
    import_error(
        |e| e.symbol("__desk_imp_desk_log").shndx = Shndx::Undef,
        ElfError::ImportWrongSection {
            name: "desk_log".into(),
            section: "(none)".into(),
        },
    );
    import_error(
        |e| e.symbol("__desk_imp_desk_log").shndx = Shndx::Abs,
        ElfError::ImportWrongSection {
            name: "desk_log".into(),
            section: "(none)".into(),
        },
    );
}

#[test]
fn refuses_an_import_that_is_not_an_object() {
    for kind in [STT_NOTYPE, STT_FUNC] {
        import_error(
            |e| e.symbol("__desk_imp_desk_log").kind = kind,
            ElfError::ImportNotObject { symbol: 6 },
        );
    }
}

#[test]
fn refuses_an_import_name_that_is_not_valid() {
    let long = format!("__desk_imp_{}", "a".repeat(32));
    for bad in [
        "__desk_imp_",
        "__desk_imp_Desk_Log",
        "__desk_imp_desk-log",
        "__desk_imp_desk log",
        "__desk_imp_\u{e9}t\u{e9}",
        long.as_str(),
    ] {
        let name = bad.to_string();
        import_error(
            move |e| e.symbol("__desk_imp_desk_log").name = name,
            ElfError::ImportBadName { symbol: 6 },
        );
    }
    // The longest valid name is 31 bytes.
    let mut e = good();
    e.symbol("__desk_imp_desk_log").name = format!("__desk_imp_{}", "a".repeat(31));
    assert_eq!(ok(&e).imports[0].0, "a".repeat(31));
}

#[test]
fn refuses_an_import_defined_twice() {
    import_error(
        |e| e.symbol("__desk_imp_desk_alloc").name = "__desk_imp_desk_log".into(),
        ElfError::ImportDuplicate { name: "desk_log".into() },
    );
}

#[test]
fn refuses_a_misaligned_import_slot() {
    import_error(
        |e| e.symbol("__desk_imp_desk_log").value = BASE + 0x2002,
        ElfError::ImportMisaligned {
            name: "desk_log".into(),
            address: BASE + 0x2002,
        },
    );
}

#[test]
fn refuses_an_import_slot_outside_the_file_backed_image() {
    import_error(
        |e| e.symbol("__desk_imp_desk_log").value = BASE + 0x2020,
        ElfError::ImportOutsideImage {
            name: "desk_log".into(),
            address: BASE + 0x2020,
        },
    );
    import_error(
        |e| e.symbol("__desk_imp_desk_log").value = 0x1000,
        ElfError::ImportOutsideImage {
            name: "desk_log".into(),
            address: 0x1000,
        },
    );
}

#[test]
fn refuses_an_import_slot_in_the_read_only_part() {
    import_error(
        |e| e.symbol("__desk_imp_desk_log").value = BASE + 0x1000,
        ElfError::ImportInReadOnly {
            name: "desk_log".into(),
            offset: 0x1000,
        },
    );
}

#[test]
fn refuses_an_import_slot_that_is_not_zero() {
    import_error(
        |e| e.segments[2].data[0] = 1,
        ElfError::ImportNotZero { name: "desk_log".into() },
    );
}

#[test]
fn refuses_two_imports_that_share_a_slot() {
    import_error(
        |e| e.symbol("__desk_imp_desk_alloc").value = BASE + 0x2000,
        ElfError::ImportSlotShared {
            name: "desk_alloc".into(),
            other: "desk_log".into(),
        },
    );
}

// ---- Entry points ----

#[test]
fn refuses_a_module_without_app_start() {
    import_error(
        |e| e.symbol("app_start").name = "app_begin".into(),
        ElfError::MissingEntry { name: "app_start" },
    );
    import_error(
        |e| e.symbol("app_start").shndx = Shndx::Undef,
        ElfError::MissingEntry { name: "app_start" },
    );
}

#[test]
fn a_module_needs_neither_app_event_nor_app_stop() {
    let mut e = good();
    e.symbols.retain(|s| s.name != "app_event" && s.name != "app_stop");
    // Dropping two symbols renumbers the imports' symbols, which no relocation here names.
    e.set_rels(
        ".rel.text",
        vec![Rel {
            offset: BASE + 0x08,
            r_type: R_386_32,
            symbol: 1,
        }],
    );
    let x = ok(&e);
    assert_eq!((x.start, x.event, x.stop), (0, None, None));
}

#[test]
fn refuses_an_entry_point_outside_the_read_only_part() {
    import_error(
        |e| e.symbol("app_start").value = BASE + 0x2000,
        ElfError::EntryOutsideReadOnly {
            name: "app_start",
            offset: 0x2000,
        },
    );
    import_error(
        |e| e.symbol("app_event").value = BASE + 0x1020,
        ElfError::EntryOutsideReadOnly {
            name: "app_event",
            offset: 0x1020,
        },
    );
    import_error(
        |e| e.symbol("app_stop").value = 0x1000,
        ElfError::EntryOutsideReadOnly {
            name: "app_stop",
            offset: 0x1000u32.wrapping_sub(BASE),
        },
    );
}

fn app() -> Elf {
    let mut e = good();
    e.symbols.retain(|s| !s.name.starts_with("app_"));
    e.symbols.push(Symbol {
        name: "desk_main".into(),
        value: BASE + 0x8,
        size: 0x10,
        kind: STT_FUNC,
        shndx: Shndx::Section(".text".into()),
    });
    e.set_rels(
        ".rel.text",
        vec![Rel {
            offset: BASE + 0x08,
            r_type: R_386_32,
            symbol: 1,
        }],
    );
    e
}

#[test]
fn an_application_has_desk_main_alone() {
    let x = read(&app().build(), BASE, KIND_APP).unwrap();
    assert_eq!((x.start, x.event, x.stop), (0x8, None, None));
    assert_eq!(x.imports.len(), 2);
}

#[test]
fn refuses_an_application_without_desk_main() {
    let mut e = app();
    e.symbols.retain(|s| s.name != "desk_main");
    assert_eq!(
        read(&e.build(), BASE, KIND_APP),
        Err(ElfError::MissingEntry { name: "desk_main" })
    );
    // A module's entry points do not stand in for it, and the module kind does not accept desk_main.
    assert_eq!(
        read(&good().build(), BASE, KIND_APP),
        Err(ElfError::ApplicationHasModuleEntry { name: "app_event" })
    );
    assert_eq!(
        read(&app().build(), BASE, KIND_MODULE),
        Err(ElfError::MissingEntry { name: "app_start" })
    );
}

#[test]
fn refuses_an_application_that_also_defines_app_event_or_app_stop() {
    let sym = |name: &str| Symbol {
        name: name.into(),
        value: BASE + 0x10,
        size: 4,
        kind: STT_FUNC,
        shndx: Shndx::Section(".text".into()),
    };
    let mut e = app();
    e.symbols.push(sym("app_event"));
    assert_eq!(
        read(&e.build(), BASE, KIND_APP),
        Err(ElfError::ApplicationHasModuleEntry { name: "app_event" })
    );
    let mut e = app();
    e.symbols.push(sym("app_stop"));
    assert_eq!(
        read(&e.build(), BASE, KIND_APP),
        Err(ElfError::ApplicationHasModuleEntry { name: "app_stop" })
    );
}

// ---- Section and table damage ----

#[test]
fn refuses_a_section_name_that_is_not_in_the_string_table() {
    refuse_patched(
        |b| {
            let at = shdr(b, 1) + SH_NAME;
            put32(b, at, 0xffff);
        },
        ElfError::BadString {
            index: 1,
            offset: 0xffff,
        },
    );
}

#[test]
fn refuses_a_symbol_name_that_is_not_in_the_string_table() {
    refuse_patched(
        |b| {
            // Symbol 3 is app_start; its name offset is the first word of its 16-byte entry.
            let symtab = get32(b, shdr(b, 7) + SH_OFFSET) as usize;
            put32(b, symtab + 3 * 16, 0x7fff);
        },
        ElfError::BadString {
            index: 8,
            offset: 0x7fff,
        },
    );
}

#[test]
fn refuses_a_relocation_section_linked_to_nothing_useful() {
    refuse_patched(
        |b| {
            let at = shdr(b, REL_TEXT) + SH_LINK;
            put32(b, at, 0);
        },
        ElfError::BadSectionLink { index: REL_TEXT },
    );
    refuse_patched(
        |b| {
            let at = shdr(b, REL_TEXT) + SH_INFO;
            put32(b, at, 99);
        },
        ElfError::BadSectionLink { index: REL_TEXT },
    );
    refuse_patched(
        |b| {
            let at = shdr(b, 7) + SH_LINK;
            put32(b, at, 99);
        },
        ElfError::BadSectionLink { index: 7 },
    );
    refuse_patched(
        |b| {
            put16(b, E_SHSTRNDX, 0);
        },
        ElfError::BadSectionLink { index: 0 },
    );
}

#[test]
fn refuses_a_relocation_section_with_the_wrong_entry_size() {
    refuse_patched(
        |b| {
            let at = shdr(b, REL_TEXT) + SH_ENTSIZE;
            put32(b, at, 12);
        },
        ElfError::BadEntrySize {
            index: REL_TEXT,
            size: 12,
        },
    );
    refuse_patched(
        |b| {
            let at = shdr(b, 7) + SH_ENTSIZE;
            put32(b, at, 24);
        },
        ElfError::BadEntrySize { index: 7, size: 24 },
    );
}

#[test]
fn refuses_a_table_that_lies_outside_the_file() {
    let g = good().build();
    let len = g.len() as u32;
    let rodata_offset = get32(&g, shdr(&g, REL_RODATA) + SH_OFFSET);
    let shstr_size = get32(&g, shdr(&g, 9) + SH_SIZE);
    refuse_patched(
        |b| {
            let at = shdr(b, REL_RODATA) + SH_SIZE;
            put32(b, at, 0x10000);
        },
        ElfError::SectionOutsideFile {
            index: REL_RODATA,
            offset: rodata_offset,
            size: 0x10000,
        },
    );
    refuse_patched(
        |b| {
            let at = shdr(b, 7) + SH_OFFSET;
            put32(b, at, len);
        },
        ElfError::SectionOutsideFile {
            index: 7,
            offset: len,
            size: 128,
        },
    );
    refuse_patched(
        |b| {
            let at = shdr(b, 9) + SH_OFFSET;
            put32(b, at, len - 1);
        },
        ElfError::SectionOutsideFile {
            index: 9,
            offset: len - 1,
            size: shstr_size,
        },
    );
}

// ---- Never a panic ----

/// A fixed-seed integer generator (the project's tests use no clocks or randomness).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

#[test]
fn no_prefix_of_a_good_file_panics() {
    let b = good().build();
    for n in 0..b.len() {
        let _ = read(&b[..n], BASE, KIND_MODULE);
    }
}

#[test]
fn damaged_files_are_refused_or_read_but_never_panic() {
    let b = good().build();
    let mut rng = Lcg(0x5eed);
    for _ in 0..3000 {
        let mut c = b.clone();
        for _ in 0..1 + rng.next() % 4 {
            let at = rng.next() as usize % c.len();
            match rng.next() % 3 {
                0 => c[at] = rng.next() as u8,
                1 => c[at] ^= 1 << (rng.next() % 8),
                _ => {
                    let at = at & !3;
                    if at + 4 <= c.len() {
                        put32(
                            &mut c,
                            at,
                            [0, 1, 0x7fff_ffff, 0xffff_ffff, 0x40_0000][rng.next() as usize % 5],
                        );
                    }
                }
            }
        }
        for kind in [KIND_MODULE, KIND_APP] {
            let _ = read(&c, BASE, kind);
        }
    }
}

// ---- Dynamic linking, relocation values, and the rest of the review ----

fn push_segment(e: &mut Elf, p_type: u32) {
    e.segments.push(Segment {
        p_type,
        vaddr: 0,
        flags: PF_R,
        data: vec![],
        bss: 0,
    });
}

#[test]
fn refuses_a_dynamic_segment() {
    let mut e = good();
    push_segment(&mut e, PT_DYNAMIC);
    refuse_elf(&e, ElfError::DynamicSegment { index: 3 });
}

#[test]
fn refuses_an_interpreter_segment() {
    let mut e = good();
    push_segment(&mut e, PT_INTERP);
    refuse_elf(&e, ElfError::InterpreterSegment { index: 3 });
}

#[test]
fn refuses_a_dynamic_relocation_section_that_applies_to_no_section() {
    for (name, rela) in [(".rel.dyn", false), (".rela.dyn", true)] {
        let mut e = good();
        let rels = vec![Rel {
            offset: BASE + 0x08,
            r_type: 8,
            symbol: 0,
        }];
        e.sections.push(Section {
            name: name.into(),
            flags: 0,
            addr: 0,
            body: Body::Rel { target: 0, rels, rela },
        });
        refuse_elf(&e, ElfError::RelocationSectionWithoutTarget { index: 14 });
    }
}

#[test]
fn refuses_a_relocation_section_that_is_itself_allocated() {
    let mut e = good();
    e.section(".rel.rodata").flags = SHF_ALLOC;
    refuse_elf(&e, ElfError::AllocatedRelocationSection { index: REL_RODATA });
    // Whatever its target, and whether it is REL or RELA.
    let mut e = good();
    e.sections.push(Section {
        name: ".rela.plt".into(),
        flags: SHF_ALLOC,
        addr: 0,
        body: Body::Rel {
            target: 6,
            rels: vec![],
            rela: true,
        },
    });
    refuse_elf(&e, ElfError::AllocatedRelocationSection { index: 14 });
}

fn with_external_symbol(shndx: Shndx) -> Elf {
    let mut e = good();
    e.symbols.push(Symbol {
        name: "ext".into(),
        value: 0,
        size: 0,
        kind: STT_NOTYPE,
        shndx,
    });
    e
}

#[test]
fn refuses_an_r_386_pc32_against_an_undefined_symbol() {
    let mut e = with_external_symbol(Shndx::Undef);
    e.add_rel(
        ".rel.text",
        Rel {
            offset: BASE + 0x38,
            r_type: R_386_PC32,
            symbol: 8,
        },
    );
    refuse_elf(
        &e,
        ElfError::RelocationAgainstUndefined {
            section: REL_TEXT,
            index: 4,
            symbol: 8,
        },
    );
    let mut e = good();
    e.add_rel(
        ".rel.text",
        Rel {
            offset: BASE + 0x38,
            r_type: R_386_PC32,
            symbol: 0,
        },
    );
    refuse_elf(
        &e,
        ElfError::RelocationAgainstUndefined {
            section: REL_TEXT,
            index: 4,
            symbol: 0,
        },
    );
}

#[test]
fn refuses_an_r_386_pc32_against_an_absolute_symbol() {
    let mut e = with_external_symbol(Shndx::Abs);
    e.add_rel(
        ".rel.text",
        Rel {
            offset: BASE + 0x38,
            r_type: R_386_PC32,
            symbol: 8,
        },
    );
    refuse_elf(
        &e,
        ElfError::RelocationAgainstAbsolute {
            section: REL_TEXT,
            index: 4,
            symbol: 8,
        },
    );
}

#[test]
fn refuses_an_r_386_pc32_against_a_symbol_that_does_not_exist() {
    let mut e = good();
    e.add_rel(
        ".rel.text",
        Rel {
            offset: BASE + 0x38,
            r_type: R_386_PC32,
            symbol: 99,
        },
    );
    refuse_elf(
        &e,
        ElfError::BadSymbolIndex {
            section: REL_TEXT,
            index: 4,
            symbol: 99,
        },
    );
}

#[test]
fn refuses_a_read_only_part_larger_than_the_image() {
    // A read-only segment with a zero-filled tail at the end: nothing writable follows, so the image ends before the read-only part.
    let mut e = good();
    e.segments.pop();
    e.segments[1].bss = 0x10;
    refuse_elf(
        &e,
        ElfError::ReadOnlyLargerThanImage {
            ro_size: 0x1030,
            image_size: 0x1020,
        },
    );
}

#[test]
fn refuses_a_relocated_word_whose_value_is_outside_the_image() {
    let set = |e: &mut Elf, value: u32| e.segments[0].data[0x08..0x0c].copy_from_slice(&value.to_le_bytes());
    for value in [0, BASE - 1, BASE + 0x2041, 0xffff_ffff] {
        let mut e = good();
        set(&mut e, value);
        refuse_elf(&e, ElfError::RelocationValueOutOfRange { offset: 0x08, value });
    }
    // The first byte, and one past the last byte of memory, are valid pointers.
    for value in [BASE, BASE + 0x2040] {
        let mut e = good();
        set(&mut e, value);
        assert_eq!(ok(&e).relocs, ok(&good()).relocs);
    }
}

#[test]
fn refuses_a_name_that_is_not_utf8() {
    let g = good().build();
    let find = |b: &[u8], what: &[u8]| b.windows(what.len()).position(|w| w == what).unwrap();
    // A symbol name.
    let mut b = g.clone();
    let at = find(&b, b"app_start");
    b[at] = 0xff;
    let strtab = get32(&b, shdr(&b, 8) + SH_OFFSET) as usize;
    refuse(
        &b,
        ElfError::BadString {
            index: 8,
            offset: (at - strtab) as u32,
        },
    );
    // A section name.
    let mut b = g.clone();
    let at = find(&b, b".desk.imp");
    b[at + 1] = 0xc3;
    let shstrtab = get32(&b, shdr(&b, 9) + SH_OFFSET) as usize;
    refuse(
        &b,
        ElfError::BadString {
            index: 3,
            offset: (at - shstrtab) as u32,
        },
    );
}

#[test]
fn refuses_a_section_flagged_thread_local_whatever_its_name() {
    let mut e = good();
    e.sections.push(Section {
        name: ".cell".into(),
        flags: SHF_TLS,
        addr: 0,
        body: Body::Bytes(vec![]),
    });
    refuse_elf(&e, ElfError::ThreadLocalSection { index: 14 });
}

#[test]
fn an_import_name_longer_than_the_reader_looks_at_is_still_a_bad_name() {
    let mut e = good();
    e.symbol("__desk_imp_desk_log").name = format!("__desk_imp_{}", "a".repeat(5000));
    refuse_elf(&e, ElfError::ImportBadName { symbol: 6 });
    // The cut can fall inside a character.
    let mut e = good();
    e.symbol("__desk_imp_desk_log").name = format!("__desk_imp_{}", "\u{e9}".repeat(2000));
    refuse_elf(&e, ElfError::ImportBadName { symbol: 6 });
}

#[test]
fn stops_at_the_257th_import_however_many_there_are() {
    let mut e = good();
    e.set_rels(".rel.data", vec![]);
    e.set_rels(
        ".rel.text",
        vec![Rel {
            offset: BASE + 0x08,
            r_type: R_386_32,
            symbol: 1,
        }],
    );
    e.segments[2].data = vec![0; 5000 * 4];
    e.symbols.retain(|s| !s.name.starts_with("__desk_imp_"));
    for i in 0..5000 {
        e.symbols.push(Symbol {
            name: format!("__desk_imp_f{i}"),
            value: BASE + 0x2000 + 4 * i as u32,
            size: 4,
            kind: STT_OBJECT,
            shndx: Shndx::Section(".desk.imp".into()),
        });
    }
    refuse_elf(&e, ElfError::TooManyImports { count: 257 });
}
