// The x87: its state at entry and the maths `core` does not have.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `_start` runs FNINIT, which leaves the control word at 037Fh: 64-bit mantissa (extended precision), round to nearest, all
// exceptions masked. `core` has `f64` arithmetic and formatting but no `sqrt`, `sin` and the like (those are in `std`), so
// the few that Desk needs are here. Rust cannot name an x87 register as an asm operand, only as a clobber, so each value goes
// through memory.
use core::arch::asm;

/// The control word as it is now.
pub fn control_word() -> u16 {
    let mut cw: u16 = 0;
    // SAFETY: FNSTCW stores two bytes through the pointer.
    unsafe { asm!("fnstcw word ptr [{p}]", p = in(reg) &mut cw, options(nostack)) };
    cw
}

/// Sets the precision control to 53 bits (control word 027Fh), which makes `f64` arithmetic round as IEEE doubles do. Needed only
/// when results must match another build bit for bit.
pub fn set_double_precision() {
    let cw: u16 = 0x027F;
    // SAFETY: FLDCW loads two bytes from the pointer.
    unsafe { asm!("fldcw word ptr [{p}]", p = in(reg) &cw, options(nostack)) };
}

macro_rules! x87_unary {
    ($(#[$m:meta])* $name:ident, $($op:literal),+) => {
        $(#[$m])*
        pub fn $name(x: f64) -> f64 {
            let mut r: f64 = 0.0;
            // SAFETY: loads `x` onto the x87 stack, applies the operation and stores the result, popping the stack back to how it
            // was found; the clobbers declare the use of the x87 registers.
            unsafe {
                asm!(
                    "fld qword ptr [{x}]",
                    $($op,)+
                    "fstp qword ptr [{r}]",
                    x = in(reg) &x,
                    r = in(reg) &mut r,
                    out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                    out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
                    options(nostack),
                );
            }
            r
        }
    };
}

x87_unary!(
    /// Square root (FSQRT).
    sqrt, "fsqrt"
);
x87_unary!(
    /// Sine, for arguments within +-2^63 radians (FSIN).
    sin, "fsin"
);
x87_unary!(
    /// Cosine (FCOS).
    cos, "fcos"
);

/// The arctangent of y/x with the quadrant (FPATAN).
pub fn atan2(y: f64, x: f64) -> f64 {
    let mut r: f64 = 0.0;
    // SAFETY: as `x87_unary`, with two operands pushed and one result stored.
    unsafe {
        asm!(
            "fld qword ptr [{y}]",
            "fld qword ptr [{x}]",
            "fpatan",
            "fstp qword ptr [{r}]",
            y = in(reg) &y,
            x = in(reg) &x,
            r = in(reg) &mut r,
            out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
            out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            options(nostack),
        );
    }
    r
}
