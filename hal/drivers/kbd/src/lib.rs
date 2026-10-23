// The keyboard decoder: scancode set 1 bytes in, key events out, and a US key map from keys to characters.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Pure: no ports, no interrupts, no allocation. The bytes come from the 8042 (`desk-i8042`), which translates the keyboard's own
// scancode set 2 into set 1 when its configuration byte says so (`docs/C-INPUT.md`). In set 1 a make code is the key's number and
// a break code is the make code with bit 7 set; the keys added by the 101-key layout send `0xE0` first, and Pause sends the six
// bytes `E1 1D 45 E1 9D C5` on press and nothing on release.
#![cfg_attr(not(any(test, feature = "std")), no_std)]

use desk_hal::input::{KeyCode, KeyEvent, Modifiers, Scancode};

const EXTENDED: u8 = 0xE0;
const PAUSE_PREFIX: u8 = 0xE1;
/// What follows `E1` in the Pause sequence.
const PAUSE_TAIL: [u8; 5] = [0x1D, 0x45, 0xE1, 0x9D, 0xC5];

/// Where the decoder is in a multi-byte sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Idle,
    /// After `E0`.
    Extended,
    /// After `E1`: how many of `PAUSE_TAIL` have matched.
    Pause(u8),
}

/// Turns scancode set 1 bytes into key events, and keeps the modifier and lock state. Feed it every byte in order.
#[derive(Clone, Copy, Debug)]
pub struct Decoder {
    state: State,
    left_shift: bool,
    right_shift: bool,
    left_ctrl: bool,
    right_ctrl: bool,
    left_alt: bool,
    right_alt: bool,
    left_gui: bool,
    right_gui: bool,
    /// Lock keys toggle on press only, and a held key repeats its make code, so the keys' own state is tracked too.
    caps_down: bool,
    num_down: bool,
    scroll_down: bool,
    locks: Modifiers,
}

impl Default for Decoder {
    fn default() -> Self {
        Decoder::new()
    }
}

impl Decoder {
    pub const fn new() -> Decoder {
        Decoder {
            state: State::Idle,
            left_shift: false,
            right_shift: false,
            left_ctrl: false,
            right_ctrl: false,
            left_alt: false,
            right_alt: false,
            left_gui: false,
            right_gui: false,
            caps_down: false,
            num_down: false,
            scroll_down: false,
            locks: Modifiers::NONE,
        }
    }

    /// A decoder that starts with the lock keys in the state `locks` names (only `CAPS`, `NUM` and `SCROLL` count; other bits are
    /// ignored) and nothing held. A platform whose firmware already owns the lock state and the keyboard's LEDs seeds the decoder
    /// from it, so the first Caps Lock press toggles from what the LED shows: under DOS that is the BIOS's flag byte at 0040:0017.
    pub const fn with_locks(locks: Modifiers) -> Decoder {
        Decoder {
            locks: Modifiers(locks.0 & (Modifiers::CAPS.0 | Modifiers::NUM.0 | Modifiers::SCROLL.0)),
            ..Decoder::new()
        }
    }

    /// The modifier and lock state now.
    pub fn modifiers(&self) -> Modifiers {
        let mut m = self.locks;
        if self.left_shift || self.right_shift {
            m = m.union(Modifiers::SHIFT);
        }
        if self.left_ctrl || self.right_ctrl {
            m = m.union(Modifiers::CTRL);
        }
        if self.left_alt || self.right_alt {
            m = m.union(Modifiers::ALT);
        }
        if self.left_gui || self.right_gui {
            m = m.union(Modifiers::GUI);
        }
        m
    }

    /// Forgets a half-received sequence and every held key, as after a keyboard reset. The lock state stays: the LEDs would not
    /// have changed.
    pub fn reset(&mut self) {
        *self = Decoder {
            locks: self.locks,
            ..Decoder::new()
        };
    }

