// The `desk_*` interface the patched std platform layer calls (patch/overlay/library/std/src/sys/pal/desk/abi.rs), over the d0 runtime.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use alloc::alloc::{alloc, alloc_zeroed, dealloc, realloc, Layout};
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::ptr::null;

fn layout(size: usize, align: usize) -> Layout {
    // SAFETY: std passes the layout of a live allocation or one it is about to make, which is already valid.
    unsafe { Layout::from_size_align_unchecked(size, align) }
}

/// # Safety
/// std's contract for the system allocator.
#[no_mangle]
pub unsafe extern "C" fn desk_alloc(size: usize, align: usize) -> *mut c_void {
    alloc(layout(size, align)).cast()
}

/// # Safety
/// std's contract for the system allocator.
#[no_mangle]
pub unsafe extern "C" fn desk_alloc_zeroed(size: usize, align: usize) -> *mut c_void {
    alloc_zeroed(layout(size, align)).cast()
}

/// # Safety
/// std's contract for the system allocator.
#[no_mangle]
pub unsafe extern "C" fn desk_dealloc(p: *mut c_void, size: usize, align: usize) {
    dealloc(p.cast(), layout(size, align))
}

/// # Safety
/// std's contract for the system allocator.
#[no_mangle]
pub unsafe extern "C" fn desk_realloc(p: *mut c_void, old: usize, align: usize, new: usize) -> *mut c_void {
    realloc(p.cast(), layout(old, align), new).cast()
}

/// # Safety
/// `buf` points at `len` bytes (or `len` is 0).
#[no_mangle]
pub unsafe extern "C" fn desk_write(fd: i32, buf: *const u8, len: usize) -> isize {
    // Standard error goes to standard output: see lib.rs.
    let handle = if fd == 2 { 1 } else { fd };
    if len == 0 {
        return 0;
    }
    match rt::dos::write_all(handle as u16, core::slice::from_raw_parts(buf, len)) {
        Ok(()) => len as isize,
        Err(_) => -1,
    }
}

/// # Safety
/// `buf` points at `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn desk_read(fd: i32, buf: *mut u8, len: usize) -> isize {
    if len == 0 {
        return 0;
    }
    match rt::dos::read(fd as u16, core::slice::from_raw_parts_mut(buf, len)) {
        Ok(n) => n as isize,
        Err(_) => -1,
    }
}

#[no_mangle]
pub extern "C" fn desk_exit(code: i32) -> ! {
    rt::dos::exit(code)
}

/// # Safety
/// `buf` points at `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn desk_random(buf: *mut u8, len: usize) {
    crate::random_bytes(core::slice::from_raw_parts_mut(buf, len));
}

struct Cache(UnsafeCell<Option<(Vec<String>, Vec<String>)>>);

// SAFETY: the program is single-threaded; the type exists to be a `static`.
unsafe impl Sync for Cache {}

static CACHE: Cache = Cache(UnsafeCell::new(None));

/// The arguments and the `NAME=value` environment entries, read from the PSP and the environment block once and kept for good.
fn cache() -> &'static (Vec<String>, Vec<String>) {
    // SAFETY: single-threaded, and the value is never replaced once set.
    unsafe {
        let slot = &mut *CACHE.0.get();
        slot.get_or_insert_with(|| {
            // argv[0] is the program's path, as std programs expect; the runtime keeps it apart from the command tail.
            let mut args = alloc::vec![rt::env::program_path().unwrap_or_default()];
            args.extend(rt::env::args());
            let vars = rt::env::vars().into_iter().map(|(k, v)| alloc::format!("{k}={v}")).collect();
            (args, vars)
        })
    }
}

#[no_mangle]
pub extern "C" fn desk_arg_count() -> usize {
    cache().0.len()
}

/// # Safety
/// `len` is writable.
#[no_mangle]
pub unsafe extern "C" fn desk_arg(i: usize, len: *mut usize) -> *const u8 {
    match cache().0.get(i) {
        Some(s) => {
            *len = s.len();
            s.as_ptr()
        }
        None => null(),
    }
}

/// # Safety
/// `len` is writable.
#[no_mangle]
pub unsafe extern "C" fn desk_env_entry(i: usize, len: *mut usize) -> *const u8 {
    match cache().1.get(i) {
        Some(s) => {
            *len = s.len();
            s.as_ptr()
        }
        None => null(),
    }
}

/// # Safety
/// `name` points at `name_len` bytes and `len` is writable.
#[no_mangle]
pub unsafe extern "C" fn desk_env_get(name: *const u8, name_len: usize, len: *mut usize) -> *const u8 {
    let want = core::slice::from_raw_parts(name, name_len);
    for entry in &cache().1 {
        let b = entry.as_bytes();
        if b.len() > name_len && b[name_len] == b'=' && b[..name_len].eq_ignore_ascii_case(want) {
            *len = b.len() - name_len - 1;
            return b[name_len + 1..].as_ptr();
        }
    }
    null()
}

#[no_mangle]
pub extern "C" fn desk_now_ns() -> u64 {
    crate::clock::now_ns()
}

#[no_mangle]
pub extern "C" fn desk_unix_ns() -> u64 {
    crate::clock::unix_ns()
}

#[no_mangle]
pub extern "C" fn desk_sleep_ns(ns: u64) {
    let end = crate::clock::now_ns().saturating_add(ns);
    while crate::clock::now_ns() < end {
        core::hint::spin_loop();
    }
}
