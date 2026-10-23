// The DAC: palette programming, read-back, the bounded retrace wait and fades, against the register-file model.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use desk_hal::{HalError, PortIo, Rgb6};
use desk_hal_mock::{MockPortBus, VgaModel};
use desk_vga::dac::{self, ENTRIES};

fn bus() -> (MockPortBus, std::rc::Rc<std::cell::RefCell<VgaModel>>) {
    let bus = MockPortBus::new();
    let vga = bus.attach(0x3C0, 0x3DF, VgaModel::new());
    (bus, vga)
}

fn ramp() -> [Rgb6; ENTRIES] {
    let mut p = [Rgb6::BLACK; ENTRIES];
    for (i, c) in p.iter_mut().enumerate() {
        *c = Rgb6::new((i / 4) as u8, (63 - i / 4) as u8, (i % 64) as u8);
    }
    p
}

#[test]
fn a_palette_is_one_index_write_and_triples_of_data() {
    let (bus, vga) = bus();
    dac::set_palette(&bus, 5, &[Rgb6::new(1, 2, 3), Rgb6::new(63, 62, 61)]).unwrap();
    assert_eq!(
        bus.writes(),
        vec![
            (0x3C8, 5),
            (0x3C9, 1),
            (0x3C9, 2),
            (0x3C9, 3),
            (0x3C9, 63),
            (0x3C9, 62),
            (0x3C9, 61)
        ]
    );
    assert_eq!(vga.borrow().dac[5], [1, 2, 3]);
    assert_eq!(vga.borrow().dac[6], [63, 62, 61]);
    assert_eq!(vga.borrow().dac[7], [0, 0, 0]);
}

#[test]
fn values_are_six_bits() {
    let (bus, vga) = bus();
    // A colour built by hand with bits above the sixth is masked on the way out.
    dac::set_palette(
        &bus,
        0,
        &[Rgb6 {
            r: 0xFF,
            g: 0x40,
            b: 0x41,
        }],
    )
    .unwrap();
    assert_eq!(vga.borrow().dac[0], [63, 0, 1]);
}

#[test]
fn all_256_entries_round_trip() {
    let (bus, _vga) = bus();
    let want = ramp();
    dac::set_palette(&bus, 0, &want).unwrap();
    let mut got = [Rgb6::BLACK; ENTRIES];
    dac::read_palette(&bus, 0, &mut got).unwrap();
    assert_eq!(got, want);
    // Reads go through 0x3C7, writes through 0x3C8.
    let w = bus.writes();
    assert_eq!(w.iter().filter(|(p, _)| *p == 0x3C8).count(), 1);
    assert_eq!(w.iter().filter(|(p, _)| *p == 0x3C7).count(), 1);
    assert_eq!(w.iter().filter(|(p, _)| *p == 0x3C9).count(), 768);
}

#[test]
fn ranges_past_the_end_are_refused_and_write_nothing() {
    let (bus, _vga) = bus();
    assert_eq!(dac::set_palette(&bus, 250, &[Rgb6::BLACK; 7]), Err(HalError::InvalidArgument));
    assert_eq!(
        dac::read_palette(&bus, 255, &mut [Rgb6::BLACK; 2]),
        Err(HalError::InvalidArgument)
    );
    assert!(bus.log().is_empty());
    dac::set_palette(&bus, 255, &[Rgb6::new(9, 9, 9)]).unwrap();
    dac::set_palette(&bus, 0, &[]).unwrap();
    assert_eq!(bus.writes().len(), 4);
}

#[test]
fn a_partial_read_starts_at_the_requested_entry() {
    let (bus, _vga) = bus();
    dac::set_palette(&bus, 0, &ramp()).unwrap();
    let mut got = [Rgb6::BLACK; 3];
    dac::read_palette(&bus, 100, &mut got).unwrap();
    assert_eq!(&got, &ramp()[100..103]);
}

