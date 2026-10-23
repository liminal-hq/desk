// The native platform for the shared HAL: the real port bus, the hardware drivers as statics, and the interrupt registry.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Drivers live in `hal/` and are written once; what is specific to the kernel is here: the one `unsafe` step that makes the port
// bus, and the table of interrupt handlers the IDT stubs dispatch through (`docs/HAL.md`, "Interrupts"). A handler is a function
// pointer plus a context pointer. The kernel, not the driver, sends the end-of-interrupt after the handler returns.

use crate::{idt, sync};
use core::cell::UnsafeCell;
use desk_hal::irq::{Chain, InterruptController, Irq, IrqHandler, IrqRegistry};
use desk_hal::x86::RawPorts;
use desk_hal::HalError;
use desk_pic::Pic8259;
use desk_pit::PitClock;
use desk_serial::Serial16550;

/// The machine's port bus.
// SAFETY: the kernel runs in ring 0, where `in` and `out` are permitted, and it is the only code that holds the bus.
pub static PORTS: RawPorts = unsafe { RawPorts::new() };

pub static PIC: Pic8259<RawPorts> = Pic8259::new(PORTS);
pub static SERIAL: Serial16550<RawPorts> = Serial16550::new(PORTS, desk_serial::COM1);
pub static CLOCK: PitClock = PitClock::new();

/// The handlers of IRQ 0 to 15. Written with interrupts off by `register`, read by `irq_dispatch` in interrupt context.
pub struct IrqTable {
    handlers: UnsafeCell<[Option<IrqHandler>; 16]>,
}

// SAFETY: one processor. Writers run inside `without_interrupts`, so an interrupt (the only other reader) never sees a
// half-written entry, and an entry is a pair of words that is copied out whole.
unsafe impl Sync for IrqTable {}

pub static IRQS: IrqTable = IrqTable { handlers: UnsafeCell::new([None; 16]) };

impl IrqRegistry for IrqTable {
    /// Installs the handler and the IDT gate for the line. The kernel always sends the end-of-interrupt itself, so only
    /// `Chain::None` is accepted. `Busy` if the line already has a handler.
    fn register(&self, irq: Irq, handler: IrqHandler, chain: Chain) -> Result<(), HalError> {
        if chain != Chain::None {
            return Err(HalError::Unsupported);
        }
        sync::without_interrupts(|| {
            // SAFETY: interrupts are off, so nothing else touches the table.
            let table = unsafe { &mut *self.handlers.get() };
            let slot = &mut table[usize::from(irq.line())];
            if slot.is_some() {
                return Err(HalError::Busy);
            }
            *slot = Some(handler);
            idt::set_irq_gate(irq.line());
            Ok(())
        })
    }

    fn unregister(&self, irq: Irq) -> Result<(), HalError> {
        sync::without_interrupts(|| {
            // SAFETY: as in `register`.
            let table = unsafe { &mut *self.handlers.get() };
            match table[usize::from(irq.line())].take() {
                Some(_) => Ok(()),
                None => Err(HalError::InvalidArgument),
            }
        })
    }
}

/// Called by the IDT stub of hardware interrupt `line`, with interrupts off. A spurious IRQ 7 or 15 runs no handler and gets
/// the acknowledgement `eoi_spurious` says; anything else runs its handler (if any) and then gets the end-of-interrupt.
#[no_mangle]
extern "C" fn irq_dispatch(line: u32) {
    let Ok(irq) = Irq::new(line as u8) else { return };
    if PIC.is_spurious(irq) {
        PIC.eoi_spurious(irq);
        return;
    }
    // SAFETY: interrupts are off in the handler, so no writer is running; the entry is copied out whole.
    let handler = unsafe { (*IRQS.handlers.get())[usize::from(irq.line())] };
    if let Some(h) = handler {
        // SAFETY: the handler is registered under the `IrqHandler` contract, and the interrupt gate keeps it from re-entry.
        unsafe { h.call() };
    }
    PIC.eoi(irq);
}
