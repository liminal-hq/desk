// Tests of packing, `info` and `cross`: the container a good ELF file becomes, what the packer refuses, and the cross check's verdicts.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
mod elfgen;

use desk_module::{crc32, Builder, FormatError, Module, KIND_APP, KIND_MODULE};
use dmdpack::pack::{ALIGN, DEFAULT_SDK};
use dmdpack::{cross, info, pack, CrossError, ElfError, PackError};
use elfgen::*;

const OTHER_BASE: u32 = 0x0080_0000;

fn packed() -> Vec<u8> {
    pack(&good().build(), KIND_MODULE, BASE, DEFAULT_SDK).expect("a good module packs")
}

/// The module `good()` describes as an application: `desk_main` alone.
fn good_app() -> Elf {
    let mut e = good();
    e.symbol("app_start").name = "desk_main".into();
    e.symbol("app_event").name = "helper_a".into();
    e.symbol("app_stop").name = "helper_b".into();
    e
}

/// The same module linked at `OTHER_BASE`, as the linker would do it: every relocated word holds its address plus the move.
fn rebased(file: &[u8], base: u32) -> Vec<u8> {
    let m = Module::parse(file, u16::MAX).unwrap();
    let h = m.header();
    let delta = base.wrapping_sub(h.link_base);
    let mut image = m.image().to_vec();
    for r in m.relocations() {
        let at = r as usize;
        let v = u32::from_le_bytes(image[at..at + 4].try_into().unwrap()).wrapping_add(delta);
        image[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    Builder {
        sdk: h.sdk,
        kind: h.kind,
        align: h.align,
        link_base: base,
        image,
        mem_size: h.mem_size,
        ro_size: h.ro_size,
        start: h.start,
        event: h.event,
        stop: h.stop,
        relocs: m.relocations().collect(),
        imports: m.imports().map(|i| (i.name.to_string(), i.slot)).collect(),
    }
    .write()
    .unwrap()
}

/// Recomputes the checksum of a file whose bytes a test changed.
fn resum(mut file: Vec<u8>) -> Vec<u8> {
    file[84..88].copy_from_slice(&[0; 4]);
    let crc = crc32(&file);
    file[84..88].copy_from_slice(&crc.to_le_bytes());
    file
}

// ---- Packing ----

#[test]
fn a_good_module_becomes_the_container_the_reader_described() {
    let file = packed();
    let m = Module::parse(&file, DEFAULT_SDK).expect("the packed file parses");
    let h = m.header();
    assert_eq!((h.kind, h.sdk, h.align, h.link_base), (KIND_MODULE, 1, ALIGN, BASE));
    assert_eq!((h.file_size, h.mem_size, h.ro_size), (0x2010, 0x2040, 0x1020));
    assert_eq!((h.start, h.event, h.stop), (0, 0x10, 0x20));
    assert_eq!(h.export_count, 0);
    // The relocations the reader found: the two words of `.text` that are absolute, the one in `.rodata` and the one in `.data`.
    assert_eq!(m.relocations().collect::<Vec<_>>(), [0x08, 0x0c, 0x1008, 0x2008]);
    // The imports in slot order, with their names.
    let imports: Vec<(&str, u32)> = m.imports().map(|i| (i.name, i.slot)).collect();
    assert_eq!(imports, [("desk_log", 0x2000), ("desk_alloc", 0x2004)]);
    assert_eq!(m.image().len(), 0x2010);
    assert_eq!(&m.image()[0x1000..0x1006], b"hello\n");
}

#[test]
fn an_application_packs_with_one_entry_and_the_kind_2() {
    let file = pack(&good_app().build(), KIND_APP, BASE, DEFAULT_SDK).expect("a good application packs");
    let m = Module::parse(&file, DEFAULT_SDK).unwrap();
    assert_eq!(
        (m.header().kind, m.header().start, m.header().event, m.header().stop),
        (KIND_APP, 0, 0, 0)
    );
    // The same file is not an application: it defines `app_event`.
    assert_eq!(
        pack(&good().build(), KIND_APP, BASE, DEFAULT_SDK),
        Err(PackError::Elf(ElfError::ApplicationHasModuleEntry { name: "app_event" }))
    );
    // And an application is not a module: it has no `app_start`.
    assert_eq!(
        pack(&good_app().build(), KIND_MODULE, BASE, DEFAULT_SDK),
        Err(PackError::Elf(ElfError::MissingEntry { name: "app_start" }))
    );
}

#[test]
fn the_sdk_version_is_written_as_given() {
    let file = pack(&good().build(), KIND_MODULE, BASE, 3).unwrap();
    assert_eq!(Module::parse(&file, 3).unwrap().header().sdk, 3);
    assert_eq!(
        Module::parse(&file, 2).err(),
        Some(FormatError::SdkTooNew { needs: 3, have: 2 })
    );
}

#[test]
fn a_link_at_another_base_is_refused_unless_it_is_asked_for() {
    let e = good().build();
    assert_eq!(
        pack(&e, KIND_MODULE, OTHER_BASE, DEFAULT_SDK),
        Err(PackError::Elf(ElfError::WrongLinkBase {
            found: BASE,
            wanted: OTHER_BASE
        }))
    );
}

#[test]
fn an_object_file_is_refused() {
    let mut e = good().build();
    put16(&mut e, E_TYPE, 1);
    assert_eq!(
        pack(&e, KIND_MODULE, BASE, DEFAULT_SDK),
        Err(PackError::Elf(ElfError::NotExecutable { e_type: 1 }))
    );
}

#[test]
fn a_library_and_garbage_are_refused() {
    assert_eq!(
        pack(&good().build(), 3, BASE, DEFAULT_SDK),
        Err(PackError::Elf(ElfError::UnsupportedKind { kind: 3 }))
    );
    assert_eq!(
        pack(b"not an elf", KIND_MODULE, BASE, DEFAULT_SDK),
        Err(PackError::Elf(ElfError::NotElf))
    );
    assert!(pack(&[], KIND_MODULE, BASE, DEFAULT_SDK).is_err());
}

#[test]
fn every_refusal_prints_one_line() {
    let e = PackError::Elf(ElfError::NotElf);
    assert!(!e.to_string().contains('\n'));
    let e = PackError::Format(FormatError::BadMagic);
    assert!(e.to_string().starts_with("the packed file is not a valid image: "));
}

// ---- Info ----

#[test]
fn info_names_the_kind_the_counts_and_the_imports() {
    let text = info(&packed()).unwrap();
    for line in [
        "kind          module (.DMD)",
        "abi           i486 flat",
        "sdk           1",
        "header size   96",
        "link base     0x00400000",
        "alignment     16",
        "image bytes   8208",
        "memory bytes  8256",
        "relocations   4",
        "imports       2",
        "exports       0",
        "  import desk_log at 0x2000",
        "  import desk_alloc at 0x2004",
    ] {
        assert!(text.lines().any(|l| l == line), "no line {line:?} in\n{text}");
    }
    let app = pack(&good_app().build(), KIND_APP, BASE, DEFAULT_SDK).unwrap();
    assert!(info(&app).unwrap().starts_with("kind          application (.DKX)\n"));
}

#[test]
fn info_refuses_what_is_not_an_image() {
    assert_eq!(info(b"DMD1"), Err(FormatError::TooShort { len: 4 }));
    let mut f = packed();
    f[0] ^= 1;
    assert_eq!(info(&f), Err(FormatError::BadMagic));
}

// ---- Cross ----

#[test]
fn two_links_of_one_module_agree() {
    let a = packed();
    assert_eq!(cross(&a, &rebased(&a, OTHER_BASE)), Ok(()));
    // A link below the first one, and the check the other way round.
    let low = rebased(&a, 0x0010_0000);
    assert_eq!(cross(&a, &low), Ok(()));
    assert_eq!(cross(&low, &a), Ok(()));
}

#[test]
fn a_move_that_changes_the_middle_bytes_exercises_more_than_the_high_half() {
    // A link base is a multiple of 4096, so a move never changes the low 12 bits of a word, but 0x35000 changes bytes 1 and 2 where
    // a move of 0x400000 changes only byte 2 (a loader that added to the high half alone would pass that one). Carries
    // across the bytes are in it too, and the other direction.
    let a = packed();
    let moved = rebased(&a, BASE + 0x35000);
    assert_eq!(cross(&a, &moved), Ok(()));
    assert_eq!(cross(&moved, &a), Ok(()));
    // 0x4000_0000 more changes byte 3 (a link base up to the end of the 32-bit space is allowed).
    let high = rebased(&a, BASE + 0x4000_0000);
    assert_eq!(cross(&a, &high), Ok(()));
    assert_eq!(cross(&high, &a), Ok(()));
    // A relocated word with any one of its four bytes wrong is caught, so the comparison covers every byte of every word.
    let m = Module::parse(&moved, u16::MAX).unwrap();
    let word = m.header().image_offset as usize + m.relocations().next().unwrap() as usize;
    for byte in 0..4 {
        let mut bad = moved.clone();
        bad[word + byte] ^= 0x01;
        match cross(&a, &resum(bad)) {
            Err(CrossError::ImageDiffers { .. }) => {}
            other => panic!("a wrong byte {byte} of a relocated word was not caught: {other:?}"),
        }
    }
}

#[test]
fn two_files_at_one_link_base_prove_nothing_and_are_refused() {
    let a = packed();
    assert_eq!(cross(&a, &a), Err(CrossError::SameBase(BASE)));
    // A copy that differs only in its checksum is the same case: the base is what is compared, before anything else.
    let b = rebased(&a, BASE);
    assert_eq!(cross(&a, &b), Err(CrossError::SameBase(BASE)));
    assert!(CrossError::SameBase(BASE).to_string().contains("0x400000"));
}

#[test]
fn a_changed_relocated_word_is_named_by_its_offset() {
    let a = packed();
    let mut b = rebased(&a, OTHER_BASE);
    let at = 96 + 0x1008;
    b[at] ^= 0x10;
    let b = resum(b);
    match cross(&a, &b) {
        Err(CrossError::ImageDiffers { offset, .. }) => assert_eq!(offset, 0x1008),
        other => panic!("expected the changed word to be named, got {other:?}"),
    }
}

#[test]
fn a_module_linked_without_emit_relocs_is_caught() {
    // Without `--emit-relocs` the ELF has no relocation sections, so the packer lists none; the file is valid and its words are the
    // link base's. The other link, made correctly, has them moved.
    let right = packed();
    let m = Module::parse(&right, u16::MAX).unwrap();
    let h = m.header();
    let no_relocs = Builder {
        sdk: h.sdk,
        kind: h.kind,
        align: h.align,
        link_base: h.link_base,
        image: m.image().to_vec(),
        mem_size: h.mem_size,
        ro_size: h.ro_size,
        start: h.start,
        event: h.event,
        stop: h.stop,
        relocs: Vec::new(),
        imports: m.imports().map(|i| (i.name.to_string(), i.slot)).collect(),
    }
    .write()
    .unwrap();
    let second = rebased(&right, OTHER_BASE);
    // The first byte of the first word the second link moved (0x00401000 against 0x00801000: the third byte) is named.
    match cross(&no_relocs, &second) {
        Err(CrossError::ImageDiffers { offset, .. }) => assert_eq!(offset, 0x0a),
        other => panic!("expected the unmoved word at 0x8, got {other:?}"),
    }
    // A table that lists a word whose value is no address of the image cannot be loaded: the check says so rather than comparing.
    let mut bad = Builder {
        sdk: h.sdk,
        kind: h.kind,
        align: h.align,
        link_base: h.link_base,
        image: m.image().to_vec(),
        mem_size: h.mem_size,
        ro_size: h.ro_size,
        start: h.start,
        event: h.event,
        stop: h.stop,
        relocs: m.relocations().collect(),
        imports: m.imports().map(|i| (i.name.to_string(), i.slot)).collect(),
    };
    bad.relocs[3] = 0x1010;
    match cross(&bad.write().unwrap(), &second) {
        Err(CrossError::Load(why)) => assert!(why.contains("relocation"), "{why}"),
        other => panic!("expected a load refusal, got {other:?}"),
    }
}

#[test]
fn differing_relocation_tables_are_named_by_entry() {
    let a = packed();
    let second = rebased(&a, OTHER_BASE);
    let m = Module::parse(&second, u16::MAX).unwrap();
    let h = m.header();
    // The second link's image is right but its table lists a different last word, so the bytes agree and the tables do not.
    let mut relocs: Vec<u32> = m.relocations().collect();
    relocs[3] = 0x100c;
    let b = Builder {
        sdk: h.sdk,
        kind: h.kind,
        align: h.align,
        link_base: h.link_base,
        image: m.image().to_vec(),
        mem_size: h.mem_size,
        ro_size: h.ro_size,
        start: h.start,
        event: h.event,
        stop: h.stop,
        relocs,
        imports: m.imports().map(|i| (i.name.to_string(), i.slot)).collect(),
    }
    .write()
    .unwrap();
    assert_eq!(
        cross(&a, &b),
        Err(CrossError::RelocsDiffer {
            index: 3,
            a: Some(0x2008),
            b: Some(0x100c)
        })
    );
    // With the images equal, a table with an extra entry differs where the shorter one ends.
    let mut longer: Vec<u32> = m.relocations().collect();
    longer.push(0x200c);
    let c = Builder {
        sdk: h.sdk,
        kind: h.kind,
        align: h.align,
        link_base: h.link_base,
        image: m.image().to_vec(),
        mem_size: h.mem_size,
        ro_size: h.ro_size,
        start: h.start,
        event: h.event,
        stop: h.stop,
        relocs: longer,
        imports: m.imports().map(|i| (i.name.to_string(), i.slot)).collect(),
    }
    .write()
    .unwrap();
    assert_eq!(
        cross(&a, &c),
        Err(CrossError::RelocsDiffer {
            index: 4,
            a: None,
            b: Some(0x200c)
        })
    );
}

#[test]
fn layout_differences_are_named() {
    let a = packed();
    let m = Module::parse(&a, u16::MAX).unwrap();
    let h = *m.header();
    let make = |f: &dyn Fn(&mut Builder)| {
        let mut b = Builder {
            sdk: h.sdk,
            kind: h.kind,
            align: h.align,
            link_base: OTHER_BASE,
            image: Module::parse(&rebased(&a, OTHER_BASE), u16::MAX).unwrap().image().to_vec(),
            mem_size: h.mem_size,
            ro_size: h.ro_size,
            start: h.start,
            event: h.event,
            stop: h.stop,
            relocs: m.relocations().collect(),
            imports: m.imports().map(|i| (i.name.to_string(), i.slot)).collect(),
        };
        f(&mut b);
        b.write().unwrap()
    };
    let layout = |field: &'static str, a: u32, b: u32| Err(CrossError::Layout { field, a, b });
    assert_eq!(
        cross(&a, &make(&|b| b.mem_size += 16)),
        layout("memory size", h.mem_size, h.mem_size + 16)
    );
    assert_eq!(cross(&a, &make(&|b| b.start = 4)), layout("start", 0, 4));
    assert_eq!(cross(&a, &make(&|b| b.stop = 0x24)), layout("stop", 0x20, 0x24));
    assert_eq!(
        cross(
            &a,
            &make(&|b| {
                b.kind = KIND_APP;
                b.event = 0;
                b.stop = 0;
            })
        ),
        layout("kind", 1, 2)
    );
    assert_eq!(
        cross(&a, &make(&|b| b.imports[1].0 = "desk_free".into())),
        Err(CrossError::ImportsDiffer { index: 1 })
    );
}

#[test]
fn a_file_that_is_not_an_image_is_named_by_side() {
    let a = packed();
    assert!(matches!(cross(b"junk", &a), Err(CrossError::Format { which: 'a', .. })));
    assert!(matches!(cross(&a, b"junk"), Err(CrossError::Format { which: 'b', .. })));
}

#[test]
fn every_cross_error_prints_one_line() {
    let errors = [
        CrossError::Format {
            which: 'a',
            error: FormatError::BadMagic,
        },
        CrossError::Load("x".into()),
        CrossError::SameBase(0x40_0000),
        CrossError::Layout {
            field: "start",
            a: 1,
            b: 2,
        },
        CrossError::ImageDiffers { offset: 3, a: 1, b: 2 },
        CrossError::RelocsDiffer {
            index: 1,
            a: Some(4),
            b: None,
        },
        CrossError::ImportsDiffer { index: 0 },
    ];
    for e in errors {
        assert!(!e.to_string().contains('\n'), "{e}");
    }
}
