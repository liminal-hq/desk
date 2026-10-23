// The core's side of the hand-off: pop the rings the handlers fill, decode, return events.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_hal::ring::Consumer;
use desk_hal::{InputSource, KeyEvent, MouseEvent};
use desk_kbd::Decoder;

/// The consumer ends of the keyboard and mouse rings and the decoder. Lives in the core's loop; never touched from an interrupt.
pub struct Ps2Input {
    keys: Consumer<'static, u8, 64>,
    mouse: Consumer<'static, MouseEvent, 32>,
    decoder: Decoder,
}

impl Ps2Input {
    pub fn new(keys: Consumer<'static, u8, 64>, mouse: Consumer<'static, MouseEvent, 32>) -> Self {
        Ps2Input {
            keys,
            mouse,
            decoder: Decoder::new(),
        }
    }

    /// Whether either ring holds something. A cheap test for a loop that wants to `hlt` when idle (check it with interrupts off).
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.mouse.is_empty()
    }

    /// Scancode bytes the keyboard ring dropped because the core fell behind.
    pub fn keys_dropped(&self) -> u32 {
        self.keys.dropped()
    }

    /// Mouse events the mouse ring dropped for the same reason.
    pub fn mouse_dropped(&self) -> u32 {
        self.mouse.dropped()
    }

    /// The decoder, for its modifier state.
    pub fn decoder(&self) -> &Decoder {
        &self.decoder
    }
}

impl InputSource for Ps2Input {
    fn poll_key(&mut self) -> Option<KeyEvent> {
        while let Some(byte) = self.keys.pop() {
            if let Some(event) = self.decoder.feed(byte) {
                return Some(event);
            }
        }
        None
    }

    fn poll_mouse(&mut self) -> Option<MouseEvent> {
        self.mouse.pop()
    }
}
