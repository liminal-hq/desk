// Building, parsing and loading: the round trip, the CRC, relocation at many addresses, binding, the reserved fields and the
// refusals that need a destination. The last test mutates a valid file 10,000 times and expects an error or a result, never a panic.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod common;

use common::*;
use desk_module::{
    crc32, load_into, Builder, FormatError, Import, LoadError, Module, Placed, KIND_APP, KIND_LIBRARY, KIND_MODULE, LINK_BASE,
};
use std::cell::{Cell, RefCell};

const LOG: u32 = 0x0001_0000;
const ALLOC: u32 = 0x0002_0000;

fn resolve(name: &str) -> Option<u32> {
    match name {
        "desk_log" => Some(LOG),
        "desk_alloc" => Some(ALLOC),
        _ => None,
    }
}

/// Loads the sample at `addr` into a buffer of `mem_size` bytes filled with 0xAA.
fn load_at(addr: u32) -> (Vec<u8>, Placed) {
    let bytes = sample_bytes();
    let m = parse(&bytes).unwrap();
    let mut dest = vec![0xAA; MEM_SIZE as usize];
    let placed = load_into(&m, KIND_MODULE, &mut dest, addr, &resolve).unwrap();
    (dest, placed)
}

#[test]
fn the_crc_of_the_check_string() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b""), 0);
}

#[test]
fn a_builder_round_trips() {
    let b = sample();
    let bytes = b.write().unwrap();
    let m = parse(&bytes).unwrap();
    let h = m.header();
    assert_eq!((h.header_size, h.flags, h.abi, h.sdk, h.kind), (96, 0, 1, 1, KIND_MODULE));
    assert_eq!((h.link_base, h.align), (b.link_base, b.align));
    assert_eq!(
        (h.image_offset, h.file_size, h.mem_size, h.ro_size),
        (96, FILE_SIZE, MEM_SIZE, RO_SIZE)
    );
    assert_eq!((h.start, h.event, h.stop), (b.start, b.event, b.stop));
    assert_eq!((h.reloc_count, h.import_count, h.export_count), (5, 2, 0));
    assert_eq!(h.export_offset, h.import_offset + 16);
    assert_eq!(h.strings_offset, h.export_offset);
    assert_eq!(h.strings_size, ("desk_log".len() + 1 + "desk_alloc".len() + 1) as u32);
    assert_eq!(bytes.len() as u32, h.strings_offset + h.strings_size);
    assert_eq!(m.image(), &b.image[..]);
    assert_eq!(m.relocations().collect::<Vec<_>>(), b.relocs);
    let imports: Vec<Import> = m.imports().collect();
    assert_eq!(
        imports,
        [
            Import {
                name: "desk_log",
                slot: SLOT_LOG
            },
            Import {
                name: "desk_alloc",
                slot: SLOT_ALLOC
            }
        ]
    );
    // The stored checksum is the CRC of the file with its own field zero.
    let mut zeroed = bytes.clone();
    zeroed[off::CRC..off::CRC + 4].fill(0);
    assert_eq!(h.crc, crc32(&zeroed));
    // Writing again gives the same bytes.
    assert_eq!(b.write().unwrap(), bytes);
}

#[test]
fn the_header_fields_are_where_the_magic_file_says() {
    let bytes = sample_bytes();
    assert_eq!(&bytes[0..4], b"DMD1");
    let word = |o: usize| u16::from_le_bytes([bytes[o], bytes[o + 1]]);
    assert_eq!((word(4), word(8), word(10), word(12)), (96, 1, 1, KIND_MODULE));
}

#[test]
fn an_empty_module_is_a_header_alone() {
    let bytes = Builder::default().write().unwrap();
    assert_eq!(bytes.len(), 96);
    assert!(matches!(
        parse(&bytes),
        Err(FormatError::EntryOutOfRange { which: "start", .. })
    ));
    let bytes = Builder {
        image: vec![0xC3],
        ro_size: 1,
        mem_size: 1,
        ..Builder::default()
    }
    .write()
    .unwrap();
    let m = parse(&bytes).unwrap();
    assert_eq!(m.image(), [0xC3]);
    assert_eq!(m.relocations().count() + m.imports().count(), 0);
}

