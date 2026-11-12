// Every way a file is refused, one test each: a valid file damaged in one place, expecting that exact error.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use desk_module::{
    crc32, Builder, FormatError, Module, ABI_I486_FLAT, HEADER_SIZE, KIND_APP, KIND_LIBRARY, KIND_MODULE, MAX_IMPORTS,
    MAX_MEMORY, MAX_NAME,
};

#[test]
fn the_sample_is_valid() {
    let bytes = sample_bytes();
    let m = parse(&bytes).unwrap();
    assert_eq!(m.header().kind, KIND_MODULE);
    assert_eq!(parse(&sample_odd().write().unwrap()).map(|m| m.image().len()), Ok(0x41));
}

// The header.

#[test]
fn too_short() {
    let bytes = sample_bytes();
    assert_eq!(refusal(&[]), FormatError::TooShort { len: 0 });
    assert_eq!(refusal(&bytes[..4]), FormatError::TooShort { len: 4 });
    assert_eq!(refusal(&bytes[..95]), FormatError::TooShort { len: 95 });
}

#[test]
fn bad_magic() {
    for magic in [*b"XMD1", *b"dmd1", *b"DMDx", *b"\0\0\0\0", *b"MZ\x90\0"] {
        let bytes = damaged(|b| b[0..4].copy_from_slice(&magic));
        assert_eq!(refusal(&bytes), FormatError::BadMagic, "{magic:?}");
    }
}

#[test]
fn another_major_version_is_not_a_bad_magic() {
    for digit in *b"029" {
        let bytes = damaged(|b| b[3] = digit);
        assert_eq!(refusal(&bytes), FormatError::UnsupportedVersion(digit - b'0'));
    }
}

#[test]
fn header_size() {
    // 80 was the first draft's header; a header may grow later, but never shrink, and stays a multiple of 4.
    for size in [0, 80, 92, 95, 97, 98] {
        let bytes = damaged(|b| set16(b, off::HEADER_SIZE, size));
        assert_eq!(refusal(&bytes), FormatError::BadHeaderSize(size), "{size}");
    }
    assert_eq!(
        sample_bytes()[off::HEADER_SIZE..off::HEADER_SIZE + 2],
        HEADER_SIZE.to_le_bytes()
    );
    assert_eq!(HEADER_SIZE, 96);
    assert_eq!(Builder::default().write().unwrap().len(), 96);
}

#[test]
fn unknown_flags() {
    for flags in [1, 0x8000, 0xFFFF] {
        let bytes = damaged(|b| set16(b, off::FLAGS, flags));
        assert_eq!(refusal(&bytes), FormatError::UnknownFlags(flags));
    }
}

#[test]
fn reserved_fields_must_be_zero() {
    let bytes = damaged(|b| b[14] = 1);
    assert_eq!(refusal(&bytes), FormatError::ReservedNotZero { offset: 14 });
    for at in [88, 91, 95] {
        let bytes = damaged(|b| b[at] = 1);
        assert_eq!(refusal(&bytes), FormatError::ReservedNotZero { offset: 88 }, "byte {at}");
    }
}

#[test]
fn unsupported_abi() {
    for abi in [0, 2, 0xFFFF] {
        let bytes = damaged(|b| set16(b, off::ABI, abi));
        assert_eq!(refusal(&bytes), FormatError::UnsupportedAbi(abi));
    }
    assert_eq!(ABI_I486_FLAT, 1);
}

#[test]
fn sdk_too_new() {
    let bytes = damaged(|b| set16(b, off::SDK, 2));
    assert_eq!(refusal(&bytes), FormatError::SdkTooNew { needs: 2, have: 1 });
    // The same file is fine for a core that has that table, and an older file is fine for a newer core.
    assert!(Module::parse(&bytes, 2).is_ok());
    assert!(Module::parse(&sample_bytes(), 7).is_ok());
    let bytes = damaged(|b| set16(b, off::SDK, 0xFFFF));
    assert_eq!(refusal(&bytes), FormatError::SdkTooNew { needs: 0xFFFF, have: 1 });
}

#[test]
fn unknown_kind() {
    for kind in [0, 4, 0xFFFF] {
        let bytes = damaged(|b| set16(b, off::KIND, kind));
        assert_eq!(refusal(&bytes), FormatError::UnknownKind(kind));
    }
}

