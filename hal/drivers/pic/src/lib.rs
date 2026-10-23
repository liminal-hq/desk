// The two cascaded 8259 interrupt controllers: initialisation (native only), masking, end-of-interrupt and spurious checks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The reference is the native kernel's `kernel/src/pic.rs`: `init(REMAPPED, 0xFFFE)` writes its exact sequence, the I/O delays
// included. Under DOS the controllers belong to the BIOS, which programmed them at vectors 08h and 70h; the hosted platform never
// calls `init`, only masks, unmasks and acknowledges through the same `InterruptController` methods, which is what DJGPP programs
// do as well.
#![cfg_attr(not(any(test, feature = "std")), no_std)]

use desk_hal::irq::{InterruptController, Irq};
use desk_hal::PortIo;

pub const MASTER: u16 = 0x20;
pub const SLAVE: u16 = 0xA0;

/// Vector bases: the BIOS's (IRQ 0-7 at 08h, which collides with CPU exceptions in protected mode; IRQ 8-15 at 70h), and the
/// kernel's (20h and 28h, just past the exceptions).
pub const BIOS_BASES: (u8, u8) = (0x08, 0x70);
pub const REMAPPED_BASES: (u8, u8) = (0x20, 0x28);

const ICW1_INIT_ICW4: u8 = 0x11; // edge-triggered, cascaded, ICW4 follows
const ICW3_SLAVE_ON_IRQ2: u8 = 0x04;
const ICW3_SLAVE_ID: u8 = 0x02;
const ICW4_8086: u8 = 0x01;
const OCW2_EOI: u8 = 0x20;
const OCW3_READ_ISR: u8 = 0x0B;

/// Both controllers, reached through `ports`.
#[derive(Clone, Copy, Debug)]
pub struct Pic8259<P> {
    ports: P,
}

impl<P: PortIo> Pic8259<P> {
    /// Names the controllers; touches no hardware. The hosted platform stops here.
    pub const fn new(ports: P) -> Self {
        Pic8259 { ports }
    }

    /// Re-initialises both controllers with new vector bases and sets the masks (bit n masks IRQ n; bits 8-15 are the slave's).
    /// Native only: under DOS this would take the interrupts away from the BIOS. Each write is followed by an I/O delay, which the
    /// 8259 needs between initialisation words on old machines.
    pub fn init(&self, bases: (u8, u8), mask: u16) {
        let p = &self.ports;
        let steps = [
            (MASTER, ICW1_INIT_ICW4),
            (SLAVE, ICW1_INIT_ICW4),
            (MASTER + 1, bases.0),
            (SLAVE + 1, bases.1),
            (MASTER + 1, ICW3_SLAVE_ON_IRQ2),
            (SLAVE + 1, ICW3_SLAVE_ID),
            (MASTER + 1, ICW4_8086),
            (SLAVE + 1, ICW4_8086),
        ];
        for (port, value) in steps {
            p.outb(port, value);
            p.io_wait();
        }
        self.set_mask(mask);
    }

    /// Both mask registers at once.
    pub fn set_mask(&self, mask: u16) {
        self.ports.outb(MASTER + 1, mask as u8);
        self.ports.outb(SLAVE + 1, (mask >> 8) as u8);
    }

    pub fn mask_bits(&self) -> u16 {
        u16::from(self.ports.inb(MASTER + 1)) | (u16::from(self.ports.inb(SLAVE + 1)) << 8)
    }

    /// The end-of-interrupt for the master alone: what a spurious IRQ 15 needs (the slave raised nothing, but the master saw
    /// IRQ 2).
    pub fn eoi_master(&self) {
        self.ports.outb(MASTER, OCW2_EOI);
    }

    fn in_service(&self, command_port: u16) -> u8 {
        self.ports.outb(command_port, OCW3_READ_ISR);
        self.ports.inb(command_port)
    }

    fn line(irq: Irq) -> (u16, u8) {
        if irq.is_slave() {
            (SLAVE + 1, irq.line() - 8)
        } else {
            (MASTER + 1, irq.line())
        }
    }
}

/// `mask`, `unmask` and `set_mask` read and write the shared mask registers without turning interrupts off; see the contract on
/// `InterruptController` (the caller holds the platform's critical section).
impl<P: PortIo> InterruptController for Pic8259<P> {
    fn mask(&self, irq: Irq) {
        let (port, bit) = Self::line(irq);
        self.ports.outb(port, self.ports.inb(port) | (1 << bit));
    }

    /// Unmasks the line, and for a slave line the cascade (IRQ 2) too, without which it cannot reach the CPU.
    fn unmask(&self, irq: Irq) {
        let (port, bit) = Self::line(irq);
        self.ports.outb(port, self.ports.inb(port) & !(1 << bit));
        if irq.is_slave() {
            self.unmask(Irq::CASCADE);
        }
    }

