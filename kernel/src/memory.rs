// The BIOS memory map and the physical frame allocator.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{println, sync::without_interrupts, BootInfo};

pub const FRAME_SIZE: u32 = 4096;
/// The bitmap covers this much physical memory; RAM above it is ignored for now.
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FRAMES: usize = (MAX_BYTES / FRAME_SIZE as u64) as usize;
pub const KIND_USABLE: u32 = 1;

/// One entry of the map, in the layout of the E820 call (the loader converts an E801 answer to the same shape).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Region {
    pub base: u64,
    pub len: u64,
    pub kind: u32,
    pub acpi: u32,
}

extern "C" {
    static __kernel_start: u8;
    static __kernel_end: u8;
}

// One bit per frame; a set bit means the frame is in use or does not exist.
static mut BITMAP: [u32; MAX_FRAMES / 32] = [!0; MAX_FRAMES / 32];
static mut FREE: usize = 0;

pub fn source_name(source: u32) -> &'static str {
    match source {
        1 => "E820",
        2 => "E801",
        _ => "nothing",
    }
}

/// The map the loader left.
pub fn regions(info: &BootInfo) -> &[Region] {
    // SAFETY: the loader stores `map_count` entries of 24 bytes straight after the 32-byte header.
    unsafe {
        let first = (info as *const BootInfo as *const u8).add(32) as *const Region;
        core::slice::from_raw_parts(first, info.map_count as usize)
    }
}

fn bitmap() -> &'static mut [u32; MAX_FRAMES / 32] {
    // SAFETY: single processor; every user goes through `without_interrupts` or runs before interrupts are on.
    unsafe { &mut *core::ptr::addr_of_mut!(BITMAP) }
}

fn set_used(frame: usize) {
    let bm = bitmap();
    if bm[frame / 32] & (1 << (frame % 32)) == 0 {
        bm[frame / 32] |= 1 << (frame % 32);
        // SAFETY: as for `bitmap`.
        unsafe { *core::ptr::addr_of_mut!(FREE) -= 1 };
    }
}

fn set_free(frame: usize) {
    let bm = bitmap();
    if bm[frame / 32] & (1 << (frame % 32)) != 0 {
        bm[frame / 32] &= !(1 << (frame % 32));
        // SAFETY: as for `bitmap`.
        unsafe { *core::ptr::addr_of_mut!(FREE) += 1 };
    }
}

fn is_free(frame: usize) -> bool {
    bitmap()[frame / 32] & (1 << (frame % 32)) == 0
}

/// Builds the allocator from the map: usable regions are freed (rounded inward to whole frames), then everything else is
/// reserved (rounded outward), so an overlap between a usable and a reserved entry resolves to reserved. The first megabyte
/// (interrupt vectors, BIOS data, the loader, video memory, option ROMs) and the kernel image stay reserved.
pub fn init(info: &BootInfo) {
    let limit = MAX_FRAMES as u64;
    let fs = FRAME_SIZE as u64;
    for r in regions(info).iter().filter(|r| r.kind == KIND_USABLE) {
        let first = (r.base + fs - 1) / fs;
        let last = (r.base + r.len) / fs;
        for f in first..last.min(limit) {
            set_free(f as usize);
        }
    }
    for r in regions(info).iter().filter(|r| r.kind != KIND_USABLE && r.len != 0) {
        let first = r.base / fs;
        let last = (r.base + r.len + fs - 1) / fs;
        for f in first..last.min(limit) {
            set_used(f as usize);
        }
    }
    for f in 0..(0x100000 / FRAME_SIZE) as usize {
        set_used(f);
    }
    let (start, end) = kernel_range();
    for f in (start / FRAME_SIZE)..end.div_ceil(FRAME_SIZE) {
        set_used(f as usize);
    }
}

/// True when `base..base + len` overlaps usable RAM in the map, the first megabyte or the kernel image: memory that something else
/// owns. A frame buffer must not.
pub fn overlaps_ram(info: &BootInfo, base: u32, len: u32) -> bool {
    let (b, e) = (u64::from(base), u64::from(base) + u64::from(len));
    let hit = |lo: u64, hi: u64| b < hi && lo < e;
    let (ks, ke) = kernel_range();
    hit(0, 0x10_0000) || hit(u64::from(ks), u64::from(ke)) || regions(info).iter().any(|r| r.kind == KIND_USABLE && hit(r.base, r.base + r.len))
}

/// Marks the frames of `base..base + len` as in use, so the allocator never hands out device memory. Frames beyond what the bitmap
/// covers need no marking: they are never allocated.
pub fn reserve(base: u32, len: u32) {
    let fs = u64::from(FRAME_SIZE);
    let first = u64::from(base) / fs;
    let last = (u64::from(base) + u64::from(len)).div_ceil(fs);
    for f in first..last.min(MAX_FRAMES as u64) {
        set_used(f as usize);
    }
}

/// The physical range the kernel image and its .bss and stack occupy.
pub fn kernel_range() -> (u32, u32) {
    (core::ptr::addr_of!(__kernel_start) as u32, core::ptr::addr_of!(__kernel_end) as u32)
}

/// Prints the map and the totals the boot transcript records.
pub fn report(info: &BootInfo) {
    let mut usable_kib = 0u64;
    for r in regions(info) {
        let kind = match r.kind {
            1 => "usable",
            2 => "reserved",
            3 => "ACPI reclaimable",
            4 => "ACPI NVS",
            5 => "bad",
            _ => "unknown",
        };
        println!("  map {:#010x}-{:#010x} {}", r.base, r.base + r.len, kind);
        if r.kind == KIND_USABLE {
            usable_kib += r.len / 1024;
        }
    }
    println!("memory: usable RAM {} KiB", usable_kib);
    let (start, end) = kernel_range();
    println!("memory: kernel image {:#x}-{:#x}", start, end);
    println!("memory: {} frames of {} KiB free after reservations", free_frames(), FRAME_SIZE / 1024);
}

pub fn free_frames() -> usize {
    // SAFETY: a plain read on a single processor.
    unsafe { *core::ptr::addr_of!(FREE) }
}

/// Takes one free frame and returns its physical address.
pub fn alloc_frame() -> Option<u32> {
    alloc_contiguous(1)
}

/// Takes `count` adjacent free frames (first fit, lowest address) and returns the address of the first.
pub fn alloc_contiguous(count: usize) -> Option<u32> {
    without_interrupts(|| {
        let mut run = 0;
        for f in 0..MAX_FRAMES {
            run = if is_free(f) { run + 1 } else { 0 };
            if run == count {
                let first = f + 1 - count;
                (first..=f).for_each(set_used);
                return Some(first as u32 * FRAME_SIZE);
            }
        }
        None
    })
}

pub fn free_frame(addr: u32) {
    without_interrupts(|| set_free((addr / FRAME_SIZE) as usize));
}
