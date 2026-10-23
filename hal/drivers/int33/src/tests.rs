// Host tests: the INT 33h wrapper against `Int33Model`, and the whole DOS input path (mock 8042, IRQ 1 service, ring, shared decoder,
// mouse driver) against the transcript the DOS program is also checked against.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cell::RefCell;
use std::rc::Rc;

use desk_hal::irq::{Chain, InterruptController, Irq, IrqHandler, IrqRegistry};
use desk_hal::ring::SpscRing;
use desk_hal::{InputSource, KeyEvent, Modifiers, MouseEvent, PortIo};
use desk_hal_mock::ps2::I8042Model;
use desk_hal_mock::{Int33Kind, Int33Model, MockIrq, MockPortBus};
use desk_i8042::irq::{IrqStats, KbdIrq};

use super::*;
use crate::transcript::{write_key, write_mouse};

// ---- The wrapper ----

#[test]
fn an_empty_vector_and_an_iret_vector_both_mean_no_driver() {
    assert_eq!(
        Mouse::detect(Int33Model::new(Int33Kind::NoDriver)).err(),
        Some(Error::NoDriver)
    );
    assert_eq!(
        Mouse::detect(Int33Model::new(Int33Kind::IretOnly)).err(),
        Some(Error::NoDriver)
    );
}

#[test]
fn a_driver_that_answers_zero_to_reset_is_no_driver() {
    struct Dead;
    impl Bios for Dead {
        fn int(&self, _: u8, regs: &mut BiosRegs, _: Option<&mut [u8]>) -> Result<(), HalError> {
            regs.eax = 0;
            Ok(())
        }
        fn read_low(&self, segment: u16, offset: u16, out: &mut [u8]) -> Result<(), HalError> {
            // A vector that points somewhere (0000:00CC holds 1000:0000) at a byte that is not an IRET.
            let at = (u32::from(segment) << 4) + u32::from(offset);
            for (i, o) in out.iter_mut().enumerate() {
                *o = match at + i as u32 {
                    0xCE => 0x00,
                    0xCF => 0x10,
                    _ => 0x90,
                };
            }
            Ok(())
        }
    }
    assert_eq!(Mouse::detect(Dead).err(), Some(Error::NoDriver));
}

#[test]
fn detect_resets_the_driver_and_leaves_the_cursor_hidden() {
    let m = Mouse::detect(Int33Model::wheel_mouse()).unwrap();
    assert_eq!((m.buttons(), m.has_wheel()), (3, true));
    let model = &m.bios;
    assert_eq!(model.calls(), [func::RESET, func::WHEEL_API]);
    assert_eq!(model.hide_counter(), -1, "Desk draws its own cursor");
    assert!(!model.calls().contains(&func::SHOW));
}

#[test]
fn two_buttons_are_reported_either_way_the_driver_says_it() {
    let m = Mouse::detect(Int33Model::two_button()).unwrap();
    assert_eq!((m.buttons(), m.has_wheel()), (2, false));
    let m = Mouse::detect(Int33Model::two_button().with_ms_convention()).unwrap();
    assert_eq!(m.buttons(), 2, "BX = FFFFh is the Microsoft driver's way of saying two");
}

#[test]
fn range_and_position_follow_the_video_mode() {
    let m = Mouse::detect(Int33Model::two_button()).unwrap();
    // 640x480 for VBE; mode 13h is 640 virtual columns over 200 rows.
    m.set_range((0, 639), (0, 479)).unwrap();
    m.set_position(320, 240).unwrap();
    assert_eq!(m.position().unwrap().x, 320);
    m.set_position(700, -5).unwrap();
    let p = m.position().unwrap();
    assert_eq!((p.x, p.y), (639, 0), "the driver clamps to the range");
    m.set_range((0, 639), (0, 199)).unwrap();
    m.set_position(10, 400).unwrap();
    assert_eq!(m.position().unwrap().y, 199);
    m.set_range((50, 10), (0, 9)).unwrap();
    m.set_position(0, 0).unwrap();
    assert_eq!(m.position().unwrap().x, 10, "a reversed pair is swapped, as the drivers do");
}

