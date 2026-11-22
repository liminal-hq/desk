// A mock interrupt controller and handler registry that delivers interrupts the way a platform does.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::cell::RefCell;

use desk_hal::irq::{chains, Chain, InterruptController, Irq, IrqHandler, IrqRegistry};
use desk_hal::HalError;

struct State {
    mask: u16,
    handlers: [Option<(IrqHandler, Chain)>; 16],
    eois: Vec<Irq>,
    chained: Vec<Irq>,
    /// False for a platform with no previous owner of any vector, as the native kernel is.
    has_previous: bool,
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
                has_previous: true,
            }),
        }
    }
}

impl MockIrq {
    pub fn new() -> MockIrq {
        MockIrq::default()
    }

    /// A mock of the native platform: no line has a previous owner, so `register` refuses `Chain::Previous` and a handler's
    /// `Action::Chain` is treated as `Eoi`.
    pub fn native() -> MockIrq {
        let mock = MockIrq::default();
        mock.state.borrow_mut().has_previous = false;
        mock
    }

    /// Delivers an interrupt on `irq` as the platform's dispatch would: nothing if the line is masked or has no handler;
    /// otherwise the handler runs, then either the end-of-interrupt is sent or the previous owner runs and sends it (recorded in
    /// `chained`). The previous owner runs if the line was registered with `Chain::Previous` or the handler returned
    /// `Action::Chain` (`chains`). Returns whether the handler ran.
    pub fn raise(&self, irq: Irq) -> bool {
        let (entry, has_previous) = {
            let st = self.state.borrow();
            if st.mask & (1 << irq.line()) != 0 {
                return false;
            }
            (st.handlers[usize::from(irq.line())], st.has_previous)
        };
        let Some((handler, chain)) = entry else { return false };
        // SAFETY: the mock delivers one interrupt at a time, so the handler is never re-entered.
        let action = unsafe { handler.call() };
        if chains(chain, action, has_previous) {
            self.state.borrow_mut().chained.push(irq);
        } else {
            self.eoi(irq);
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
        if chain == Chain::Previous && !self.state.borrow().has_previous {
            return Err(HalError::Unsupported);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;
    use core::sync::atomic::{AtomicU32, Ordering};
    use desk_hal::irq::{Action, DecidingIrqService, IrqService};

    /// Asks to chain on every second interrupt, starting with the first.
    struct Alternate(u32);
    impl DecidingIrqService for Alternate {
        fn on_irq(&mut self) -> Action {
            self.0 += 1;
            if self.0 % 2 == 1 {
                Action::Chain
            } else {
                Action::Eoi
            }
        }
    }

    struct Plain(&'static AtomicU32);
    impl IrqService for Plain {
        fn on_irq(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn alternating() -> IrqHandler {
        IrqHandler::from_deciding(Box::leak(Box::new(Alternate(0))))
    }

    #[test]
    fn a_handler_alternating_chain_and_eoi_is_recorded_so() {
        let irq = MockIrq::new();
        irq.register(Irq::TIMER, alternating(), Chain::None).unwrap();
        for _ in 0..4 {
            assert!(irq.raise(Irq::TIMER));
        }
        assert_eq!(irq.chained(), [Irq::TIMER, Irq::TIMER]);
        assert_eq!(irq.eois(), [Irq::TIMER, Irq::TIMER]);
    }

    #[test]
    fn a_line_registered_to_chain_keeps_chaining_whatever_the_handler_returns() {
        let irq = MockIrq::new();
        irq.register(Irq::TIMER, alternating(), Chain::Previous).unwrap();
        for _ in 0..4 {
            irq.raise(Irq::TIMER);
        }
        assert_eq!(irq.chained().len(), 4);
        assert!(irq.eois().is_empty());
    }

    #[test]
    fn an_irq_service_still_gets_the_chain_it_was_registered_with() {
        static HITS: AtomicU32 = AtomicU32::new(0);
        let irq = MockIrq::new();
        let plain = || IrqHandler::from_static(Box::leak(Box::new(Plain(&HITS))));
        irq.register(Irq::TIMER, plain(), Chain::Previous).unwrap();
        irq.register(Irq::KEYBOARD, plain(), Chain::None).unwrap();
        irq.raise(Irq::TIMER);
        irq.raise(Irq::KEYBOARD);
        assert_eq!(HITS.load(Ordering::Relaxed), 2);
        assert_eq!(irq.chained(), [Irq::TIMER]);
        assert_eq!(irq.eois(), [Irq::KEYBOARD]);
    }

    #[test]
    fn with_no_previous_owner_chain_is_refused_at_registration_and_treated_as_eoi_from_a_handler() {
        let irq = MockIrq::native();
        assert_eq!(irq.register(Irq::TIMER, alternating(), Chain::Previous), Err(HalError::Unsupported));
        irq.register(Irq::TIMER, alternating(), Chain::None).unwrap();
        for _ in 0..4 {
            irq.raise(Irq::TIMER);
        }
        assert!(irq.chained().is_empty());
        assert_eq!(irq.eois().len(), 4);
    }
}
