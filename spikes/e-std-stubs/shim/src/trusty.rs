// The libc-style symbols the `trusty` platform layer of std expects.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use alloc::alloc::{alloc, dealloc, Layout};
use core::ffi::c_void;
use core::ptr::{null_mut, read_volatile};

const ALIGN: usize = 8;

fn layout(size: usize, align: usize) -> Option<Layout> {
    Layout::from_size_align(size.max(1), align.max(ALIGN)).ok()
}

/// # Safety
/// The C contract of `malloc`.
#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    match layout(size, ALIGN) {
        Some(l) => alloc(l).cast(),
        None => null_mut(),
    }
}

/// # Safety
/// The C contract of `memalign`.
#[no_mangle]
pub unsafe extern "C" fn memalign(align: usize, size: usize) -> *mut c_void {
    match layout(size, align) {
        Some(l) => alloc(l).cast(),
        None => null_mut(),
    }
}

/// # Safety
/// The C contract of `posix_memalign`.
#[no_mangle]
pub unsafe extern "C" fn posix_memalign(out: *mut *mut c_void, align: usize, size: usize) -> i32 {
    let p = memalign(align, size);
    if p.is_null() {
        return 12; // ENOMEM
    }
    *out = p;
    0
}

/// # Safety
/// The C contract of `calloc`.
#[no_mangle]
pub unsafe extern "C" fn calloc(n: usize, size: usize) -> *mut c_void {
    let Some(total) = n.checked_mul(size) else { return null_mut() };
    let p = malloc(total);
    if !p.is_null() {
        core::ptr::write_bytes(p.cast::<u8>(), 0, total);
    }
    p
}

/// The usable size of a block from the runtime's heap: each block has an eight-byte header (block start, block length) before it.
unsafe fn usable(p: *mut c_void) -> usize {
    let hdr = (p as usize - 8) as *const usize;
    read_volatile(hdr) + read_volatile(hdr.add(1)) - p as usize
}

/// # Safety
/// The C contract of `free`. The runtime's heap frees by header, so the size and alignment are not needed.
#[no_mangle]
pub unsafe extern "C" fn free(p: *mut c_void) {
    if !p.is_null() {
        dealloc(p.cast(), Layout::from_size_align_unchecked(1, ALIGN));
    }
}

/// # Safety
/// The C contract of `realloc`.
#[no_mangle]
pub unsafe extern "C" fn realloc(p: *mut c_void, size: usize) -> *mut c_void {
    if p.is_null() {
        return malloc(size);
    }
    let old = usable(p);
    let new = malloc(size);
    if !new.is_null() {
        core::ptr::copy_nonoverlapping(p.cast::<u8>(), new.cast::<u8>(), old.min(size));
        free(p);
    }
    new
}

#[repr(C)]
pub struct IoVec {
    base: *const u8,
    len: usize,
}

/// # Safety
/// The C contract of `writev`: `iov` points at `count` valid entries.
#[no_mangle]
pub unsafe extern "C" fn writev(fd: i32, iov: *const IoVec, count: i32) -> isize {
    let handle = if fd == 2 { 1 } else { fd };
    let mut total = 0isize;
    for i in 0..count.max(0) as usize {
        let v = &*iov.add(i);
        if v.len == 0 {
            continue;
        }
        if rt::dos::write_all(handle as u16, core::slice::from_raw_parts(v.base, v.len)).is_err() {
            return -1;
        }
        total += v.len as isize;
    }
    total
}

/// The random seed for `HashMap`.
///
/// # Safety
/// The C contract: `buf` points at `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn trusty_rng_secure_rand(buf: *mut u8, len: usize) {
    crate::random_bytes(core::slice::from_raw_parts_mut(buf, len));
}