#[test]
fn the_extended_queries_answer() {
    let m = Mouse::detect(Int33Model::two_button()).unwrap();
    assert_eq!(m.storage_size(), Ok(0x0100));
    assert_eq!(
        m.driver_type(),
        Ok(Some(DriverType {
            version: (8, 6),
            kind: 4,
            irq: 0
        }))
    );
    m.show().unwrap();
    m.hide().unwrap();
}

#[test]
fn motion_is_signed_and_a_read_clears_it() {
    let m = Mouse::detect(Int33Model::two_button()).unwrap();
    m.bios.move_by(7, -9);
    assert_eq!(m.motion(), Ok((7, -9)));
    assert_eq!(m.motion(), Ok((0, 0)));
}

// ---- The mouse half of the input source ----

fn mouse_input(model: Int33Model) -> MouseInput<Int33Model> {
    MouseInput::new(Mouse::detect(model).unwrap())
}

fn mouse_lines(input: &mut impl InputSource) -> Vec<String> {
    std::iter::from_fn(|| input.poll_mouse())
        .map(|e| {
            let mut s = String::new();
            write_mouse(&mut s, &e).unwrap();
            s
        })
        .collect()
}

#[test]
fn motion_buttons_and_the_wheel_become_mouse_events() {
    let mut mi = mouse_input(Int33Model::wheel_mouse());
    assert_eq!(mi.poll(), None, "nothing moved");
    mi.mouse().bios.move_by(5, -3);
    assert_eq!(
        mi.poll(),
        Some(MouseEvent {
            dx: 5,
            dy: -3,
            buttons: 0,
            wheel: 0
        })
    );
    assert_eq!(mi.poll(), None);
    mi.mouse().bios.press(MouseEvent::LEFT);
    assert_eq!(mi.poll().map(|e| (e.dx, e.dy, e.buttons)), Some((0, 0, 1)));
    assert_eq!(mi.poll(), None, "a held button is a state, not a stream of events");
    mi.mouse().bios.move_by(2, 2);
    assert_eq!(mi.poll().map(|e| (e.dx, e.dy, e.buttons)), Some((2, 2, 1)));
    mi.mouse().bios.release(MouseEvent::LEFT);
    mi.mouse().bios.scroll(1);
    assert_eq!(mi.poll().map(|e| (e.buttons, e.wheel)), Some((0, 1)));
    assert_eq!(mi.poll(), None, "the wheel counter is cleared by the read");
    mi.mouse().bios.move_by(1, 1);
    mi.mouse().bios.move_by(2, 3);
    assert_eq!(
        mi.poll().map(|e| (e.dx, e.dy)),
        Some((3, 4)),
        "motion between polls is summed"
    );
}

#[test]
fn a_two_button_driver_without_the_wheel_api_never_reports_a_wheel() {
    let mut mi = mouse_input(Int33Model::two_button());
    mi.mouse().bios.scroll(3);
    assert_eq!(mi.poll(), None);
}

#[test]
fn a_driver_that_stops_answering_reads_as_a_still_mouse() {
    struct Gone;
    impl Bios for Gone {
        fn int(&self, _: u8, _: &mut BiosRegs, _: Option<&mut [u8]>) -> Result<(), HalError> {
            Err(HalError::Platform(0x8021))
        }
        fn read_low(&self, _: u16, _: u16, _: &mut [u8]) -> Result<(), HalError> {
            Ok(())
        }
    }
    let mut mi = MouseInput::new(Mouse {
        bios: Gone,
        buttons: 3,
        wheel: false,
    });
    assert_eq!(mi.poll(), None);
}

