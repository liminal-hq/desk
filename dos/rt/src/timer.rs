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
//
// The d0 hook above stays at the BIOS rate (about 18.2 Hz) and is what the shell uses. `hook_rate` below is the opt-in for a program
// that wants a faster tick: it reprograms the PIT, counts ticks in a Rust handler (the interrupt registry in `irq.rs`) and chains to the
// BIOS handler only when the BIOS clock is due (`desk_pit::BiosChain`), so DOS's clock keeps its time. Only one of the two is
// installed at a time.
use crate::irq::DosIrq;
use crate::{cleanup, dpmi};
use core::arch::global_asm;
use core::ptr::{addr_of, addr_of_mut, read_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use desk_hal::irq::{Action, Chain, DecidingIrqService, Irq, IrqHandler, IrqRegistry};
use desk_hal::x86::RawPorts;
use desk_hal::HalError;
use desk_pit::{bios_ticks_between, check_divisor, clocks_at, clocks_for_micros, BiosChain, Pit, BIOS_TICKS_PER_DAY};

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
        if HOOKED || FAST_ACTIVE.load(Ordering::Relaxed) {
            return Err(dpmi::Error(0));
        }
        let (_, _, _, _, master, _) = dpmi::version();
        let vector = if master == 0 { 8 } else { master };
        // Lock the whole image (code, data, bss, stack): the handler and every static it reaches are inside it.
        let start = dpmi::ptr_to_linear(addr_of!(__image_start));
        let end = dpmi::ptr_to_linear(addr_of!(__image_end));
        dpmi::lock(start, end - start)?;
        let old = dpmi::get_pm_vector(vector)?;
        // A second install starts from zero. The handler is not reachable yet, so a plain write is safe.
        RT_IRQ0_TICKS = 0;
        RT_IRQ0_DSEL = dpmi::data_selector() as u32;
        RT_IRQ0_OLD = [old.1, old.0 as u32];
        let cs: u16;
        core::arch::asm!("mov {0:x}, cs", out(reg) cs, options(nomem, nostack, preserves_flags));
        dpmi::set_pm_vector(vector, cs, rt_irq0 as *const () as usize as u32)?;
        VECTOR = vector;
        HOOKED = true;
        if !cleanup::register(unhook) {
            // The list is full: put the vector back rather than leave a hook that nothing would remove.
            unhook_now();
            return Err(dpmi::Error(CLEANUP_FULL));
        }
        Ok(old)
    }
}

/// The `dpmi::Error` code `hook` returns when the clean-up list is full (no DPMI function returns it).
pub const CLEANUP_FULL: u16 = 0xFFFF;

/// Puts the previous handler back. Safe to call twice, and from the clean-up list.
pub fn unhook() {
    unhook_now();
    cleanup::forget(unhook);
}

