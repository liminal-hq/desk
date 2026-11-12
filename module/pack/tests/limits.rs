// Tests that the ELF reader's time and memory depend on the size of the file and not on what its tables point at: many headers sharing
// one long name, and many segments and relocations.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Memory is measured by a counting allocator that adds up every byte the reader asks for (a total, not the largest live size, so the
// measure needs no more than `fetch_add`). The tests in this file take a lock so that none runs while another is measuring. Time is
// bounded generously (a file that took the reader half a minute must now take well under the bound, which a slow machine still meets)
// and the work itself is also bounded by the counts the tests assert.
mod elfgen;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use dmdpack::{read, KIND_MODULE};
use elfgen::*;

struct Counting;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call is forwarded unchanged to the system allocator; the counter is only read and added to.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        System.alloc(layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed);
        System.alloc_zeroed(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATED.fetch_add(new_size, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Runs the reader and returns its result with the bytes it allocated and the time it took.
fn measured(bytes: &[u8]) -> (Result<dmdpack::Extract, dmdpack::ElfError>, usize, Duration) {
    ALLOCATED.store(0, Ordering::Relaxed);
    let start = Instant::now();
    let result = read(bytes, BASE, KIND_MODULE);
    (result, ALLOCATED.load(Ordering::Relaxed), start.elapsed())
}

const LONG: usize = 1_000_000;
const SECONDS: u64 = 5;

#[test]
fn many_section_headers_sharing_one_long_name_cost_no_more_than_the_file() {
    let _lock = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut e = good();
    let first_extra = e.sections.len() + 1;
    e.sections.push(Section {
        name: "x".repeat(LONG),
        flags: 0,
        addr: 0,
        body: Body::Alloc(0),
    });
    for _ in 0..3999 {
        e.sections.push(Section {
            name: "s".into(),
            flags: 0,
            addr: 0,
            body: Body::Alloc(0),
        });
    }
    let mut b = e.build();
    let long = get32(&b, shdr(&b, first_extra) + SH_NAME);
    for i in first_extra..first_extra + 4000 {
        let at = shdr(&b, i) + SH_NAME;
        put32(&mut b, at, long);
    }
    assert!(b.len() > LONG && b.len() < LONG + 300_000, "a file of about a megabyte");
    let (result, allocated, took) = measured(&b);
    assert_eq!(result.map(|x| x.mem_size), Ok(0x2040));
    // Copying the name once per header would be 4000 megabytes.
    assert!(
        allocated < 1 << 20,
        "the reader allocated {allocated} bytes for a file of {}",
        b.len()
    );
    assert!(took < Duration::from_secs(SECONDS), "took {took:?}");
}

#[test]
fn many_symbols_sharing_one_long_name_cost_no_more_than_the_file() {
    let _lock = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut e = good();
    let first = e.symbols.len() + 1;
    e.symbols.push(Symbol {
        name: "y".repeat(LONG),
        value: 0,
        size: 0,
        kind: STT_NOTYPE,
        shndx: Shndx::Abs,
    });
    for _ in 0..3999 {
        e.symbols.push(Symbol {
            name: "z".into(),
            value: 0,
            size: 0,
            kind: STT_NOTYPE,
            shndx: Shndx::Abs,
        });
    }
    let mut b = e.build();
    let symtab = get32(&b, shdr(&b, 7) + SH_OFFSET) as usize;
    let long = get32(&b, symtab + 16 * first);
    for i in first..first + 4000 {
        put32(&mut b, symtab + 16 * i, long);
    }
    assert!(b.len() > LONG && b.len() < LONG + 200_000, "a file of about a megabyte");
    let (result, allocated, took) = measured(&b);
    assert_eq!(result.map(|x| x.mem_size), Ok(0x2040));
    assert!(
        allocated < 1 << 20,
        "the reader allocated {allocated} bytes for a file of {}",
        b.len()
    );
    assert!(took < Duration::from_secs(SECONDS), "took {took:?}");
}

/// A module of `n` four-byte read-only segments, and `relocs` relocations spread over them (so most are duplicates).
fn many_segments(n: u32, relocs: u32) -> Vec<u8> {
    let sec = |name: &str, flags: u32, addr: u32, body: Body| Section {
        name: name.into(),
        flags,
        addr,
        body,
    };
    let sections = vec![
        sec(".text", SHF_ALLOC | SHF_EXEC, BASE, Body::Alloc(8 * n)),
        sec(".symtab", 0, 0, Body::Symtab),
        sec(".strtab", 0, 0, Body::Strtab),
        sec(".shstrtab", 0, 0, Body::Shstrtab),
        sec(
            ".rel.text",
            0,
            0,
            Body::Rel {
                target: 1,
                rela: false,
                rels: (0..relocs)
                    .map(|i| Rel {
                        offset: BASE + 8 * (i % n),
                        r_type: R_386_32,
                        symbol: 1,
                    })
                    .collect(),
            },
        ),
    ];
    let symbols = vec![
        Symbol {
            name: ".text".into(),
            value: BASE,
            size: 0,
            kind: STT_SECTION,
            shndx: Shndx::Section(".text".into()),
        },
        Symbol {
            name: "app_start".into(),
            value: BASE,
            size: 4,
            kind: STT_FUNC,
            shndx: Shndx::Section(".text".into()),
        },
    ];
    let segments = (0..n)
        .map(|i| Segment {
            p_type: PT_LOAD,
            vaddr: BASE + 8 * i,
            flags: PF_R | PF_X,
            data: BASE.to_le_bytes().to_vec(),
            bss: 0,
        })
        .collect();
    Elf {
        segments,
        sections,
        symbols,
    }
    .build()
}

#[test]
fn thirty_thousand_segments_and_a_million_duplicate_relocations_are_read_in_seconds() {
    let _lock = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let b = many_segments(30_000, 1_000_000);
    assert!(b.len() > 8_000_000, "a file of about ten megabytes");
    let (result, allocated, took) = measured(&b);
    let x = result.unwrap();
    assert_eq!(x.relocs.len(), 30_000);
    assert_eq!(x.relocs[29_999], 8 * 29_999);
    assert!(
        allocated < 4 * b.len(),
        "the reader allocated {allocated} bytes for a file of {}",
        b.len()
    );
    assert!(took < Duration::from_secs(SECONDS), "took {took:?}");
}