#[test]
fn motion_survives_a_failed_button_read() {
    use core::cell::Cell;
    // Function 000Bh answers and clears its counters; function 0003h then fails. The motion must still be reported.
    struct Flaky {
        inner: Int33Model,
        fail_position: Cell<bool>,
    }
    impl Bios for Flaky {
        fn int(&self, vector: u8, regs: &mut BiosRegs, buffer: Option<&mut [u8]>) -> Result<(), HalError> {
            if regs.eax as u16 == func::POSITION && self.fail_position.get() {
                return Err(HalError::Platform(0x8021));
            }
            self.inner.int(vector, regs, buffer)
        }
        fn read_low(&self, segment: u16, offset: u16, out: &mut [u8]) -> Result<(), HalError> {
            self.inner.read_low(segment, offset, out)
        }
    }
    let mut mi = MouseInput::new(
        Mouse::detect(Flaky {
            inner: Int33Model::two_button(),
            fail_position: Cell::new(false),
        })
        .unwrap(),
    );
    mi.mouse().bios.inner.move_by(4, 5);
    mi.mouse().bios.fail_position.set(true);
    assert_eq!(mi.poll().map(|e| (e.dx, e.dy, e.buttons)), Some((4, 5, 0)));
    mi.mouse().bios.fail_position.set(false);
    assert_eq!(mi.poll(), None);
}

// ---- Clicks shorter than one poll: the press and release counters ----

fn edges(mi: &mut MouseInput<Int33Model>) -> Vec<(i16, i16, u8)> {
    std::iter::from_fn(|| mi.poll()).map(|e| (e.dx, e.dy, e.buttons)).collect()
}

#[test]
fn the_wrapper_reads_and_clears_the_press_and_release_counters() {
    let m = Mouse::detect(Int33Model::wheel_mouse()).unwrap();
    m.bios.move_by(10, 4);
    m.bios.click(MouseEvent::LEFT);
    m.bios.click(MouseEvent::LEFT);
    m.bios.click(MouseEvent::RIGHT);
    let p = m.presses(0).unwrap();
    assert_eq!((p.count, p.x, p.y), (2, 330, 104), "the position at the last press");
    assert_eq!(m.presses(0).unwrap().count, 0, "a read clears the count");
    assert_eq!(m.releases(0).unwrap().count, 2);
    assert_eq!(m.presses(1).unwrap().count, 1, "per button");
    assert_eq!(m.releases(2).unwrap().count, 0);
    let calls = m.bios.calls();
    assert!(calls.contains(&func::PRESSES) && calls.contains(&func::RELEASES));
    // A button the driver does not have counts nothing.
    let two = Mouse::detect(Int33Model::two_button()).unwrap();
    two.bios.press(MouseEvent::MIDDLE);
    assert_eq!(two.presses(2).unwrap().count, 0);
}

#[test]
fn a_click_inside_one_poll_is_a_press_then_a_release() {
    let mut mi = mouse_input(Int33Model::wheel_mouse());
    mi.mouse().bios.click(MouseEvent::LEFT);
    assert_eq!(edges(&mut mi), [(0, 0, 1), (0, 0, 0)]);
    assert_eq!(mi.poll(), None, "and nothing is left over for the next poll");
}

#[test]
fn a_release_and_a_press_inside_one_poll_while_held_is_a_release_then_a_press() {
    let mut mi = mouse_input(Int33Model::wheel_mouse());
    mi.mouse().bios.press(MouseEvent::LEFT);
    assert_eq!(edges(&mut mi), [(0, 0, 1)]);
    mi.mouse().bios.click(MouseEvent::LEFT);
    assert_eq!(edges(&mut mi), [(0, 0, 0), (0, 0, 1)]);
    mi.mouse().bios.release(MouseEvent::LEFT);
    assert_eq!(edges(&mut mi), [(0, 0, 0)]);
}

#[test]
fn a_click_with_motion_puts_the_motion_and_the_wheel_on_the_first_event() {
    let mut mi = mouse_input(Int33Model::wheel_mouse());
    mi.mouse().bios.move_by(7, -2);
    mi.mouse().bios.scroll(1);
    mi.mouse().bios.click(MouseEvent::LEFT);
    let all: Vec<MouseEvent> = std::iter::from_fn(|| mi.poll()).collect();
    assert_eq!(
        all,
        [
            MouseEvent {
                dx: 7,
                dy: -2,
                buttons: 1,
                wheel: 1
            },
            MouseEvent {
                dx: 0,
                dy: 0,
                buttons: 0,
                wheel: 0
            },
        ]
    );
}

