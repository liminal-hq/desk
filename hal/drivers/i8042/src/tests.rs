// Host tests: the set-up sequences and the interrupt services against the mock 8042 and scripted port sequences.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::cell::RefCell;
use std::rc::Rc;

use desk_hal::irq::{Chain, Irq, IrqHandler, IrqRegistry};
use desk_hal::ring::SpscRing;
use desk_hal::{InputSource, KeyCode, Modifiers, MouseEvent, PortIo};
use desk_hal_mock::ps2::I8042Model;
use desk_hal_mock::{Access, MockIrq, MockPortBus};

use super::*;

/// A controller on a mock bus with the model attached.
fn rig(model: I8042Model) -> (MockPortBus, Rc<RefCell<I8042Model>>) {
    let bus = MockPortBus::new();
    let m = bus.attach(0x60, 0x64, model);
    (bus, m)
}

fn ok_controller() -> ControllerInfo {
    ControllerInfo {
        dual_channel: true,
        translation: true,
    }
}

// ---- Scripted port sequences: the exact bytes on the wire ----

#[test]
fn set_interrupts_reads_modifies_and_writes_the_configuration_byte() {
    let bus = MockPortBus::scripted([
        // write_command(0x20): input buffer empty, then the command
        Access::inb(0x64, 0x00),
        Access::outb(0x64, 0x20),
        // read_any: output buffer full, the byte (translation and system flag set, IRQ 1 on, stray bit 7 kept)
        Access::inb(0x64, 0x01),
        Access::inb(0x60, 0xC5),
        // write_config(0xC7): command 0x60, then the data byte with both IRQ bits set
        Access::inb(0x64, 0x00),
        Access::outb(0x64, 0x60),
        Access::inb(0x64, 0x00),
        Access::outb(0x60, 0xC7),
    ]);
    I8042::new(&bus).set_interrupts(true, true).unwrap();
    assert!(bus.script_done());
}

#[test]
fn the_waits_poll_the_status_port_until_it_changes() {
    // Two busy reads, then free: the command goes out on the third. A reply that is two polls late still arrives.
    let bus = MockPortBus::scripted([
        Access::inb(0x64, 0x02),
        Access::inb(0x64, 0x02),
        Access::inb(0x64, 0x00),
        Access::outb(0x64, 0xAD),
    ]);
    I8042::new(&bus).write_command(command::DISABLE_KBD).unwrap();
    assert!(bus.script_done());
}

#[test]
fn a_keyboard_reset_handshake_on_the_wire() {
    // F5 (ack), FF (ack, then the self-test result 0xAA), a flush that finds nothing, F4 (ack). Translation is on, so no F0.
    let bus = MockPortBus::scripted([
        Access::inb(0x64, 0x00),
        Access::outb(0x60, 0xF5),
        Access::inb(0x64, 0x01),
        Access::inb(0x60, 0xFA),
        Access::inb(0x64, 0x00),
        Access::outb(0x60, 0xFF),
        Access::inb(0x64, 0x01),
        Access::inb(0x60, 0xFA),
        Access::inb(0x64, 0x01),
        Access::inb(0x60, 0xAA),
        Access::inb(0x64, 0x00),
        Access::inb(0x64, 0x00),
        Access::outb(0x60, 0xF4),
        Access::inb(0x64, 0x01),
        Access::inb(0x60, 0xFA),
    ]);
    let info = I8042::new(&bus).init_keyboard(ok_controller()).unwrap();
    assert!(bus.script_done());
    assert_eq!(
        info,
        KeyboardInfo {
            set1: true,
            translated: true
        }
    );
}

