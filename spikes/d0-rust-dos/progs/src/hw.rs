// Hardware proof: an IRQ 0 hook counting ticks for about a second, and VGA mode 13h written and read back.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use rt::{dpmi, env, println, timer, vga};

static mut BEFORE: (u16, u32) = (0, 0);

/// Registered first, so it runs last: says whether exit left the machine as it was found.
fn report() {
    // SAFETY: single-threaded; BEFORE was set before this was registered.
    let before = unsafe { BEFORE };
    let back = dpmi::get_pm_vector(8).unwrap() == before && vga::get_mode() == 0x03;
    println!(
        "hw: after the clean-up list the timer vector and text mode are back: {}",
        back
    );
}

pub fn main() -> i32 {
    let args = env::args();
    let (maj, min, _, _, master, slave) = dpmi::version();
    println!("hw: dpmi {}.{} pic bases {:#04x} {:#04x}", maj, min, master, slave);

    // Timer.
    let before = dpmi::get_pm_vector(8).unwrap();
    let bios0 = timer::bios_ticks();
    timer::hook().expect("hook");
    let hooked = dpmi::get_pm_vector(8).unwrap();
    println!("hw: vector 8 moved to our handler: {}", before != hooked);
    let mut spins: u64 = 0;
    while timer::ticks() < 19 && spins < 4_000_000_000 {
        spins += 1;
        core::hint::spin_loop();
    }
    let ticks = timer::ticks();
    let bios = timer::bios_ticks().wrapping_sub(bios0);
    println!(
        "hw: ticks counted in about a second: {} (expected 18 to 20): {}",
        if (18..=20).contains(&ticks) {
            "in range"
        } else {
            "OUT OF RANGE"
        },
        ticks >= 18
    );
    println!(
        "hw: BIOS tick counter advanced by the same amount (within 1): {}",
        bios.abs_diff(ticks) <= 1
    );
    timer::unhook();
    let after = dpmi::get_pm_vector(8).unwrap();
    println!("hw: vector 8 restored: {}", before == after);
    let t = timer::ticks();
    for _ in 0..3_000_000 {
        core::hint::spin_loop();
    }
    println!("hw: no ticks counted after unhook: {}", timer::ticks() == t);

    // VGA.
    let mode0 = vga::get_mode();
    vga::enter_mode13();
    let m13 = vga::get_mode();
    vga::draw_pattern();
    let sum = vga::checksum();
    let want = vga::expected_checksum();
    vga::leave();
    let mode1 = vga::get_mode();
    println!("hw: mode before {:#04x}, in mode {:#04x}, after {:#04x}", mode0, m13, mode1);
    println!(
        "hw: frame buffer checksum {:#010x}, pattern in memory {:#010x}, match {}",
        sum,
        want,
        sum == want
    );

    // The interrupt left hooked on purpose when asked: the exit path must put it back.
    if args.first().map(alloc::string::String::as_str) == Some("leave-hooked") {
        // SAFETY: single-threaded, before the hook.
        unsafe { BEFORE = before };
        rt::cleanup::register(report);
        timer::hook().expect("hook");
        vga::enter_mode13();
        println!("hw: exiting with the timer hooked and mode 13h set");
    }
    // Bypasses the clean-up list: what the host itself does about a hooked vector when the program ends.
    if args.first().map(alloc::string::String::as_str) == Some("abandon") {
        timer::hook().expect("hook");
        println!("hw: ending with the timer hooked and no clean-up");
        rt::dos::exit_now(0);
    }
    0
}
