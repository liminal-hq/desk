// Keyboard and PS/2 mouse: set-up, the IRQ 1 and IRQ 12 hand-off, and the event loop that drains and prints what arrives.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The handlers (`desk_i8042::KbdIrq` and `MouseIrq`) only read a byte and push it into a ring; this file hands them their ends
// of the rings and keeps the other ends, which the loop in `run` drains with interrupts on. See `docs/C-INPUT.md`.

use crate::hal::{IRQS, PORTS};
use crate::{pit, println, sync};
use alloc::boxed::Box;
use core::arch::asm;
use desk_hal::irq::{Chain, Irq, IrqHandler, IrqRegistry};
use desk_hal::ring::SpscRing;
use desk_hal::{InputSource, KeyCode, KeyEvent, Modifiers, MouseEvent};
use desk_i8042::{IrqStats, KbdIrq, MouseIrq, Ps2Input, I8042};

static KEY_BYTES: SpscRing<u8, 64> = SpscRing::new();
static MOUSE_EVENTS: SpscRing<MouseEvent, 32> = SpscRing::new();
static STATS: IrqStats = IrqStats::new();

/// How the set-up goes wrong on purpose, for the negative check.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    None,
    /// IRQ 1 stays masked: no key can arrive.
    KeyboardMasked,
}

/// Brings up the controller and both devices, registers the handlers and unmasks the lines. Prints one line per step. A missing
/// device is reported and skipped, not fatal: a machine may have only a keyboard.
///
/// `None` when the controller itself fails its set-up: a machine without a working 8042 still boots, without input.
pub fn init(fault: Fault) -> Option<Ps2Input> {
    let ctl = I8042::new(PORTS);
    let info = match ctl.init() {
        Ok(info) => info,
        Err(e) => {
            println!("input: controller unavailable: {}", e);
            return None;
        }
    };
    println!(
        "input: 8042 {} channel, translation {}",
        if info.dual_channel { "dual" } else { "single" },
        if info.translation { "on" } else { "off" }
    );
    // A keyboard that could not be put in scancode set 1 would feed the decoder bytes it cannot read, so it counts as unavailable.
    let kbd = ctl.init_keyboard(info).map(|k| k.set1.then_some(k));
    match kbd {
        Ok(Some(k)) => println!(
            "input: keyboard ready, scancode set 1 {}",
            if k.translated { "translated by the controller" } else { "selected on the keyboard" }
        ),
        Ok(None) => println!("input: keyboard unavailable: scancode set 1 could not be selected"),
        Err(e) => println!("input: keyboard unavailable: {}", e),
    }
    let kbd_ok = matches!(kbd, Ok(Some(_)));
    let mouse = if info.dual_channel { ctl.init_mouse(true) } else { Err(desk_i8042::Error::NoDevice) };
    match mouse {
        Ok(m) => println!("input: mouse ready, id {} ({}-byte packets)", m.id, m.packet_len()),
        Err(e) => println!("input: mouse unavailable: {}", e),
    }

    let (ktx, krx) = KEY_BYTES.split().expect("the key ring is split once");
    let (mtx, mrx) = MOUSE_EVENTS.split().expect("the mouse ring is split once");
    let wheel = mouse.is_ok_and(|m| m.id == 3);
    let kbd_irq = Box::leak(Box::new(KbdIrq::new(PORTS, ktx, &STATS)));
    let mouse_irq = Box::leak(Box::new(MouseIrq::new(PORTS, mtx, &STATS, wheel)));
    // Registering a handler unmasks its line; a device that is not there gets none, and the negative test masks IRQ 1 again.
    if kbd_ok {
        IRQS.register(Irq::KEYBOARD, IrqHandler::from_static(kbd_irq), Chain::None).expect("IRQ 1 is free");
        if fault == Fault::KeyboardMasked {
            crate::pic::mask(Irq::KEYBOARD);
        }
    }
    if mouse.is_ok() {
        IRQS.register(Irq::MOUSE, IrqHandler::from_static(mouse_irq), Chain::None).expect("IRQ 12 is free");
    }
    // A byte that arrived since the set-up would sit in the output buffer, and a controller may raise no interrupt for a byte
    // that was already there when the interrupts were enabled. Drop it so the next byte raises the edge.
    ctl.flush();
    ctl.set_interrupts(kbd_ok, mouse.is_ok()).expect("enabling the controller's interrupts");
    println!("input: IRQ 1 and IRQ 12 handlers registered, controller interrupts on");
    Some(Ps2Input::new(krx, mrx))
}

fn modifier_names(m: Modifiers) -> alloc::string::String {
    let mut out = alloc::string::String::new();
    for (bit, name) in [
        (Modifiers::SHIFT, "shift"),
        (Modifiers::CTRL, "ctrl"),
        (Modifiers::ALT, "alt"),
        (Modifiers::GUI, "gui"),
        (Modifiers::CAPS, "caps"),
        (Modifiers::NUM, "num"),
        (Modifiers::SCROLL, "scroll"),
    ] {
        if m.contains(bit) {
            if !out.is_empty() {
                out.push('+');
            }
            out.push_str(name);
        }
    }
    if out.is_empty() {
        out.push('-');
    }
    out
}

pub fn print_key(e: KeyEvent) {
    let ch = if e.pressed { desk_kbd::char_for(e.code, e.modifiers) } else { None };
    match ch {
        Some(c) => println!("key: {:?} down mods={} char={:?}", e.code, modifier_names(e.modifiers), c),
        None => println!("key: {:?} {} mods={}", e.code, if e.pressed { "down" } else { "up" }, modifier_names(e.modifiers)),
    }
}

fn print_mouse(e: MouseEvent) {
    println!("mouse: dx={} dy={} buttons={} wheel={}", e.dx, e.dy, e.buttons, e.wheel);
}

/// Drains and prints events until Escape is released, or panics after `timeout_ticks` without it. Idles with `hlt`, checking the
/// rings with interrupts off so an interrupt cannot slip in between the check and the halt.
pub fn run(input: &mut Ps2Input, timeout_ticks: u32) {
    let start = pit::ticks();
    let (mut keys, mut mice) = (0u32, 0u32);
    loop {
        while let Some(m) = input.poll_mouse() {
            print_mouse(m);
            mice += 1;
        }
        while let Some(k) = input.poll_key() {
            print_key(k);
            keys += 1;
            if k.code == KeyCode::Escape && !k.pressed {
                println!("input: escape released after {} key events and {} mouse events", keys, mice);
                println!(
                    "input: dropped keys {}, dropped mouse events {}, mouse resyncs {}",
                    input.keys_dropped(),
                    input.mouse_dropped(),
                    IrqStats::get(&STATS.mouse_resyncs)
                );
                return;
            }
        }
        if pit::ticks().wrapping_sub(start) > timeout_ticks {
            panic!("input: no Escape within {} ticks ({} key events, {} mouse events)", timeout_ticks, keys, mice);
        }
        idle(input);
    }
}

/// One idle step: the work is done, so sleep until an interrupt brings an event or a tick. The rings are checked with interrupts
/// off so an interrupt cannot slip in between the check and the `hlt`.
pub fn idle(input: &Ps2Input) {
    sync::without_interrupts(|| {
        if input.is_empty() {
            // `sti` takes effect after the next instruction, so an interrupt that arrives now wakes the `hlt` instead of
            // being missed. without_interrupts restores IF afterwards.
            // SAFETY: waits for the next interrupt; the timer guarantees one.
            unsafe { asm!("sti", "hlt", "cli") };
        }
    });
}