#[test]
fn a_library_is_reserved() {
    let bytes = damaged(|b| set16(b, off::KIND, KIND_LIBRARY));
    assert_eq!(refusal(&bytes), FormatError::KindNotSupported(3));
}

#[test]
fn exports_are_reserved() {
    let bytes = damaged(|b| set32(b, off::EXPORT_COUNT, 1));
    assert_eq!(refusal(&bytes), FormatError::ExportsNotSupported { count: 1 });
    let bytes = damaged(|b| set32(b, off::EXPORT_COUNT, u32::MAX));
    assert_eq!(refusal(&bytes), FormatError::ExportsNotSupported { count: u32::MAX });
}

#[test]
fn the_export_table_sits_where_the_imports_end() {
    let good = get32(&sample_bytes(), off::EXPORT_OFFSET);
    for wrong in [good - 8, good + 4, good + 8, 0, u32::MAX] {
        let bytes = damaged(|b| set32(b, off::EXPORT_OFFSET, wrong));
        assert_eq!(
            refusal(&bytes),
            FormatError::ExportTableMisplaced { offset: wrong },
            "{wrong:#x}"
        );
    }
}

#[test]
fn bad_alignment() {
    for align in [0, 1, 8, 15, 17, 24, 48, 4095, 8192, 1 << 31] {
        let bytes = damaged(|b| set32(b, off::ALIGN, align));
        assert_eq!(refusal(&bytes), FormatError::BadAlignment(align), "{align}");
    }
    for align in [16, 32, 256, 4096] {
        assert!(parse(&damaged(|b| set32(b, off::ALIGN, align))).is_ok(), "{align}");
    }
}

#[test]
fn bad_link_base() {
    for base in [0, 1, 0x40_0001, 0x40_0800, 0xFFFF_FFFF] {
        let bytes = damaged(|b| set32(b, off::LINK_BASE, base));
        assert_eq!(refusal(&bytes), FormatError::BadLinkBase(base), "{base:#x}");
    }
    // Any other multiple of 4096 is fine, as the packer's cross check needs.
    assert!(parse(&damaged(|b| set32(b, off::LINK_BASE, 0x80_0000))).is_ok());
    assert!(parse(&damaged(|b| set32(b, off::LINK_BASE, 0x1000))).is_ok());
}

#[test]
fn a_link_base_that_leaves_no_room_for_the_image() {
    // The last page of the address space cannot hold a 0x2000-byte image.
    let bytes = damaged(|b| {
        set32(b, off::LINK_BASE, 0xFFFF_F000);
        set32(b, off::MEM_SIZE, 0x2000);
    });
    assert_eq!(refusal(&bytes), FormatError::BadLinkBase(0xFFFF_F000));
    let bytes = damaged(|b| {
        set32(b, off::LINK_BASE, 0xFFFF_F000);
        set32(b, off::MEM_SIZE, 0xF00);
    });
    assert!(parse(&bytes).is_ok());
}

#[test]
fn sizes_out_of_order() {
    let bytes = damaged(|b| set32(b, off::FILE_SIZE, MEM_SIZE + 4));
    assert_eq!(refusal(&bytes), FormatError::SizesOutOfOrder);
    let bytes = damaged(|b| set32(b, off::MEM_SIZE, FILE_SIZE - 4));
    assert_eq!(refusal(&bytes), FormatError::SizesOutOfOrder);
    let bytes = damaged(|b| set32(b, off::RO_SIZE, FILE_SIZE + 4));
    assert_eq!(refusal(&bytes), FormatError::SizesOutOfOrder);
}

#[test]
fn too_large() {
    let bytes = damaged(|b| set32(b, off::MEM_SIZE, MAX_MEMORY + 1));
    assert_eq!(refusal(&bytes), FormatError::TooLarge(MAX_MEMORY + 1));
    let bytes = damaged(|b| set32(b, off::MEM_SIZE, u32::MAX));
    assert_eq!(refusal(&bytes), FormatError::TooLarge(u32::MAX));
    assert!(parse(&damaged(|b| set32(b, off::MEM_SIZE, MAX_MEMORY))).is_ok());
}