#[test]
fn the_mouse_goes_through_the_aux_prefix_on_every_byte() {
    // Reset: D4 FF, ack, AA, 00; defaults: D4 F6, ack; enable: D4 F4, ack. All replies carry the aux source bit (0x21).
    let mut s = Vec::new();
    for (cmd, replies) in [(0xFFu8, &[0xFAu8, 0xAA, 0x00][..]), (0xF6, &[0xFA]), (0xF4, &[0xFA])] {
        s.extend([
            Access::inb(0x64, 0x00),
            Access::outb(0x64, 0xD4),
            Access::inb(0x64, 0x00),
            Access::outb(0x60, cmd),
        ]);
        for &r in replies {
            s.extend([Access::inb(0x64, 0x21), Access::inb(0x60, r)]);
        }
    }
    let bus = MockPortBus::scripted(s);
    let info = I8042::new(&bus).init_mouse(false).unwrap();
    assert!(bus.script_done());
    assert_eq!(info, MouseInfo { id: 0 });
    assert_eq!(info.packet_len(), 3);
}

// ---- The controller ----

#[test]
fn controller_init_runs_the_documented_sequence_and_reports_what_it_found() {
    let (bus, model) = rig(I8042Model::new());
    // A stale byte from before (a key held while the machine booted) is flushed.
    model.borrow_mut().press_scancode(0x1E);
    let info = I8042::new(&bus).init().unwrap();
    assert_eq!(
        info,
        ControllerInfo {
            dual_channel: true,
            translation: true
        }
    );
    let m = model.borrow();
    assert_eq!(
        m.commands,
        [0xAD, 0xA7, 0x20, 0x60, 0xAA, 0x60, 0xA8, 0x20, 0xA7, 0xAB, 0xA9, 0xAE, 0xA8, 0x20]
    );
    assert_eq!(m.config & 0x03, 0, "interrupts stay off until the caller turns them on");
    assert_ne!(m.config & 0x40, 0, "translation on");
    assert_eq!(m.config & 0x30, 0, "both clocks running");
    assert!(m.kbd_port_enabled && m.aux_port_enabled);
    assert!(m.output().is_empty(), "the stale byte was flushed");
}

#[test]
fn controller_init_never_touches_the_output_port_where_the_a20_gate_lives() {
    let (bus, model) = rig(I8042Model::new());
    I8042::new(&bus).init().unwrap();
    let m = model.borrow();
    assert_eq!(m.output_port, 0xCF, "unchanged, so bit 1 (A20) is still set");
    assert!(
        !m.commands.contains(&0xD1) && !m.commands.contains(&0xFE),
        "no write-output-port, no pulse-reset"
    );
}

#[test]
fn a_controller_that_resets_its_configuration_in_the_self_test_is_configured_again() {
    let mut m = I8042Model::new();
    m.self_test_resets_config = true;
    let (bus, model) = rig(m);
    let info = I8042::new(&bus).init().unwrap();
    assert!(info.translation);
    assert_ne!(model.borrow().config & 0x40, 0);
}

#[test]
fn a_single_channel_controller_is_reported_and_the_mouse_port_left_alone() {
    let mut m = I8042Model::new();
    m.single_channel = true;
    let (bus, model) = rig(m);
    let info = I8042::new(&bus).init().unwrap();
    assert!(!info.dual_channel);
    assert!(!model.borrow().commands.contains(&0xA9), "no aux port test");
}

#[test]
fn a_controller_that_cannot_translate_says_so() {
    let mut m = I8042Model::new();
    m.no_translation = true;
    let (bus, _) = rig(m);
    assert!(!I8042::new(&bus).init().unwrap().translation);
}

#[test]
fn failed_self_test_and_port_tests_are_errors_with_the_code() {
    let mut m = I8042Model::new();
    m.self_test_result = 0xFC;
    let (bus, _) = rig(m);
    assert_eq!(I8042::new(&bus).init(), Err(Error::ControllerSelfTest(0xFC)));

    let mut m = I8042Model::new();
    m.kbd_port_test_result = 0x02;
    let (bus, _) = rig(m);
    assert_eq!(I8042::new(&bus).init(), Err(Error::PortTest { aux: false, code: 2 }));

    let mut m = I8042Model::new();
    m.aux_port_test_result = 0x04;
    let (bus, _) = rig(m);
    assert_eq!(I8042::new(&bus).init(), Err(Error::PortTest { aux: true, code: 4 }));
}

