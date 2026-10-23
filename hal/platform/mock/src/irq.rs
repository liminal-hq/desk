// A mock interrupt controller and handler registry that delivers interrupts the way a platform does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::cell::RefCell;

use desk_hal::irq::{Chain, InterruptController, Irq, IrqHandler, IrqRegistry};
use desk_hal::HalError;

struct State {
    mask: u16,
    handlers: [Option<(IrqHandler, Chain)>; 16],
    eois: Vec<Irq>,
    chained: Vec<Irq>,
}

/// Starts with every line masked, as the platforms leave the lines Desk has not claimed.
pub struct MockIrq {
    state: RefCell<State>,
}

impl Default for MockIrq {
    fn default() -> Self {
        MockIrq {
            state: RefCell::new(State {
                mask: 0xFFFF,
                handlers: [None; 16],
                eois: Vec::new(),
                chained: Vec::new(),
            }),
        }
    }
}

impl MockIrq {
    pub fn new() -> MockIrq {
        MockIrq::default()
    }

    /// Delivers an interrupt on `irq` as the platform's dispatch would: nothing if the line is masked or has no handler;
    /// otherwise the handler runs, then either the end-of-interrupt is sent (`Chain::None`) or the previous owner runs and sends it
    /// (`Chain::Previous`, recorded in `chained`). Returns whether the handler ran.
    pub fn raise(&self, irq: Irq) -> bool {
        let entry = {
            let st = self.state.borrow();
            if st.mask & (1 << irq.line()) != 0 {
                return false;
            }
            st.handlers[usize::from(irq.line())]
        };
        let Some((handler, chain)) = entry else { return false };
        // SAFETY: the mock delivers one interrupt at a time, so the handler is never re-entered.
        unsafe { handler.call() };
        match chain {
            Chain::None => self.eoi(irq),
            Chain::Previous => self.state.borrow_mut().chained.push(irq),
        }
        true
    }

    /// The end-of-interrupts sent so far, in order.
    pub fn eois(&self) -> Vec<Irq> {
        self.state.borrow().eois.clone()
    }

    /// The interrupts passed on to a previous owner.
    pub fn chained(&self) -> Vec<Irq> {
        self.state.borrow().chained.clone()
    }
}

impl InterruptController for MockIrq {
    fn mask(&self, irq: Irq) {
        self.state.borrow_mut().mask |= 1 << irq.line();
    }
    fn unmask(&self, irq: Irq) {
        self.state.borrow_mut().mask &= !(1 << irq.line());
    }
    fn is_masked(&self, irq: Irq) -> bool {
        self.state.borrow().mask & (1 << irq.line()) != 0
    }
    fn eoi(&self, irq: Irq) {
        self.state.borrow_mut().eois.push(irq);
    }
}

impl IrqRegistry for MockIrq {
    fn register(&self, irq: Irq, handler: IrqHandler, chain: Chain) -> Result<(), HalError> {
        {
            let slot = &mut self.state.borrow_mut().handlers[usize::from(irq.line())];
            if slot.is_some() {
                return Err(HalError::Busy);
            }
            *slot = Some((handler, chain));
        }
        self.unmask(irq);
        Ok(())
    }

    fn unregister(&self, irq: Irq) -> Result<(), HalError> {
        if self.state.borrow_mut().handlers[usize::from(irq.line())].take().is_none() {
            return Err(HalError::InvalidArgument);
        }
        self.mask(irq);
        Ok(())
    }
}