#[test]
fn too_many_imports() {
    let bytes = damaged(|b| set32(b, off::IMPORT_COUNT, MAX_IMPORTS + 1));
    assert_eq!(refusal(&bytes), FormatError::TooManyImports(MAX_IMPORTS + 1));
    let bytes = damaged(|b| set32(b, off::IMPORT_COUNT, u32::MAX));
    assert_eq!(refusal(&bytes), FormatError::TooManyImports(u32::MAX));
}

#[test]
fn the_most_imports_are_fine() {
    let mut b = sample();
    b.image = vec![0; 0x800];
    b.mem_size = 0x800;
    b.ro_size = 0x40;
    b.relocs = vec![];
    b.imports = (0..MAX_IMPORTS).map(|i| (format!("import_{i}"), 0x100 + i * 4)).collect();
    let bytes = b.write().unwrap();
    assert_eq!(parse(&bytes).unwrap().imports().count(), 256);
}

// The layout.

#[test]
fn a_table_past_the_end_of_the_file() {
    let good = sample_bytes();
    let bytes = damaged(|b| set32(b, off::RELOC_OFFSET, 0x1000_0000));
    assert_eq!(
        refusal(&bytes),
        FormatError::TableOutsideFile {
            table: "relocations",
            offset: 0x1000_0000,
            len: 20
        }
    );
    let bytes = damaged(|b| set32(b, off::RELOC_COUNT, u32::MAX));
    let offset = get32(&bytes, off::RELOC_OFFSET);
    assert_eq!(
        refusal(&bytes),
        FormatError::TableOutsideFile {
            table: "relocations",
            offset,
            len: u32::MAX as u64 * 4
        }
    );
    // One byte more than the file holds, and the exact size.
    let size = get32(&good, off::STRINGS_SIZE);
    let offset = get32(&good, off::STRINGS_OFFSET);
    let bytes = damaged(|b| set32(b, off::STRINGS_SIZE, size + 1));
    assert_eq!(
        refusal(&bytes),
        FormatError::TableOutsideFile {
            table: "strings",
            offset,
            len: size as u64 + 1
        }
    );
    let bytes = damaged(|b| set32(b, off::STRINGS_OFFSET, offset + 1));
    assert_eq!(
        refusal(&bytes),
        FormatError::TableOutsideFile {
            table: "strings",
            offset: offset + 1,
            len: size as u64
        }
    );
    let bytes = damaged(|b| {
        set32(b, off::IMPORT_OFFSET, good.len() as u32 - 15);
        set32(b, off::EXPORT_OFFSET, good.len() as u32 + 1);
    });
    assert!(matches!(
        refusal(&bytes),
        FormatError::TableOutsideFile { table: "imports", .. }
    ));
}

#[test]
fn tables_that_overlap() {
    // The import table moved back onto the relocations (the export table follows it, as the header requires).
    let reloc_offset = get32(&sample_bytes(), off::RELOC_OFFSET);
    let bytes = damaged(|b| {
        set32(b, off::IMPORT_OFFSET, reloc_offset);
        set32(b, off::EXPORT_OFFSET, reloc_offset + 16);
    });
    assert_eq!(
        refusal(&bytes),
        FormatError::TablesOverlap {
            table: "imports",
            offset: reloc_offset
        }
    );
    // The relocations moved back into the image.
    let bytes = damaged(|b| set32(b, off::RELOC_OFFSET, 100));
    assert_eq!(
        refusal(&bytes),
        FormatError::TablesOverlap {
            table: "relocations",
            offset: 100
        }
    );
    // The image moved into the header.
    let bytes = damaged(|b| set32(b, off::IMAGE_OFFSET, 92));
    assert_eq!(
        refusal(&bytes),
        FormatError::TablesOverlap {
            table: "image",
            offset: 92
        }
    );
}

#[test]
fn tables_with_a_gap() {
    let good = get32(&sample_bytes(), off::IMPORT_OFFSET);
    let bytes = damaged(|b| {
        set32(b, off::IMPORT_OFFSET, good + 4);
        set32(b, off::EXPORT_OFFSET, good + 4 + 16);
    });
    assert_eq!(
        refusal(&bytes),
        FormatError::TableGap {
            table: "imports",
            offset: good + 4,
            expected: good
        }
    );
    let bytes = damaged(|b| set32(b, off::IMAGE_OFFSET, 100));
    assert_eq!(
        refusal(&bytes),
        FormatError::TableGap {
            table: "image",
            offset: 100,
            expected: 96
        }
    );
}