#[test]
fn a_hung_or_absent_controller_times_out_instead_of_hanging() {
    // Input buffer full forever.
    let mut m = I8042Model::new();
    m.input_busy_reads = u32::MAX;
    let (bus, _) = rig(m);
    assert_eq!(I8042::new(&bus).init(), Err(Error::Timeout));
    // A floating bus reads all ones: the input buffer looks full, so the first command times out.
    let bus = MockPortBus::new();
    assert_eq!(I8042::new(&bus).init(), Err(Error::Timeout));
    assert_eq!(HalError::from(Error::Timeout), HalError::Timeout);
}

#[test]
fn a_slow_controller_within_the_bound_works() {
    let mut m = I8042Model::new();
    m.input_busy_reads = POLLS - 1;
    let (bus, _) = rig(m);
    assert!(I8042::new(&bus).init().is_ok());
}

#[test]
fn a_reply_that_never_comes_times_out() {
    let (bus, model) = rig(I8042Model::new());
    // The controller is fine, but the reply to the read-config command is swallowed.
    let c = I8042::new(&bus);
    model.borrow_mut().clear_output();
    c.write_command(command::SELF_TEST).unwrap();
    model.borrow_mut().clear_output();
    assert_eq!(c.read_any(1_000), Err(Error::Timeout));
}

#[test]
fn flush_is_bounded() {
    let (bus, model) = rig(I8042Model::new());
    for _ in 0..(FLUSH_LIMIT + 10) {
        model.borrow_mut().press_scancode(0x1E);
    }
    assert_eq!(I8042::new(&bus).flush(), FLUSH_LIMIT);
    assert_eq!(model.borrow().output().len(), 10);
}

// ---- The keyboard ----

#[test]
fn keyboard_init_resets_and_enables() {
    let (bus, model) = rig(I8042Model::new());
    let info = I8042::new(&bus).init_keyboard(ok_controller()).unwrap();
    assert_eq!(
        info,
        KeyboardInfo {
            set1: true,
            translated: true
        }
    );
    let m = model.borrow();
    assert_eq!(m.keyboard.knobs.received, [0xF5, 0xFF, 0xF4]);
    assert!(m.keyboard.scanning);
}

#[test]
fn keyboard_resend_is_retried_up_to_the_limit() {
    let mut m = I8042Model::new();
    m.keyboard.knobs.resend_next = 1;
    let (bus, model) = rig(m);
    I8042::new(&bus).init_keyboard(ok_controller()).unwrap();
    assert_eq!(
        model.borrow().keyboard.knobs.received,
        [0xF5, 0xF5, 0xFF, 0xF4],
        "the first command is sent twice"
    );

    let mut m = I8042Model::new();
    m.keyboard.knobs.resend_next = RESEND_LIMIT as u8;
    let (bus, _) = rig(m);
    assert!(
        I8042::new(&bus).init_keyboard(ok_controller()).is_ok(),
        "the last allowed resend still succeeds"
    );

    let mut m = I8042Model::new();
    m.keyboard.knobs.resend_next = RESEND_LIMIT as u8 + 1;
    let (bus, _) = rig(m);
    assert_eq!(I8042::new(&bus).init_keyboard(ok_controller()), Err(Error::Resend));
}

#[test]
fn a_missing_keyboard_is_no_device() {
    let (bus, _) = rig(I8042Model::without_devices());
    assert_eq!(I8042::new(&bus).init_keyboard(ok_controller()), Err(Error::NoDevice));
}

#[test]
fn a_keyboard_that_never_acknowledges_is_no_device() {
    let mut m = I8042Model::new();
    m.keyboard.knobs.silent = true;
    let (bus, _) = rig(m);
    assert_eq!(I8042::new(&bus).init_keyboard(ok_controller()), Err(Error::NoDevice));
}

