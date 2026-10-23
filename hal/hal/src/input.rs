// Input events and the trait the core drains them through.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// These are the types an input driver and the Desk core share (`docs/HAL.md`, "Input"; the spike's result is `docs/C-INPUT.md`).
// A key is named by a `KeyCode` that does not depend on a scancode set, so the same event stream comes out of a PS/2 keyboard
// under the native kernel and out of the BIOS-independent IRQ 1 handler under DOS.

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
}