    fn is_masked(&self, irq: Irq) -> bool {
        let (port, bit) = Self::line(irq);
        self.ports.inb(port) & (1 << bit) != 0
    }

    fn eoi(&self, irq: Irq) {
        if irq.is_slave() {
            self.ports.outb(SLAVE, OCW2_EOI);
        }
        self.ports.outb(MASTER, OCW2_EOI);
    }

    /// IRQ 7 and 15 can be raised by noise and withdrawn before the CPU acknowledges them; the controller then reports the lowest
    /// priority line with its in-service bit clear. Other lines are never spurious.
    fn is_spurious(&self, irq: Irq) -> bool {
        match irq.line() {
            7 => self.in_service(MASTER) & 0x80 == 0,
            15 => self.in_service(SLAVE) & 0x80 == 0,
            _ => false,
        }
    }

    /// Nothing for a spurious IRQ 7; the master's end-of-interrupt for a spurious IRQ 15, because the master did see IRQ 2.
    fn eoi_spurious(&self, irq: Irq) {
        if irq.is_slave() {
            self.eoi_master();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use desk_hal_mock::{Access, MockPortBus};

    #[test]
    fn native_init_matches_the_kernel_with_its_delays() {
        // kernel/src/pic.rs: eight initialisation words each followed by io_wait (a write to port 0x80), then the masks 0xFE, 0xFF.
        let words = [
            (0x20, 0x11),
            (0xA0, 0x11),
            (0x21, 0x20),
            (0xA1, 0x28),
            (0x21, 0x04),
            (0xA1, 0x02),
            (0x21, 0x01),
            (0xA1, 0x01),
        ];
        let mut script: Vec<Access> = words
            .iter()
            .flat_map(|&(p, v)| [Access::outb(p, v), Access::outb(0x80, 0)])
            .collect();
        script.extend([Access::outb(0x21, 0xFE), Access::outb(0xA1, 0xFF)]);
        let bus = MockPortBus::scripted(script);
        bus.record_post_delays(true);
        Pic8259::new(&bus).init(REMAPPED_BASES, 0xFFFE);
        assert!(bus.script_done());
    }

    #[test]
    fn masking_reads_modifies_and_writes_one_bit() {
        let bus = MockPortBus::scripted([
            Access::inb(0x21, 0xFE),
            Access::outb(0x21, 0xFF), // mask IRQ 0
            Access::inb(0xA1, 0xFF),
            Access::outb(0xA1, 0xEF), // unmask IRQ 12 on the slave...
            Access::inb(0x21, 0xFF),
            Access::outb(0x21, 0xFB), // ...and the cascade on the master
            Access::inb(0xA1, 0xEF),
        ]);
        let pic = Pic8259::new(&bus);
        pic.mask(Irq::TIMER);
        pic.unmask(Irq::MOUSE);
        assert!(!pic.is_masked(Irq::MOUSE));
        assert!(bus.script_done());
    }

    #[test]
    fn a_slave_line_is_acknowledged_on_both_controllers() {
        let bus = MockPortBus::new();
        let pic = Pic8259::new(&bus);
        pic.eoi(Irq::KEYBOARD);
        pic.eoi(Irq::MOUSE);
        assert_eq!(bus.writes(), [(0x20, 0x20), (0xA0, 0x20), (0x20, 0x20)]);
    }

    #[test]
    fn spurious_interrupts_are_told_apart_by_the_in_service_register() {
        let bus = MockPortBus::scripted([
            Access::outb(0x20, 0x0B),
            Access::inb(0x20, 0x00), // IRQ 7 not in service: spurious
            Access::outb(0x20, 0x0B),
            Access::inb(0x20, 0x80), // in service: real
            Access::outb(0xA0, 0x0B),
            Access::inb(0xA0, 0x00), // IRQ 15 spurious
        ]);
        let pic = Pic8259::new(&bus);
        assert!(pic.is_spurious(Irq::LPT1_OR_SPURIOUS));
        assert!(!pic.is_spurious(Irq::LPT1_OR_SPURIOUS));
        assert!(pic.is_spurious(Irq::IDE_SECONDARY_OR_SPURIOUS));
        assert!(!pic.is_spurious(Irq::TIMER), "other lines read nothing");
        assert!(bus.script_done());
    }

    #[test]
    fn a_spurious_irq_15_is_acknowledged_on_the_master_only() {
        let bus = MockPortBus::new();
        let pic = Pic8259::new(&bus);
        pic.eoi_spurious(Irq::LPT1_OR_SPURIOUS);
        assert!(bus.writes().is_empty(), "a spurious IRQ 7 gets no end-of-interrupt");
        pic.eoi_spurious(Irq::IDE_SECONDARY_OR_SPURIOUS);
        assert_eq!(bus.writes(), [(0x20, 0x20)]);
    }
}
