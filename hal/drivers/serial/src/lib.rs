// The 16550 UART: 8N1 at a chosen rate, polled, written once against the HAL's port I/O.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// The reference is the native kernel's `kernel/src/serial.rs`. With `COM1` and 115,200 baud, `init` writes the same seven port
// values in the same order and `write_byte` polls the line status the same bounded number of times, then writes the byte whether
// or not the transmitter came free, so a missing UART costs time but never hangs the machine. Beyond the kernel's version: any of
// the four standard bases, any rate that divides 115,200, a loopback `probe`, and a polled `read_byte`. Interrupt-driven
// transmission and reception come later, as an `IrqService` feeding an `SpscRing`.
#![cfg_attr(not(any(test, feature = "std")), no_std)]

use core::fmt;

use desk_hal::{DebugOut, HalError, PortIo};

pub const COM1: u16 = 0x3F8;
pub const COM2: u16 = 0x2F8;
pub const COM3: u16 = 0x3E8;
pub const COM4: u16 = 0x2E8;

/// The UART's clock divided by 16: the rate with a divisor of 1.
pub const MAX_BAUD: u32 = 115_200;

/// How many times `write_byte` reads the line status before writing anyway (the kernel's bound); `probe` uses the same bound to
/// wait for its byte to come back.
pub const WRITE_POLLS: u32 = 100_000;

/// The receive FIFO's depth, and so the most stale bytes `probe` drains before it starts.
const RX_FIFO: u32 = 16;

// Register offsets from the base port.
const DATA: u16 = 0; // receive and transmit buffer; divisor low with DLAB
const IER: u16 = 1; // interrupt enable; divisor high with DLAB
const FCR: u16 = 2; // FIFO control (write)
const LCR: u16 = 3; // line control
const MCR: u16 = 4; // modem control
const LSR: u16 = 5; // line status

const LCR_DLAB: u8 = 0x80;
const LCR_8N1: u8 = 0x03;
const FCR_ENABLE_CLEAR_14: u8 = 0xC7; // enable and clear both FIFOs, 14-byte receive trigger
const MCR_DTR_RTS_OUT2: u8 = 0x0B; // OUT2 gates the UART's interrupt onto the bus on a PC
const MCR_LOOPBACK: u8 = 0x1E; // loopback, with OUT1, OUT2 and RTS
const LSR_DATA_READY: u8 = 0x01;
const LSR_THR_EMPTY: u8 = 0x20;
const PROBE_BYTE: u8 = 0xAE;

/// The divisor latch value for `baud`: `InvalidArgument` unless `baud` divides 115,200 exactly and the divisor fits the 16-bit
/// latch (so 0, 1, rates above 115,200 and odd rates such as 100,000 are refused rather than rounded to a rate the other end does
/// not expect).
pub const fn divisor_for(baud: u32) -> Result<u16, HalError> {
    if baud == 0 || baud > MAX_BAUD || !MAX_BAUD.is_multiple_of(baud) || MAX_BAUD / baud > u16::MAX as u32 {
        return Err(HalError::InvalidArgument);
    }
    Ok((MAX_BAUD / baud) as u16)
}

/// One UART at `base`, reached through `ports`.
#[derive(Clone, Copy, Debug)]
pub struct Serial16550<P> {
    ports: P,
    base: u16,
}

impl<P: PortIo> Serial16550<P> {
    /// Names the UART; touches no hardware.
    pub const fn new(ports: P, base: u16) -> Self {
        Serial16550 { ports, base }
    }

    pub const fn base(&self) -> u16 {
        self.base
    }

    /// Programs the UART: interrupts off, the divisor for `baud`, 8 data bits, no parity, one stop bit, FIFOs on and cleared, DTR,
    /// RTS and OUT2 set.
    pub fn init(&self, baud: u32) -> Result<(), HalError> {
        let divisor = divisor_for(baud)?;
        let p = &self.ports;
        p.outb(self.base + IER, 0x00);
        p.outb(self.base + LCR, LCR_DLAB);
        p.outb(self.base + DATA, divisor as u8);
        p.outb(self.base + IER, (divisor >> 8) as u8);
        p.outb(self.base + LCR, LCR_8N1);
        p.outb(self.base + FCR, FCR_ENABLE_CLEAR_14);
        p.outb(self.base + MCR, MCR_DTR_RTS_OUT2);
        Ok(())
    }

