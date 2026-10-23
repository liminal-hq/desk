// A port bus that records and replays I/O.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::fmt;

use desk_hal::port::{PortIo, POST_PORT};

/// The width of a port access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    Byte,
    Word,
    Dword,
}

impl Width {
    fn mask(self) -> u32 {
        match self {
            Width::Byte => 0xFF,
            Width::Word => 0xFFFF,
            Width::Dword => 0xFFFF_FFFF,
        }
    }
}

/// One port access as it happened (or, in a script, as it is expected to happen).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Access {
    In { port: u16, width: Width, value: u32 },
    Out { port: u16, width: Width, value: u32 },
}

impl Access {
    pub const fn inb(port: u16, value: u8) -> Access {
        Access::In {
            port,
            width: Width::Byte,
            value: value as u32,
        }
    }
    pub const fn outb(port: u16, value: u8) -> Access {
        Access::Out {
            port,
            width: Width::Byte,
            value: value as u32,
        }
    }
    pub fn port(&self) -> u16 {
        match *self {
            Access::In { port, .. } | Access::Out { port, .. } => port,
        }
    }
}

impl fmt::Debug for Access {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (dir, port, width, value) = match *self {
            Access::In { port, width, value } => ("in ", port, width, value),
            Access::Out { port, width, value } => ("out", port, width, value),
        };
        let w = match width {
            Width::Byte => 'b',
            Width::Word => 'w',
            Width::Dword => 'l',
        };
        write!(f, "{dir}{w} {port:#05x} {value:#04x}")
    }
}

/// A model of a chip on the bus. `port` is the absolute port; the model is attached to a range of them.
pub trait Device {
    fn read(&mut self, port: u16, width: Width) -> u32;
    fn write(&mut self, port: u16, width: Width, value: u32);
}

struct Mapping {
    first: u16,
    last: u16,
    device: Rc<RefCell<dyn Device>>,
}

#[derive(Default)]
struct State {
    log: Vec<Access>,
    script: Option<VecDeque<Access>>,
    queued: BTreeMap<u16, VecDeque<u32>>,
    devices: Vec<Mapping>,
    record_post: bool,
}

/// The mock bus. Reads are answered, in order of precedence, by the strict script (if one is set), by values queued for that port,
/// by the device attached to the port, or by a floating bus (all ones). Every access is logged, except writes to the POST delay
/// port, which are counted out unless `record_post_delays` is on (they are timing, not behaviour).
#[derive(Default)]
pub struct MockPortBus {
    state: RefCell<State>,
}

impl MockPortBus {
    pub fn new() -> MockPortBus {
        MockPortBus::default()
    }

    /// A bus that replays `script` strictly: each access must be the next one in it, and reads return the scripted value. A
    /// mismatch panics with the position and both accesses, which is the test failure.
    pub fn scripted(script: impl IntoIterator<Item = Access>) -> MockPortBus {
        let bus = MockPortBus::new();
        bus.state.borrow_mut().script = Some(script.into_iter().collect());
        bus
    }

    /// Whether every scripted access has happened.
    pub fn script_done(&self) -> bool {
        self.state.borrow().script.as_ref().is_none_or(VecDeque::is_empty)
    }

    /// Logs writes to the POST port too (to check that a driver waits between accesses to a slow device).
    pub fn record_post_delays(&self, on: bool) {
        self.state.borrow_mut().record_post = on;
    }

    /// The next read of `port` returns `value` (queued values are used in order, before any device).
    pub fn queue_read(&self, port: u16, value: u32) {
        self.state.borrow_mut().queued.entry(port).or_default().push_back(value);
    }

