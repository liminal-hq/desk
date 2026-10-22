// The kernel's own GDT: a null descriptor and flat 4 GiB ring 0 code and data segments.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::arch::asm;

pub const KERNEL_CODE: u16 = 0x08;
pub const KERNEL_DATA: u16 = 0x10;

// Base 0, limit 0xFFFFF in 4 KiB pages (4 GiB), 32-bit; access 0x9A is code (execute/read), 0x92 is data (read/write), both
// present at ring 0.
static GDT: [u64; 3] = [0, 0x00CF_9A00_0000_FFFF, 0x00CF_9200_0000_FFFF];

#[repr(C, packed)]
struct Pointer {
    limit: u16,
    base: u32,
}

/// Loads the GDT and reloads every segment register. The loader already left a similar GDT in use, but it lives in the loader's
/// memory, which the kernel is free to overwrite.
pub fn init() {
    let ptr = Pointer { limit: (core::mem::size_of_val(&GDT) - 1) as u16, base: GDT.as_ptr() as u32 };
    // SAFETY: the table is static and valid; the far return reloads CS with the new code selector and the moves reload the
    // data segments with the new data selector.
    unsafe {
        asm!(
            "lgdt [{ptr}]",
            "push {code}",
            "lea {tmp}, [2f]",
            "push {tmp}",
            "retf",
            "2:",
            "mov ax, {data}",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            ptr = in(reg) &ptr,
            code = const KERNEL_CODE as u32,
            data = const KERNEL_DATA,
            tmp = out(reg) _,
            out("ax") _,
        );
    }
}
