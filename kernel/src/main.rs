// Kernel entry: the assembly start-up stub, then Rust's kmain runs the bring-up sequence that the boot transcript records.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
#![no_main]

extern crate alloc;

mod bios;
mod biostest;
mod console;
mod exit;
mod fpu;
mod gfx;
mod gdt;
mod hal;
mod heap;
mod idt;
mod input;
mod memory;
mod pic;
mod pit;
#[path = "../../spikes/a-vga-vbe/prog/src/scene.rs"]
mod scene;
mod serial;
mod sync;
mod vga;
mod video;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::arch::{asm, global_asm};

// The loader jumps here in 32-bit protected mode with flat segments, interrupts off and EAX holding the address of the boot
// information block. The stub gives the kernel its own stack, clears .bss (the flat image does not contain it) and calls
// `kmain`. EAX is parked in EBX because the clearing loop uses EAX.
global_asm!(
    r#"
    .section .text.boot, "ax"
    .global _start
_start:
    cli
    cld
    mov ebx, eax
    mov esp, offset __stack_top
    mov edi, offset __bss_start
    mov ecx, offset __bss_end
    sub ecx, edi
    shr ecx, 2
    xor eax, eax
    rep stosd
    push ebx
    call kmain
2:  cli
    hlt
    jmp 2b

    .section .bss.stack, "aw", @nobits
    .balign 16
__stack_bottom:
    .skip 65536
    .global __stack_top
__stack_top:
"#
);

/// The block the loader leaves at physical 0x500 (see `docs/NATIVE-BOOT.md`).
#[repr(C)]
pub struct BootInfo {
    magic: u32,
    boot_drive: u32,
    test_mode: u32,
    map_source: u32,
    map_count: u32,
    /// Boot information version: 1 adds the next two fields (they were reserved before, and the loader did not write them).
    version: u32,
    /// Address of the 32-byte video block (see `video.rs`), 0 for none.
    video: u32,
    /// Address of the BIOS 8x16 font (4096 bytes), 0 for none.
    font: u32,
    // E820-style entries follow at offset 32.
}

const BOOT_MAGIC: u32 = 0x4B53_4544; // "DESK", little-endian
const TICK_WAIT: u32 = 10;

#[derive(Clone, Copy, PartialEq)]
enum TestMode {
    Normal = 0,
    DivideError = 1,
    GeneralProtection = 2,
    InvalidOpcode = 3,
    Panic = 4,
    Hang = 5,
    NoTick = 6,
    Input = 7,
    InputMasked = 8,
    InputIdle = 9,
    Gfx = 10,
    Bios = 11,
    BiosSolo = 12,
}

impl TestMode {
    fn from_u32(v: u32) -> TestMode {
        match v {
            1 => TestMode::DivideError,
            2 => TestMode::GeneralProtection,
            3 => TestMode::InvalidOpcode,
            4 => TestMode::Panic,
            5 => TestMode::Hang,
            6 => TestMode::NoTick,
            7 => TestMode::Input,
            8 => TestMode::InputMasked,
            9 => TestMode::InputIdle,
            10 => TestMode::Gfx,
            11 => TestMode::Bios,
            12 => TestMode::BiosSolo,
            _ => TestMode::Normal,
        }
    }
}

#[no_mangle]
pub extern "C" fn kmain(boot_info: *const BootInfo) -> ! {
    serial::init();
    console::init();
    println!("Desk native kernel {} (i486)", env!("CARGO_PKG_VERSION"));
    // SAFETY: the loader wrote the block at a fixed low address that nothing else uses; the magic is checked before use.
    let info = unsafe { &*boot_info };
    if info.magic != BOOT_MAGIC {
        panic!("boot info magic {:#x} is wrong", info.magic);
    }
    let mode = TestMode::from_u32(info.test_mode);
    println!("boot: drive {:#04x}, memory map from {} ({} entries)", info.boot_drive, memory::source_name(info.map_source), info.map_count);

    gdt::init();
    println!("gdt: loaded, flat 4 GiB code and data segments");
    idt::init();
    println!("idt: 32 exception gates and the timer gate installed");
    pic::init();
    println!("pic: remapped to 0x20 and 0x28, only IRQ0 unmasked");
    pit::init(100);
    println!("pit: channel 0 at {} Hz", pit::HZ);

    memory::init(info);
    memory::report(info);
    let free_before = memory::free_frames();
    let a = memory::alloc_frame().expect("no frame");
    let b = memory::alloc_frame().expect("no frame");
    assert!(a != b && a % 4096 == 0 && b % 4096 == 0);
    memory::free_frame(a);
    memory::free_frame(b);
    assert_eq!(memory::free_frames(), free_before);
    println!("frames: alloc and free of two frames ok");
    bios::init().expect("the BIOS trampoline");
    println!(
        "bios: real-mode trampoline at {:#x}, bounce buffer {:#x} ({} bytes), stack {:#x}",
        bios::LOW_BASE,
        bios::BOUNCE_BASE,
        bios::BOUNCE_LEN,
        bios::stack_base()
    );
    heap::init();
    let video = video::init(info);

    let v: Vec<u32> = (1..=10).map(|x| x * x).collect();
    let sum: u32 = v.iter().sum();
    let mut s = String::from("desk");
    s.push_str("-native");
    let f = format!("{}:{}:{:#06x}", s, v.len(), sum);
    println!("alloc: Vec sum {}, String \"{}\", format \"{}\"", sum, s, f);
    drop(v);
    drop(s);
    drop(f);
    println!("heap: {} KiB free after the demo", heap::free_bytes() / 1024);

    if mode == TestMode::NoTick {
        println!("test: timer masked on purpose");
        pic::mask_timer();
    }
    timer_demo();

    fpu::init();
    fpu::demo();

    run_test_mode(mode);
    input_section(mode, &video, info);
    println!("boot complete");
    exit::success()
}

