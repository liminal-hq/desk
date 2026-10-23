// The IDT, the exception stubs and the exception dump.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{exit, gdt, println};
use core::arch::{asm, global_asm};

// One small stub per CPU exception. Exceptions without a hardware error code push a zero so every frame has the same shape,
// then all push their vector number and share `isr_common`, which saves the registers and calls `exception_handler`. The mask
// lists the vectors that push an error code (8, 10-14, 17, 21, 29, 30). The stub addresses are collected in `isr_table`.
global_asm!(
    r#"
    .section .rodata.isr_table, "a"
    .balign 4
    .global isr_table
isr_table:

    .macro isr n
    .section .text.isr, "ax"
isr\n:
    .if ((1 << \n) & 0x60227D00) == 0
    push 0
    .endif
    push \n
    jmp isr_common
    .section .rodata.isr_table, "a"
    .long isr\n
    .endm

    .irp n, 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31
    isr \n
    .endr

    .section .text.isr, "ax"
isr_common:
    pushad
    cld
    push esp
    call exception_handler
2:  cli
    hlt
    jmp 2b

    .global irq0_stub
irq0_stub:
    pushad
    cld
    call timer_irq
    popad
    iretd
"#
);

extern "C" {
    static isr_table: [u32; 32];
    fn irq0_stub();
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Gate {
    offset_low: u16,
    selector: u16,
    zero: u8,
    flags: u8,
    offset_high: u16,
}

const MISSING: Gate = Gate { offset_low: 0, selector: 0, zero: 0, flags: 0, offset_high: 0 };

static mut IDT: [Gate; 256] = [MISSING; 256];

#[repr(C, packed)]
struct Pointer {
    limit: u16,
    base: u32,
}

fn gate(handler: u32) -> Gate {
    // 0x8E: present, ring 0, 32-bit interrupt gate (clears IF on entry).
    Gate { offset_low: handler as u16, selector: gdt::KERNEL_CODE, zero: 0, flags: 0x8E, offset_high: (handler >> 16) as u16 }
}

/// Fills the table with the 32 exception gates and the timer gate (vector 0x20), then loads it.
pub fn init() {
    // SAFETY: single processor with interrupts off; nothing else touches IDT before `lidt` makes it live.
    unsafe {
        let idt = &mut *core::ptr::addr_of_mut!(IDT);
        for (v, slot) in idt.iter_mut().take(32).enumerate() {
            *slot = gate(isr_table[v]);
        }
        idt[0x20] = gate(irq0_stub as *const () as u32);
        let ptr = Pointer { limit: (core::mem::size_of::<[Gate; 256]>() - 1) as u16, base: idt.as_ptr() as u32 };
        asm!("lidt [{0}]", in(reg) &ptr);
    }
}

/// The registers as `isr_common` leaves them on the stack: `pushad` order (EDI first in memory), then the vector and error
/// code, then what the CPU pushed.
#[repr(C)]
pub struct Frame {
    edi: u32,
    esi: u32,
    ebp: u32,
    esp_at_pushad: u32,
    ebx: u32,
    edx: u32,
    ecx: u32,
    eax: u32,
    vector: u32,
    error: u32,
    eip: u32,
    cs: u32,
    eflags: u32,
}

const NAMES: [&str; 32] = [
    "divide error",
    "debug",
    "non-maskable interrupt",
    "breakpoint",
    "overflow",
    "bound range exceeded",
    "invalid opcode",
    "device not available",
    "double fault",
    "coprocessor segment overrun",
    "invalid TSS",
    "segment not present",
    "stack-segment fault",
    "general protection fault",
    "page fault",
    "reserved",
    "x87 floating-point error",
    "alignment check",
    "machine check",
    "SIMD floating-point exception",
    "virtualisation exception",
    "control protection exception",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "reserved",
    "hypervisor injection",
    "VMM communication",
    "security exception",
    "reserved",
];

fn cr(n: u8) -> u32 {
    let v: u32;
    // SAFETY: reading a control register has no side effects.
    unsafe {
        match n {
            0 => asm!("mov {0}, cr0", out(reg) v),
            2 => asm!("mov {0}, cr2", out(reg) v),
            _ => asm!("mov {0}, cr3", out(reg) v),
        }
    }
    v
}

#[no_mangle]
extern "C" fn exception_handler(f: &Frame) -> ! {
    let name = NAMES.get(f.vector as usize).copied().unwrap_or("unknown");
    println!("\nEXCEPTION {:#04x}: {}", f.vector, name);
    println!("error code {:#010x}", f.error);
    println!("EIP {:#010x}  CS {:#06x}  EFLAGS {:#010x}", f.eip, f.cs, f.eflags);
    println!("EAX {:#010x}  EBX {:#010x}  ECX {:#010x}  EDX {:#010x}", f.eax, f.ebx, f.ecx, f.edx);
    println!("ESI {:#010x}  EDI {:#010x}  EBP {:#010x}  ESP {:#010x}", f.esi, f.edi, f.ebp, f.esp_at_pushad);
    println!("CR0 {:#010x}  CR2 {:#010x}  CR3 {:#010x}", cr(0), cr(2), cr(3));
    println!("halting with a failure status");
    exit::failure()
}

#[no_mangle]
extern "C" fn timer_irq() {
    crate::pit::tick();
}
