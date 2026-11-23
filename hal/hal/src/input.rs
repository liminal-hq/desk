// Input events and the trait the core drains them through.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// These are the types an input driver and the Desk core share (`docs/HAL.md`, "Input"; the spike's result is `docs/C-INPUT.md`).
// A key is named by a `KeyCode` that does not depend on a scancode set, so the same event stream comes out of a PS/2 keyboard
// under the native kernel and out of the BIOS-independent IRQ 1 handler under DOS.

use core::sync::atomic::{AtomicU32, Ordering};

/// A raw scancode byte as port 0x60 delivers it with the 8042's translation on (scancode set 1).
pub type Scancode = u8;

/// The state of the modifier keys and lock keys after an event, as a bit set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers(pub u8);

impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);
    /// Either Shift key is down.
    pub const SHIFT: Modifiers = Modifiers(1);
    /// Either Control key is down.
    pub const CTRL: Modifiers = Modifiers(2);
    /// Either Alt key is down (the right one is AltGr on some layouts; this layout does not distinguish).
    pub const ALT: Modifiers = Modifiers(4);
    /// Either Windows or Super key is down.
    pub const GUI: Modifiers = Modifiers(8);
    /// Caps Lock is on (a toggle, not a held key).
    pub const CAPS: Modifiers = Modifiers(16);
    /// Num Lock is on.
    pub const NUM: Modifiers = Modifiers(32);
    /// Scroll Lock is on.
    pub const SCROLL: Modifiers = Modifiers(64);

    /// Whether every bit of `other` is set.
    pub const fn contains(self, other: Modifiers) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 | other.0)
    }

    pub const fn without(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 & !other.0)
    }
}

/// A physical key on a US PC keyboard, independent of the scancode set. Letter and digit keys are named for the legend on a US
/// layout; the character they produce is `desk-kbd`'s business.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Escape,
    Backspace,
    Tab,
    Enter,
    Space,
    /// `-` and `_`.
    Minus,
    /// `=` and `+`.
    Equals,
    /// `[` and `{`.
    LeftBracket,
    /// `]` and `}`.
    RightBracket,
    /// `\` and `|`.
    Backslash,
    /// `;` and `:`.
    Semicolon,
    /// `'` and `"`.
    Apostrophe,
    /// `` ` `` and `~`.
    Grave,
    /// `,` and `<`.
    Comma,
    /// `.` and `>`.
    Period,
    /// `/` and `?`.
    Slash,
    /// The extra key between Left Shift and Z on 102-key layouts (`\` and `|` on a US layout).
    IntlBackslash,
    CapsLock,
    NumLock,
    ScrollLock,
    LeftShift,
    RightShift,
    LeftCtrl,
    RightCtrl,
    LeftAlt,
    RightAlt,
    LeftGui,
    RightGui,
    Menu,
    Up,
    Down,
    Left,
    Right,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    PrintScreen,
    /// Alt+Print Screen on an 84-key layout, scancode 0x54.
    SysRq,
    /// The Pause key. It has no release code, so only a `pressed` event is ever produced.
    Pause,
    /// Control+Pause.
    Break,
    Kp0,
    Kp1,
    Kp2,
    Kp3,
    Kp4,
    Kp5,
    Kp6,
    Kp7,
    Kp8,
    Kp9,
    KpPeriod,
    KpPlus,
    KpMinus,
    KpStar,
    KpSlash,
    KpEnter,
    /// A scancode this decoder has no key for; `extended` is set when it followed the `0xE0` prefix.
    Unknown {
        extended: bool,
        scancode: u8,
    },
}

/// A key going down or up. `modifiers` is the modifier state after the event: pressing Shift reports it with `SHIFT` set,
/// releasing it reports it clear. A key held down repeats as further `pressed` events (typematic).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub pressed: bool,
    pub modifiers: Modifiers,
}