#[test]
fn two_clicks_in_one_poll_are_reported_as_one() {
    let mut mi = mouse_input(Int33Model::wheel_mouse());
    mi.mouse().bios.click(MouseEvent::LEFT);
    mi.mouse().bios.click(MouseEvent::LEFT);
    assert_eq!(edges(&mut mi), [(0, 0, 1), (0, 0, 0)]);
    assert_eq!(mi.poll(), None);
}

#[test]
fn the_other_buttons_keep_the_state_only_reading() {
    let mut mi = mouse_input(Int33Model::wheel_mouse());
    mi.mouse().bios.click(MouseEvent::RIGHT);
    mi.mouse().bios.click(MouseEvent::MIDDLE);
    assert_eq!(mi.poll(), None, "a right or middle click between polls is not seen");
    // A click of the left with the right held keeps the right's bit on both events.
    mi.mouse().bios.press(MouseEvent::RIGHT);
    assert_eq!(edges(&mut mi), [(0, 0, 2)]);
    mi.mouse().bios.click(MouseEvent::LEFT);
    assert_eq!(edges(&mut mi), [(0, 0, 3), (0, 0, 2)]);
}

#[test]
fn an_edge_the_state_shows_is_the_states_not_the_counters() {
    let mut mi = mouse_input(Int33Model::wheel_mouse());
    mi.mouse().bios.press(MouseEvent::LEFT);
    assert_eq!(
        edges(&mut mi),
        [(0, 0, 1)],
        "one press event, though one press was counted too"
    );
    mi.mouse().bios.release(MouseEvent::LEFT);
    assert_eq!(edges(&mut mi), [(0, 0, 0)]);
    // Up, a whole click, then down again inside one poll: the state's press; the click inside is folded away.
    mi.mouse().bios.click(MouseEvent::LEFT);
    mi.mouse().bios.press(MouseEvent::LEFT);
    assert_eq!(edges(&mut mi), [(0, 0, 1)]);
}

#[test]
fn a_driver_that_answers_the_counters_with_zeros_changes_nothing() {
    let mut mi = mouse_input(Int33Model::wheel_mouse().without_counters());
    mi.mouse().bios.click(MouseEvent::LEFT);
    assert_eq!(mi.poll(), None, "no counters, so the quick click is lost, as before");
    mi.mouse().bios.press(MouseEvent::LEFT);
    mi.mouse().bios.move_by(3, 3);
    assert_eq!(edges(&mut mi), [(3, 3, 1)]);
    mi.mouse().bios.release(MouseEvent::LEFT);
    assert_eq!(edges(&mut mi), [(0, 0, 0)]);
}

#[test]
fn clicks_from_before_the_input_existed_are_not_reported() {
    let model = Int33Model::wheel_mouse();
    let mouse = Mouse::detect(model).unwrap();
    mouse.bios.click(MouseEvent::LEFT);
    let mut mi = MouseInput::new(mouse);
    assert_eq!(mi.poll(), None);
}

#[test]
fn a_driver_that_fails_the_counter_calls_still_reports_motion_and_state() {
    struct NoCounters(Int33Model);
    impl Bios for NoCounters {
        fn int(&self, vector: u8, regs: &mut BiosRegs, buffer: Option<&mut [u8]>) -> Result<(), HalError> {
            if matches!(regs.eax as u16, 0x0005 | 0x0006) {
                return Err(HalError::Timeout);
            }
            self.0.int(vector, regs, buffer)
        }
        fn read_low(&self, segment: u16, offset: u16, out: &mut [u8]) -> Result<(), HalError> {
            self.0.read_low(segment, offset, out)
        }
    }
    let mut mi = MouseInput::new(Mouse::detect(NoCounters(Int33Model::wheel_mouse())).unwrap());
    mi.mouse().bios.0.move_by(2, 1);
    mi.mouse().bios.0.press(MouseEvent::LEFT);
    assert_eq!(mi.poll().map(|e| (e.dx, e.dy, e.buttons)), Some((2, 1, 1)));
}

// ---- The whole path ----