#[test]
fn a_string_table_that_starts_early() {
    let good = get32(&sample_bytes(), off::STRINGS_OFFSET);
    let bytes = damaged(|b| set32(b, off::STRINGS_OFFSET, good - 4));
    assert_eq!(
        refusal(&bytes),
        FormatError::TablesOverlap {
            table: "strings",
            offset: good - 4
        }
    );
}

#[test]
fn padding_must_be_zero() {
    let good = sample_odd().write().unwrap();
    for pad in 0..3 {
        let mut bytes = good.clone();
        bytes[96 + 0x41 + pad] = 0x55;
        refresh_crc(&mut bytes);
        assert_eq!(
            refusal(&bytes),
            FormatError::NonZeroPadding {
                at: 96 + 0x41 + pad as u32
            }
        );
    }
    assert!(parse(&good).is_ok());
}

#[test]
fn trailing_bytes() {
    let good = sample_bytes();
    for extra in [1, 3, 4, 100] {
        let bytes = damaged(|b| b.extend(std::iter::repeat_n(0, extra)));
        assert_eq!(
            refusal(&bytes),
            FormatError::TrailingBytes { at: good.len() as u32 },
            "{extra}"
        );
    }
}

#[test]
fn bad_checksum() {
    let good = sample_bytes();
    let stored = get32(&good, off::CRC);
    let mut bytes = good.clone();
    bytes[0x70] ^= 0x10;
    let mut zeroed = bytes.clone();
    zeroed[off::CRC..off::CRC + 4].fill(0);
    assert_eq!(
        refusal(&bytes),
        FormatError::BadChecksum {
            stored,
            computed: crc32(&zeroed)
        }
    );
    let mut bytes = good;
    set32(&mut bytes, off::CRC, stored ^ 1);
    assert_eq!(
        refusal(&bytes),
        FormatError::BadChecksum {
            stored: stored ^ 1,
            computed: stored
        }
    );
}

#[test]
fn a_single_flipped_bit_anywhere_is_refused() {
    let good = sample_bytes();
    for at in 0..good.len() {
        for bit in 0..8 {
            let mut bytes = good.clone();
            bytes[at] ^= 1 << bit;
            let e = refusal(&bytes);
            // Past the header the layout is intact, so the checksum is what finds the flip.
            if at >= 96 || (off::CRC..off::CRC + 4).contains(&at) {
                assert!(matches!(e, FormatError::BadChecksum { .. }), "byte {at} bit {bit}: {e}");
            }
        }
    }
}

#[test]
fn every_prefix_is_refused() {
    for good in [
        sample_bytes(),
        sample_odd().write().unwrap(),
        Builder::default().write().unwrap(),
    ] {
        for n in 0..good.len() {
            let e = refusal(&good[..n]);
            if n < 96 {
                assert_eq!(e, FormatError::TooShort { len: n });
            } else {
                assert!(matches!(e, FormatError::TableOutsideFile { .. }), "{n} bytes: {e}");
            }
        }
    }
}

// The entry points.

#[test]
fn entries_must_be_in_the_read_only_part() {
    for at in [RO_SIZE, RO_SIZE + 4, FILE_SIZE, u32::MAX] {
        let bytes = damaged(|b| set32(b, off::START, at));
        assert_eq!(
            refusal(&bytes),
            FormatError::EntryOutOfRange {
                which: "start",
                offset: at
            }
        );
        let bytes = damaged(|b| set32(b, off::EVENT, at));
        assert_eq!(
            refusal(&bytes),
            FormatError::EntryOutOfRange {
                which: "event",
                offset: at
            }
        );
        let bytes = damaged(|b| set32(b, off::STOP, at));
        assert_eq!(
            refusal(&bytes),
            FormatError::EntryOutOfRange {
                which: "stop",
                offset: at
            }
        );
    }
    // The last read-only byte is in range, and zero means none.
    assert!(parse(&damaged(|b| set32(b, off::START, RO_SIZE - 1))).is_ok());
    assert!(parse(&damaged(|b| set32(b, off::EVENT, 0))).is_ok());
}

