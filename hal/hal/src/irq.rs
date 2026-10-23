// Interrupts: the lines, the controller that masks and acknowledges them, and how a handler is registered.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A handler is a function pointer plus a context pointer, never a closure: the platform's assembly stub has to call it from an
// interrupt with nothing but two words it stored at registration, and under a DPMI host both words and everything they reach must
// sit in locked memory. `IrqHandler::from_static` builds the pair from a `&'static mut T`, so ordinary driver code needs no
// `unsafe`: the context moves to the handler for good, and the handler is the only code that touches it.
//
// Rules for a handler (`docs/HAL.md`, "Interrupts"): no allocation, no lock the core might hold, bounded work, no DOS or BIOS
// call, no floating point, and anything shared with the core goes through an atomic or an `SpscRing`. The platform guarantees a
// handler is never re-entered: it runs with interrupts off and the end-of-interrupt is sent after it returns.

use crate::HalError;

/// One of the sixteen ISA interrupt lines of the two cascaded 8259s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Irq(u8);

impl Irq {
    pub const TIMER: Irq = Irq(0);
    pub const KEYBOARD: Irq = Irq(1);
    pub const CASCADE: Irq = Irq(2);
    pub const COM2: Irq = Irq(3);
    pub const COM1: Irq = Irq(4);
    pub const FLOPPY: Irq = Irq(6);
    pub const LPT1_OR_SPURIOUS: Irq = Irq(7);
    pub const RTC: Irq = Irq(8);
    pub const MOUSE: Irq = Irq(12);
    pub const FPU: Irq = Irq(13);
    pub const IDE_PRIMARY: Irq = Irq(14);
    pub const IDE_SECONDARY_OR_SPURIOUS: Irq = Irq(15);

    /// A line by number. Lines above 15 do not exist on an ISA PC.
    pub const fn new(line: u8) -> Result<Irq, HalError> {
        if line < 16 {
            Ok(Irq(line))
        } else {
            Err(HalError::InvalidArgument)
        }
    }

    pub const fn line(self) -> u8 {
        self.0
    }

    /// True for lines 8 to 15, which sit on the slave controller and reach the CPU through IRQ 2.
    pub const fn is_slave(self) -> bool {
        self.0 >= 8
    }
}

/// The interrupt controller: masking and end-of-interrupt. Implemented by the 8259 driver for both platforms (under DOS without
/// re-initialising it), and by the mock.
///
/// `mask` and `unmask` are a read-modify-write of a mask register shared by every line on the controller, so an interrupt handler
/// (ours, the BIOS's or a TSR's) that changes the mask between the read and the write would have its change lost. Callers outside
/// interrupt context call them with interrupts off (the platform's critical section); a driver has no way to turn interrupts off
/// itself, by design.
pub trait InterruptController {
    /// Stops the line from interrupting.
    fn mask(&self, irq: Irq);
    /// Lets the line interrupt.
    fn unmask(&self, irq: Irq);
    fn is_masked(&self, irq: Irq) -> bool;
    /// Acknowledges the interrupt being serviced on the line. A platform calls this after the handler returns; drivers never do,
    /// because under DOS a chained BIOS handler sends it instead, and a second one would acknowledge a different interrupt.
    fn eoi(&self, irq: Irq);
    /// True if an interrupt on IRQ 7 or 15 is spurious (the controller's in-service bit is clear), in which case it must get no
    /// end-of-interrupt on that controller. Controllers without spurious interrupts return false.
    fn is_spurious(&self, irq: Irq) -> bool {
        let _ = irq;
        false
    }
}

/// The work an interrupt does. Implemented by a driver's interrupt-side half (a tick counter, a scancode reader).
pub trait IrqService {
    /// Runs in interrupt context; see the rules at the top of this module.
    fn on_irq(&mut self);
}

/// A registered handler: what a platform's interrupt stub calls. Two words, so the stub can keep it in a locked table.
#[derive(Clone, Copy, Debug)]
pub struct IrqHandler {
    func: unsafe fn(*mut ()),
    ctx: *mut (),
}

