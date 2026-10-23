// A file of the Desk platform layer for std, copied over the toolchain's library sources by patch/mkstd.sh.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The interface between std and the Desk runtime: every function std needs from the system, as a plain C function the runtime
//! defines. Only integers and pointers cross it. The runtime for the hosted track (DOS) and the one for the native track each
//! implement the same set.

use crate::ffi::c_void;

unsafe extern "C" {
    /// Writes to file descriptor 1 (standard output) or 2 (standard error). Returns the bytes written, or -1.
    pub fn desk_write(fd: i32, buf: *const u8, len: usize) -> isize;
    /// Reads from file descriptor 0 (standard input). Returns the bytes read, 0 at end of input, or -1.
    pub fn desk_read(fd: i32, buf: *mut u8, len: usize) -> isize;
    /// Ends the program at once with the given status.
    pub fn desk_exit(code: i32) -> !;

    /// Memory for the system allocator. A null return is an allocation failure.
    pub fn desk_alloc(size: usize, align: usize) -> *mut c_void;
    pub fn desk_alloc_zeroed(size: usize, align: usize) -> *mut c_void;
    pub fn desk_dealloc(ptr: *mut c_void, size: usize, align: usize);
    pub fn desk_realloc(ptr: *mut c_void, old_size: usize, align: usize, new_size: usize) -> *mut c_void;

    /// Fills the buffer with random bytes (the seed for `HashMap`).
    pub fn desk_random(buf: *mut u8, len: usize);

    /// The command-line arguments, as bytes the runtime owns for the life of the program: how many, and the `i`th (null past the end).
    pub fn desk_arg_count() -> usize;
    pub fn desk_arg(i: usize, len: *mut usize) -> *const u8;
    /// The `i`th environment entry as `NAME=value` bytes (null past the end), and the value of one variable (null when unset).
    pub fn desk_env_entry(i: usize, len: *mut usize) -> *const u8;
    pub fn desk_env_get(name: *const u8, name_len: usize, len: *mut usize) -> *const u8;

    /// A monotonic clock in nanoseconds from an arbitrary start, and the wall clock in nanoseconds since 1970-01-01 UTC.
    pub fn desk_now_ns() -> u64;
    pub fn desk_unix_ns() -> u64;
    /// Waits at least this many nanoseconds.
    pub fn desk_sleep_ns(ns: u64);
}