    /// Checks that a UART answers: in loopback mode a byte sent must come back. Stale received bytes are drained first, and the
    /// echo is awaited on the line status (it takes one character time, about 87 µs at 115,200 baud), with the same bound as
    /// `write_byte`. Leaves the modem control as `init` sets it. Call after `init`. `NoDevice` if nothing answers (a floating bus
    /// reads all ones, so it looks ready but echoes 0xFF).
    pub fn probe(&self) -> Result<(), HalError> {
        let p = &self.ports;
        p.outb(self.base + MCR, MCR_LOOPBACK);
        for _ in 0..RX_FIFO {
            if self.read_byte().is_none() {
                break;
            }
        }
        p.outb(self.base + DATA, PROBE_BYTE);
        let mut echoed = None;
        for _ in 0..WRITE_POLLS {
            echoed = self.read_byte();
            if echoed.is_some() {
                break;
            }
        }
        p.outb(self.base + MCR, MCR_DTR_RTS_OUT2);
        match echoed {
            Some(PROBE_BYTE) => Ok(()),
            _ => Err(HalError::NoDevice),
        }
    }

    /// Whether the transmitter can take a byte now.
    pub fn can_write(&self) -> bool {
        self.ports.inb(self.base + LSR) & LSR_THR_EMPTY != 0
    }

    /// Sends a byte, waiting a bounded time for the transmitter. As in the kernel, the byte is written even if the wait ran out, so
    /// a missing or stuck UART loses output instead of stalling the machine.
    pub fn write_byte(&self, byte: u8) {
        for _ in 0..WRITE_POLLS {
            if self.can_write() {
                break;
            }
        }
        self.ports.outb(self.base + DATA, byte);
    }

    /// A received byte, if one is waiting. Never blocks.
    pub fn read_byte(&self) -> Option<u8> {
        if self.ports.inb(self.base + LSR) & LSR_DATA_READY != 0 {
            Some(self.ports.inb(self.base + DATA))
        } else {
            None
        }
    }
}

impl<P: PortIo> fmt::Write for Serial16550<P> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        s.bytes().for_each(|b| self.write_byte(b));
        Ok(())
    }
}