#[test]
fn a_failed_self_test_is_reported_with_its_code() {
    let mut m = I8042Model::new();
    m.keyboard.bat_result = 0xFC;
    let (bus, _) = rig(m);
    assert_eq!(
        I8042::new(&bus).init_keyboard(ok_controller()),
        Err(Error::DeviceSelfTest(0xFC))
    );
}

#[test]
fn without_translation_the_keyboard_is_switched_to_set_1() {
    let mut m = I8042Model::new();
    m.no_translation = true;
    let (bus, model) = rig(m);
    let c = I8042::new(&bus);
    let ctl = c.init().unwrap();
    assert!(!ctl.translation);
    let info = c.init_keyboard(ctl).unwrap();
    assert_eq!(
        info,
        KeyboardInfo {
            set1: true,
            translated: false
        }
    );
    let m = model.borrow();
    assert_eq!(m.keyboard.set, 1);
    assert_eq!(m.keyboard.knobs.received, [0xF5, 0xFF, 0xF0, 0x01, 0xF0, 0x00, 0xF4]);
}

#[test]
fn a_mouse_byte_during_the_keyboard_reset_is_not_taken_for_a_reply() {
    let mut m = I8042Model::new();
    m.mouse.streaming = true;
    let (bus, model) = rig(m);
    model.borrow_mut().send_mouse_bytes(&[0xFA, 0x09, 0x01, 0x01]);
    assert!(I8042::new(&bus).init_keyboard(ok_controller()).is_ok());
}

// ---- The mouse ----

#[test]
fn mouse_init_resets_sets_defaults_and_streams() {
    let (bus, model) = rig(I8042Model::new());
    let info = I8042::new(&bus).init_mouse(false).unwrap();
    assert_eq!(info, MouseInfo { id: 0 });
    let m = model.borrow();
    assert_eq!(m.mouse.knobs.received, [0xFF, 0xF6, 0xF4]);
    assert!(m.mouse.streaming);
}

#[test]
fn the_wheel_probe_sends_200_100_80_and_reads_the_id() {
    let mut m = I8042Model::new();
    m.mouse.wheel_capable = true;
    let (bus, model) = rig(m);
    let info = I8042::new(&bus).init_mouse(true).unwrap();
    assert_eq!(info, MouseInfo { id: 3 });
    assert_eq!(info.packet_len(), 4);
    let m = model.borrow();
    assert_eq!(
        m.mouse.knobs.received,
        [0xFF, 0xF6, 0xF3, 200, 0xF3, 100, 0xF3, 80, 0xF2, 0xF3, 100, 0xF4]
    );
    assert_eq!(m.mouse.sample_rate, 100, "the sample rate is put back");
}

#[test]
fn a_mouse_without_a_wheel_answers_the_probe_with_id_0() {
    let (bus, _) = rig(I8042Model::new());
    let info = I8042::new(&bus).init_mouse(true).unwrap();
    assert_eq!(info, MouseInfo { id: 0 });
}

#[test]
fn a_missing_mouse_is_no_device_and_one_that_asks_for_resends_is_retried() {
    let mut m = I8042Model::new();
    m.mouse.knobs.absent = true;
    let (bus, _) = rig(m);
    assert_eq!(I8042::new(&bus).init_mouse(false), Err(Error::NoDevice));

    let mut m = I8042Model::new();
    m.mouse.knobs.resend_next = 2;
    let (bus, model) = rig(m);
    assert!(I8042::new(&bus).init_mouse(false).is_ok());
    assert_eq!(model.borrow().mouse.knobs.received[..3], [0xFF, 0xFF, 0xFF]);

    let mut m = I8042Model::new();
    m.mouse.knobs.resend_next = 9;
    let (bus, _) = rig(m);
    assert_eq!(I8042::new(&bus).init_mouse(false), Err(Error::Resend));
}

