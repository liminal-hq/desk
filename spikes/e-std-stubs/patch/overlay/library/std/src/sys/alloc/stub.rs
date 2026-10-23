// A file of the Desk platform layer for std, copied over the toolchain's library sources by patch/mkstd.sh.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{MIN_ALIGN, realloc_fallback};
use crate::alloc::{GlobalAlloc, Layout, System};

// The system allocator of the "stub" platform: it has no memory to give, so every request fails. A program on this platform brings
// its own `#[global_allocator]`.
#[stable(feature = "alloc_system_type", since = "1.28.0")]
unsafe impl GlobalAlloc for System {
    #[inline]
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        let _ = (MIN_ALIGN, realloc_fallback as unsafe fn(&System, *mut u8, Layout, usize) -> *mut u8);
        crate::ptr::null_mut()
    }

    #[inline]
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