#[test]
fn kind_round_trips_for_modules_and_applications() {
    for kind in [KIND_MODULE, KIND_APP] {
        let bytes = Builder {
            kind,
            event: 0,
            stop: 0,
            ..sample()
        }
        .write()
        .unwrap();
        assert_eq!(parse(&bytes).unwrap().header().kind, kind);
        assert_eq!(u16::from_le_bytes([bytes[12], bytes[13]]), kind);
    }
}

#[test]
fn loading_at_the_link_base_changes_only_the_slots() {
    let (dest, placed) = load_at(LINK_BASE);
    let stored = sample().image;
    let mut expected = stored.clone();
    expected[SLOT_LOG as usize..][..4].copy_from_slice(&LOG.to_le_bytes());
    expected[SLOT_ALLOC as usize..][..4].copy_from_slice(&ALLOC.to_le_bytes());
    assert_eq!(&dest[..FILE_SIZE as usize], &expected[..]);
    assert_eq!(
        placed,
        Placed {
            base: LINK_BASE,
            end: LINK_BASE + MEM_SIZE,
            start: LINK_BASE + 0x10,
            event: Some(LINK_BASE + 0x20),
            stop: Some(LINK_BASE + 0x30)
        }
    );
}

#[test]
fn the_bss_tail_is_zeroed_over_whatever_was_there() {
    let (dest, _) = load_at(LINK_BASE);
    assert!(dest[FILE_SIZE as usize..].iter().all(|&b| b == 0));
    assert_eq!(dest.len(), MEM_SIZE as usize);
}

#[test]
fn bytes_past_the_memory_size_are_not_touched() {
    let bytes = sample_bytes();
    let m = parse(&bytes).unwrap();
    let mut dest = vec![0xAA; MEM_SIZE as usize + 32];
    load_into(&m, KIND_MODULE, &mut dest, LINK_BASE, &resolve).unwrap();
    assert!(dest[MEM_SIZE as usize..].iter().all(|&b| b == 0xAA));
}

/// What loading at `addr` must give: every listed word moved by the delta, every slot bound, and no other byte changed.
fn check_relocated(addr: u32) {
    let (dest, placed) = load_at(addr);
    let stored = sample().image;
    let delta = addr.wrapping_sub(LINK_BASE);
    let word = |image: &[u8], o: u32| get32(image, o as usize);
    for o in 0..FILE_SIZE {
        let in_slot = (SLOT_LOG..SLOT_ALLOC + 4).contains(&o);
        let in_reloc = RELOCS.iter().any(|&r| (r..r + 4).contains(&o));
        if !in_slot && !in_reloc {
            assert_eq!(dest[o as usize], stored[o as usize], "byte {o:#x} at {addr:#x}");
        }
    }
    for &r in &RELOCS {
        assert_eq!(
            word(&dest, r),
            word(&stored, r).wrapping_add(delta),
            "word {r:#x} at {addr:#x}"
        );
        // Each image minus its delta at the listed words equals the stored image.
        assert_eq!(word(&dest, r).wrapping_sub(delta), word(&stored, r));
    }
    assert_eq!(word(&dest, SLOT_LOG), LOG);
    assert_eq!(word(&dest, SLOT_ALLOC), ALLOC);
    assert_eq!(placed.base, addr);
    assert_eq!(placed.end, addr + MEM_SIZE);
    assert_eq!(placed.start, addr + 0x10);
}

#[test]
fn loading_above_the_link_base_moves_every_listed_word_by_the_delta() {
    check_relocated(LINK_BASE + 0x1000);
    check_relocated(0x0010_0000);
}

