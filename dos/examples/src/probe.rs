// The platform probes: video class, processor family, x87 (with the control word proved unchanged), free memory and the game port.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Only what is the same on every run is printed as a value; what depends on the machine (the amount of memory, what the game port
// answers) is printed as a fact about its shape. With the argument `x87-restore` the control word is first set to a value that is
// not the power-on one, and the probe must leave it, and a value on the x87 stack, exactly as they were.
use core::arch::asm;
use rt::probe::{self, Cpu, VideoClass, VideoSource};
use rt::{env, fpu, println};

/// 24-bit precision, round toward zero, every exception masked: not 037Fh, which is what `_start` leaves.
const ODD_CONTROL_WORD: u16 = 0x0C7F;

fn load_control_word(cw: u16) {
    // SAFETY: FLDCW loads two bytes from the pointer; every exception stays masked in the values used here.
    unsafe { asm!("fldcw word ptr [{p}]", p = in(reg) &cw, options(nostack)) };
}

/// Pushes 1.0 on the x87 stack.
fn push_one() {
    // SAFETY: FLD1 pushes one value; the caller pops it with `pop_bits`, so the stack is balanced.
    unsafe {
        asm!("fld1", out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _, out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _, options(nostack))
    };
}

/// Pops the top of the x87 stack and returns the bits of the f64 it held.
fn pop_bits() -> u64 {
    let mut bits: u64 = 0;
    // SAFETY: FSTP stores eight bytes through the pointer and pops the value `push_one` pushed.
    unsafe {
        asm!("fstp qword ptr [{p}]", p = in(reg) &mut bits, out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _, out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _, options(nostack));
    }
    bits
}

const ONE: u64 = 0x3FF0_0000_0000_0000;

fn restore_mode() -> i32 {
    load_control_word(ODD_CONTROL_WORD);
    push_one();
    let before = fpu::control_word();
    let found = probe::x87();
    let after = fpu::control_word();
    let top = pop_bits();
    println!("probe: control word set to {:#06x}, now {:#06x}", ODD_CONTROL_WORD, before);
    println!(
        "probe: x87 present {}, control word found {:#06x}",
        found.present,
        found.control_word.unwrap_or(0)
    );
    println!("probe: control word after the probe {:#06x}", after);
    println!(
        "probe: control word unchanged {}, reported as found {}, value on the stack kept {}",
        before == after,
        found.control_word == Some(before),
        top == ONE
    );
    load_control_word(0x037F);
    println!("probe: control word put back to {:#06x}", fpu::control_word());
    0
}

pub fn main() -> i32 {
    if env::args().first().map(alloc::string::String::as_str) == Some("x87-restore") {
        return restore_mode();
    }

    let v = probe::video();
    let how = match v.source {
        VideoSource::DisplayCombination => "display combination code",
        VideoSource::EgaInformation => "EGA information",
        VideoSource::Equipment => "equipment word",
        VideoSource::None => "nothing answered",
    };
    println!("probe: video class {:?} (by {}, code {:#04x})", v.class, how, v.code);
    println!("probe: video has mode 13h and 12h: {}", v.class == VideoClass::Vga);

    let cpu = probe::cpu();
    println!("probe: processor {:?}", cpu);
    println!(
        "probe: a 486 or later (the AC flag can be set): {}; CPUID exists (the ID flag can be toggled, CPUID not run): {}",
        cpu != Cpu::I386,
        cpu == Cpu::Later
    );

    let before = fpu::control_word();
    let x = probe::x87();
    let after = fpu::control_word();
    println!(
        "probe: x87 present {}, control word found {:#06x}, after the probe {:#06x}",
        x.present,
        x.control_word.unwrap_or(0),
        after
    );
    println!(
        "probe: control word unchanged {}, same as the probe reported {}",
        before == after,
        x.control_word == Some(before)
    );

    match probe::memory() {
        Some(m) => {
            let block = m.largest_free_block.is_some_and(|b| (0x1_0000..=0xFFFF_F000).contains(&b));
            let pages = m.free_pages.is_none_or(|p| p >= 16);
            println!(
                "probe: memory largest free block in range: {}, free pages in range: {}",
                block, pages
            );
        }
        None => println!("probe: memory not reported"),
    }

    let j = probe::joystick_raw();
    println!(
        "probe: joystick poll ended within the bound: {}, axes within a nibble: {}",
        j.reads >= 1 && j.reads <= probe::JOYSTICK_POLLS,
        j.axes <= 0x0F
    );
    // Whether a stick answered, and what an empty port reads, depend on how the machine is set up; neither is printed.
    let _ = (j.stick_a(), j.stick_b(), j.first);
    0
}
