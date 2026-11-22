// memcpy, memmove, memset and memcmp for the stage 1 build, where nothing else provides them; memcpy and memset again, faster, for the ELF build.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// In the ELF build `compiler_builtins` supplies these (the `compiler-builtins-mem` feature). For the PE-flavoured COFF target its
// object cannot be read by DJGPP's `ld` (weak externals), so the runtime defines the four itself, with the x86 string
// instructions: written as loops, the compiler would turn them back into calls to themselves.
//
// In the ELF build only `memcpy` and `memset` are defined here, with `rep movsd` and `rep stosd`. They are strong symbols, so they
// override the weak ones in `compiler_builtins`, which still supplies `memmove` and `memcmp`.
use core::arch::asm;
use core::ffi::c_void;

/// # Safety
/// The usual `memcpy` contract: `n` valid bytes at both pointers, not overlapping.
#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    asm!("push esi", "mov esi, {s}", "rep movsb", "pop esi", s = in(reg) src, inout("edi") dst => _, inout("ecx") n => _, options(preserves_flags));
    dst
}

/// # Safety
/// The usual `memmove` contract: `n` valid bytes at both pointers, which may overlap.
#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn memmove(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    if (dst as usize).wrapping_sub(src as usize) >= n {
        // No overlap that a forward copy would corrupt.
        memcpy(dst, src, n);
    } else {
        // Copy backwards from the last byte.
        asm!(
            "push esi", "mov esi, {s}", "std", "rep movsb", "cld", "pop esi",
            s = in(reg) (src as *const u8).add(n).wrapping_sub(1),
            inout("edi") (dst as *mut u8).add(n).wrapping_sub(1) => _,
            inout("ecx") n => _,
        );
    }
    dst
}

/// # Safety
/// The usual `memset` contract: `n` valid bytes at `dst`.
#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void {
    asm!("rep stosb", inout("edi") dst => _, inout("ecx") n => _, in("al") c as u8, options(preserves_flags));
    dst
}

/// # Safety
/// The usual `memcmp` contract: `n` valid bytes at both pointers.
#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn memcmp(a: *const c_void, b: *const c_void, n: usize) -> i32 {
    if n == 0 {
        return 0;
    }
    let r: i32;
    // REPE CMPSB stops after the first differing byte, or when the count runs out; ESI and EDI are then one past the last pair
    // compared, so the bytes at ESI-1 and EDI-1 are the ones that differed.
    asm!(
        "push esi",
        "mov esi, {a}",
        "repe cmpsb",
        "je 2f",
        "movzx eax, byte ptr [esi - 1]",
        "movzx edx, byte ptr [edi - 1]",
        "sub eax, edx",
        "jmp 3f",
        "2:",
        "xor eax, eax",
        "3:",
        "pop esi",
        a = in(reg) a,
        inout("edi") b => _,
        inout("ecx") n => _,
        out("eax") r,
        out("edx") _,
    );
    r
}

/// Below this many bytes a plain byte loop is used. Anything shorter than 7 could not hold a head of up to 3 bytes, one whole word and
/// a tail, and `rep` has a start-up cost that a few bytes do not repay; 16 is a round number above that floor. It is a judgement, not a
/// measurement: no timing tool exists yet (R137), so no speed is claimed.
#[cfg(not(target_os = "windows"))]
const SHORT: usize = 16;

/// # Safety
/// The usual `memcpy` contract: `n` valid bytes at both pointers, not overlapping.
#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    let d = dst as *mut u8;
    let s = src as *const u8;
    // Volatile accesses keep the compiler from turning these loops back into calls to `memcpy`.
    let mut i = 0;
    if n >= SHORT {
        // Head: bytes until the destination is 4-byte aligned.
        let head = (d as usize).wrapping_neg() & 3;
        while i < head {
            // SAFETY: `i < head <= n`, and the contract makes `n` bytes valid at both pointers.
            unsafe { d.add(i).write_volatile(s.add(i).read_volatile()) };
            i += 1;
        }
        let words = (n - head) / 4;
        if words > 0 {
            // SAFETY: `words * 4` bytes remain valid at both pointers past `head`, and `d + head` is aligned. The direction flag is clear:
            // `_start` executes `cld`, and the ELF build never sets the flag again (the only `std` is in the stage-1 `memmove`, which is followed by `cld`).
            // ESI is saved by hand because the compiler may not be given it; no `pure`, `nomem` or `nostack`, so the stores are kept.
            unsafe {
                asm!(
                    "push esi", "mov esi, {s}", "rep movsd", "pop esi",
                    s = in(reg) s.add(head),
                    inout("edi") d.add(head) => _,
                    inout("ecx") words => _,
                    options(preserves_flags),
                );
            }
        }
        i = head + words * 4;
    }
    // Tail, or the whole of a short run.
    while i < n {
        // SAFETY: `i < n`, valid at both pointers by the contract.
        unsafe { d.add(i).write_volatile(s.add(i).read_volatile()) };
        i += 1;
    }
    dst
}

/// # Safety
/// The usual `memset` contract: `n` valid bytes at `dst`.
#[cfg(not(target_os = "windows"))]
#[no_mangle]
pub unsafe extern "C" fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void {
    let d = dst as *mut u8;
    let b = c as u8;
    let mut i = 0;
    if n >= SHORT {
        let head = (d as usize).wrapping_neg() & 3;
        while i < head {
            // SAFETY: `i < head <= n`, and `n` bytes are valid at `dst`.
            unsafe { d.add(i).write_volatile(b) };
            i += 1;
        }
        let words = (n - head) / 4;
        if words > 0 {
            // SAFETY: `words * 4` bytes remain valid past `head` and `d + head` is aligned. The direction flag is clear, as in `memcpy`.
            // EAX is the fill word; no `pure`, `nomem` or `nostack`, so the stores are kept.
            unsafe {
                asm!(
                    "rep stosd",
                    inout("edi") d.add(head) => _,
                    inout("ecx") words => _,
                    in("eax") u32::from(b) * 0x0101_0101,
                    options(preserves_flags),
                );
            }
        }
        i = head + words * 4;
    }
    while i < n {
        // SAFETY: `i < n`, valid at `dst` by the contract.
        unsafe { d.add(i).write_volatile(b) };
        i += 1;
    }
    dst
}