#[test]
fn loading_at_eight_addresses_from_a_generator() {
    let mut rng = Rng(0x2545_F491);
    let mut addrs = vec![
        0xFFFF_FEF0, // MEM_SIZE below the top of the address space: the end is 0xFFFF_FFF0
        0x0000_0010, // below the link base
        0x0000_0020,
    ];
    while addrs.len() < 8 {
        // Multiples of the alignment (16), anywhere up to the top; the last 0x100 bytes are left for the image.
        let addr = (rng.next() & !0xF) % 0xFFFF_FF00;
        addrs.push(addr);
    }
    assert!(addrs.iter().any(|&a| a < LINK_BASE) && addrs.iter().any(|&a| a > LINK_BASE));
    for addr in addrs {
        check_relocated(addr);
    }
}

#[test]
fn the_alignment_is_the_files_own() {
    let bytes = Builder { align: 4096, ..sample() }.write().unwrap();
    let m = parse(&bytes).unwrap();
    let mut dest = vec![0; MEM_SIZE as usize];
    assert!(load_into(&m, KIND_MODULE, &mut dest, 0x50_0000, &resolve).is_ok());
    assert_eq!(
        load_into(&m, KIND_MODULE, &mut dest, 0x50_0010, &resolve),
        Err(LoadError::DestMisaligned {
            addr: 0x50_0010,
            align: 4096
        })
    );
}

// The refusals that need a destination.

#[test]
fn an_application_is_not_loadable_as_a_module_and_the_reverse() {
    let app = Builder {
        kind: KIND_APP,
        event: 0,
        stop: 0,
        ..sample()
    }
    .write()
    .unwrap();
    let module = sample_bytes();
    let mut dest = vec![0xAA; MEM_SIZE as usize];
    let m = parse(&app).unwrap();
    assert_eq!(
        load_into(&m, KIND_MODULE, &mut dest, LINK_BASE, &resolve),
        Err(LoadError::KindNotLoadable {
            kind: KIND_APP,
            wanted: KIND_MODULE
        })
    );
    let m = parse(&module).unwrap();
    assert_eq!(
        load_into(&m, KIND_APP, &mut dest, LINK_BASE, &resolve),
        Err(LoadError::KindNotLoadable {
            kind: KIND_MODULE,
            wanted: KIND_APP
        })
    );
    assert!(
        dest.iter().all(|&b| b == 0xAA),
        "a refused module leaves the destination alone"
    );
    // The application loads where applications are loaded, and has no event or stop.
    let m = parse(&app).unwrap();
    let placed = load_into(&m, KIND_APP, &mut dest, LINK_BASE, &resolve).unwrap();
    assert_eq!((placed.event, placed.stop), (None, None));
}

#[test]
fn kinds_zero_and_three_never_reach_a_loader() {
    for kind in [0, KIND_LIBRARY] {
        let bytes = damaged(|b| set16(b, off::KIND, kind));
        let e = refusal(&bytes);
        assert!(
            matches!(e, FormatError::UnknownKind(0) | FormatError::KindNotSupported(3)),
            "{e}"
        );
    }
}

#[test]
fn a_destination_one_byte_too_small() {
    let bytes = sample_bytes();
    let m = parse(&bytes).unwrap();
    let mut dest = vec![0xAA; MEM_SIZE as usize - 1];
    assert_eq!(
        load_into(&m, KIND_MODULE, &mut dest, LINK_BASE, &resolve),
        Err(LoadError::DestTooSmall {
            need: MEM_SIZE,
            have: MEM_SIZE as usize - 1
        })
    );
    assert!(dest.iter().all(|&b| b == 0xAA));
    assert_eq!(
        load_into(&m, KIND_MODULE, &mut [], LINK_BASE, &resolve),
        Err(LoadError::DestTooSmall { need: MEM_SIZE, have: 0 })
    );
}