fn unhook_now() {
    // SAFETY: restores the saved far pointer; after this the handler is unreachable.
    unsafe {
        if !HOOKED {
            return;
        }
        let old = read_volatile(addr_of!(RT_IRQ0_OLD));
        let _ = dpmi::set_pm_vector(VECTOR, old[1] as u16, old[0]);
        HOOKED = false;
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

/// Ticks counted since the hook was installed: the fast timer's count while `hook_rate` is installed, the d0 hook's otherwise.
pub fn ticks() -> u32 {
    if is_fast() {
        return fast_ticks();
    }
    // SAFETY: a volatile read of a word the handler increments; a 32-bit aligned access is atomic.
    unsafe { read_volatile(addr_of_mut!(RT_IRQ0_TICKS)) }
}

/// The BIOS tick counter at 0040:006C, which the BIOS's own IRQ 0 handler (reached by chaining) increments.
pub fn bios_ticks() -> u32 {
    // SAFETY: conventional memory is mapped at linear 0; the counter is a word-aligned dword.
    unsafe { read_volatile(dpmi::linear_to_ptr::<u32>(0x46C)) }
}

// ---- The fast timer: a reprogrammed PIT, opt-in ----

/// Whether `hook_rate` is installed (and the PIT is not at the BIOS rate).
static FAST_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Interrupts counted since `hook_rate`, written only by the handler (plain load and store: no compare-and-swap).
static FAST_TICKS: AtomicU32 = AtomicU32::new(0);
/// The divisor the PIT was programmed with.
static FAST_DIVISOR: AtomicU32 = AtomicU32::new(0);
/// The last value `clocks` returned, so the stopwatch never runs backwards.
static FAST_LAST: AtomicU32 = AtomicU32::new(0);

/// The interrupt side: count, then say whether the BIOS clock is due. No division, no allocation, no PIT access.
struct FastTick {
    chain: BiosChain,
}

impl DecidingIrqService for FastTick {
    fn on_irq(&mut self) -> Action {
        let t = FAST_TICKS.load(Ordering::Relaxed);
        FAST_TICKS.store(t.wrapping_add(1), Ordering::Relaxed);
        if self.chain.tick() {
            Action::Chain
        } else {
            Action::Eoi
        }
    }
}

/// The handler's state: a `static` in the locked image, rebuilt by every `hook_rate` while the line is not registered.
static mut FAST: FastTick = FastTick {
    chain: BiosChain::new(desk_pit::MAX_DIVISOR),
};

fn pit() -> Pit<RawPorts> {
    // SAFETY: a DPMI client may use the PIT ports, as it may the PIC's and the VGA's (`RawPorts::new`'s contract).
    Pit::new(unsafe { RawPorts::new() })
}

/// Reprograms the PIT to `divisor` (2 to 65,536, the rule of `desk_pit::check_divisor`; 60 Hz is 19,886 and 560 Hz is 2,131) and
/// counts its interrupts with `ticks`, chaining to the BIOS handler only when the BIOS clock is due (about 18.2 times a second), so
/// the DOS clock and `bios_ticks` keep their time. The program asks for this: nothing else in the runtime changes the rate.
///
/// `InvalidArgument` (nothing changed) for a divisor the chip cannot count; `Busy` if either hook is already installed, the clean-up
/// list is full or the timer line is taken. The clean-up entry goes in first, so a full list refuses before anything changes.
/// `unhook_rate`, which exit and a panic also run, puts the vector back and only then the BIOS rate, so a handler cannot run after
/// the rate is restored. In the few microseconds between the vector and the rate being set the handler counts a tick at the BIOS
/// rate, which can at worst delay one BIOS clock tick.
pub fn hook_rate(divisor: u32) -> Result<(), HalError> {
    check_divisor(divisor)?;
    if is_hooked() || is_fast() {
        return Err(HalError::Busy);
    }
    if !cleanup::register(unhook_rate) {
        return Err(HalError::Busy);
    }
    FAST_TICKS.store(0, Ordering::Relaxed);
    FAST_LAST.store(0, Ordering::Relaxed);
    FAST_DIVISOR.store(divisor, Ordering::Relaxed);
    // SAFETY: the line is not registered, so the handler cannot run and nothing else holds a reference to `FAST`; the one reference
    // made here goes to the registry, which `unhook_rate` unregisters before the next `hook_rate` makes another.
    let handler = unsafe {
        *addr_of_mut!(FAST) = FastTick {
            chain: BiosChain::new(divisor),
        };
        IrqHandler::from_deciding(&mut *addr_of_mut!(FAST))
    };
    if let Err(e) = DosIrq.register(Irq::TIMER, handler, Chain::None) {
        cleanup::forget(unhook_rate);
        return Err(e);
    }
    FAST_ACTIVE.store(true, Ordering::Relaxed);
    // The divisor passed `check_divisor`, so this cannot fail.
    let _ = pit().program_divisor(divisor);
    Ok(())
}

/// Stops the fast timer: the vector goes back first and then the BIOS rate. Safe to call twice, with nothing installed, and from
/// the clean-up list. If the host refuses to put the vector back the rate is left as it is (the handler is still installed and
/// still counts) and the list entry stays, so it is tried again at exit.
pub fn unhook_rate() {
    if unhook_rate_now() {
        cleanup::forget(unhook_rate);
    }
}

fn unhook_rate_now() -> bool {
    if !is_fast() {
        return true;
    }
    // The registry's own clean-up entry may have put the vector back already (it is above ours on the list, so it runs first).
    if DosIrq::is_installed(Irq::TIMER) && DosIrq.unregister(Irq::TIMER).is_err() {
        return false;
    }
    pit().restore_bios_rate();
    FAST_ACTIVE.store(false, Ordering::Relaxed);
    true
}

/// Whether `hook_rate` is installed: the PIT is running at the rate it was given, not the BIOS's.
pub fn is_fast() -> bool {
    FAST_ACTIVE.load(Ordering::Relaxed)
}

/// Interrupts counted by `hook_rate`'s handler since it was installed; still readable after `unhook_rate`, until the next `hook_rate`.
pub fn fast_ticks() -> u32 {
    FAST_TICKS.load(Ordering::Relaxed)
}

/// A monotonic stopwatch in PIT input clocks (1,193,182 a second, 838 ns each) since `hook_rate`, finer than a tick: the tick count
/// times the divisor plus the part of the current period the PIT's latched counter shows (`desk_pit::clocks_at`). 0 when the fast
/// timer is not installed.
///
/// It wraps at 2^32 clocks (about 3,599 seconds): take differences with `wrapping_sub`, and call it at least every 30 minutes (the
/// guard that stops it running backwards compares as a signed difference). A reading taken in the instant the counter has reloaded
/// but the interrupt has not yet been counted would be one period too low, so `clocks` returns the previous reading then. It reads
/// the latch of channel 0 with interrupts on, which is safe because neither handler touches the PIT, and retries if a tick lands
/// between reading the tick count and the latch.
pub fn clocks() -> u32 {
    if !is_fast() {
        return 0;
    }
    let divisor = FAST_DIVISOR.load(Ordering::Relaxed);
    let chip = pit();
    let (ticks, count) = loop {
        let before = fast_ticks();
        let count = chip.read_counter();
        if fast_ticks() == before {
            break (before, count);
        }
    };
    let now = clocks_at(ticks, divisor, count);
    let last = FAST_LAST.load(Ordering::Relaxed);
    if (now.wrapping_sub(last) as i32) < 0 {
        return last;
    }
    FAST_LAST.store(now, Ordering::Relaxed);
    now
}

/// Waits `us` microseconds by the stopwatch and returns true; returns false at once, without waiting, unless `hook_rate` is installed
/// (`is_fast`). The stopwatch needs the PIT in the rate-generator mode `hook_rate` sets: in the BIOS's own mode the latched count runs
/// twice per period and says nothing about sub-tick time, so a program that has not asked for the fast timer gets no delay it could
/// not trust. Rounded to the nearest 838 ns clock, and at least that long; at most about 3,599 s.
pub fn delay_us(us: u32) -> bool {
    if !is_fast() {
        return false;
    }
    let want = clocks_for_micros(us);
    let start = clocks();
    while clocks().wrapping_sub(start) < want {
        if !is_fast() {
            return false;
        }
        core::hint::spin_loop();
    }
    true
}

/// The latch of channel 0 as the chip gives it (`Pit::read_counter`), for a program that measures the PIT itself. It is time only in
/// the mode `hook_rate` sets (see `clocks`); in the BIOS's mode it counts down by two and covers each period twice.
pub fn read_counter() -> u16 {
    pit().read_counter()
}

/// BIOS ticks since midnight: the counter at 0040:006C, kept below `BIOS_TICKS_PER_DAY` (1,573,040). The BIOS clock counts it at
/// 18.2065 Hz whether or not `hook_rate` is installed.
pub fn bios_elapsed() -> u32 {
    bios_ticks() % BIOS_TICKS_PER_DAY
}

/// BIOS ticks from an earlier `bios_elapsed` reading to now, across midnight if the counter went back to 0 in between (exact for a
/// span under a day).
pub fn bios_elapsed_since(start: u32) -> u32 {
    bios_ticks_between(start, bios_ticks())
}