/// A mouse report: the motion and the button state at the end of one packet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseEvent {
    /// Motion to the right is positive.
    pub dx: i16,
    /// Motion down the screen is positive (the device reports up as positive; the driver negates it).
    pub dy: i16,
    /// Bit 0 left, bit 1 right, bit 2 middle. The state of the buttons now, not changes.
    pub buttons: u8,
    /// Wheel motion, positive towards the user (scrolling down); zero on a mouse without a wheel.
    pub wheel: i8,
}

impl MouseEvent {
    pub const LEFT: u8 = 1;
    pub const RIGHT: u8 = 2;
    pub const MIDDLE: u8 = 4;
}

/// What the core's event loop drains: decoded input, queued by interrupt handlers.
pub trait InputSource {
    /// The next key event the interrupt side has queued, if any. Called by the core's event loop, never from an interrupt.
    fn poll_key(&mut self) -> Option<KeyEvent>;
    /// The next mouse report, if any.
    fn poll_mouse(&mut self) -> Option<MouseEvent>;
    /// Whether there is a mouse at all. The hosted platform has none when no DOS mouse driver is installed, and the core then runs
    /// keyboard-only (no cursor to draw, every action reachable by key: `docs/SPEC.md`, "Input"). Native platforms assume one.
    fn has_mouse(&self) -> bool {
        true
    }
}

/// What one tick of input looks like: see `InputLatch::frame`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    /// The bits that are down at the moment of the frame.
    pub held: u32,
    /// Every bit that went down since the previous frame, including one that went down and up again in between.
    pub pressed: u32,
    /// Every bit that went up since the previous frame.
    pub released: u32,
}

/// A press-edge latch over a 32-bit mask (a key, a button or a direction per bit), so a tap shorter than one tick still counts
/// as a press.
///
/// The producer (an interrupt handler or an event callback) calls `set` when bits go down and `release` when they go up. The
/// consumer calls `frame` once per tick and gets the bits held now, the bits pressed since its last call and the bits released
/// since then. A bit that goes down and up again between two frames shows in `pressed` and `released` with `held` clear, which
/// is the point of the latch. A `set` of a bit that is already down (a key repeat) is not an edge, and nor is a `release` of a
/// bit that is already up.
///
/// Concurrency contract: one producer context and one consumer context, on a single core. The producer's updates are plain
/// load-then-store sequences, which is sound only because the consumer cannot run in the middle of one: the producer is an
/// interrupt handler (or otherwise cannot be preempted by the consumer), and on a single core the consumer's own steps are
/// atomic `swap`s (`xchg`) that an interrupt can only fall between, never inside. Two producers, or a producer on another core,
/// would lose edges. The latch uses only load, store and `swap`, so it compiles to no compare-and-swap (`hal/clippy.toml`).
/// Because the consumer takes its three words in sequence, an edge that arrives while `frame` runs lands in this frame or the
/// next, never in neither.
///
/// Nothing uses this yet: it is host-tested only. The first user will be a game or the shell's keyboard handling (`docs/HAL.md`,
/// "Input").
#[derive(Debug, Default)]
pub struct InputLatch {
    held: AtomicU32,
    pressed: AtomicU32,
    released: AtomicU32,
}

impl InputLatch {
    pub const fn new() -> InputLatch {
        InputLatch {
            held: AtomicU32::new(0),
            pressed: AtomicU32::new(0),
            released: AtomicU32::new(0),
        }
    }

    /// The bits in `mask` went down. Producer side.
    pub fn set(&self, mask: u32) {
        let held = self.held.load(Ordering::Relaxed);
        let fresh = mask & !held;
        self.pressed
            .store(self.pressed.load(Ordering::Relaxed) | fresh, Ordering::Relaxed);
        self.held.store(held | mask, Ordering::Release);
    }

    /// The bits in `mask` went up. Producer side.
    pub fn release(&self, mask: u32) {
        let held = self.held.load(Ordering::Relaxed);
        let was_down = mask & held;
        self.released
            .store(self.released.load(Ordering::Relaxed) | was_down, Ordering::Relaxed);
        self.held.store(held & !mask, Ordering::Release);
    }