#[test]
fn an_application_has_one_entry() {
    let app = |event, stop| {
        Builder {
            kind: KIND_APP,
            event,
            stop,
            ..sample()
        }
        .write()
        .unwrap()
    };
    assert!(parse(&app(0, 0)).is_ok());
    assert_eq!(
        refusal(&app(0x20, 0)),
        FormatError::EntryOutOfRange {
            which: "event",
            offset: 0x20
        }
    );
    assert_eq!(
        refusal(&app(0, 0x30)),
        FormatError::EntryOutOfRange {
            which: "stop",
            offset: 0x30
        }
    );
}

// The relocations.

fn with_relocs(relocs: &[u32]) -> Vec<u8> {
    Builder {
        relocs: relocs.to_vec(),
        ..sample()
    }
    .write()
    .unwrap()
}

#[test]
fn relocations_out_of_order() {
    assert_eq!(
        refusal(&with_relocs(&[0x4C, 0x44])),
        FormatError::RelocsOutOfOrder { index: 1 }
    );
    assert_eq!(
        refusal(&with_relocs(&[0x44, 0x4C, 0x48])),
        FormatError::RelocsOutOfOrder { index: 2 }
    );
    assert_eq!(
        refusal(&with_relocs(&[0x44, 0x44])),
        FormatError::RelocsOutOfOrder { index: 1 }
    );
}

#[test]
fn relocations_that_overlap() {
    for gap in 1..4 {
        assert_eq!(
            refusal(&with_relocs(&[0x44, 0x44 + gap])),
            FormatError::RelocsOutOfOrder { index: 1 },
            "{gap}"
        );
    }
    // Exactly 4 apart is fine.
    assert!(parse(&with_relocs(&[0x44, 0x48, 0x4C])).is_ok());
}

#[test]
fn a_relocation_in_the_bss_tail() {
    for at in [FILE_SIZE, FILE_SIZE + 4, MEM_SIZE - 4] {
        assert_eq!(
            refusal(&with_relocs(&[0x44, at])),
            FormatError::RelocOutsideImage { index: 1, offset: at },
            "{at:#x}"
        );
    }
}

#[test]
fn a_relocation_straddling_the_end_of_the_file_part() {
    for at in [FILE_SIZE - 3, FILE_SIZE - 1] {
        assert_eq!(
            refusal(&with_relocs(&[at])),
            FormatError::RelocOutsideImage { index: 0, offset: at }
        );
    }
    assert!(parse(&with_relocs(&[FILE_SIZE - 4])).is_ok());
    assert_eq!(
        refusal(&with_relocs(&[u32::MAX])),
        FormatError::RelocOutsideImage {
            index: 0,
            offset: u32::MAX
        }
    );
}

// The imports.

/// The sample with other imports. A slot that lands inside the image is zeroed, so only the slot's position is at issue.
fn with_imports(imports: &[(&str, u32)]) -> Vec<u8> {
    let mut b = Builder {
        imports: imports.iter().map(|(n, s)| (n.to_string(), *s)).collect(),
        ..sample()
    };
    for (_, slot) in &b.imports {
        if let Some(word) = b.image.get_mut(*slot as usize..*slot as usize + 4) {
            word.fill(0);
        }
    }
    b.write().unwrap()
}

#[test]
fn bad_import_names() {
    let long = "a".repeat(MAX_NAME + 1);
    let edge = "a".repeat(MAX_NAME);
    for bad in ["", "Desk_log", "DESK", "desk-log", "desk log", "desk.log", "dësk", &long] {
        let e = refusal(&with_imports(&[("ok_name", SLOT_LOG), (bad, SLOT_ALLOC)]));
        assert_eq!(e, FormatError::BadImportName { index: 1 }, "{bad:?}");
    }
    assert!(parse(&with_imports(&[(&edge, SLOT_LOG)])).is_ok());
    assert!(parse(&with_imports(&[("a_0_9", SLOT_LOG)])).is_ok());
}

#[test]
fn a_name_with_a_nul_cannot_be_written() {
    let b = Builder {
        imports: vec![("a\0b".to_string(), SLOT_LOG)],
        ..sample()
    };
    assert_eq!(b.write(), Err(FormatError::BadImportName { index: 0 }));
}