#[test]
fn a_misaligned_destination() {
    let bytes = sample_bytes();
    let m = parse(&bytes).unwrap();
    let mut dest = vec![0xAA; MEM_SIZE as usize];
    for addr in [LINK_BASE + 1, LINK_BASE + 8, LINK_BASE + 15] {
        assert_eq!(
            load_into(&m, KIND_MODULE, &mut dest, addr, &resolve),
            Err(LoadError::DestMisaligned { addr, align: 16 })
        );
    }
    assert!(dest.iter().all(|&b| b == 0xAA));
}

#[test]
fn a_destination_that_runs_off_the_top_of_memory() {
    let bytes = sample_bytes();
    let m = parse(&bytes).unwrap();
    let mut dest = vec![0; MEM_SIZE as usize];
    assert_eq!(
        load_into(&m, KIND_MODULE, &mut dest, 0xFFFF_FFF0, &resolve),
        Err(LoadError::DestWraps {
            addr: 0xFFFF_FFF0,
            need: MEM_SIZE
        })
    );
    // The last place that fits ends at 0xFFFF_FFF0.
    assert!(load_into(&m, KIND_MODULE, &mut dest, 0xFFFF_FEF0, &resolve).is_ok());
}

#[test]
fn a_destination_ending_exactly_at_two_to_the_thirty_second() {
    // The end address (one past the last byte) is 2^32, which a u32 cannot hold, so the placement is refused.
    let bytes = sample_bytes();
    let m = parse(&bytes).unwrap();
    let mut dest = vec![0xAA; MEM_SIZE as usize];
    let addr = 0xFFFF_FF00;
    assert_eq!(addr as u64 + MEM_SIZE as u64, 1 << 32);
    assert_eq!(
        load_into(&m, KIND_MODULE, &mut dest, addr, &resolve),
        Err(LoadError::DestWraps { addr, need: MEM_SIZE })
    );
    assert!(dest.iter().all(|&b| b == 0xAA));
}

#[test]
fn a_destination_at_address_zero() {
    let bytes = sample_bytes();
    let m = parse(&bytes).unwrap();
    let mut dest = vec![0xAA; MEM_SIZE as usize];
    assert_eq!(load_into(&m, KIND_MODULE, &mut dest, 0, &resolve), Err(LoadError::DestNull));
    assert!(dest.iter().all(|&b| b == 0xAA));
    assert!(LoadError::DestNull.to_string().contains("null"));
    // The next aligned address is a place.
    assert!(load_into(&m, KIND_MODULE, &mut dest, 16, &resolve).is_ok());
}

#[test]
fn each_import_is_resolved_exactly_once() {
    let bytes = sample_bytes();
    let m = parse(&bytes).unwrap();
    let mut dest = vec![0xAA; MEM_SIZE as usize];
    let calls = Cell::new(0);
    let counting = |name: &str| {
        calls.set(calls.get() + 1);
        resolve(name)
    };
    load_into(&m, KIND_MODULE, &mut dest, LINK_BASE, &counting).unwrap();
    assert_eq!(calls.get(), 2, "two imports, two calls");
    // A refusal stops at the first missing name.
    calls.set(0);
    let first_missing = |_: &str| {
        calls.set(calls.get() + 1);
        None
    };
    assert!(load_into(&m, KIND_MODULE, &mut dest, LINK_BASE, &first_missing).is_err());
    assert_eq!(calls.get(), 1);
}

#[test]
fn a_resolver_whose_answer_changes_cannot_leave_a_slot_unbound() {
    let bytes = sample_bytes();
    let m = parse(&bytes).unwrap();
    // Answers on the first call for each name and then never again: the answers of the first call are the ones written.
    let seen = RefCell::new(Vec::new());
    let once = |name: &str| {
        let mut seen = seen.borrow_mut();
        if seen.iter().any(|n: &String| n == name) {
            return None;
        }
        seen.push(name.to_string());
        resolve(name)
    };
    let mut dest = vec![0xAA; MEM_SIZE as usize];
    load_into(&m, KIND_MODULE, &mut dest, LINK_BASE, &once).unwrap();
    assert_eq!(get32(&dest, SLOT_LOG as usize), LOG);
    assert_eq!(get32(&dest, SLOT_ALLOC as usize), ALLOC);
    // And a resolver that starts answering only on the second round gets a refusal, never an unbound slot.
    let calls = Cell::new(0);
    let late = |name: &str| {
        calls.set(calls.get() + 1);
        if calls.get() > 2 {
            resolve(name)
        } else {
            None
        }
    };
    let mut dest = vec![0xAA; MEM_SIZE as usize];
    assert_eq!(
        load_into(&m, KIND_MODULE, &mut dest, LINK_BASE, &late),
        Err(LoadError::MissingImport {
            index: 0,
            name: "desk_log"
        })
    );
    assert!(dest.iter().all(|&b| b == 0xAA));
}