    /// Takes one byte. `None` while a sequence is incomplete, for the bytes that carry no key (the fake shifts around Print
    /// Screen and Insert, ACK, resend and overrun codes), and for the second half of a sequence already reported.
    pub fn feed(&mut self, byte: Scancode) -> Option<KeyEvent> {
        match self.state {
            State::Idle => match byte {
                EXTENDED => {
                    self.state = State::Extended;
                    None
                }
                PAUSE_PREFIX => {
                    self.state = State::Pause(0);
                    None
                }
                // 0x00 and 0xFF are the keyboard's buffer overrun or error code in set 1; 0xFA and 0xFE are ACK and resend.
                0x00 | 0xFA | 0xFE | 0xFF => None,
                _ => self.key(false, byte),
            },
            State::Extended => {
                self.state = State::Idle;
                match byte {
                    // The error marker the driver queues for a byte the link flagged bad: the sequence is lost.
                    0xFF => None,
                    // Another prefix straight after a prefix: start over with the new one.
                    EXTENDED => {
                        self.state = State::Extended;
                        None
                    }
                    PAUSE_PREFIX => {
                        self.state = State::Pause(0);
                        None
                    }
                    // The fake shifts the keyboard wraps around Print Screen, Insert and the cursor block when Shift or Num Lock
                    // is down (E0 2A, E0 AA, E0 36, E0 B6): not keys, and not to disturb the real Shift state.
                    0x2A | 0xAA | 0x36 | 0xB6 => None,
                    _ => self.key(true, byte),
                }
            }
            State::Pause(done) => {
                if byte == 0xFF {
                    self.state = State::Idle;
                    return None;
                }
                if byte == PAUSE_TAIL[usize::from(done)] {
                    if usize::from(done) + 1 == PAUSE_TAIL.len() {
                        self.state = State::Idle;
                        return Some(KeyEvent {
                            code: KeyCode::Pause,
                            pressed: true,
                            modifiers: self.modifiers(),
                        });
                    }
                    self.state = State::Pause(done + 1);
                    None
                } else {
                    // Not the Pause sequence: drop it and read this byte afresh.
                    self.state = State::Idle;
                    self.feed(byte)
                }
            }
        }
    }

    fn key(&mut self, extended: bool, byte: u8) -> Option<KeyEvent> {
        let pressed = byte & 0x80 == 0;
        let scancode = byte & 0x7F;
        let code = if extended {
            extended_key(scancode)
        } else {
            plain_key(scancode)
        };
        self.apply(code, pressed);
        Some(KeyEvent {
            code,
            pressed,
            modifiers: self.modifiers(),
        })
    }

    fn apply(&mut self, code: KeyCode, pressed: bool) {
        match code {
            KeyCode::LeftShift => self.left_shift = pressed,
            KeyCode::RightShift => self.right_shift = pressed,
            KeyCode::LeftCtrl => self.left_ctrl = pressed,
            KeyCode::RightCtrl => self.right_ctrl = pressed,
            KeyCode::LeftAlt => self.left_alt = pressed,
            KeyCode::RightAlt => self.right_alt = pressed,
            KeyCode::LeftGui => self.left_gui = pressed,
            KeyCode::RightGui => self.right_gui = pressed,
            KeyCode::CapsLock => Self::toggle(&mut self.caps_down, &mut self.locks, Modifiers::CAPS, pressed),
            KeyCode::NumLock => Self::toggle(&mut self.num_down, &mut self.locks, Modifiers::NUM, pressed),
            KeyCode::ScrollLock => Self::toggle(&mut self.scroll_down, &mut self.locks, Modifiers::SCROLL, pressed),
            _ => {}
        }
    }

    fn toggle(down: &mut bool, locks: &mut Modifiers, bit: Modifiers, pressed: bool) {
        if pressed && !*down {
            *locks = if locks.contains(bit) {
                locks.without(bit)
            } else {
                locks.union(bit)
            };
        }
        *down = pressed;
    }
}

