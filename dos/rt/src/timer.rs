// A hook on IRQ 0 that counts ticks: the proof that a protected-mode interrupt handler works behind the stub and the host.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The hardware interrupt arrives at a protected-mode vector (INT 08h with the usual PIC base, which `dpmi::version` reports).
// The host calls the handler with the code selector and the interrupted code's data segment registers, on a stack of its own, so
// the handler loads the flat data selector itself, reading it through CS (same base as DS, so the same offsets name the same
// bytes) because it cannot trust DS yet. It increments a counter and jumps to the previous handler, which acknowledges the
// interrupt and reflects it to the BIOS, so the system clock keeps time. Everything the handler touches, the handler itself
// included, is locked in memory so the host never needs to page it in at interrupt time.
use crate::{cleanup, dpmi};
use core::arch::global_asm;
use core::ptr::{addr_of, addr_of_mut, read_volatile};

#[no_mangle]
static mut RT_IRQ0_TICKS: u32 = 0;
/// The data selector for the handler to load, in a place it can read through CS.
#[no_mangle]
static mut RT_IRQ0_DSEL: u32 = 0;
/// The previous handler as a far pointer: 32-bit offset, then 16-bit selector (the layout of `jmp fword ptr`).
#[no_mangle]
static mut RT_IRQ0_OLD: [u32; 2] = [0; 2];

global_asm!(
    ".text",
    ".global rt_irq0",
    "rt_irq0:",
    "push eax",
    "push ds",
    "mov ax, cs:[{dsel}]",
    "mov ds, ax",
    "inc dword ptr [{ticks}]",
    "pop ds",
    "pop eax",
    "jmp fword ptr cs:[{old}]",
    dsel = sym RT_IRQ0_DSEL,
    ticks = sym RT_IRQ0_TICKS,
    old = sym RT_IRQ0_OLD,
);

extern "C" {
    fn rt_irq0();
    static __image_start: u8;
    static __image_end: u8;
}

static mut VECTOR: u8 = 0;
static mut HOOKED: bool = false;

/// Hooks IRQ 0. Returns the (selector, offset) of the handler it replaced.
pub fn hook() -> Result<(u16, u32), dpmi::Error> {
    // SAFETY: single-threaded set-up before the handler can run; the statics are only touched by the handler after the vector is set.
    unsafe {
        if HOOKED {
            return Err(dpmi::Error(0));
        }
        let (_, _, _, _, master, _) = dpmi::version();
        let vector = if master == 0 { 8 } else { master };
        // Lock the whole image (code, data, bss, stack): the handler and every static it reaches are inside it.
        let start = dpmi::ptr_to_linear(addr_of!(__image_start));
        let end = dpmi::ptr_to_linear(addr_of!(__image_end));
        dpmi::lock(start, end - start)?;
        let old = dpmi::get_pm_vector(vector)?;
        RT_IRQ0_DSEL = dpmi::data_selector() as u32;
        RT_IRQ0_OLD = [old.1, old.0 as u32];
        let cs: u16;
        core::arch::asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
        dpmi::set_pm_vector(vector, cs, rt_irq0 as *const () as usize as u32)?;
        VECTOR = vector;
        HOOKED = true;
        cleanup::register(unhook);
        Ok(old)
    }
}

/// Puts the previous handler back. Safe to call twice, and from the clean-up list.
pub fn unhook() {
    // SAFETY: restores the saved far pointer; after this the handler is unreachable.
    unsafe {
        if !HOOKED {
            return;
        }
        let old = read_volatile(addr_of!(RT_IRQ0_OLD));
        let _ = dpmi::set_pm_vector(VECTOR, old[1] as u16, old[0]);
        HOOKED = false;
        cleanup::forget(unhook);
    }
}

/// Whether the hook is installed.
pub fn is_hooked() -> bool {
    // SAFETY: single-threaded read.
    unsafe { HOOKED }
}

/// The protected-mode vector number that carries IRQ 0.
pub fn vector() -> u8 {
    // SAFETY: single-threaded read.
    unsafe { VECTOR }
}

/// Ticks counted since the hook was installed.
pub fn ticks() -> u32 {
    // SAFETY: a volatile read of a word the handler increments; a 32-bit aligned access is atomic.
    unsafe { read_volatile(addr_of_mut!(RT_IRQ0_TICKS)) }
}

/// The BIOS tick counter at 0040:006C, which the BIOS's own IRQ 0 handler (reached by chaining) increments.
pub fn bios_ticks() -> u32 {
    // SAFETY: conventional memory is mapped at linear 0; the counter is a word-aligned dword.
    unsafe { read_volatile(dpmi::linear_to_ptr::<u32>(0x46C)) }
}