#[test]
fn a_grown_header_is_accepted() {
    // A later revision may grow the header: 4 more bytes, every table moved down by 4. A reader of version 1 reads the fields it knows.
    let mut bytes = sample_bytes();
    for _ in 0..4 {
        bytes.insert(96, 0);
    }
    set16(&mut bytes, off::HEADER_SIZE, 100);
    for at in [
        off::IMAGE_OFFSET,
        off::RELOC_OFFSET,
        off::IMPORT_OFFSET,
        off::EXPORT_OFFSET,
        off::STRINGS_OFFSET,
    ] {
        let v = get32(&bytes, at);
        set32(&mut bytes, at, v + 4);
    }
    refresh_crc(&mut bytes);
    let m = parse(&bytes).unwrap();
    assert_eq!(m.header().header_size, 100);
    assert_eq!(m.image(), &sample().image[..]);
    assert_eq!(m.imports().count(), 2);
    let mut dest = vec![0; MEM_SIZE as usize];
    assert!(load_into(&m, KIND_MODULE, &mut dest, LINK_BASE, &resolve).is_ok());
    // Padding in the grown part is not the reader's to judge, but the image must start where the header ends.
    let mut gap = bytes.clone();
    set32(&mut gap, off::IMAGE_OFFSET, 104);
    refresh_crc(&mut gap);
    assert!(matches!(
        refusal(&gap),
        FormatError::TableGap { table: "image", .. } | FormatError::TableOutsideFile { .. }
    ));
}

#[test]
fn a_missing_import_names_itself() {
    let bytes = sample_bytes();
    let m = parse(&bytes).unwrap();
    let mut dest = vec![0xAA; MEM_SIZE as usize];
    let only_log = |name: &str| (name == "desk_log").then_some(LOG);
    assert_eq!(
        load_into(&m, KIND_MODULE, &mut dest, LINK_BASE, &only_log),
        Err(LoadError::MissingImport {
            index: 1,
            name: "desk_alloc"
        })
    );
    let none = |_: &str| None;
    assert_eq!(
        load_into(&m, KIND_MODULE, &mut dest, LINK_BASE, &none),
        Err(LoadError::MissingImport {
            index: 0,
            name: "desk_log"
        })
    );
    assert!(dest.iter().all(|&b| b == 0xAA), "nothing is copied when an import is missing");
}

#[test]
fn a_relocated_word_pointing_outside_the_image() {
    for (value, expect) in [
        (LINK_BASE - 1, true),
        (LINK_BASE + MEM_SIZE + 1, true),
        (0, true),
        (u32::MAX, true),
        (LINK_BASE + MEM_SIZE, false),
        (LINK_BASE, false),
    ] {
        let mut b = sample();
        b.image[0x4C..0x50].copy_from_slice(&value.to_le_bytes());
        let bytes = b.write().unwrap();
        let m = parse(&bytes).expect("a bad target is the loader's to find, not the parser's");
        let mut dest = vec![0xAA; MEM_SIZE as usize];
        let r = load_into(&m, KIND_MODULE, &mut dest, LINK_BASE, &resolve);
        if expect {
            assert_eq!(
                r,
                Err(LoadError::RelocTargetOutOfRange {
                    index: 1,
                    offset: 0x4C,
                    value
                }),
                "{value:#x}"
            );
            assert!(dest.iter().all(|&b| b == 0xAA));
        } else {
            assert!(r.is_ok(), "{value:#x}");
        }
    }
}

