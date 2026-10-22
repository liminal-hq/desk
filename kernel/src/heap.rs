// The kernel heap: a first-fit free list with coalescing, behind the global allocator.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{memory, println, sync::without_interrupts};
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::mem::size_of;
use core::ptr::null_mut;

const HEAP_FRAMES: usize = 1024; // 4 MiB

/// A free block. Blocks are kept sorted by address so neighbours can be merged on free. Every block and every allocation is a
/// multiple of 8 bytes and 8-byte aligned, which keeps any remainder big enough to hold a node.
struct Node {
    size: usize,
    next: *mut Node,
}

const GRAIN: usize = 8;

struct FreeList {
    head: *mut Node,
    free: usize,
}

impl FreeList {
    fn round(layout: Layout) -> (usize, usize) {
        (layout.size().max(1).next_multiple_of(GRAIN).max(size_of::<Node>()), layout.align().max(GRAIN))
    }

    /// Adds a block, merging it with a neighbour on either side.
    unsafe fn insert(&mut self, addr: usize, size: usize) {
        let mut prev: *mut Node = null_mut();
        let mut cur = self.head;
        while !cur.is_null() && (cur as usize) < addr {
            prev = cur;
            cur = (*cur).next;
        }
        let mut node = addr as *mut Node;
        node.write(Node { size, next: cur });
        if !cur.is_null() && addr + size == cur as usize {
            (*node).size += (*cur).size;
            (*node).next = (*cur).next;
        }
        if prev.is_null() {
            self.head = node;
        } else if prev as usize + (*prev).size == addr {
            (*prev).size += (*node).size;
            (*prev).next = (*node).next;
            node = prev;
        } else {
            (*prev).next = node;
        }
        let _ = node;
        self.free += size;
    }

    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let (size, align) = Self::round(layout);
        let mut prev: *mut Node = null_mut();
        let mut cur = self.head;
        while !cur.is_null() {
            let start = cur as usize;
            let end = start + (*cur).size;
            let at = start.next_multiple_of(align);
            if at + size <= end && (at == start || at - start >= size_of::<Node>()) && (at + size == end || end - (at + size) >= size_of::<Node>()) {
                let next = (*cur).next;
                // Unlink the block, then give back whatever is left in front of and behind the allocation.
                if prev.is_null() {
                    self.head = next;
                } else {
                    (*prev).next = next;
                }
                self.free -= end - start;
                if at > start {
                    self.insert(start, at - start);
                }
                if at + size < end {
                    self.insert(at + size, end - (at + size));
                }
                return at as *mut u8;
            }
            prev = cur;
            cur = (*cur).next;
        }
        null_mut()
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let (size, _) = Self::round(layout);
        self.insert(ptr as usize, size);
    }
}

pub struct Heap(UnsafeCell<FreeList>);

// SAFETY: one processor; every access runs inside `without_interrupts`.
unsafe impl Sync for Heap {}

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        without_interrupts(|| (*self.0.get()).alloc(layout))
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        without_interrupts(|| (*self.0.get()).dealloc(ptr, layout))
    }
}

#[global_allocator]
static HEAP: Heap = Heap(UnsafeCell::new(FreeList { head: null_mut(), free: 0 }));

/// Takes a contiguous run of frames from the frame allocator and hands it to the heap.
pub fn init() {
    let base = memory::alloc_contiguous(HEAP_FRAMES).expect("no room for the heap");
    let size = HEAP_FRAMES * memory::FRAME_SIZE as usize;
    // SAFETY: the frames were just reserved for the heap and are identity-mapped (there is no paging).
    unsafe { (*HEAP.0.get()).insert(base as usize, size) };
    println!("heap: {} KiB at {:#x}", size / 1024, base);
}

pub fn free_bytes() -> usize {
    // SAFETY: a plain read on a single processor.
    unsafe { (*HEAP.0.get()).free }
}
