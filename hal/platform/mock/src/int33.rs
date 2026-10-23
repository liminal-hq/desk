// A model of a DOS mouse driver behind INT 33h, for the tests of `desk-int33`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// It answers the functions the driver crate calls the way the Microsoft driver, CTMOUSE and the DOSBox built-in driver do (functions
// 0000h to 0006h, 0007h, 0008h, 000Bh, 0011h, 0015h, 0021h and 0024h), keeps the position in a range and the motion in counters that a
// read clears, and records the AX of every call. Tests move the "hardware" with `move_by`, `press`, `release`, `click` (a click between two polls) and `scroll`; it counts the presses
// and releases of each button for functions 0005h and 0006h, and `without_counters` makes it a driver that does not.

use alloc::vec::Vec;
use core::cell::RefCell;

use desk_hal::{Bios, BiosRegs, HalError};

/// What the model is: no driver, or a driver with this many buttons and optionally the wheel API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Int33Kind {
    /// The vector in the table is 0000:0000.
    NoDriver,
    /// The vector points at an `IRET`, as a DOS BIOS leaves it when nothing hooked it.
    IretOnly,
    Driver {
        buttons: u16,
        wheel: bool,
    },
}

struct State {
    kind: Int33Kind,
    /// The Microsoft driver answers BX = FFFFh for two buttons; this makes the model do the same.
    ms_two_button_convention: bool,
    hide_counter: i16,
    x: i32,
    y: i32,
    range_x: (i32, i32),
    range_y: (i32, i32),
    buttons: u8,
    mickeys: (i32, i32),
    wheel: i32,
    /// Presses and releases of each button since the counter was last read, and where the pointer was at the last one.
    presses: [(u16, i32, i32); 3],
    releases: [(u16, i32, i32); 3],
    /// Whether functions 0005h and 0006h count; a driver without them leaves the registers as they came.
    counts: bool,
    calls: Vec<u16>,
}

impl State {
    fn reset(&mut self) {
        self.hide_counter = -1;
        self.range_x = (0, 639);
        self.range_y = (0, 199);
        self.x = 320;
        self.y = 100;
        self.mickeys = (0, 0);
        self.wheel = 0;
        self.presses = [(0, 0, 0); 3];
        self.releases = [(0, 0, 0); 3];
    }

    fn clamp(&mut self) {
        self.x = self.x.clamp(self.range_x.0, self.range_x.1);
        self.y = self.y.clamp(self.range_y.0, self.range_y.1);
    }
}

/// The driver. `Bios::int` answers vector 33h only; any other vector is `Unsupported`.
pub struct Int33Model {
    state: RefCell<State>,
}

/// Where `read_low` finds the code the vector points at.
const HANDLER: (u16, u16) = (0xC800, 0x0010);

impl Int33Model {
    pub fn new(kind: Int33Kind) -> Int33Model {
        let mut st = State {
            kind,
            ms_two_button_convention: false,
            hide_counter: -1,
            x: 0,
            y: 0,
            range_x: (0, 639),
            range_y: (0, 199),
            buttons: 0,
            mickeys: (0, 0),
            wheel: 0,
            presses: [(0, 0, 0); 3],
            releases: [(0, 0, 0); 3],
            counts: true,
            calls: Vec::new(),
        };
        st.reset();
        Int33Model { state: RefCell::new(st) }
    }

    /// A three-button driver with the wheel API.
    pub fn wheel_mouse() -> Int33Model {
        Int33Model::new(Int33Kind::Driver { buttons: 3, wheel: true })
    }

    /// A two-button driver without the wheel API.
    pub fn two_button() -> Int33Model {
        Int33Model::new(Int33Kind::Driver {
            buttons: 2,
            wheel: false,
        })
    }

    /// Makes function 0000h answer BX = FFFFh for two buttons.
    pub fn with_ms_convention(self) -> Int33Model {
        self.state.borrow_mut().ms_two_button_convention = true;
        self
    }

    /// The mouse moves by `dx`, `dy` mickeys: the counters grow and the position follows one for one inside its range.
    pub fn move_by(&self, dx: i32, dy: i32) {
        let mut st = self.state.borrow_mut();
        st.mickeys.0 += dx;
        st.mickeys.1 += dy;
        st.x += dx;
        st.y += dy;
        st.clamp();
    }

    /// Makes functions 0005h and 0006h answer as a driver that never implemented them: nothing is counted.
    pub fn without_counters(self) -> Int33Model {
        self.state.borrow_mut().counts = false;
        self
    }

    /// Presses the buttons in `button` (bit 0 left, 1 right, 2 middle); each one that was up is counted as a press.
    pub fn press(&self, button: u8) {
        let mut st = self.state.borrow_mut();
        for i in 0..3 {
            if button & (1 << i) != 0 && st.buttons & (1 << i) == 0 {
                let (x, y) = (st.x, st.y);
                let c = &mut st.presses[i];
                *c = (c.0.saturating_add(1), x, y);
            }
        }
        st.buttons |= button & 7;
    }

    /// Releases the buttons in `button`; each one that was down is counted as a release.
    pub fn release(&self, button: u8) {
        let mut st = self.state.borrow_mut();
        for i in 0..3 {
            if button & (1 << i) != 0 && st.buttons & (1 << i) != 0 {
                let (x, y) = (st.x, st.y);
                let c = &mut st.releases[i];
                *c = (c.0.saturating_add(1), x, y);
            }
        }
        st.buttons &= !(button & 7);
    }

