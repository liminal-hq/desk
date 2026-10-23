// The interrupt services: read a byte in the handler, queue it, decode nothing the core could decode later.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `KbdIrq` (IRQ 1) pushes raw scancode bytes into an `SpscRing<u8, 64>`; the decoder runs on the core side. `MouseIrq` (IRQ 12)
// assembles the three-byte (or, with a wheel, four-byte) packet, which has to be done byte by byte as they arrive, and pushes one
// `MouseEvent` per packet into an `SpscRing<MouseEvent, 32>`. Both follow the handler rules in `docs/HAL.md`: bounded work, no
// allocation, no lock, no call into anything but the two ports and the ring. They never send the end-of-interrupt; the platform
// does, after they return.
//
// Both look at the status register's source bit (bit 5) so that each takes only its own device's byte: if IRQ 1 and IRQ 12 are
// both pending and one handler read the other's byte, the packet or the scancode stream would lose a byte.

use core::sync::atomic::{AtomicU32, Ordering};

use desk_hal::irq::IrqService;
use desk_hal::ring::Producer;
use desk_hal::{MouseEvent, PortIo};

use crate::{status, DATA_PORT, STATUS_PORT};

/// The most bytes one interrupt reads. The controller's buffer holds one byte, so one is the usual number; more than one means
/// a second byte arrived before the CPU got here, and the loop saves an interrupt. The bound keeps a stuck status bit from
/// holding the handler.
const MAX_BYTES_PER_IRQ: u32 = 8;

/// Counters the handlers keep and the core reads. Single 32-bit words updated with `fetch_add`, so a handler never needs a lock.
#[derive(Debug, Default)]
pub struct IrqStats {
    /// Keyboard bytes queued.
    pub kbd_bytes: AtomicU32,
    /// Keyboard bytes read with a parity or timeout error flagged, and dropped.
    pub kbd_errors: AtomicU32,
    /// Mouse bytes consumed.
    pub mouse_bytes: AtomicU32,
    /// Mouse bytes dropped while looking for the start of a packet.
    pub mouse_resyncs: AtomicU32,
    /// Packets dropped because the device flagged an overflow in X or Y.
    pub mouse_overflows: AtomicU32,
    /// Mouse bytes read with a parity or timeout error flagged: the packet in progress is abandoned.
    pub mouse_errors: AtomicU32,
}

impl IrqStats {
    pub const fn new() -> IrqStats {
        IrqStats {
            kbd_bytes: AtomicU32::new(0),
            kbd_errors: AtomicU32::new(0),
            mouse_bytes: AtomicU32::new(0),
            mouse_resyncs: AtomicU32::new(0),
            mouse_overflows: AtomicU32::new(0),
            mouse_errors: AtomicU32::new(0),
        }
    }

    pub fn get(counter: &AtomicU32) -> u32 {
        counter.load(Ordering::Relaxed)
    }

    fn bump(counter: &AtomicU32) {
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

/// Reads the next byte from the wanted device, if its byte is in the output buffer. `Err(())` means a byte was read but flagged
/// by the link as bad.
fn take<P: PortIo>(ports: &P, aux: bool) -> Option<Result<u8, ()>> {
    let s = ports.inb(STATUS_PORT);
    if s & status::OUTPUT_FULL == 0 || (s & status::AUX_OUTPUT != 0) != aux {
        return None;
    }
    let byte = ports.inb(DATA_PORT);
    if s & (status::PARITY_ERROR | status::TIMEOUT_ERROR) != 0 {
        Some(Err(()))
    } else {
        Some(Ok(byte))
    }
}

/// IRQ 1: moves scancode bytes from the controller to the ring.
pub struct KbdIrq<P: 'static> {
    ports: P,
    tx: Producer<'static, u8, 64>,
    stats: &'static IrqStats,
}

impl<P: PortIo> KbdIrq<P> {
    pub fn new(ports: P, tx: Producer<'static, u8, 64>, stats: &'static IrqStats) -> Self {
        KbdIrq { ports, tx, stats }
    }
}

impl<P: PortIo + Send + 'static> IrqService for KbdIrq<P> {
    fn on_irq(&mut self) {
        for _ in 0..MAX_BYTES_PER_IRQ {
            match take(&self.ports, false) {
                None => break,
                Some(Ok(byte)) => {
                    // A full ring drops the byte and counts it in the ring itself.
                    let _ = self.tx.push(byte);
                    IrqStats::bump(&self.stats.kbd_bytes);
                }
                Some(Err(())) => IrqStats::bump(&self.stats.kbd_errors),
            }
        }
    }
}

/// IRQ 12: assembles mouse packets and queues one `MouseEvent` for each.
///
/// The first byte of a packet always has bit 3 set, which is how a stream that lost a byte finds its feet again: while the
/// first byte lacks bit 3 it is dropped, one byte at a time, until one has it. A first byte with the X or Y overflow bit set is
/// kept in step (the rest of the packet is consumed) but produces no event.
pub struct MouseIrq<P: 'static> {
    ports: P,
    tx: Producer<'static, MouseEvent, 32>,
    stats: &'static IrqStats,
    packet: [u8; 4],
    index: u8,
    len: u8,
}

impl<P: PortIo> MouseIrq<P> {
    /// `wheel` selects four-byte packets (the device ID was 3 after the set-up).
    pub fn new(ports: P, tx: Producer<'static, MouseEvent, 32>, stats: &'static IrqStats, wheel: bool) -> Self {
        MouseIrq {
            ports,
            tx,
            stats,
            packet: [0; 4],
            index: 0,
            len: if wheel { 4 } else { 3 },
        }
    }

    /// Takes one byte of the stream.
    fn feed(&mut self, byte: u8) {
        if self.index == 0 && byte & 0x08 == 0 {
            IrqStats::bump(&self.stats.mouse_resyncs);
            return;
        }
        self.packet[usize::from(self.index)] = byte;
        self.index += 1;
        if self.index < self.len {
            return;
        }
        self.index = 0;
        match decode(&self.packet, self.len == 4) {
            Some(event) => {
                let _ = self.tx.push(event);
            }
            None => IrqStats::bump(&self.stats.mouse_overflows),
        }
    }
}

/// Builds the event from a whole packet: nine-bit signed deltas (the sign bits are in the first byte), `dy` negated so down is
/// positive, the buttons from bits 0 to 2, the wheel byte as a signed count. `None` when an overflow bit is set, because the
/// delta is then meaningless.
pub fn decode(packet: &[u8; 4], wheel: bool) -> Option<MouseEvent> {
    let b0 = packet[0];
    if b0 & 0xC0 != 0 {
        return None;
    }
    let dx = i16::from(packet[1]) - if b0 & 0x10 != 0 { 256 } else { 0 };
    let dy_up = i16::from(packet[2]) - if b0 & 0x20 != 0 { 256 } else { 0 };
    Some(MouseEvent {
        dx,
        dy: -dy_up,
        buttons: b0 & 0x07,
        wheel: if wheel { packet[3] as i8 } else { 0 },
    })
}

impl<P: PortIo + Send + 'static> IrqService for MouseIrq<P> {
    fn on_irq(&mut self) {
        for _ in 0..MAX_BYTES_PER_IRQ {
            match take(&self.ports, true) {
                None => break,
                Some(Ok(byte)) => {
                    IrqStats::bump(&self.stats.mouse_bytes);
                    self.feed(byte);
                }
                Some(Err(())) => {
                    IrqStats::bump(&self.stats.mouse_errors);
                    self.index = 0;
                }
            }
        }
    }
}
