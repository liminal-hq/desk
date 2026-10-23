// The core's side of DOS input: the scancode ring and the mouse driver, as one `InputSource`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_hal::ring::Consumer;
use desk_hal::{Bios, InputSource, KeyEvent, Modifiers, MouseEvent};
use desk_kbd::Decoder;

use crate::Mouse;

/// Turns the driver's counters into `MouseEvent`s.
///
/// The PS/2 driver produces one event per three-byte packet, with the device's own counts as `dx` and `dy`. This produces one event
/// per call to `poll`, with the mickeys the driver counted since the previous call: the same unit (a mickey is the driver's name
/// for a device count; a driver at its default sensitivity passes the PS/2 counts through one for one), the same signs (right and
/// down positive), the same button bits. When a poll sees several packets' worth of motion they arrive as one event with the sum, which the
/// core, integrating deltas, cannot tell apart from several events.
///
/// What it cannot see is a button pressed and released between two polls; the core polls once per frame, which is a few
/// milliseconds, and the driver's press counters (functions 0005h and 0006h) are the fix if that matters.
pub struct MouseInput<B: Bios> {
    mouse: Mouse<B>,
    buttons: u8,
}

impl<B: Bios> MouseInput<B> {
    pub fn new(mouse: Mouse<B>) -> MouseInput<B> {
        MouseInput { mouse, buttons: 0 }
    }

    /// The driver, for the calls the platform makes at set-up and clean-up (ranges, reset).
    pub fn mouse(&self) -> &Mouse<B> {
        &self.mouse
    }

    /// The next report, if the mouse moved, a button changed or the wheel turned since the last call. A driver that stops answering
    /// reads as a mouse that is not moving.
    pub fn poll(&mut self) -> Option<MouseEvent> {
        let (dx, dy) = self.mouse.motion().ok()?;
        let state = self.mouse.position().ok()?;
        if dx == 0 && dy == 0 && state.wheel == 0 && state.buttons == self.buttons {
            return None;
        }
        self.buttons = state.buttons;
        Some(MouseEvent {
            dx,
            dy,
            buttons: state.buttons,
            wheel: state.wheel,
        })
    }
}

/// The hosted platform's `InputSource`.
///
/// The keyboard half is the same as the native kernel's: the IRQ 1 handler pushes raw scancode bytes into a ring (`desk_i8042::KbdIrq`
/// is the handler on both platforms) and the `desk_kbd::Decoder` turns them into `KeyEvent`s here, on the core's side. The mouse half
/// is `MouseInput`, or nothing when there is no driver.
pub struct DosInput<B: Bios> {
    keys: Consumer<'static, u8, 64>,
    decoder: Decoder,
    mouse: Option<MouseInput<B>>,
    dropped_seen: u32,
    recoveries: u32,
}

impl<B: Bios> DosInput<B> {
    /// `locks` is the lock state the decoder starts in (from the BIOS's flags, which the LEDs show). `mouse` is `None` for a machine
    /// without a driver.
    pub fn new(keys: Consumer<'static, u8, 64>, locks: Modifiers, mouse: Option<Mouse<B>>) -> DosInput<B> {
        DosInput {
            keys,
            decoder: Decoder::with_locks(locks),
            mouse: mouse.map(MouseInput::new),
            dropped_seen: 0,
            recoveries: 0,
        }
    }

    /// The mouse, for the platform's set-up and clean-up calls.
    pub fn mouse(&self) -> Option<&Mouse<B>> {
        self.mouse.as_ref().map(MouseInput::mouse)
    }

    /// Scancode bytes the ring dropped because the core fell behind.
    pub fn keys_dropped(&self) -> u32 {
        self.keys.dropped()
    }

    /// How many times the decoder was reset after a drop.
    pub fn recoveries(&self) -> u32 {
        self.recoveries
    }

    /// The decoder's modifier and lock state, for the platform to hand back to the BIOS on exit.
    pub fn modifiers(&self) -> Modifiers {
        self.decoder.modifiers()
    }
}

impl<B: Bios> InputSource for DosInput<B> {
    fn poll_key(&mut self) -> Option<KeyEvent> {
        loop {
            while let Some(byte) = self.keys.pop() {
                if let Some(event) = self.decoder.feed(byte) {
                    return Some(event);
                }
            }
            // A full ring drops the newest bytes, so the stream the decoder saw has a hole after what it has now consumed, and a
            // key's break code may have been in it: left alone the key would stay down for good, and a held Shift or Ctrl would
            // change every later key. Once the ring is empty the hole is behind us; forget the held keys (the locks stay).
            let dropped = self.keys.dropped();
            if dropped == self.dropped_seen {
                return None;
            }
            self.dropped_seen = dropped;
            self.recoveries += 1;
            self.decoder.reset();
        }
    }

    fn poll_mouse(&mut self) -> Option<MouseEvent> {
        self.mouse.as_mut()?.poll()
    }

    fn has_mouse(&self) -> bool {
        self.mouse.is_some()
    }
}