// SAFETY: an `IrqHandler` is built either by `from_static`, from a `&'static mut T` with `T: Send` whose ownership passes to the
// handler, or by `from_raw`, whose caller promises the same. Moving the two words to the interrupt context is therefore moving a
// `Send` value, and the platform never runs the handler twice at once.
unsafe impl Send for IrqHandler {}
// SAFETY: as above; shared references to the pair only allow copying the two words, and `call` is `unsafe` with the
// one-caller-at-a-time contract.
unsafe impl Sync for IrqHandler {}

unsafe fn trampoline<T: IrqService>(ctx: *mut ()) {
    // SAFETY: `ctx` came from the `&'static mut T` given to `from_static`, and `call`'s contract rules out a second caller.
    let svc = unsafe { &mut *(ctx as *mut T) };
    svc.on_irq();
}

impl IrqHandler {
    /// The handler for a driver's interrupt-side state. The `&'static mut` moves the state to the handler: the core keeps no
    /// reference to it, so anything the two share has to be inside it behind a shared `&'static` (an atomic, a ring's producer).
    /// Under DPMI the whole image is locked before registering, which covers `'static` data.
    pub fn from_static<T: IrqService + Send + 'static>(svc: &'static mut T) -> IrqHandler {
        IrqHandler {
            func: trampoline::<T>,
            ctx: svc as *mut T as *mut (),
        }
    }

    /// A handler from raw parts, for a platform's own handlers.
    ///
    /// # Safety
    ///
    /// `func(ctx)` must be sound to call from interrupt context at any time until the handler is unregistered, `ctx` must stay
    /// valid (and locked under DPMI) for that long, and nothing else may use what `ctx` points to without synchronisation.
    pub const unsafe fn from_raw(func: unsafe fn(*mut ()), ctx: *mut ()) -> IrqHandler {
        IrqHandler { func, ctx }
    }

    /// Runs the handler.
    ///
    /// # Safety
    ///
    /// Only the platform's dispatch calls this, and never twice at once for the same handler (interrupts are off while it runs).
    pub unsafe fn call(&self) {
        // SAFETY: the pair was built by `from_static` or `from_raw` under their contracts; the caller rules out re-entry.
        unsafe { (self.func)(self.ctx) }
    }
}

/// What happens after our handler returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chain {
    /// The platform sends the end-of-interrupt. The native kernel's only mode, and the hosted mode for a line Desk owns outright
    /// (the keyboard, the PS/2 mouse).
    None,
    /// The previous owner of the vector runs next and acknowledges the interrupt (under DOS, the BIOS's timer handler, which keeps
    /// the DOS clock). `Unsupported` where there is no previous owner.
    Previous,
}

/// Installing and removing handlers. The platform owns the vector table (the IDT natively, DPMI 0204h/0205h under DOS).
pub trait IrqRegistry {
    /// Installs `handler` on `irq` and unmasks the line. `Busy` if the line already has a handler of ours. Under DOS the platform
    /// also puts the removal on the exit clean-up list, so a panic cannot leave the vector pointing into a dead program.
    fn register(&self, irq: Irq, handler: IrqHandler, chain: Chain) -> Result<(), HalError>;
    /// Removes our handler and restores what was there before (the previous vector and its mask state under DOS; masked natively).
    fn unregister(&self, irq: Irq) -> Result<(), HalError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    use core::sync::atomic::{AtomicU32, Ordering};

    /// The handler's own state (`seen`) and what it shares with the core (`shared`, behind a `&'static`).
    struct Count {
        seen: u32,
        shared: &'static AtomicU32,
    }
    impl IrqService for Count {
        fn on_irq(&mut self) {
            self.seen += 1;
            self.shared.store(self.seen, Ordering::Release);
        }
    }

    #[test]
    fn lines_above_fifteen_do_not_exist() {
        assert_eq!(Irq::new(15).map(Irq::line), Ok(15));
        assert_eq!(Irq::new(16), Err(HalError::InvalidArgument));
        assert!(Irq::MOUSE.is_slave() && !Irq::KEYBOARD.is_slave());
    }

    #[test]
    fn a_handler_reaches_its_context() {
        static SHARED: AtomicU32 = AtomicU32::new(0);
        let svc: &'static mut Count = Box::leak(Box::new(Count {
            seen: 0,
            shared: &SHARED,
        }));
        let h = IrqHandler::from_static(svc);
        // SAFETY: the test is the only caller.
        unsafe {
            h.call();
            h.call();
        }
        assert_eq!(SHARED.load(Ordering::Acquire), 2);
    }
}