    /// Attaches a device model to `first..=last` and returns a handle to inspect it.
    pub fn attach<D: Device + 'static>(&self, first: u16, last: u16, device: D) -> Rc<RefCell<D>> {
        let device = Rc::new(RefCell::new(device));
        self.state.borrow_mut().devices.push(Mapping {
            first,
            last,
            device: device.clone(),
        });
        device
    }

    /// Everything logged so far.
    pub fn log(&self) -> Vec<Access> {
        self.state.borrow().log.clone()
    }

    /// The logged accesses, emptying the log.
    pub fn take_log(&self) -> Vec<Access> {
        core::mem::take(&mut self.state.borrow_mut().log)
    }

    /// Only the writes, as (port, value) pairs.
    pub fn writes(&self) -> Vec<(u16, u32)> {
        self.state
            .borrow()
            .log
            .iter()
            .filter_map(|a| match *a {
                Access::Out { port, value, .. } => Some((port, value)),
                Access::In { .. } => None,
            })
            .collect()
    }

    fn device_for(&self, port: u16) -> Option<Rc<RefCell<dyn Device>>> {
        let st = self.state.borrow();
        st.devices
            .iter()
            .find(|m| (m.first..=m.last).contains(&port))
            .map(|m| m.device.clone())
    }

    fn check_script(&self, actual: Access) -> Option<u32> {
        let mut st = self.state.borrow_mut();
        let pos = st.log.len();
        let script = st.script.as_mut()?;
        let Some(expected) = script.pop_front() else {
            panic!("access {pos} {actual:?} is past the end of the script");
        };
        let matches = match (expected, actual) {
            (Access::In { port: p, width: w, .. }, Access::In { port, width, .. }) => p == port && w == width,
            (e, a) => e == a,
        };
        assert!(matches, "access {pos}: expected {expected:?}, got {actual:?}");
        match expected {
            Access::In { value, .. } => Some(value),
            Access::Out { .. } => Some(0),
        }
    }

    fn read(&self, port: u16, width: Width) -> u32 {
        let scripted = self.check_script(Access::In { port, width, value: 0 });
        // A statement of its own, so the borrow ends before a device model is called.
        let queued = match scripted {
            Some(_) => None,
            None => self.state.borrow_mut().queued.get_mut(&port).and_then(VecDeque::pop_front),
        };
        let raw = if let Some(v) = scripted.or(queued) {
            v
        } else if let Some(dev) = self.device_for(port) {
            dev.borrow_mut().read(port, width)
        } else {
            u32::MAX
        };
        let value = raw & width.mask();
        self.state.borrow_mut().log.push(Access::In { port, width, value });
        value
    }

    fn write(&self, port: u16, width: Width, value: u32) {
        let value = value & width.mask();
        let access = Access::Out { port, width, value };
        if port == POST_PORT && !self.state.borrow().record_post {
            return;
        }
        self.check_script(access);
        if let Some(dev) = self.device_for(port) {
            dev.borrow_mut().write(port, width, value);
        }
        self.state.borrow_mut().log.push(access);
    }
}

impl PortIo for MockPortBus {
    fn inb(&self, port: u16) -> u8 {
        self.read(port, Width::Byte) as u8
    }
    fn outb(&self, port: u16, value: u8) {
        self.write(port, Width::Byte, value.into())
    }
    fn inw(&self, port: u16) -> u16 {
        self.read(port, Width::Word) as u16
    }
    fn outw(&self, port: u16, value: u16) {
        self.write(port, Width::Word, value.into())
    }
    fn inl(&self, port: u16) -> u32 {
        self.read(port, Width::Dword)
    }
    fn outl(&self, port: u16, value: u32) {
        self.write(port, Width::Dword, value)
    }
}

/// So a test can box a device it does not need to inspect.
impl<D: Device + ?Sized> Device for Box<D> {
    fn read(&mut self, port: u16, width: Width) -> u32 {
        (**self).read(port, width)
    }
    fn write(&mut self, port: u16, width: Width, value: u32) {
        (**self).write(port, width, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Latch(u32);
    impl Device for Latch {
        fn read(&mut self, _: u16, _: Width) -> u32 {
            self.0
        }
        fn write(&mut self, _: u16, _: Width, value: u32) {
            self.0 = value;
        }
    }

    #[test]
    fn reads_come_from_the_queue_then_the_device_then_the_floating_bus() {
        let bus = MockPortBus::new();
        let latch = bus.attach(0x300, 0x301, Latch(0x42));
        bus.queue_read(0x300, 0x99);
        assert_eq!(bus.inb(0x300), 0x99);
        assert_eq!(bus.inb(0x300), 0x42);
        bus.outw(0x301, 0x1234);
        assert_eq!(latch.borrow().0, 0x1234);
        assert_eq!(bus.inb(0x200), 0xFF);
        assert_eq!(bus.inl(0x200), 0xFFFF_FFFF);
        bus.io_wait();
        assert_eq!(bus.log().len(), 5, "the POST delay is not logged by default");
        bus.record_post_delays(true);
        bus.io_wait();
        assert_eq!(bus.writes().last(), Some(&(POST_PORT, 0)));
    }

    #[test]
    fn a_script_is_replayed() {
        let bus = MockPortBus::scripted([Access::outb(0x43, 0x00), Access::inb(0x40, 0x34), Access::inb(0x40, 0x12)]);
        bus.outb(0x43, 0);
        assert_eq!(bus.inb(0x40), 0x34);
        assert!(!bus.script_done());
        assert_eq!(bus.inb(0x40), 0x12);
        assert!(bus.script_done());
    }

    #[test]
    #[should_panic(expected = "access 0: expected out")]
    fn a_script_rejects_the_wrong_access() {
        let bus = MockPortBus::scripted([Access::outb(0x43, 0x34)]);
        bus.outb(0x43, 0x36);
    }

    #[test]
    #[should_panic(expected = "past the end of the script")]
    fn a_script_rejects_an_extra_access() {
        let bus = MockPortBus::scripted([]);
        bus.inb(0x60);
    }
}