#[derive(Clone, Copy)]
struct Bus(&'static MockPortBus);

// SAFETY: the host tests are single-threaded; the mock bus is only touched from the test's own thread.
unsafe impl Send for Bus {}

impl PortIo for Bus {
    fn inb(&self, port: u16) -> u8 {
        self.0.inb(port)
    }
    fn outb(&self, port: u16, value: u8) {
        self.0.outb(port, value)
    }
    fn inw(&self, port: u16) -> u16 {
        self.0.inw(port)
    }
    fn outw(&self, port: u16, value: u16) {
        self.0.outw(port, value)
    }
    fn inl(&self, port: u16) -> u32 {
        self.0.inl(port)
    }
    fn outl(&self, port: u16, value: u32) {
        self.0.outl(port, value)
    }
}

/// A DOS machine: the 8042 with a keyboard behind it, IRQ 1 registered the way the DOS platform does (`Chain::None`: the platform
/// sends the end-of-interrupt), the shared decoder on the core's side and an INT 33h driver.
struct Machine {
    model: Rc<RefCell<I8042Model>>,
    irq: MockIrq,
    input: DosInput<Int33Model>,
    stats: &'static IrqStats,
}

impl Machine {
    fn new(locks: Modifiers, driver: Option<Int33Model>) -> Machine {
        let bus: &'static MockPortBus = Box::leak(Box::new(MockPortBus::new()));
        let model = bus.attach(0x60, 0x64, I8042Model::new());
        let ring: &'static SpscRing<u8, 64> = Box::leak(Box::new(SpscRing::new()));
        let stats: &'static IrqStats = Box::leak(Box::new(IrqStats::new()));
        let (tx, rx) = ring.split().unwrap();
        let kbd: &'static mut KbdIrq<Bus> = Box::leak(Box::new(KbdIrq::new(Bus(bus), tx, stats)));
        let irq = MockIrq::new();
        irq.register(Irq::KEYBOARD, IrqHandler::from_static(kbd), Chain::None)
            .unwrap();
        irq.unmask(Irq::KEYBOARD);
        let mouse = driver.map(|d| Mouse::detect(d).unwrap());
        Machine {
            model,
            irq,
            input: DosInput::new(rx, locks, mouse),
            stats,
        }
    }

    fn deliver(&self) {
        for _ in 0..1000 {
            let Some(line) = self.model.borrow().pending_irq() else {
                return;
            };
            if !self.irq.raise(Irq::new(line).unwrap()) {
                return;
            }
        }
    }

    fn type_bytes(&self, bytes: &[u8]) {
        for &b in bytes {
            self.model.borrow_mut().press_scancode(b);
        }
        self.deliver();
    }

    fn key_events(&mut self) -> Vec<KeyEvent> {
        std::iter::from_fn(|| self.input.poll_key()).collect()
    }

    fn key_lines(&mut self) -> Vec<String> {
        self.key_events()
            .iter()
            .map(|e| {
                let mut s = String::new();
                write_key(&mut s, e).unwrap();
                s
            })
            .collect()
    }
}

/// The keys the DOS program is sent with `AUTOTYPE` (`a b capslock c capslock up delete f1 lshift d esc`), as scancode set 1 bytes:
/// each key is pressed and released.
const AUTOTYPED: &[&[u8]] = &[
    &[0x1E, 0x9E],             // a
    &[0x30, 0xB0],             // b
    &[0x3A, 0xBA],             // capslock
    &[0x2E, 0xAE],             // c
    &[0x3A, 0xBA],             // capslock
    &[0xE0, 0x48, 0xE0, 0xC8], // up
    &[0xE0, 0x53, 0xE0, 0xD3], // delete
    &[0x3B, 0xBB],             // f1
    &[0x2A, 0xAA],             // lshift
    &[0x20, 0xA0],             // d
    &[0x01, 0x81],             // esc
];

#[test]
fn the_keys_the_dos_program_is_sent_decode_to_the_expected_transcript() {
    let mut m = Machine::new(Modifiers::NONE, None);
    for key in AUTOTYPED {
        m.type_bytes(key);
    }
    let expected = include_str!("../../../../spikes/b-dos-input/expected/keys.txt");
    assert_eq!(m.key_lines().join("\n") + "\n", expected);
    // The platform sent one end-of-interrupt for each interrupt and the handler none of its own.
    assert_eq!(m.irq.eois(), vec![Irq::KEYBOARD; AUTOTYPED.len()]);
    assert_eq!(
        IrqStats::get(&m.stats.kbd_bytes) as usize,
        AUTOTYPED.iter().map(|k| k.len()).sum::<usize>()
    );
    assert_eq!(m.input.keys_dropped(), 0);
}

#[test]
fn the_decoder_starts_from_the_locks_the_bios_showed() {
    let mut m = Machine::new(Modifiers::CAPS, None);
    m.type_bytes(&[0x1E, 0x9E, 0x3A, 0xBA, 0x1E, 0x9E]);
    assert_eq!(
        m.key_lines(),
        [
            "key A down mods=CAPS",
            "key A up mods=CAPS",
            "key CapsLock down mods=none",
            "key CapsLock up mods=none",
            "key A down mods=none",
            "key A up mods=none"
        ]
    );
}

#[test]
fn a_full_ring_drops_bytes_safely_and_the_decoder_recovers() {
    let mut m = Machine::new(Modifiers::NONE, None);
    // Shift goes down, then far more keys than the ring holds arrive before the core looks, so the ring is full when Shift's
    // release arrives and the release is lost.
    m.type_bytes(&[0x2A]);
    for _ in 0..40 {
        m.type_bytes(&[0x1E, 0x9E]);
    }
    m.type_bytes(&[0xAA]);
    assert!(m.input.keys_dropped() > 0, "the ring overflowed");
    assert_eq!(m.input.keys_dropped(), IrqStats::get(&m.stats.kbd_bytes) - 64);
    let events = m.key_events();
    assert!(events.len() >= 30, "what fitted was decoded: {}", events.len());
    assert!(
        events.iter().skip(1).all(|e| e.modifiers.contains(Modifiers::SHIFT)),
        "Shift was held for everything that was kept"
    );
    assert_eq!(m.input.recoveries(), 1);
    // The lost release must not leave Shift stuck on.
    assert_eq!(m.input.modifiers(), Modifiers::NONE);
    m.type_bytes(&[0x30, 0xB0]);
    let after = m.key_events();
    assert_eq!(after.len(), 2);
    assert!(after.iter().all(|e| e.modifiers == Modifiers::NONE));
    assert_eq!(m.input.recoveries(), 1, "no further drops, no further recoveries");
}

#[test]
fn no_mouse_driver_means_keyboard_only() {
    let mut m = Machine::new(Modifiers::NONE, None);
    assert!(!m.input.has_mouse() && m.input.mouse().is_none());
    assert_eq!(m.input.poll_mouse(), None);
    m.type_bytes(&[0x1E]);
    assert_eq!(m.key_events().len(), 1);
}

#[test]
fn the_mouse_transcript_matches_the_native_event_shape() {
    let mut m = Machine::new(Modifiers::NONE, Some(Int33Model::wheel_mouse()));
    assert!(m.input.has_mouse());
    let hw = m.input.mouse().unwrap();
    hw.bios.move_by(5, -3);
    hw.bios.press(MouseEvent::LEFT);
    let mut lines = mouse_lines(&mut m.input);
    let hw = m.input.mouse().unwrap();
    hw.bios.move_by(2, 2);
    lines.extend(mouse_lines(&mut m.input));
    let hw = m.input.mouse().unwrap();
    hw.bios.release(MouseEvent::LEFT);
    hw.bios.press(MouseEvent::RIGHT | MouseEvent::MIDDLE);
    hw.bios.scroll(-1);
    lines.extend(mouse_lines(&mut m.input));
    m.input.mouse().unwrap().bios.release(MouseEvent::RIGHT | MouseEvent::MIDDLE);
    lines.extend(mouse_lines(&mut m.input));
    assert_eq!(
        lines,
        [
            "mouse dx=5 dy=-3 buttons=L wheel=0",
            "mouse dx=2 dy=2 buttons=L wheel=0",
            "mouse dx=0 dy=0 buttons=MR wheel=-1",
            "mouse dx=0 dy=0 buttons=- wheel=0",
        ]
    );
}