#[test]
fn a_name_not_where_the_string_table_puts_it() {
    let import_offset = get32(&sample_bytes(), off::IMPORT_OFFSET) as usize;
    let bytes = damaged(|b| {
        let at = import_offset + 8;
        let v = get32(b, at);
        set32(b, at, v + 1);
    });
    assert_eq!(refusal(&bytes), FormatError::BadImportName { index: 1 });
    let bytes = damaged(|b| set32(b, import_offset, 1));
    assert_eq!(refusal(&bytes), FormatError::BadImportName { index: 0 });
}

#[test]
fn a_name_without_its_nul() {
    let bytes = damaged(|b| *b.last_mut().unwrap() = b'x');
    assert_eq!(refusal(&bytes), FormatError::BadImportName { index: 1 });
}

#[test]
fn a_string_table_with_something_else_in_it() {
    let mut bytes = sample_bytes();
    let size = get32(&bytes, off::STRINGS_SIZE);
    bytes.extend_from_slice(b"zz\0");
    set32(&mut bytes, off::STRINGS_SIZE, size + 3);
    refresh_crc(&mut bytes);
    assert_eq!(
        refusal(&bytes),
        FormatError::BadStringTable {
            expected: size,
            size: size + 3
        }
    );
}

#[test]
fn a_duplicate_import() {
    let e = refusal(&with_imports(&[
        ("desk_log", SLOT_LOG),
        ("desk_alloc", SLOT_ALLOC),
        ("desk_log", 0x6C),
    ]));
    assert_eq!(e, FormatError::DuplicateImport { index: 2 });
    let e = refusal(&with_imports(&[("a", SLOT_LOG), ("a", SLOT_ALLOC)]));
    assert_eq!(e, FormatError::DuplicateImport { index: 1 });
}

#[test]
fn a_slot_in_the_read_only_part() {
    for slot in [0, 0x20, RO_SIZE - 4] {
        let e = refusal(&with_imports(&[("desk_log", slot)]));
        assert_eq!(e, FormatError::BadImportSlot { index: 0, slot }, "{slot:#x}");
    }
    assert!(parse(&with_imports(&[("desk_log", RO_SIZE)])).is_ok());
}

#[test]
fn a_slot_not_aligned() {
    for slot in [0x61, 0x62, 0x63] {
        let e = refusal(&with_imports(&[("desk_log", slot)]));
        assert_eq!(e, FormatError::BadImportSlot { index: 0, slot });
    }
}

#[test]
fn a_slot_outside_the_file_part() {
    for slot in [FILE_SIZE, FILE_SIZE + 4, MEM_SIZE - 4, u32::MAX - 3] {
        let e = refusal(&with_imports(&[("desk_log", SLOT_LOG), ("desk_alloc", slot)]));
        assert_eq!(e, FormatError::BadImportSlot { index: 1, slot }, "{slot:#x}");
    }
    assert!(parse(&with_imports(&[("desk_log", FILE_SIZE - 4)])).is_ok());
}

#[test]
fn a_slot_not_zero_in_the_file() {
    for at in 0..4 {
        let mut b = sample();
        b.image[SLOT_ALLOC as usize + at] = 1;
        let e = refusal(&b.write().unwrap());
        assert_eq!(
            e,
            FormatError::BadImportSlot {
                index: 1,
                slot: SLOT_ALLOC
            }
        );
    }
}

#[test]
fn a_slot_shared_by_two_imports() {
    let e = refusal(&with_imports(&[("desk_log", SLOT_LOG), ("desk_alloc", SLOT_LOG)]));
    assert_eq!(
        e,
        FormatError::BadImportSlot {
            index: 1,
            slot: SLOT_LOG
        }
    );
}

#[test]
fn a_slot_that_is_also_a_relocation() {
    // The relocated word is the slot itself, or overlaps it by one to three bytes on either side.
    for reloc in [SLOT_LOG - 3, SLOT_LOG - 2, SLOT_LOG - 1, SLOT_LOG, SLOT_LOG + 1, SLOT_LOG + 3] {
        let mut b = sample();
        b.relocs = vec![0x44, reloc];
        b.imports.truncate(1);
        let e = refusal(&b.write().unwrap());
        assert_eq!(
            e,
            FormatError::BadImportSlot {
                index: 0,
                slot: SLOT_LOG
            },
            "{reloc:#x}"
        );
    }
    // Words right next to the slots (0x5C and 0x68 in the sample) are not in the way.
    assert!(parse(&sample_bytes()).is_ok());
}
