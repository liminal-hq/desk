// The seed of the Desk's Rust core: a no_std library that the C layer calls through a small C ABI.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// There is no Rust entry point and no std. C owns `main`, the C library and the hardware; Rust owns the logic and borrows `malloc`
// and `free` from DJGPP's C library for its heap. Only integers, pointers and lengths cross the boundary.
#![no_std]

extern crate alloc;

use alloc::alloc::{GlobalAlloc, Layout};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::c_void;

extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
    fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    /// Defined in `c/panic.c`: reports through C and exits with a non-zero status. Never returns.
    fn desk_panic(line: u32) -> !;
}

/// The heap: DJGPP's C allocator.
struct CHeap;

// DJGPP's malloc returns 8-byte aligned blocks, which covers everything the core uses (an f64 at most). A stricter alignment
// is refused with null rather than handed out misaligned.
unsafe impl GlobalAlloc for CHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() > 8 {
            return core::ptr::null_mut();
        }
        malloc(layout.size()).cast()
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        free(ptr.cast());
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.align() > 8 {
            return core::ptr::null_mut();
        }
        realloc(ptr.cast(), new_size).cast()
    }
}

#[global_allocator]
static HEAP: CHeap = CHeap;

/// Reports only the source line: formatting the message here would pull `core::fmt` into the one path that must not fail.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let line = info.location().map_or(0, |l| l.line());
    // SAFETY: `desk_panic` takes an integer and is defined in C; it does not return.
    unsafe { desk_panic(line) }
}

/// The square root, computed by the x87's `fsqrt`.
fn x87_sqrt(x: f64) -> f64 {
    let mut result: f64 = 0.0;
    // SAFETY: the block loads `x` onto the x87 stack, takes its root and stores it to `result`, popping it again, so the stack is
    // left as it was found. Rust does not allow an x87 register as an operand, so the value moves through memory; the clobbers
    // tell the compiler the block uses the x87 stack. The x87 is present on every machine the Desk targets.
    unsafe {
        core::arch::asm!(
            "fld qword ptr [{x}]",
            "fsqrt",
            "fstp qword ptr [{r}]",
            x = in(reg) &x,
            r = in(reg) &mut result,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
            out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            options(nostack),
        );
    }
    result
}

/// Builds the skeleton's result line from heap-allocated data, integer maths and one x87 operation.
fn build_report() -> String {
    // A Vec grows through the global allocator (`realloc` in C).
    let squares: Vec<u32> = (1..=10u32).map(|n| n * n).collect();
    let sum: u32 = squares.iter().sum();
    // A 64-bit product, which the 32-bit target does with library help (`compiler_builtins`).
    let big = u64::from(sum) * 1_000_003;
    // Floating point goes through the x87, and `{:.6}` runs core's float formatter on it.
    let root = x87_sqrt(f64::from(sum));
    format!(
        "DESK-SKELETON ok: squares={} sum={} big={} sqrt={:.6}",
        squares.len(),
        sum,
        big,
        root
    )
}

/// Writes the skeleton's result line into `out` (at most `capacity` bytes, with a terminating NUL) and returns its length.
///
/// Returns -1 if `out` is null or `capacity` is too small for the line and its NUL.
///
/// # Safety
///
/// `out` must be null or valid for writes of `capacity` bytes.
#[no_mangle]
pub unsafe extern "C" fn desk_main(out: *mut u8, capacity: usize) -> i32 {
    let report = build_report();
    let bytes = report.as_bytes();
    if out.is_null() || bytes.len() >= capacity {
        return -1;
    }
    // SAFETY: C promises `out` is valid for `capacity` bytes, and `bytes.len() + 1 <= capacity` was checked above.
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
        *out.add(bytes.len()) = 0;
    }
    bytes.len() as i32
}

/// Panics on purpose, so the check can prove that the panic path reports through C and exits non-zero.
#[no_mangle]
pub extern "C" fn desk_selftest_panic() {
    panic!("selftest");
}