/// The key for a make code without the `E0` prefix (bit 7 already cleared).
pub fn plain_key(scancode: u8) -> KeyCode {
    use KeyCode::*;
    match scancode {
        0x01 => Escape,
        0x02 => Num1,
        0x03 => Num2,
        0x04 => Num3,
        0x05 => Num4,
        0x06 => Num5,
        0x07 => Num6,
        0x08 => Num7,
        0x09 => Num8,
        0x0A => Num9,
        0x0B => Num0,
        0x0C => Minus,
        0x0D => Equals,
        0x0E => Backspace,
        0x0F => Tab,
        0x10 => Q,
        0x11 => W,
        0x12 => E,
        0x13 => R,
        0x14 => T,
        0x15 => Y,
        0x16 => U,
        0x17 => I,
        0x18 => O,
        0x19 => P,
        0x1A => LeftBracket,
        0x1B => RightBracket,
        0x1C => Enter,
        0x1D => LeftCtrl,
        0x1E => A,
        0x1F => S,
        0x20 => D,
        0x21 => F,
        0x22 => G,
        0x23 => H,
        0x24 => J,
        0x25 => K,
        0x26 => L,
        0x27 => Semicolon,
        0x28 => Apostrophe,
        0x29 => Grave,
        0x2A => LeftShift,
        0x2B => Backslash,
        0x2C => Z,
        0x2D => X,
        0x2E => C,
        0x2F => V,
        0x30 => B,
        0x31 => N,
        0x32 => M,
        0x33 => Comma,
        0x34 => Period,
        0x35 => Slash,
        0x36 => RightShift,
        0x37 => KpStar,
        0x38 => LeftAlt,
        0x39 => Space,
        0x3A => CapsLock,
        0x3B => F1,
        0x3C => F2,
        0x3D => F3,
        0x3E => F4,
        0x3F => F5,
        0x40 => F6,
        0x41 => F7,
        0x42 => F8,
        0x43 => F9,
        0x44 => F10,
        0x45 => NumLock,
        0x46 => ScrollLock,
        0x47 => Kp7,
        0x48 => Kp8,
        0x49 => Kp9,
        0x4A => KpMinus,
        0x4B => Kp4,
        0x4C => Kp5,
        0x4D => Kp6,
        0x4E => KpPlus,
        0x4F => Kp1,
        0x50 => Kp2,
        0x51 => Kp3,
        0x52 => Kp0,
        0x53 => KpPeriod,
        0x54 => SysRq,
        0x56 => IntlBackslash,
        0x57 => F11,
        0x58 => F12,
        _ => Unknown {
            extended: false,
            scancode,
        },
    }
}

/// The key for a make code after the `E0` prefix (bit 7 already cleared).
pub fn extended_key(scancode: u8) -> KeyCode {
    use KeyCode::*;
    match scancode {
        0x1C => KpEnter,
        0x1D => RightCtrl,
        0x35 => KpSlash,
        0x37 => PrintScreen,
        0x38 => RightAlt,
        0x46 => Break,
        0x47 => Home,
        0x48 => Up,
        0x49 => PageUp,
        0x4B => Left,
        0x4D => Right,
        0x4F => End,
        0x50 => Down,
        0x51 => PageDown,
        0x52 => Insert,
        0x53 => Delete,
        0x5B => LeftGui,
        0x5C => RightGui,
        0x5D => Menu,
        _ => Unknown {
            extended: true,
            scancode,
        },
    }
}

