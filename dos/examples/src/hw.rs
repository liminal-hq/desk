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
    if let Some(status) = args.first().and_then(|m| fast_modes(m)) {
        return status;
    }
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
    // A second install starts from zero, not from where the first one stopped (t is about 19 here).
    timer::hook().expect("hook again");
    println!(
        "hw: ticks restart from zero on a second install: {}",
        t >= 18 && timer::ticks() <= 1
    );
    timer::unhook();

    // VGA.
    let mode0 = vga::get_mode();
    vga::enter_mode13().expect("clean-up list full");
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
        assert!(rt::cleanup::register(report), "clean-up list full");
        timer::hook().expect("hook");
        vga::enter_mode13().expect("clean-up list full");
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

// ---- The fast timer (`rt::timer::hook_rate`): modes `fast`, `fast-full`, `fast-leave-hooked`, `fast-panic` and `rate-check` ----

/// 60 Hz: 1,193,182 / 60 rounded to the nearest.
const DIVISOR_60_HZ: u32 = 19_886;
/// 560 Hz.
const DIVISOR_560_HZ: u32 = 2_131;

/// Spins until `done` says so, at most a few thousand million times. Returns whether it finished.
fn spin_until(mut done: impl FnMut() -> bool) -> bool {
    let mut spins: u64 = 0;
    while !done() {
        spins += 1;
        if spins > 4_000_000_000 {
            return false;
        }
        core::hint::spin_loop();
    }
    true
}

/// One second by the PIT's own counter in the BIOS's mode 3, where the latch counts down by two per input clock and wraps every 65,536
/// counts: the clocks passed are the sum of the drops between polls. A second reference, apart from the BIOS tick counter and the fast
/// timer, that says the BIOS rate is back (a PIT left at 60 Hz in mode 2 counts by one, so it would take twice as long and the BIOS
/// counter would advance far more than 18 or 19). Returns the BIOS ticks that passed.
fn bios_ticks_in_one_pit_second() -> u32 {
    let start = timer::bios_elapsed();
    let mut prev = timer::read_counter();
    let mut counts: u64 = 0;
    // The poll is a few microseconds apart, far under half a wrap (about 27 ms).
    spin_until(|| {
        let now = timer::read_counter();
        counts += u64::from(prev.wrapping_sub(now));
        prev = now;
        counts >= 2 * 1_193_182
    });
    timer::bios_elapsed_since(start)
}

fn bios_rate_line(what: &str) {
    let ticks = bios_ticks_in_one_pit_second();
    println!(
        "hw: {}: the BIOS counter advanced 18 to 19 ticks in one second by the PIT: {}",
        what,
        (18..=19).contains(&ticks)
    );
}

/// Waits for `ticks` fast ticks and reports the fast count and the BIOS counter's advance over them.
fn fast_second(label: &str, ticks: u32) {
    let bios0 = timer::bios_elapsed();
    let done = spin_until(|| timer::ticks() >= ticks);
    let counted = timer::ticks();
    let bios = timer::bios_elapsed_since(bios0);
    println!(
        "hw: {}: fast ticks in one second within {} to {}: {} (reached {}: {})",
        label,
        ticks - 1,
        ticks + 1,
        ((ticks - 1)..=(ticks + 1)).contains(&counted),
        ticks,
        done
    );
    println!(
        "hw: {}: the BIOS counter advanced 18 to 19 ticks while the PIT ran fast: {}",
        label,
        (18..=19).contains(&bios)
    );
}