#[test]
fn a_mouse_that_never_acknowledges_fails_the_set_up_with_a_timeout() {
    // It resets (and says so) but acknowledges nothing after: the first command that needs an acknowledgement times out.
    let mut m = I8042Model::new();
    m.mouse.knobs.silent = true;
    let (bus, _) = rig(m);
    assert_eq!(I8042::new(&bus).init_mouse(false), Err(Error::Timeout));
}

#[test]
fn a_key_pressed_during_the_mouse_set_up_is_not_taken_for_a_reply() {
    let (bus, model) = rig(I8042Model::new());
    model.borrow_mut().press_scancode(0xFA); // a keyboard byte that looks like an ACK
    assert!(I8042::new(&bus).init_mouse(false).is_ok());
}

// ---- Packet decoding ----

fn pkt(b0: u8, dx: u8, dy: u8, z: u8) -> [u8; 4] {
    [b0, dx, dy, z]
}

#[test]
fn packets_decode_with_sign_extension_and_a_flipped_y() {
    // Right 5, up 3 (the device says dy = +3): the event says dy = -3.
    assert_eq!(
        irq::decode(&pkt(0x08, 5, 3, 0), false),
        Some(MouseEvent {
            dx: 5,
            dy: -3,
            buttons: 0,
            wheel: 0
        })
    );
    // Left 1 (0xFF with the X sign bit), down 2 (0xFE with the Y sign bit): dx -1, dy +2.
    assert_eq!(
        irq::decode(&pkt(0x08 | 0x10 | 0x20, 0xFF, 0xFE, 0), false),
        Some(MouseEvent {
            dx: -1,
            dy: 2,
            buttons: 0,
            wheel: 0
        })
    );
    // The nine-bit extremes: -256 (sign bit, 0x00) and +255.
    assert_eq!(
        irq::decode(&pkt(0x18, 0x00, 0xFF, 0), false).map(|e| (e.dx, e.dy)),
        Some((-256, -255))
    );
    assert_eq!(
        irq::decode(&pkt(0x28, 0xFF, 0x00, 0), false).map(|e| (e.dx, e.dy)),
        Some((255, 256))
    );
}

#[test]
fn the_buttons_come_from_bits_0_to_2() {
    for (b0, want) in [(0x09, 1), (0x0A, 2), (0x0C, 4), (0x0F, 7)] {
        assert_eq!(irq::decode(&pkt(b0, 0, 0, 0), false).unwrap().buttons, want);
    }
}

#[test]
fn an_overflow_packet_has_no_event() {
    assert_eq!(irq::decode(&pkt(0x48, 1, 1, 0), false), None);
    assert_eq!(irq::decode(&pkt(0x88, 1, 1, 0), false), None);
}

#[test]
fn the_wheel_byte_is_a_signed_count_only_when_a_wheel_was_negotiated() {
    assert_eq!(irq::decode(&pkt(0x08, 0, 0, 0xFF), true).unwrap().wheel, -1);
    assert_eq!(irq::decode(&pkt(0x08, 0, 0, 0x01), true).unwrap().wheel, 1);
    assert_eq!(irq::decode(&pkt(0x08, 0, 0, 0x01), false).unwrap().wheel, 0);
}

// ---- The interrupt services over the mock controller ----

/// A byte-for-byte shared bus handle the handlers can own: the handler needs `Send + 'static`, and the mock bus is neither.
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

/// A whole machine: the model, the interrupt controller with both handlers registered, and the core's side.
struct Machine {
    model: Rc<RefCell<I8042Model>>,
    irq: MockIrq,
    input: Ps2Input,
    stats: &'static IrqStats,
}

