// A file of the Desk platform layer for std, copied over the toolchain's library sources by patch/mkstd.sh.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{MIN_ALIGN, realloc_fallback};
use crate::alloc::{GlobalAlloc, Layout, System};
use crate::sys::pal::abi;

#[stable(feature = "alloc_system_type", since = "1.28.0")]
unsafe impl GlobalAlloc for System {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = MIN_ALIGN;
        unsafe { abi::desk_alloc(layout.size(), layout.align()).cast() }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { abi::desk_alloc_zeroed(layout.size(), layout.align()).cast() }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { abi::desk_dealloc(ptr.cast(), layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = realloc_fallback;
        unsafe { abi::desk_realloc(ptr.cast(), layout.size(), layout.align(), new_size).cast() }
    }
}