fn fast_mode(before: (u16, u32)) {
    println!(
        "hw: fast: delay_us refuses while the PIT is not reprogrammed: {}",
        !timer::delay_us(10_000) && !timer::is_fast() && timer::clocks() == 0
    );
    println!(
        "hw: fast: a divisor the chip cannot count is refused and changes nothing: {}",
        timer::hook_rate(1).is_err() && timer::hook_rate(65_537).is_err() && !timer::is_fast()
    );

    timer::hook_rate(DIVISOR_60_HZ).expect("hook_rate");
    println!(
        "hw: fast: vector 8 moved to our handler: {}",
        dpmi::get_pm_vector(8).unwrap() != before
    );
    println!(
        "hw: fast: a second hook, fast or d0, is refused: {}",
        timer::hook_rate(DIVISOR_560_HZ).is_err() && timer::hook().is_err()
    );
    fast_second("60 Hz", 60);

    // The stopwatch: monotonic, and finer than a 16.7 ms tick.
    let mut last = timer::clocks();
    let mut backwards = false;
    let mut fine = false;
    let first = last;
    for _ in 0..2_000 {
        let now = timer::clocks();
        backwards |= now.wrapping_sub(last) > 0x8000_0000;
        fine |= now != last && now.wrapping_sub(first) < DIVISOR_60_HZ;
        last = now;
    }
    println!("hw: fast: the stopwatch never ran backwards: {}", !backwards);
    println!("hw: fast: the stopwatch moved by less than one tick: {}", fine);

    // 100 delays of 10,000 microseconds are one second: about 18 BIOS ticks (DOSBox's own timing is loose, hence the wide range).
    let bios0 = timer::bios_elapsed();
    let clocks0 = timer::clocks();
    let mut all = true;
    for _ in 0..100 {
        all &= timer::delay_us(10_000);
    }
    let bios = timer::bios_elapsed_since(bios0);
    let clocks = timer::clocks().wrapping_sub(clocks0);
    println!("hw: fast: delay_us returned true while reprogrammed: {}", all);
    println!(
        "hw: fast: 100 delays of 10,000 microseconds took 17 to 20 BIOS ticks: {}",
        (17..=20).contains(&bios)
    );
    println!(
        "hw: fast: and 1,193,182 to 1,205,000 stopwatch clocks: {}",
        (1_193_182..=1_205_000).contains(&clocks)
    );

    timer::unhook_rate();
    println!("hw: fast: vector 8 restored: {}", dpmi::get_pm_vector(8).unwrap() == before);
    let t = timer::fast_ticks();
    let mut quiet = true;
    for _ in 0..3_000_000 {
        quiet &= timer::fast_ticks() == t;
    }
    println!("hw: fast: no fast ticks counted after unhook: {}", quiet && !timer::is_fast());
    println!(
        "hw: fast: delay_us refuses again after unhook: {}",
        !timer::delay_us(10_000) && timer::clocks() == 0
    );
    timer::unhook_rate();
    bios_rate_line("after unhook");

    // The same again at 560 Hz, and the hook can be installed a second time.
    timer::hook_rate(DIVISOR_560_HZ).expect("hook_rate again");
    fast_second("560 Hz", 560);
    timer::unhook_rate();
    println!(
        "hw: fast: vector 8 restored again: {}",
        dpmi::get_pm_vector(8).unwrap() == before
    );
    bios_rate_line("after the second unhook");
}

/// The clean-up list full: `hook_rate` is refused with nothing changed, and the PIT is still at the BIOS rate.
fn fast_full_mode(before: (u16, u32)) {
    fn filler<const N: usize>() {
        core::hint::black_box(N);
    }
    let fillers: [fn(); 16] = [
        filler::<0>,
        filler::<1>,
        filler::<2>,
        filler::<3>,
        filler::<4>,
        filler::<5>,
        filler::<6>,
        filler::<7>,
        filler::<8>,
        filler::<9>,
        filler::<10>,
        filler::<11>,
        filler::<12>,
        filler::<13>,
        filler::<14>,
        filler::<15>,
    ];
    for f in fillers {
        assert!(rt::cleanup::register(f));
    }
    println!(
        "hw: fast-full: hook_rate refused with the clean-up list full: {}",
        timer::hook_rate(DIVISOR_60_HZ).is_err()
    );
    println!(
        "hw: fast-full: nothing changed: {}",
        !timer::is_fast() && dpmi::get_pm_vector(8).unwrap() == before && rt::cleanup::len() == rt::cleanup::SLOTS
    );
    bios_rate_line("fast-full");
}

/// Registered first, so it runs last: says whether exit put the vector back (the BIOS rate is then checked by `rate-check`, run after).
fn report_fast() {
    // SAFETY: single-threaded; BEFORE was set before this was registered.
    let before = unsafe { BEFORE };
    println!(
        "hw: after the clean-up list the timer vector is back and the fast timer is off: {}",
        dpmi::get_pm_vector(8).unwrap() == before && !timer::is_fast()
    );
}

/// Runs a mode of the fast timer. `None` if `mode` is not one of them.
pub fn fast_modes(mode: &str) -> Option<i32> {
    let before = dpmi::get_pm_vector(8).unwrap();
    match mode {
        "fast" => fast_mode(before),
        "fast-full" => fast_full_mode(before),
        "rate-check" => {
            println!(
                "hw: rate-check: the fast timer is not installed: {}",
                !timer::is_fast() && !timer::delay_us(1)
            );
            bios_rate_line("rate-check");
        }
        "fast-leave-hooked" | "fast-panic" => {
            // SAFETY: single-threaded, before the hook.
            unsafe { BEFORE = before };
            assert!(rt::cleanup::register(report_fast), "clean-up list full");
            timer::hook_rate(DIVISOR_60_HZ).expect("hook_rate");
            if mode == "fast-panic" {
                println!("hw: panicking with the PIT at 60 Hz");
                let v: alloc::vec::Vec<u32> = alloc::vec![1, 2, 3];
                let i = v.len() + core::hint::black_box(2);
                println!("unreachable {}", v[i]);
            }
            println!("hw: exiting with the PIT at 60 Hz");
        }
        _ => return None,
    }
    Some(0)
}