impl Machine {
    fn new(wheel: bool) -> Machine {
        let bus: &'static MockPortBus = Box::leak(Box::new(MockPortBus::new()));
        let model = bus.attach(0x60, 0x64, I8042Model::new());
        model.borrow_mut().mouse.streaming = true;
        let kring: &'static SpscRing<u8, 64> = Box::leak(Box::new(SpscRing::new()));
        let mring: &'static SpscRing<MouseEvent, 32> = Box::leak(Box::new(SpscRing::new()));
        let stats: &'static IrqStats = Box::leak(Box::new(IrqStats::new()));
        let (ktx, krx) = kring.split().unwrap();
        let (mtx, mrx) = mring.split().unwrap();
        let kbd: &'static mut KbdIrq<Bus> = Box::leak(Box::new(KbdIrq::new(Bus(bus), ktx, stats)));
        let mouse: &'static mut MouseIrq<Bus> = Box::leak(Box::new(MouseIrq::new(Bus(bus), mtx, stats, wheel)));
        let irq = MockIrq::new();
        irq.register(Irq::KEYBOARD, IrqHandler::from_static(kbd), Chain::None)
            .unwrap();
        irq.register(Irq::MOUSE, IrqHandler::from_static(mouse), Chain::None).unwrap();
        irq.unmask(Irq::KEYBOARD);
        irq.unmask(Irq::MOUSE);
        Machine {
            model,
            irq,
            input: Ps2Input::new(krx, mrx),
            stats,
        }
    }

    /// Raises the interrupts the controller would, until its output buffer is empty or it stops asking.
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

    fn key_bytes(&self, bytes: &[u8]) {
        for &b in bytes {
            self.model.borrow_mut().press_scancode(b);
        }
        self.deliver();
    }

    fn mouse_bytes(&self, bytes: &[u8]) {
        self.model.borrow_mut().send_mouse_bytes(bytes);
        self.deliver();
    }

    fn keys(&mut self) -> Vec<(KeyCode, bool)> {
        std::iter::from_fn(|| self.input.poll_key())
            .map(|e| (e.code, e.pressed))
            .collect()
    }

    fn mice(&mut self) -> Vec<MouseEvent> {
        std::iter::from_fn(|| self.input.poll_mouse()).collect()
    }
}

#[test]
fn keys_travel_from_the_device_through_the_interrupt_and_the_ring_to_the_core() {
    let mut m = Machine::new(false);
    m.key_bytes(&[0x2A, 0x1E, 0x9E, 0xAA, 0xE0, 0x48, 0xE0, 0xC8]);
    assert_eq!(
        m.keys(),
        [
            (KeyCode::LeftShift, true),
            (KeyCode::A, true),
            (KeyCode::A, false),
            (KeyCode::LeftShift, false),
            (KeyCode::Up, true),
            (KeyCode::Up, false)
        ]
    );
    // The platform acknowledged each byte, on the master only for IRQ 1.
    // One interrupt drained all eight bytes (the bound is 8), so one acknowledgement.
    assert_eq!(m.irq.eois(), [Irq::KEYBOARD]);
    assert_eq!(IrqStats::get(&m.stats.kbd_bytes), 8);
}

#[test]
fn the_pause_key_arrives_as_one_press() {
    let mut m = Machine::new(false);
    m.key_bytes(&[0xE1, 0x1D, 0x45, 0xE1, 0x9D, 0xC5]);
    assert_eq!(m.keys(), [(KeyCode::Pause, true)]);
}

#[test]
fn a_masked_irq_1_delivers_no_keys() {
    // The negative check: with the line masked nothing reaches the handler, the byte stays in the controller, and unmasking
    // delivers it.
    let mut m = Machine::new(false);
    m.irq.mask(Irq::KEYBOARD);
    m.key_bytes(&[0x1E, 0x9E]);
    assert_eq!(m.keys(), []);
    assert_eq!(m.model.borrow().output().len(), 2);
    m.irq.unmask(Irq::KEYBOARD);
    m.deliver();
    assert_eq!(m.keys(), [(KeyCode::A, true), (KeyCode::A, false)]);
}

#[test]
fn the_controller_with_the_irq_bit_off_raises_nothing() {
    let mut m = Machine::new(false);
    m.model.borrow_mut().config &= !0x01;
    m.key_bytes(&[0x1E]);
    assert_eq!(m.keys(), []);
}

