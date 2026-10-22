// memcpy, memmove, memset and memcmp for the stage 1 build, where nothing else provides them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// In the ELF build `compiler_builtins` supplies these (the `compiler-builtins-mem` feature). For the PE-flavoured COFF target its
// object cannot be read by DJGPP's `ld` (weak externals), so the runtime defines the four itself, with the x86 string
// instructions: written as loops, the compiler would turn them back into calls to themselves.
use core::arch::asm;
use core::ffi::c_void;

/// # Safety
/// The usual `memcpy` contract: `n` valid bytes at both pointers, not overlapping.
#[no_mangle]
pub unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    asm!("push esi", "mov esi, {s}", "rep movsb", "pop esi", s = in(reg) src, inout("edi") dst => _, inout("ecx") n => _, options(preserves_flags));
    dst
}

/// # Safety
/// The usual `memmove` contract: `n` valid bytes at both pointers, which may overlap.
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
#[no_mangle]
pub unsafe extern "C" fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void {
    asm!("rep stosb", inout("edi") dst => _, inout("ecx") n => _, in("al") c as u8, options(preserves_flags));
    dst
}

/// # Safety
/// The usual `memcmp` contract: `n` valid bytes at both pointers.
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