    /// Takes the edges latched since the last call, and the bits held now. Consumer side, once per tick.
    pub fn frame(&self) -> Frame {
        let pressed = self.pressed.swap(0, Ordering::AcqRel);
        let released = self.released.swap(0, Ordering::AcqRel);
        let held = self.held.load(Ordering::Acquire);
        Frame { held, pressed, released }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_sets_combine_and_subtract() {
        let m = Modifiers::SHIFT.union(Modifiers::CTRL);
        assert!(m.contains(Modifiers::SHIFT) && m.contains(Modifiers::CTRL) && !m.contains(Modifiers::ALT));
        assert!(m.contains(Modifiers::NONE));
        assert_eq!(m.without(Modifiers::SHIFT), Modifiers::CTRL);
        assert_eq!(Modifiers::default(), Modifiers::NONE);
    }

    #[test]
    fn a_tap_between_frames_is_pressed_and_released_with_nothing_held() {
        let l = InputLatch::new();
        l.set(1);
        l.release(1);
        assert_eq!(
            l.frame(),
            Frame {
                held: 0,
                pressed: 1,
                released: 1
            }
        );
    }

    #[test]
    fn a_held_press_is_pressed_once_then_held() {
        let l = InputLatch::new();
        l.set(4);
        assert_eq!(
            l.frame(),
            Frame {
                held: 4,
                pressed: 4,
                released: 0
            }
        );
        assert_eq!(
            l.frame(),
            Frame {
                held: 4,
                pressed: 0,
                released: 0
            }
        );
        l.release(4);
        assert_eq!(
            l.frame(),
            Frame {
                held: 0,
                pressed: 0,
                released: 4
            }
        );
    }

    #[test]
    fn a_bit_pressed_twice_is_one_edge() {
        let l = InputLatch::new();
        l.set(2);
        l.set(2);
        assert_eq!(
            l.frame(),
            Frame {
                held: 2,
                pressed: 2,
                released: 0
            }
        );
        assert_eq!(l.frame().pressed, 0);
    }

    #[test]
    fn independent_bits_do_not_disturb_each_other() {
        let l = InputLatch::new();
        l.set(0b01);
        let _ = l.frame();
        l.set(0b10);
        l.release(0b01);
        assert_eq!(
            l.frame(),
            Frame {
                held: 0b10,
                pressed: 0b10,
                released: 0b01
            }
        );
    }

    #[test]
    fn frame_clears_the_edges() {
        let l = InputLatch::new();
        l.set(8);
        l.release(8);
        assert_ne!(l.frame(), Frame::default());
        assert_eq!(l.frame(), Frame::default());
    }

    #[test]
    fn press_release_press_ends_held_with_pressed_set() {
        let l = InputLatch::new();
        l.set(1);
        l.release(1);
        l.set(1);
        assert_eq!(
            l.frame(),
            Frame {
                held: 1,
                pressed: 1,
                released: 1
            }
        );
    }

    #[test]
    fn a_release_of_a_bit_that_is_not_down_is_not_an_edge() {
        let l = InputLatch::new();
        l.release(0xF);
        assert_eq!(l.frame(), Frame::default());
    }

    #[test]
    fn all_thirty_two_bits_work() {
        let l = InputLatch::new();
        for bit in 0..32 {
            let m = 1u32 << bit;
            l.set(m);
            assert_eq!(
                l.frame(),
                Frame {
                    held: m,
                    pressed: m,
                    released: 0
                }
            );
            l.release(m);
            assert_eq!(
                l.frame(),
                Frame {
                    held: 0,
                    pressed: 0,
                    released: m
                }
            );
        }
        l.set(u32::MAX);
        assert_eq!(
            l.frame(),
            Frame {
                held: u32::MAX,
                pressed: u32::MAX,
                released: 0
            }
        );
        l.release(u32::MAX);
        assert_eq!(
            l.frame(),
            Frame {
                held: 0,
                pressed: 0,
                released: u32::MAX
            }
        );
    }
}