#[test]
fn mouse_packets_become_events_and_the_acknowledgement_covers_the_slave() {
    let mut m = Machine::new(false);
    m.mouse_bytes(&[0x09, 10, 4, 0x3A, 0xFF, 0xFE, 0x08, 0, 0]);
    assert_eq!(
        m.mice(),
        [
            MouseEvent {
                dx: 10,
                dy: -4,
                buttons: 1,
                wheel: 0
            },
            MouseEvent {
                dx: -1,
                dy: 2,
                buttons: 2,
                wheel: 0
            },
            MouseEvent {
                dx: 0,
                dy: 0,
                buttons: 0,
                wheel: 0
            }
        ]
    );
    assert!(m.irq.eois().iter().all(|i| *i == Irq::MOUSE));
}

#[test]
fn wheel_packets_are_four_bytes() {
    let mut m = Machine::new(true);
    m.mouse_bytes(&[0x08, 1, 1, 0xFF, 0x08, 0, 0, 0x01]);
    assert_eq!(
        m.mice(),
        [
            MouseEvent {
                dx: 1,
                dy: -1,
                buttons: 0,
                wheel: -1
            },
            MouseEvent {
                dx: 0,
                dy: 0,
                buttons: 0,
                wheel: 1
            }
        ]
    );
}

#[test]
fn a_bad_first_byte_is_dropped_until_the_stream_resynchronises() {
    let mut m = Machine::new(false);
    // Two stray bytes without bit 3, then a good packet.
    m.mouse_bytes(&[0x00, 0x05, 0x08, 7, 7]);
    assert_eq!(
        m.mice(),
        [MouseEvent {
            dx: 7,
            dy: -7,
            buttons: 0,
            wheel: 0
        }]
    );
    assert_eq!(IrqStats::get(&m.stats.mouse_resyncs), 2);
}

#[test]
fn a_lost_byte_costs_a_packet_or_two_and_then_the_stream_is_back_in_step() {
    let mut m = Machine::new(false);
    // Packet A is whole. Packet B loses its dx byte, so [b0, dy, next b0] is read as one packet (garbage, but it is only
    // one), and the stream is out of step until a byte without bit 3 lands in first position. Six whole packets follow.
    let a = [0x08, 1, 1];
    let b_lost = [0x08, 2];
    let c = [0x08, 3, 3];
    let mut stream = Vec::new();
    stream.extend(a);
    stream.extend(b_lost);
    for _ in 0..6 {
        stream.extend(c);
    }
    m.mouse_bytes(&stream);
    let events = m.mice();
    assert_eq!(
        events[0],
        MouseEvent {
            dx: 1,
            dy: -1,
            buttons: 0,
            wheel: 0
        }
    );
    let good = MouseEvent {
        dx: 3,
        dy: -3,
        buttons: 0,
        wheel: 0,
    };
    assert!(events.len() >= 4, "got {events:?}");
    assert_eq!(*events.last().unwrap(), good, "back in step by the end");
    assert_eq!(events[events.len() - 2], good);
}

#[test]
fn an_overflow_packet_is_consumed_without_an_event_and_stays_in_step() {
    let mut m = Machine::new(false);
    m.mouse_bytes(&[0x48, 200, 200, 0x08, 2, 2]);
    assert_eq!(
        m.mice(),
        [MouseEvent {
            dx: 2,
            dy: -2,
            buttons: 0,
            wheel: 0
        }]
    );
    assert_eq!(IrqStats::get(&m.stats.mouse_overflows), 1);
}