impl<P: PortIo> DebugOut for Serial16550<P> {
    fn write_bytes(&self, bytes: &[u8]) {
        bytes.iter().for_each(|&b| self.write_byte(b));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt::Write;
    use desk_hal_mock::devices::Uart16550;
    use desk_hal_mock::{Access, MockPortBus};

    /// The port writes `kernel/src/serial.rs` makes in `init`, in order.
    const KERNEL_INIT: [(u16, u8); 7] = [
        (0x3F9, 0x00),
        (0x3FB, 0x80),
        (0x3F8, 0x01),
        (0x3F9, 0x00),
        (0x3FB, 0x03),
        (0x3FA, 0xC7),
        (0x3FC, 0x0B),
    ];

    #[test]
    fn init_matches_the_kernel_byte_for_byte() {
        let bus = MockPortBus::scripted(KERNEL_INIT.map(|(p, v)| Access::outb(p, v)));
        Serial16550::new(&bus, COM1).init(115_200).unwrap();
        assert!(bus.script_done());
    }

    #[test]
    fn the_divisor_follows_the_rate() {
        assert_eq!(divisor_for(115_200), Ok(1));
        assert_eq!(divisor_for(9_600), Ok(12));
        assert_eq!(divisor_for(2), Ok(57_600));
        for bad in [0, 1, 100_000, 230_400, 7] {
            assert_eq!(divisor_for(bad), Err(HalError::InvalidArgument), "{bad} baud");
        }
        let bus = MockPortBus::new();
        let uart = bus.attach(COM2, COM2 + 7, Uart16550::new(COM2));
        Serial16550::new(&bus, COM2).init(9_600).unwrap();
        let u = uart.borrow();
        assert_eq!((u.divisor, u.lcr, u.fcr, u.mcr, u.ier), (12, 0x03, 0xC7, 0x0B, 0));
        assert!(Serial16550::new(&bus, COM2).init(100_000).is_err());
        assert_eq!(bus.writes().len(), 7, "a refused rate writes nothing");
    }

    #[test]
    fn text_reaches_the_transmitter() {
        let bus = MockPortBus::new();
        let uart = bus.attach(COM1, COM1 + 7, Uart16550::new(COM1));
        let mut s = Serial16550::new(&bus, COM1);
        s.init(115_200).unwrap();
        writeln!(s, "desk {}", 42).unwrap();
        s.write_bytes(b"ok");
        assert_eq!(uart.borrow().sent, b"desk 42\nok");
    }

    #[test]
    fn a_stuck_transmitter_costs_a_bounded_wait_and_the_byte_is_still_written() {
        let bus = MockPortBus::new();
        let uart = bus.attach(COM1, COM1 + 7, Uart16550::new(COM1));
        uart.borrow_mut().busy = true;
        Serial16550::new(&bus, COM1).write_byte(b'x');
        let log = bus.log();
        assert_eq!(
            log.iter().filter(|a| **a == Access::inb(COM1 + 5, 0x00)).count(),
            WRITE_POLLS as usize
        );
        assert_eq!(log.last(), Some(&Access::outb(COM1, b'x')));
        assert_eq!(uart.borrow().sent, b"x");
    }

    #[test]
    fn the_wait_ends_as_soon_as_the_transmitter_is_free() {
        let bus = MockPortBus::scripted([
            Access::inb(COM1 + 5, 0x00),
            Access::inb(COM1 + 5, 0x00),
            Access::inb(COM1 + 5, 0x20),
            Access::outb(COM1, b'y'),
        ]);
        Serial16550::new(&bus, COM1).write_byte(b'y');
        assert!(bus.script_done());
    }

    #[test]
    fn probe_finds_a_uart_and_not_an_empty_slot() {
        let bus = MockPortBus::new();
        let uart = bus.attach(COM1, COM1 + 7, Uart16550::new(COM1));
        let s = Serial16550::new(&bus, COM1);
        s.init(115_200).unwrap();
        assert_eq!(s.probe(), Ok(()));
        assert_eq!(uart.borrow().mcr, 0x0B, "the probe puts the modem control back");
        assert!(uart.borrow().sent.is_empty(), "the probe byte stays inside the chip");
        assert_eq!(Serial16550::new(&bus, COM3).probe(), Err(HalError::NoDevice));
    }

    #[test]
    fn probe_drains_stale_bytes_and_waits_for_the_echo() {
        let bus = MockPortBus::new();
        let uart = bus.attach(COM1, COM1 + 7, Uart16550::new(COM1));
        let s = Serial16550::new(&bus, COM1);
        s.init(115_200).unwrap();
        uart.borrow_mut().incoming.extend(b"old");
        assert_eq!(
            s.probe(),
            Ok(()),
            "bytes received before the probe are not mistaken for the echo"
        );

        // The echo arrives one character time later: the line status says "no data" twice first.
        let bus = MockPortBus::scripted([
            Access::outb(COM1 + 4, 0x1E),
            Access::inb(COM1 + 5, 0x60), // nothing stale
            Access::outb(COM1, 0xAE),
            Access::inb(COM1 + 5, 0x60),
            Access::inb(COM1 + 5, 0x60),
            Access::inb(COM1 + 5, 0x61),
            Access::inb(COM1, 0xAE),
            Access::outb(COM1 + 4, 0x0B),
        ]);
        assert_eq!(Serial16550::new(&bus, COM1).probe(), Ok(()));
        assert!(bus.script_done());
    }

    #[test]
    fn received_bytes_are_read_without_blocking() {
        let bus = MockPortBus::new();
        let uart = bus.attach(COM1, COM1 + 7, Uart16550::new(COM1));
        let s = Serial16550::new(&bus, COM1);
        s.init(115_200).unwrap();
        assert_eq!(s.read_byte(), None);
        uart.borrow_mut().incoming.extend(b"hi");
        assert_eq!((s.read_byte(), s.read_byte(), s.read_byte()), (Some(b'h'), Some(b'i'), None));
    }
}