    /// A click that starts and ends between two polls: a press and a release of `button` (the state ends as it began). Done on a
    /// button that is already down it is a release and a press.
    pub fn click(&self, button: u8) {
        if self.state.borrow().buttons & button & 7 != 0 {
            self.release(button);
            self.press(button);
        } else {
            self.press(button);
            self.release(button);
        }
    }

    /// Wheel detents, positive towards the user.
    pub fn scroll(&self, detents: i32) {
        self.state.borrow_mut().wheel += detents;
    }

    /// The AX of every call so far, in order.
    pub fn calls(&self) -> Vec<u16> {
        self.state.borrow().calls.clone()
    }

    /// The hide counter: -1 after a reset (hidden), 0 when the cursor shows.
    pub fn hide_counter(&self) -> i16 {
        self.state.borrow().hide_counter
    }

    /// The driver's own position and ranges.
    pub fn position(&self) -> (i32, i32) {
        let st = self.state.borrow();
        (st.x, st.y)
    }
}

impl Bios for Int33Model {
    fn int(&self, vector: u8, regs: &mut BiosRegs, buffer: Option<&mut [u8]>) -> Result<(), HalError> {
        if vector != 0x33 || buffer.is_some() {
            return Err(HalError::Unsupported);
        }
        let mut st = self.state.borrow_mut();
        let Int33Kind::Driver { buttons, wheel } = st.kind else {
            return Err(HalError::Unsupported);
        };
        let ax = regs.eax as u16;
        st.calls.push(ax);
        let (cx, dx) = (regs.ecx as u16 as i16 as i32, regs.edx as u16 as i16 as i32);
        match ax {
            0x0000 | 0x0021 => {
                st.reset();
                regs.eax = 0xFFFF;
                regs.ebx = if st.ms_two_button_convention && buttons == 2 {
                    0xFFFF
                } else {
                    buttons.into()
                };
            }
            0x0001 => st.hide_counter = (st.hide_counter + 1).min(0),
            0x0002 => st.hide_counter -= 1,
            0x0003 => {
                let w = if wheel {
                    (st.wheel.clamp(-128, 127) as i8 as u8 as u32) << 8
                } else {
                    0
                };
                st.wheel = 0;
                regs.ebx = u32::from(st.buttons) | w;
                regs.ecx = st.x as u16 as u32;
                regs.edx = st.y as u16 as u32;
            }
            0x0005 | 0x0006 if st.counts => {
                let i = (regs.ebx as u16 as usize).min(2);
                let present = (regs.ebx as u16) < buttons;
                let (n, x, y) = if ax == 0x0005 { st.presses[i] } else { st.releases[i] };
                if ax == 0x0005 {
                    st.presses[i] = (0, x, y);
                } else {
                    st.releases[i] = (0, x, y);
                }
                regs.eax = st.buttons.into();
                regs.ebx = if present { n.into() } else { 0 };
                regs.ecx = x as u16 as u32;
                regs.edx = y as u16 as u32;
            }
            0x0004 => {
                st.x = cx;
                st.y = dx;
                st.clamp();
            }
            0x0007 => st.range_x = (cx.min(dx), cx.max(dx)),
            0x0008 => st.range_y = (cx.min(dx), cx.max(dx)),
            0x000B => {
                regs.ecx = st.mickeys.0 as i16 as u16 as u32;
                regs.edx = st.mickeys.1 as i16 as u16 as u32;
                st.mickeys = (0, 0);
            }
            0x0011 if wheel => {
                regs.eax = 0x574D;
                regs.ecx = 1;
            }
            0x0015 => regs.ebx = 0x0100,
            0x0024 => {
                regs.ebx = 0x0806;
                regs.ecx = 0x0400;
                regs.eax = 0;
            }
            // Unknown functions return with the registers untouched, as the real drivers do.
            _ => {}
        }
        regs.flags = 0;
        Ok(())
    }

    fn read_low(&self, segment: u16, offset: u16, out: &mut [u8]) -> Result<(), HalError> {
        let st = self.state.borrow();
        let linear = (u32::from(segment) << 4) + u32::from(offset);
        if u64::from(linear) + out.len() as u64 > 0x10_0000 {
            return Err(HalError::InvalidArgument);
        }
        let vector = u32::from(0x33u8) * 4;
        let handler = (u32::from(HANDLER.0) << 4) + u32::from(HANDLER.1);
        for (i, o) in out.iter_mut().enumerate() {
            let addr = linear + i as u32;
            *o = match addr {
                a if (vector..vector + 4).contains(&a) => {
                    let table = match st.kind {
                        Int33Kind::NoDriver => [0, 0, 0, 0],
                        _ => {
                            let [o0, o1] = HANDLER.1.to_le_bytes();
                            let [s0, s1] = HANDLER.0.to_le_bytes();
                            [o0, o1, s0, s1]
                        }
                    };
                    table[(a - vector) as usize]
                }
                a if a == handler => match st.kind {
                    Int33Kind::IretOnly => 0xCF,
                    _ => 0x90,
                },
                _ => 0,
            };
        }
        Ok(())
    }
}