#[test]
fn the_retrace_wait_sees_a_working_display() {
    let (bus, vga) = bus();
    assert_eq!(dac::wait_vretrace_bounded(&bus, 50), Ok(()));
    // Entered during a retrace, it waits for that one to end and the next to begin.
    vga.borrow_mut().retrace_period = 3;
    assert_eq!(dac::wait_vretrace_bounded(&bus, 50), Ok(()));
    assert_ne!(bus.inb(0x3DA) & dac::VRETRACE, 0, "returns inside the retrace");
}

#[test]
fn the_retrace_wait_gives_up_on_a_dead_display() {
    let (bus, vga) = bus();
    vga.borrow_mut().retrace_period = 0;
    assert_eq!(dac::wait_vretrace_bounded(&bus, 20), Err(HalError::Timeout));
    assert_eq!(
        bus.log().len(),
        21,
        "one read to see no retrace is under way, then exactly the bound"
    );
    // A retrace bit stuck on is also a timeout, in the first phase.
    let stuck = MockPortBus::new();
    stuck.attach(0x3DA, 0x3DA, StuckHigh);
    assert_eq!(dac::wait_vretrace_bounded(&stuck, 20), Err(HalError::Timeout));
    assert_eq!(stuck.log().len(), 20);
}

struct StuckHigh;
impl desk_hal_mock::Device for StuckHigh {
    fn read(&mut self, _: u16, _: desk_hal_mock::Width) -> u32 {
        0x08
    }
    fn write(&mut self, _: u16, _: desk_hal_mock::Width, _: u32) {}
}

#[test]
fn scaling_the_palette_is_exact_at_the_ends() {
    let (bus, vga) = bus();
    let base = ramp();
    dac::set_palette_scaled(&bus, &base, 16, 16);
    for (i, want) in base.iter().enumerate() {
        assert_eq!(
            Rgb6 {
                r: vga.borrow().dac[i][0],
                g: vga.borrow().dac[i][1],
                b: vga.borrow().dac[i][2]
            },
            *want
        );
    }
    dac::set_palette_scaled(&bus, &base, 8, 16);
    assert_eq!(vga.borrow().dac[255], [base[255].r / 2, base[255].g / 2, base[255].b / 2]);
    dac::set_palette_scaled(&bus, &base, 0, 16);
    assert!(vga.borrow().dac.iter().all(|c| *c == [0, 0, 0]));
}

#[test]
fn a_fade_takes_sixteen_frames_and_ends_black_then_restores() {
    let (bus, vga) = bus();
    let base = ramp();
    dac::set_palette(&bus, 0, &base).unwrap();
    bus.take_log();
    let out = dac::fade_out(&bus, &base, dac::FADE_STEPS);
    assert_eq!(
        out,
        dac::FadeReport {
            synced: 16,
            timed_out: 0
        }
    );
    assert!(vga.borrow().dac.iter().all(|c| *c == [0, 0, 0]), "the last frame is black");
    let w = bus.writes();
    assert_eq!(
        w.iter().filter(|(p, _)| *p == 0x3C8).count(),
        16,
        "one palette write per frame"
    );
    assert_eq!(w.iter().filter(|(p, _)| *p == 0x3C9).count(), 16 * 768);

    let inn = dac::fade_in(&bus, &base, dac::FADE_STEPS);
    assert_eq!(inn.synced, 16);
    for (i, c) in base.iter().enumerate() {
        assert_eq!(vga.borrow().dac[i], [c.r, c.g, c.b], "entry {i} is back");
    }
}

#[test]
fn a_fade_with_no_retrace_still_completes() {
    let (bus, vga) = bus();
    vga.borrow_mut().retrace_period = 0;
    // The default bound is a million polls per phase; use a short fade of two frames and let the mock's log grow.
    let base = ramp();
    let r = dac::fade_out(&bus, &base, 2);
    assert_eq!(r, dac::FadeReport { synced: 0, timed_out: 2 });
    assert!(vga.borrow().dac.iter().all(|c| *c == [0, 0, 0]));
}