#[test]
fn every_error_prints_on_one_line_and_names_its_number() {
    let format_errors = [
        FormatError::TooShort { len: 7 },
        FormatError::BadMagic,
        FormatError::UnsupportedVersion(2),
        FormatError::SdkTooNew { needs: 9, have: 1 },
        FormatError::TableOutsideFile {
            table: "imports",
            offset: 4096,
            len: 16,
        },
        FormatError::BadChecksum { stored: 1, computed: 2 },
        FormatError::BadImportSlot { index: 3, slot: 0x62 },
    ];
    for e in format_errors {
        let text = e.to_string();
        assert!(!text.is_empty() && !text.contains('\n'), "{text}");
    }
    assert!(FormatError::UnsupportedVersion(2).to_string().contains('2'));
    assert!(FormatError::BadImportSlot { index: 3, slot: 0x62 }
        .to_string()
        .contains("0x62"));
    let e = LoadError::MissingImport {
        index: 1,
        name: "desk_alloc",
    };
    assert_eq!(e.to_string(), "missing import 1: desk_alloc");
    let e = LoadError::DestTooSmall { need: 256, have: 255 };
    assert!(e.to_string().contains("256") && e.to_string().contains("255"));
}

#[test]
fn ten_thousand_mutations_never_panic() {
    let good = sample_bytes();
    let good_app = Builder {
        kind: KIND_APP,
        event: 0,
        stop: 0,
        ..sample()
    }
    .write()
    .unwrap();
    let mut rng = Rng(0x1234_5678);
    let (mut parsed, mut refused, mut loaded) = (0, 0, 0);
    for round in 0..10_000 {
        let mut b = if round % 7 == 0 { good_app.clone() } else { good.clone() };
        for _ in 0..1 + rng.below(4) {
            let at = rng.below(b.len() as u32) as usize;
            match rng.below(6) {
                0 => b[at] ^= 1 << rng.below(8),
                1 => b[at] = rng.next() as u8,
                // A whole word of the header or a table: the interesting values are the edges.
                2 | 3 => {
                    let at = (at & !3).min(b.len().saturating_sub(4));
                    let v =
                        [0, 1, 4, 0x7F, 0x80, 0x100, 0x1000, 0xFFFF, 0x7FFF_FFFF, u32::MAX, rng.next()][rng.below(11) as usize];
                    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
                }
                4 => b.truncate(at),
                _ => b.push(rng.next() as u8),
            }
            if b.len() < 4 {
                break;
            }
        }
        // Half the files are given a good checksum, so the mutation gets past it to the checks behind.
        if rng.below(2) == 0 {
            refresh_crc(&mut b);
        }
        match Module::parse(&b, CORE_SDK) {
            Err(_) => refused += 1,
            Ok(m) => {
                parsed += 1;
                let size = m.header().mem_size as usize;
                // Loading with a destination of the right size (or too small: both are results), and with a resolver that fails.
                let mut dest = vec![0xAA; size.min(1 << 20)];
                let addr = (rng.next() & !0xFFF) | if rng.below(4) == 0 { 0 } else { m.header().link_base };
                if load_into(&m, m.header().kind, &mut dest, addr, &resolve).is_ok() {
                    loaded += 1;
                }
                let _ = load_into(&m, m.header().kind, &mut dest, addr, &|_| None);
                let _ = m.imports().count() + m.relocations().count() + m.image().len();
            }
        }
    }
    assert_eq!(parsed + refused, 10_000);
    // Both outcomes happen, so the test reaches the checks behind the checksum and the loader and is not only bouncing off the magic.
    assert!(
        refused > 1000 && parsed > 100 && loaded > 50,
        "refused {refused}, parsed {parsed}, loaded {loaded}"
    );
}
