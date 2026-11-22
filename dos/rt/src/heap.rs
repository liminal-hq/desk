// The global allocator, over DPMI memory blocks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A first-fit free list, sorted by address so neighbours merge when a block is freed. Each allocation carries an eight-byte header
// (where its block starts and how long it is), so `dealloc` needs nothing from the `Layout`. The heap grows by asking the host for
// a block of at least `CHUNK` bytes (function 0501h) and adding it to the free list; blocks are not returned before exit, when the
// host reclaims them all. Single-threaded and not reentrant: an interrupt handler must never allocate.
use crate::dpmi;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::null_mut;

/// The smallest block requested from the host. 0501h gives page-aligned memory; asking for a good-sized chunk keeps calls rare.
const CHUNK: usize = 256 * 1024;
const HEADER: usize = 8;
/// The smallest leftover worth keeping as a free node (a node is two words).
const MIN_NODE: usize = 16;

#[repr(C)]
struct Node {
    size: usize,
    next: *mut Node,
}

struct State {
    free: *mut Node,
    /// Bytes obtained from the host, for the statistics
    total: usize,
}

pub struct Heap(UnsafeCell<State>);

// SAFETY: the program is single-threaded; the type exists to be a `static`.
unsafe impl Sync for Heap {}

#[global_allocator]
pub static HEAP: Heap = Heap(UnsafeCell::new(State {
    free: null_mut(),
    total: 0,
}));

/// Rounds `x` up to a multiple of the power of two `a`, or `None` if that does not fit in a `usize`.
fn round_up(x: usize, a: usize) -> Option<usize> {
    Some(x.checked_add(a - 1)? & !(a - 1))
}

impl Heap {
    /// Bytes the heap has obtained from the host so far.
    pub fn total_bytes(&self) -> usize {
        // SAFETY: single-threaded read.
        unsafe { (*self.0.get()).total }
    }

    /// Adds `[start, start + size)` to the free list, merging with its neighbours.
    ///
    /// # Safety
    /// The region must be unused memory of at least `MIN_NODE` bytes, 8-byte aligned.
    unsafe fn insert(&self, start: usize, size: usize) {
        let st = &mut *self.0.get();
        let mut prev: *mut Node = null_mut();
        let mut cur = st.free;
        while !cur.is_null() && (cur as usize) < start {
            prev = cur;
            cur = (*cur).next;
        }
        let node = start as *mut Node;
        (*node).size = size;
        (*node).next = cur;
        if prev.is_null() {
            st.free = node;
        } else {
            (*prev).next = node;
        }
        // Merge forward, then backward.
        if !cur.is_null() && start.wrapping_add(size) == cur as usize {
            (*node).size += (*cur).size;
            (*node).next = (*cur).next;
        }
        if !prev.is_null() && (prev as usize).wrapping_add((*prev).size) == start {
            (*prev).size += (*node).size;
            (*prev).next = (*node).next;
        }
    }

    /// Takes a first-fit block for `layout` from the free list, or null.
    ///
    /// # Safety
    /// The free list must be intact.
    unsafe fn take(&self, layout: Layout) -> *mut u8 {
        let st = &mut *self.0.get();
        let align = layout.align().max(8);
        // A size or alignment that cannot be rounded up in 32 bits can never be satisfied: fail the allocation, do not wrap.
        let Some(size) = round_up(layout.size().max(1), 8) else {
            return null_mut();
        };
        let mut prev: *mut Node = null_mut();
        let mut cur = st.free;
        while !cur.is_null() {
            let start = cur as usize;
            let node_size = (*cur).size;
            // The node lies inside the address space, so `start + node_size` and `start + HEADER` cannot overflow; the caller's
            // size and alignment can push `user` and `end` past it, and then no block can hold the request.
            let Some(user) = round_up(start + HEADER, align) else {
                return null_mut();
            };
            let Some(end) = user.checked_add(size) else {
                return null_mut();
            };
            if end <= start + node_size {
                let rest = start + node_size - end;
                let next = (*cur).next;
                let (taken_end, new_next);
                if rest >= MIN_NODE {
                    // Split: the tail stays on the list in this node's place.
                    let tail = end as *mut Node;
                    (*tail).size = rest;
                    (*tail).next = next;
                    taken_end = end;
                    new_next = tail;
                } else {
                    taken_end = start + node_size;
                    new_next = next;
                }
                if prev.is_null() {
                    st.free = new_next;
                } else {
                    (*prev).next = new_next;
                }
                let hdr = (user - HEADER) as *mut usize;
                *hdr = start;
                *hdr.add(1) = taken_end - start;
                return user as *mut u8;
            }
            prev = cur;
            cur = (*cur).next;
        }
        null_mut()
    }

    /// Gets more memory from the host: enough for `layout` and at least `CHUNK`.
    unsafe fn grow(&self, layout: Layout) -> bool {
        let need = layout
            .size()
            .max(1)
            .checked_add(layout.align().max(8))
            .and_then(|n| n.checked_add(HEADER + MIN_NODE));
        let Some(want) = need.and_then(|n| round_up(n.max(CHUNK), 4096)) else {
            return false;
        };
        match dpmi::alloc_block(want as u32) {
            Ok(b) => {
                let p = dpmi::linear_to_ptr::<u8>(b.linear) as usize;
                (*self.0.get()).total += want;
                self.insert(p, want);
                true
            }
            Err(_) => false,
        }
    }
}

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = self.take(layout);
        if !p.is_null() {
            return p;
        }
        if !self.grow(layout) {
            return null_mut();
        }
        self.take(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        let hdr = (ptr as usize - HEADER) as *const usize;
        let start = *hdr;
        let size = *hdr.add(1);
        self.insert(start, size);
    }
}
