// The text form of an event, one line each: what the DOS demo prints and what the host tests compare it with.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The same two functions format the events on the host (from the mock devices) and under DOS (from the real IRQ 1 path and INT 33h),
// so one expected file can serve both.

use core::fmt::{self, Write};
use desk_hal::{KeyEvent, Modifiers, MouseEvent};

/// `none`, or the set bits joined with `+` in the order SHIFT, CTRL, ALT, GUI, CAPS, NUM, SCROLL.
pub fn write_modifiers(w: &mut dyn Write, m: Modifiers) -> fmt::Result {
    const NAMES: [(Modifiers, &str); 7] = [
        (Modifiers::SHIFT, "SHIFT"),
        (Modifiers::CTRL, "CTRL"),
        (Modifiers::ALT, "ALT"),
        (Modifiers::GUI, "GUI"),
        (Modifiers::CAPS, "CAPS"),
        (Modifiers::NUM, "NUM"),
        (Modifiers::SCROLL, "SCROLL"),
    ];
    let mut first = true;
    for (bit, name) in NAMES {
        if m.contains(bit) {
            if !first {
                w.write_char('+')?;
            }
            w.write_str(name)?;
            first = false;
        }
    }
    if first {
        w.write_str("none")?;
    }
    Ok(())
}

/// `key A down mods=none`
pub fn write_key(w: &mut dyn Write, e: &KeyEvent) -> fmt::Result {
    write!(w, "key {:?} {} mods=", e.code, if e.pressed { "down" } else { "up" })?;
    write_modifiers(w, e.modifiers)
}

/// `mouse dx=3 dy=-2 buttons=LR wheel=0` (`-` for no buttons).
pub fn write_mouse(w: &mut dyn Write, e: &MouseEvent) -> fmt::Result {
    write!(w, "mouse dx={} dy={} buttons=", e.dx, e.dy)?;
    if e.buttons & 7 == 0 {
        w.write_char('-')?;
    }
    for (bit, name) in [(MouseEvent::LEFT, 'L'), (MouseEvent::MIDDLE, 'M'), (MouseEvent::RIGHT, 'R')] {
        if e.buttons & bit != 0 {
            w.write_char(name)?;
        }
    }
    write!(w, " wheel={}", e.wheel)
}