#[test]
fn a_keyboard_interrupt_does_not_steal_a_mouse_byte_or_the_reverse() {
    let mut m = Machine::new(false);
    // Both devices have a byte waiting; the model presents the front one. Raise the wrong line first.
    m.model.borrow_mut().send_mouse_bytes(&[0x08, 4, 4]);
    m.irq.mask(Irq::MOUSE);
    assert!(m.irq.raise(Irq::KEYBOARD), "the handler ran");
    assert_eq!(
        m.model.borrow().output().len(),
        3,
        "IRQ 1 left the mouse's bytes where they were"
    );
    m.irq.unmask(Irq::MOUSE);
    m.deliver();
    assert_eq!(m.mice().len(), 1);
}

#[test]
fn one_interrupt_drains_what_the_controller_has_up_to_the_bound() {
    let mut m = Machine::new(false);
    for b in [0x1E, 0x9E, 0x30] {
        m.model.borrow_mut().press_scancode(b);
    }
    assert!(m.irq.raise(Irq::KEYBOARD));
    assert!(m.model.borrow().output().is_empty());
    assert_eq!(m.keys().len(), 3);
}

#[test]
fn a_full_keyboard_ring_drops_and_counts_and_order_survives() {
    let mut m = Machine::new(false);
    // 70 make codes of distinct keys cycling a, b: 64 fit, 6 are dropped; the core drains them in order.
    let bytes: Vec<u8> = (0..70).map(|i| if i % 2 == 0 { 0x1E } else { 0x30 }).collect();
    m.key_bytes(&bytes);
    assert_eq!(m.input.keys_dropped(), 6);
    let keys = m.keys();
    assert_eq!(keys.len(), 64);
    assert_eq!(keys[0].0, KeyCode::A);
    assert_eq!(keys[1].0, KeyCode::B);
    // Draining made room: later bytes get through again.
    m.key_bytes(&[0x2E]);
    assert_eq!(m.keys(), [(KeyCode::C, true)]);
}

#[test]
fn a_full_mouse_ring_drops_and_the_ring_wraps_across_many_packets() {
    let mut m = Machine::new(false);
    for _ in 0..40 {
        m.mouse_bytes(&[0x08, 1, 0]);
    }
    assert_eq!(m.input.mouse_dropped(), 8);
    assert_eq!(m.mice().len(), 32);
    // Many rounds of fill and drain wrap the indices without losing or reordering anything.
    for round in 0..200u32 {
        m.mouse_bytes(&[0x08, (round % 100) as u8, 0, 0x08, 1, 1]);
        let e = m.mice();
        assert_eq!(e.len(), 2, "round {round}");
        assert_eq!(e[0].dx, (round % 100) as i16);
    }
}

#[test]
fn modifiers_reach_the_core_with_the_events() {
    let mut m = Machine::new(false);
    m.key_bytes(&[0x1D, 0x38, 0xE0, 0x53]);
    let events: Vec<_> = std::iter::from_fn(|| m.input.poll_key()).collect();
    assert_eq!(events[2].code, KeyCode::Delete);
    assert!(events[2].modifiers.contains(Modifiers::CTRL.union(Modifiers::ALT)));
    assert_eq!(m.input.decoder().modifiers(), Modifiers::CTRL.union(Modifiers::ALT));
}

#[test]
fn bytes_flagged_with_a_link_error_are_dropped_and_counted() {
    // Scripted: the status shows output full with the parity bit; the byte is read (to clear the buffer) and not queued.
    let bus: &'static MockPortBus = Box::leak(Box::new(MockPortBus::scripted([
        Access::inb(0x64, 0x81),
        Access::inb(0x60, 0x1E),
        Access::inb(0x64, 0x00),
    ])));
    let ring: &'static SpscRing<u8, 64> = Box::leak(Box::new(SpscRing::new()));
    let stats: &'static IrqStats = Box::leak(Box::new(IrqStats::new()));
    let (tx, mut rx) = ring.split().unwrap();
    let mut kbd = KbdIrq::new(Bus(bus), tx, stats);
    kbd.on_irq();
    assert!(bus.script_done());
    assert_eq!(rx.pop(), None);
    assert_eq!(IrqStats::get(&stats.kbd_errors), 1);
}

use desk_hal::irq::{InterruptController, IrqService};