fn timer_demo() {
    // Interrupts come on here. A bounded spin proves IRQ0 is arriving at all, because `hlt` would otherwise wait forever.
    // SAFETY: the IDT and PIC are set up; the timer handler only touches an atomic counter and the PIC.
    unsafe { asm!("sti") };
    let mut spins = 0u32;
    while pit::ticks() == 0 {
        spins += 1;
        if spins > 200_000_000 {
            panic!("the timer did not tick");
        }
    }
    let start = pit::ticks();
    while pit::ticks() - start < TICK_WAIT {
        // SAFETY: interrupts are enabled, so hlt returns on the next tick.
        unsafe { asm!("hlt") };
    }
    let elapsed = pit::ticks() - start;
    println!("timer: waited for {} ticks with hlt, elapsed {} ticks", TICK_WAIT, elapsed);
    assert!(elapsed >= TICK_WAIT);
}

/// The keyboard and mouse section, after the checks above. A normal boot sets the devices up and goes on. The input test modes
/// then wait for events that the run script injects (`docs/C-INPUT.md`): `Input` waits for Escape, `InputMasked` leaves IRQ 1
/// masked so the wait must fail, and `InputIdle` waits long enough that the run script's watchdog ends the run. `Gfx` hands the
/// devices to the graphics tour.
fn input_section(mode: TestMode, video: &video::Video, info: &BootInfo) {
    let fault = if mode == TestMode::InputMasked { input::Fault::KeyboardMasked } else { input::Fault::None };
    let Some(mut ps2) = input::init(fault) else {
        if mode == TestMode::Normal {
            return;
        }
        panic!("input: no keyboard controller, so the input test cannot run");
    };
    if mode == TestMode::Bios || mode == TestMode::BiosSolo {
        println!("input: ready");
        biostest::run(info, video, ps2, mode == TestMode::BiosSolo);
        return;
    }
    if mode == TestMode::Gfx {
        // The graphics tour (`gfx.rs`, `docs/D-NATIVE-VGA.md`) needs the keyboard and mouse for the window and for the run script's
        // acknowledgements of its screenshots.
        println!("input: ready");
        gfx::tour(video, ps2);
        return;
    }
    let timeout_ticks = match mode {
        TestMode::Input => 1500,
        TestMode::InputMasked => 500,
        TestMode::InputIdle => 100_000,
        _ => return,
    };
    println!("input: ready");
    input::run(&mut ps2, timeout_ticks);
}

/// Deliberate failures for the negative checks; the loader's `test_mode` byte selects one.
fn run_test_mode(mode: TestMode) {
    match mode {
        TestMode::Normal | TestMode::NoTick | TestMode::Input | TestMode::InputMasked | TestMode::InputIdle | TestMode::Gfx | TestMode::Bios | TestMode::BiosSolo => {}
        TestMode::DivideError => {
            println!("test: dividing by zero");
            // SAFETY: deliberately faults; the exception handler reports it and exits.
            unsafe { asm!("xor edx, edx", "mov eax, 1", "xor ecx, ecx", "div ecx", out("eax") _, out("edx") _, out("ecx") _) };
        }
        TestMode::GeneralProtection => {
            println!("test: loading a segment selector beyond the GDT");
            // SAFETY: deliberately faults with #GP; the exception handler reports it and exits.
            unsafe { asm!("mov ax, 0x28", "mov ds, ax", out("ax") _) };
        }
        TestMode::InvalidOpcode => {
            println!("test: executing ud2");
            // SAFETY: deliberately faults with #UD.
            unsafe { asm!("ud2") };
        }
        TestMode::Panic => {
            println!("test: panicking");
            panic!("deliberate panic for the negative check");
        }
        TestMode::Hang => {
            println!("test: hanging with interrupts off");
            loop {
                // SAFETY: stops the CPU for good so the run script's watchdog has to end the run.
                unsafe { asm!("cli", "hlt") };
            }
        }
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    static PANICKING: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
    if !PANICKING.swap(true, core::sync::atomic::Ordering::SeqCst) {
        println!("\nKERNEL PANIC: {}", info);
    }
    exit::failure()
}