/// The character a key produces on a US layout with `modifiers` held, or `None` for keys that produce none (arrows, function
/// keys, modifiers). Shift flips letters and selects the upper legend of the other keys; Caps Lock flips letters only; the keypad
/// types digits while Num Lock is on and Shift is not held; Control turns a letter into its control code (`Ctrl+A` is `0x01`).
/// Enter, Tab, Backspace, Escape and Space give `\n`, `\t`, `\u{8}`, `\u{1b}` and a space. Alt and Gui do not change the
/// character; the caller decides what a chord means.
pub fn char_for(code: KeyCode, modifiers: Modifiers) -> Option<char> {
    use KeyCode::*;
    let shift = modifiers.contains(Modifiers::SHIFT);
    let upper = shift != modifiers.contains(Modifiers::CAPS);
    let pick = |lower: char, shifted: char| Some(if shift { shifted } else { lower });
    let letter = |c: char| {
        if modifiers.contains(Modifiers::CTRL) {
            Some(char::from(c as u8 - b'a' + 1))
        } else if upper {
            Some(c.to_ascii_uppercase())
        } else {
            Some(c)
        }
    };
    let keypad = |digit: char| {
        if modifiers.contains(Modifiers::NUM) && !shift {
            Some(digit)
        } else {
            None
        }
    };
    match code {
        A => letter('a'),
        B => letter('b'),
        C => letter('c'),
        D => letter('d'),
        E => letter('e'),
        F => letter('f'),
        G => letter('g'),
        H => letter('h'),
        I => letter('i'),
        J => letter('j'),
        K => letter('k'),
        L => letter('l'),
        M => letter('m'),
        N => letter('n'),
        O => letter('o'),
        P => letter('p'),
        Q => letter('q'),
        R => letter('r'),
        S => letter('s'),
        T => letter('t'),
        U => letter('u'),
        V => letter('v'),
        W => letter('w'),
        X => letter('x'),
        Y => letter('y'),
        Z => letter('z'),
        Num1 => pick('1', '!'),
        Num2 => pick('2', '@'),
        Num3 => pick('3', '#'),
        Num4 => pick('4', '$'),
        Num5 => pick('5', '%'),
        Num6 => pick('6', '^'),
        Num7 => pick('7', '&'),
        Num8 => pick('8', '*'),
        Num9 => pick('9', '('),
        Num0 => pick('0', ')'),
        Minus => pick('-', '_'),
        Equals => pick('=', '+'),
        LeftBracket => pick('[', '{'),
        RightBracket => pick(']', '}'),
        Backslash | IntlBackslash => pick('\\', '|'),
        Semicolon => pick(';', ':'),
        Apostrophe => pick('\'', '"'),
        Grave => pick('`', '~'),
        Comma => pick(',', '<'),
        Period => pick('.', '>'),
        Slash => pick('/', '?'),
        Space => Some(' '),
        Enter | KpEnter => Some('\n'),
        Tab => Some('\t'),
        Backspace => Some('\u{8}'),
        Escape => Some('\u{1b}'),
        KpSlash => Some('/'),
        KpStar => Some('*'),
        KpMinus => Some('-'),
        KpPlus => Some('+'),
        Kp0 => keypad('0'),
        Kp1 => keypad('1'),
        Kp2 => keypad('2'),
        Kp3 => keypad('3'),
        Kp4 => keypad('4'),
        Kp5 => keypad('5'),
        Kp6 => keypad('6'),
        Kp7 => keypad('7'),
        Kp8 => keypad('8'),
        Kp9 => keypad('9'),
        KpPeriod => keypad('.'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds all of `bytes` and collects the events.
    fn feed_all(d: &mut Decoder, bytes: &[u8]) -> Vec<KeyEvent> {
        bytes.iter().filter_map(|&b| d.feed(b)).collect()
    }

    fn ev(code: KeyCode, pressed: bool, modifiers: Modifiers) -> KeyEvent {
        KeyEvent {
            code,
            pressed,
            modifiers,
        }
    }

    #[test]
    fn a_decoder_can_start_with_the_locks_the_firmware_shows() {
        let mut d = Decoder::with_locks(Modifiers::CAPS.union(Modifiers::SHIFT).union(Modifiers::NUM));
        assert_eq!(
            d.modifiers(),
            Modifiers::CAPS.union(Modifiers::NUM),
            "held keys are not locks and are dropped"
        );
        // The first Caps Lock press toggles from on to off.
        let e = feed_all(&mut d, &[0x3A, 0xBA]);
        assert_eq!(e[0].modifiers, Modifiers::NUM);
        assert_eq!(e[1].modifiers, Modifiers::NUM);
    }

    #[test]
    fn make_and_break_codes_are_one_key_going_down_then_up() {
        let mut d = Decoder::new();
        assert_eq!(
            feed_all(&mut d, &[0x1E, 0x9E]),
            [ev(KeyCode::A, true, Modifiers::NONE), ev(KeyCode::A, false, Modifiers::NONE)]
        );
    }

    #[test]
    fn every_plain_code_maps_to_a_key_and_its_break_to_the_same_key() {
        let mut d = Decoder::new();
        for sc in 0x01..=0x58u8 {
            if matches!(sc, 0x55 | 0x59..=0x7F) {
                continue;
            }
            let make = d.feed(sc).unwrap();
            let brk = d.feed(sc | 0x80).unwrap();
            assert!(make.pressed && !brk.pressed, "{sc:#04x}");
            assert_eq!(make.code, brk.code);
            assert_eq!(make.code, plain_key(sc));
            assert!(!matches!(make.code, KeyCode::Unknown { .. }), "{sc:#04x} is a key");
        }
        // 0x55 is the one hole in the range, and the codes above 0x58 are not on a 101-key board.
        assert_eq!(
            Decoder::new().feed(0x55),
            Some(ev(
                KeyCode::Unknown {
                    extended: false,
                    scancode: 0x55
                },
                true,
                Modifiers::NONE
            ))
        );
    }

    #[test]
    fn the_plain_table_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for sc in 0x01..=0x58u8 {
            let k = plain_key(sc);
            if !matches!(k, KeyCode::Unknown { .. }) {
                assert!(seen.insert(k), "{k:?} appears twice");
            }
        }
        assert_eq!(seen.len(), 0x58 - 1); // 0x01..=0x58 less the hole at 0x55
    }

    #[test]
    fn the_extended_prefix_selects_the_other_key() {
        let mut d = Decoder::new();
        // Keypad 4 versus Left, keypad Enter versus Enter, and the right-hand Ctrl and Alt.
        assert_eq!(d.feed(0x4B).unwrap().code, KeyCode::Kp4);
        assert_eq!(d.feed(0xE0), None);
        assert_eq!(d.feed(0x4B).unwrap().code, KeyCode::Left);
        assert_eq!(feed_all(&mut d, &[0xE0, 0x1C])[0].code, KeyCode::KpEnter);
        assert_eq!(
            feed_all(&mut d, &[0xE0, 0x9C]),
            [ev(KeyCode::KpEnter, false, Modifiers::NONE)]
        );
        let e = feed_all(&mut d, &[0xE0, 0x1D, 0xE0, 0x38]);
        assert_eq!(e[0], ev(KeyCode::RightCtrl, true, Modifiers::CTRL));
        assert_eq!(e[1], ev(KeyCode::RightAlt, true, Modifiers::CTRL.union(Modifiers::ALT)));
    }

    #[test]
    fn the_whole_extended_block_decodes() {
        let cases = [
            (0x47, KeyCode::Home),
            (0x48, KeyCode::Up),
            (0x49, KeyCode::PageUp),
            (0x4B, KeyCode::Left),
            (0x4D, KeyCode::Right),
            (0x4F, KeyCode::End),
            (0x50, KeyCode::Down),
            (0x51, KeyCode::PageDown),
            (0x52, KeyCode::Insert),
            (0x53, KeyCode::Delete),
            (0x35, KeyCode::KpSlash),
            (0x37, KeyCode::PrintScreen),
            (0x46, KeyCode::Break),
            (0x5B, KeyCode::LeftGui),
            (0x5C, KeyCode::RightGui),
            (0x5D, KeyCode::Menu),
        ];
        for (sc, key) in cases {
            let mut d = Decoder::new();
            let down = feed_all(&mut d, &[0xE0, sc]);
            let up = feed_all(&mut d, &[0xE0, sc | 0x80]);
            assert_eq!((down[0].code, down[0].pressed), (key, true), "{sc:#04x}");
            assert_eq!((up[0].code, up[0].pressed), (key, false), "{sc:#04x}");
        }
        let mut d = Decoder::new();
        assert_eq!(
            feed_all(&mut d, &[0xE0, 0x7F]),
            [ev(
                KeyCode::Unknown {
                    extended: true,
                    scancode: 0x7F
                },
                true,
                Modifiers::NONE
            )]
        );
    }

    #[test]
    fn print_screen_swallows_its_fake_shifts() {
        // Make: E0 2A E0 37. Break: E0 B7 E0 AA. Only the real key is reported, and the Shift state is untouched.
        let mut d = Decoder::new();
        let down = feed_all(&mut d, &[0xE0, 0x2A, 0xE0, 0x37]);
        let up = feed_all(&mut d, &[0xE0, 0xB7, 0xE0, 0xAA]);
        assert_eq!(down, [ev(KeyCode::PrintScreen, true, Modifiers::NONE)]);
        assert_eq!(up, [ev(KeyCode::PrintScreen, false, Modifiers::NONE)]);
        assert_eq!(d.modifiers(), Modifiers::NONE);
    }

    #[test]
    fn fake_shifts_do_not_release_a_real_shift() {
        // Shift held while Insert is typed: the keyboard sends E0 AA before and E0 2A after, which are not the real Shift key.
        let mut d = Decoder::new();
        d.feed(0x2A);
        let e = feed_all(&mut d, &[0xE0, 0xAA, 0xE0, 0x52, 0xE0, 0xD2, 0xE0, 0x2A]);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0], ev(KeyCode::Insert, true, Modifiers::SHIFT));
        assert!(d.modifiers().contains(Modifiers::SHIFT));
    }

    #[test]
    fn the_pause_sequence_is_one_press_and_no_release() {
        let mut d = Decoder::new();
        let e = feed_all(&mut d, &[0xE1, 0x1D, 0x45, 0xE1, 0x9D, 0xC5]);
        assert_eq!(e, [ev(KeyCode::Pause, true, Modifiers::NONE)]);
        // The decoder is idle again: the next key is read normally, and Num Lock was not touched by the 0x45 inside.
        assert_eq!(d.feed(0x1E).unwrap().code, KeyCode::A);
        assert_eq!(d.modifiers(), Modifiers::NONE);
    }

    #[test]
    fn pause_split_across_calls_still_decodes() {
        let mut d = Decoder::new();
        for b in [0xE1, 0x1D, 0x45, 0xE1, 0x9D] {
            assert_eq!(d.feed(b), None);
        }
        assert_eq!(d.feed(0xC5).unwrap().code, KeyCode::Pause);
    }

    #[test]
    fn a_broken_pause_sequence_is_dropped_and_the_next_byte_read_afresh() {
        let mut d = Decoder::new();
        assert_eq!(d.feed(0xE1), None);
        assert_eq!(d.feed(0x1D), None);
        // 0x1E is not 0x45: the partial sequence goes, and 0x1E is the key A.
        assert_eq!(d.feed(0x1E), Some(ev(KeyCode::A, true, Modifiers::NONE)));
        // A prefix inside a broken sequence starts a new one.
        assert_eq!(d.feed(0xE1), None);
        assert_eq!(d.feed(0xE0), None);
        assert_eq!(d.feed(0x48).unwrap().code, KeyCode::Up);
    }

    #[test]
    fn an_error_marker_inside_a_sequence_drops_the_sequence() {
        let mut d = Decoder::new();
        // E0, then a lost byte: the next key is read on its own, not as an extended key.
        assert_eq!(
            feed_all(&mut d, &[0xE0, 0xFF, 0x48]),
            [ev(KeyCode::Kp8, true, Modifiers::NONE)]
        );
        assert_eq!(
            feed_all(&mut d, &[0xE1, 0x1D, 0xFF, 0x1E]),
            [ev(KeyCode::A, true, Modifiers::NONE)]
        );
    }

    #[test]
    fn control_pause_is_break() {
        let mut d = Decoder::new();
        let e = feed_all(&mut d, &[0xE0, 0x46, 0xE0, 0xC6]);
        assert_eq!((e[0].code, e[0].pressed, e[1].pressed), (KeyCode::Break, true, false));
    }

    #[test]
    fn protocol_bytes_in_the_stream_are_not_keys() {
        let mut d = Decoder::new();
        assert_eq!(feed_all(&mut d, &[0x00, 0xFA, 0xFE, 0xFF]), []);
        // A prefix followed by a prefix keeps the second.
        assert_eq!(feed_all(&mut d, &[0xE0, 0xE0, 0x48])[0].code, KeyCode::Up);
    }

    #[test]
    fn the_modifiers_follow_the_keys_and_are_reported_after_the_event() {
        let mut d = Decoder::new();
        let e = feed_all(&mut d, &[0x2A, 0x1E, 0x9E, 0xAA, 0x1E]);
        assert_eq!(e[0], ev(KeyCode::LeftShift, true, Modifiers::SHIFT));
        assert_eq!(e[1], ev(KeyCode::A, true, Modifiers::SHIFT));
        assert_eq!(e[2], ev(KeyCode::A, false, Modifiers::SHIFT));
        assert_eq!(e[3], ev(KeyCode::LeftShift, false, Modifiers::NONE));
        assert_eq!(e[4], ev(KeyCode::A, true, Modifiers::NONE));
    }

    #[test]
    fn two_shift_keys_hold_shift_until_both_are_up() {
        let mut d = Decoder::new();
        feed_all(&mut d, &[0x2A, 0x36, 0xAA]);
        assert!(d.modifiers().contains(Modifiers::SHIFT));
        feed_all(&mut d, &[0xB6]);
        assert_eq!(d.modifiers(), Modifiers::NONE);
    }

    #[test]
    fn ctrl_alt_delete_reports_all_three_with_the_chord_state() {
        let mut d = Decoder::new();
        let e = feed_all(&mut d, &[0x1D, 0x38, 0xE0, 0x53, 0xE0, 0xD3, 0xB8, 0x9D]);
        let all = Modifiers::CTRL.union(Modifiers::ALT);
        assert_eq!(e[2], ev(KeyCode::Delete, true, all));
        assert_eq!(e[3], ev(KeyCode::Delete, false, all));
        assert_eq!(e[5].modifiers, Modifiers::NONE);
    }

    #[test]
    fn lock_keys_toggle_on_press_and_ignore_repeats() {
        let mut d = Decoder::new();
        // Caps Lock down, repeated twice while held, up: on once. Down and up again: off.
        feed_all(&mut d, &[0x3A, 0x3A, 0x3A, 0xBA]);
        assert!(d.modifiers().contains(Modifiers::CAPS));
        feed_all(&mut d, &[0x3A, 0xBA]);
        assert!(!d.modifiers().contains(Modifiers::CAPS));
        feed_all(&mut d, &[0x45, 0xC5, 0x46, 0xC6]);
        assert_eq!(d.modifiers(), Modifiers::NUM.union(Modifiers::SCROLL));
    }

    #[test]
    fn a_reset_forgets_held_keys_but_not_the_locks() {
        let mut d = Decoder::new();
        feed_all(&mut d, &[0x3A, 0xBA, 0x1D, 0xE0]);
        d.reset();
        assert_eq!(d.modifiers(), Modifiers::CAPS);
        assert_eq!(d.feed(0x48).unwrap().code, KeyCode::Kp8, "the dangling prefix is gone");
    }

    #[test]
    fn letters_digits_and_punctuation_have_both_legends() {
        let none = Modifiers::NONE;
        let shift = Modifiers::SHIFT;
        assert_eq!(char_for(KeyCode::A, none), Some('a'));
        assert_eq!(char_for(KeyCode::A, shift), Some('A'));
        assert_eq!(char_for(KeyCode::Num1, none), Some('1'));
        assert_eq!(char_for(KeyCode::Num1, shift), Some('!'));
        assert_eq!(char_for(KeyCode::Num0, shift), Some(')'));
        assert_eq!(char_for(KeyCode::Semicolon, shift), Some(':'));
        assert_eq!(char_for(KeyCode::Grave, shift), Some('~'));
        assert_eq!(char_for(KeyCode::Slash, shift), Some('?'));
        assert_eq!(char_for(KeyCode::Space, none), Some(' '));
        assert_eq!(char_for(KeyCode::Enter, none), Some('\n'));
        assert_eq!(char_for(KeyCode::Up, none), None);
        assert_eq!(char_for(KeyCode::F1, none), None);
        assert_eq!(char_for(KeyCode::LeftShift, shift), None);
    }

    #[test]
    fn caps_lock_flips_letters_only_and_shift_flips_it_back() {
        let caps = Modifiers::CAPS;
        assert_eq!(char_for(KeyCode::Q, caps), Some('Q'));
        assert_eq!(char_for(KeyCode::Q, caps.union(Modifiers::SHIFT)), Some('q'));
        assert_eq!(char_for(KeyCode::Num2, caps), Some('2'));
    }

    #[test]
    fn control_makes_control_codes() {
        assert_eq!(char_for(KeyCode::A, Modifiers::CTRL), Some('\u{1}'));
        assert_eq!(char_for(KeyCode::Z, Modifiers::CTRL.union(Modifiers::SHIFT)), Some('\u{1a}'));
    }

    #[test]
    fn the_keypad_types_digits_with_num_lock_and_not_with_shift() {
        assert_eq!(char_for(KeyCode::Kp5, Modifiers::NUM), Some('5'));
        assert_eq!(char_for(KeyCode::Kp5, Modifiers::NONE), None);
        assert_eq!(char_for(KeyCode::Kp5, Modifiers::NUM.union(Modifiers::SHIFT)), None);
        assert_eq!(char_for(KeyCode::KpPlus, Modifiers::NONE), Some('+'));
    }

    #[test]
    fn every_printable_key_yields_a_distinct_character_pair() {
        let mut d = Decoder::new();
        let mut plain = std::collections::HashSet::new();
        for sc in 0x02..=0x35u8 {
            if matches!(sc, 0x0E | 0x0F | 0x1C | 0x1D | 0x2A) {
                continue;
            }
            let code = d.feed(sc).unwrap().code;
            let lower = char_for(code, Modifiers::NONE).unwrap();
            assert!(plain.insert(lower), "{lower:?} twice");
            assert!(char_for(code, Modifiers::SHIFT).is_some());
        }
    }
}
