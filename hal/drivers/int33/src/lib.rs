// The DOS mouse driver interface (INT 33h) and the DOS input source: what the hosted platform hands the core instead of PS/2 packets.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Under DOS the mouse belongs to a driver (CTMOUSE, the vendor's, or the DOSBox built-in one) that talks to the PS/2 port or a serial
// port and offers INT 33h. Desk does not touch the mouse hardware there. This crate is the whole of what it needs from the driver:
//
// - `Mouse` wraps the calls, through the HAL's `Bios` trait (a real-mode software interrupt; under DPMI function 0300h);
// - `MouseInput` turns "motion since the last call" (function 000Bh, in mickeys) and "buttons now" (0003h) into the `MouseEvent`
//   the native kernel's PS/2 driver produces, so the core cannot tell the tracks apart;
// - `DosInput` is the `InputSource` for the hosted platform: scancodes from the IRQ 1 ring through the shared `desk-kbd` decoder,
//   mouse reports from `MouseInput`. A machine with no mouse driver simply has no mouse reports (the SPEC's keyboard-only mode).
//
// Nothing here is specific to a DOS memory model or to DPMI, so it is tested on the host against `Int33Model`.
#![cfg_attr(not(any(test, feature = "std")), no_std)]

mod input;
pub mod transcript;

pub use input::{DosInput, MouseInput};

use desk_hal::{Bios, BiosRegs, HalError};

/// The software interrupt the mouse driver hooks.
pub const VECTOR: u8 = 0x33;

/// Function numbers (the value of AX on entry).
pub mod func {
    /// Reset the driver and detect it: AX = FFFFh and BX = buttons when installed, AX = 0 when not.
    pub const RESET: u16 = 0x0000;
    /// Show the cursor (increments the driver's hide counter; the cursor shows when it reaches 0).
    pub const SHOW: u16 = 0x0001;
    /// Hide the cursor (decrements the counter).
    pub const HIDE: u16 = 0x0002;
    /// Position and buttons: BX = buttons (bit 0 left, 1 right, 2 middle), CX = x, DX = y.
    pub const POSITION: u16 = 0x0003;
    /// Set the position to CX, DX.
    pub const SET_POSITION: u16 = 0x0004;
    /// Set the horizontal range to CX..=DX.
    pub const RANGE_X: u16 = 0x0007;
    /// Set the vertical range to CX..=DX.
    pub const RANGE_Y: u16 = 0x0008;
    /// Motion since the last call, in mickeys: CX = horizontal, DX = vertical, both signed 16 bit; the counters are then zero.
    pub const MOTION: u16 = 0x000B;
    /// The CuteMouse wheel API's detection: AX = 574Dh ("WM") when the driver reports a wheel.
    pub const WHEEL_API: u16 = 0x0011;
    /// The size in bytes of the buffer the driver's state needs: BX.
    pub const STORAGE: u16 = 0x0015;
    /// Software reset: like `RESET`, and the driver reinitialises without touching the hardware.
    pub const SOFT_RESET: u16 = 0x0021;
    /// Driver version and type: BH.BL = version, CH = type, CL = IRQ (0 for PS/2).
    pub const TYPE: u16 = 0x0024;
}

/// The answer of `func::WHEEL_API` when the driver implements the wheel API.
pub const WHEEL_API_SIGNATURE: u16 = 0x574D;

/// Why a mouse could not be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// No mouse driver is installed: the INT 33h vector is empty or points at an `IRET`, or function 0000h answered 0. The
    /// platform runs keyboard-only.
    NoDriver,
    /// The platform could not make the call.
    Hal(HalError),
}

impl From<HalError> for Error {
    fn from(e: HalError) -> Error {
        Error::Hal(e)
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::NoDriver => f.write_str("no mouse driver"),
            Error::Hal(e) => write!(f, "mouse call failed: {e}"),
        }
    }
}

/// What `func::TYPE` reports. Absent from drivers older than version 7.00, so every field is optional knowledge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriverType {
    /// Major and minor version.
    pub version: (u8, u8),
    /// 1 bus, 2 serial, 3 InPort, 4 PS/2, 5 HP.
    pub kind: u8,
    /// The IRQ the driver uses, 0 for PS/2 (the controller's line).
    pub irq: u8,
}

/// What `detect` learned from function 0000h.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Detected {
    /// Buttons the driver reports. BX = FFFFh from the Microsoft driver means two; 3 is a three-button mouse.
    pub buttons: u8,
}

/// The mouse driver, called through a `Bios`.
pub struct Mouse<B: Bios> {
    pub(crate) bios: B,
    buttons: u8,
    wheel: bool,
}

impl<B: Bios> Mouse<B> {
    /// Finds the driver and resets it (function 0000h), so the state is the driver's default: cursor hidden, the full range for the
    /// current video mode, the position at the centre. Returns `Error::NoDriver` for an empty vector, a vector that points at an
    /// `IRET` (the classic "no driver" a DOS BIOS leaves) and an answer of 0.
    pub fn detect(bios: B) -> Result<Mouse<B>, Error> {
        // The real-mode interrupt vector table is the first 256 words of memory: INT 33h is at 0000:00CC (offset, then segment).
        let mut ivt = [0u8; 4];
        bios.read_low(0, u16::from(VECTOR) * 4, &mut ivt)?;
        let (offset, segment) = (u16::from_le_bytes([ivt[0], ivt[1]]), u16::from_le_bytes([ivt[2], ivt[3]]));
        if segment == 0 && offset == 0 {
            return Err(Error::NoDriver);
        }
        let mut first = [0u8; 1];
        bios.read_low(segment, offset, &mut first)?;
        if first[0] == 0xCF {
            return Err(Error::NoDriver);
        }
        let mut mouse = Mouse {
            bios,
            buttons: 0,
            wheel: false,
        };
        let found = mouse.reset()?;
        mouse.buttons = found.buttons;
        mouse.wheel = mouse.wheel_api()?;
        Ok(mouse)
    }

    fn call(&self, regs: &mut BiosRegs) -> Result<(), Error> {
        self.bios.int(VECTOR, regs, None)?;
        Ok(())
    }

    fn func(&self, ax: u16, bx: u16, cx: u16, dx: u16) -> Result<BiosRegs, Error> {
        let mut regs = BiosRegs {
            eax: ax.into(),
            ebx: bx.into(),
            ecx: cx.into(),
            edx: dx.into(),
            ..BiosRegs::default()
        };
        self.call(&mut regs)?;
        Ok(regs)
    }

    /// Function 0000h: reset to the defaults and report the button count.
    pub fn reset(&mut self) -> Result<Detected, Error> {
        let r = self.func(func::RESET, 0, 0, 0)?;
        if r.eax as u16 != 0xFFFF {
            return Err(Error::NoDriver);
        }
        // The Microsoft convention is BX = 2 for two buttons; some drivers answer FFFFh for two.
        let buttons = match r.ebx as u16 {
            0xFFFF | 0 => 2,
            n => n.min(3) as u8,
        };
        Ok(Detected { buttons })
    }

    /// Buttons the driver reported at `detect` (2 or 3).
    pub fn buttons(&self) -> u8 {
        self.buttons
    }

    /// Whether the driver implements the CuteMouse wheel API (function 0011h answers 574Dh).
    pub fn has_wheel(&self) -> bool {
        self.wheel
    }

    fn wheel_api(&self) -> Result<bool, Error> {
        let r = self.func(func::WHEEL_API, 0, 0, 0)?;
        Ok(r.eax as u16 == WHEEL_API_SIGNATURE)
    }

    /// Function 0001h. Desk draws its own cursor, so the platform never calls this except to prove the call works.
    pub fn show(&self) -> Result<(), Error> {
        self.func(func::SHOW, 0, 0, 0).map(|_| ())
    }

    /// Function 0002h.
    pub fn hide(&self) -> Result<(), Error> {
        self.func(func::HIDE, 0, 0, 0).map(|_| ())
    }

    /// Function 0003h: the driver's own position and the buttons now (bit 0 left, 1 right, 2 middle), and the wheel counter since
    /// the last call (BH, signed) when the driver has the wheel API, else 0.
    pub fn position(&self) -> Result<Position, Error> {
        let r = self.func(func::POSITION, 0, 0, 0)?;
        Ok(Position {
            buttons: (r.ebx & 0x07) as u8,
            wheel: if self.wheel { (r.ebx >> 8) as u8 as i8 } else { 0 },
            x: r.ecx as u16 as i16,
            y: r.edx as u16 as i16,
        })
    }

    /// Function 0004h.
    pub fn set_position(&self, x: i16, y: i16) -> Result<(), Error> {
        self.func(func::SET_POSITION, 0, x as u16, y as u16).map(|_| ())
    }

    /// Functions 0007h and 0008h: the range the driver keeps its own position in. The drivers swap a reversed pair. In mode 13h the
    /// driver's horizontal coordinates are doubled (a 640 wide virtual screen over 320 pixels), so pass 0..=639 for that mode.
    pub fn set_range(&self, x: (i16, i16), y: (i16, i16)) -> Result<(), Error> {
        self.func(func::RANGE_X, 0, x.0 as u16, x.1 as u16)?;
        self.func(func::RANGE_Y, 0, y.0 as u16, y.1 as u16)?;
        Ok(())
    }

    /// Function 000Bh: motion since the last call, in mickeys (right and down positive), and the counters are zero afterwards.
    pub fn motion(&self) -> Result<(i16, i16), Error> {
        let r = self.func(func::MOTION, 0, 0, 0)?;
        Ok((r.ecx as u16 as i16, r.edx as u16 as i16))
    }

    /// Function 0015h: the bytes the driver's state needs (a sanity check that the driver answers the extended calls).
    pub fn storage_size(&self) -> Result<u16, Error> {
        Ok(self.func(func::STORAGE, 0, 0, 0)?.ebx as u16)
    }

    /// Function 0024h. `None` from a driver that does not implement it (AX comes back FFFFh or unchanged).
    pub fn driver_type(&self) -> Result<Option<DriverType>, Error> {
        let r = self.func(func::TYPE, 0, 0, 0)?;
        let ax = r.eax as u16;
        if ax == 0xFFFF || ax == func::TYPE {
            return Ok(None);
        }
        Ok(Some(DriverType {
            version: ((r.ebx >> 8) as u8, r.ebx as u8),
            kind: (r.ecx >> 8) as u8,
            irq: r.ecx as u8,
        }))
    }
}

/// The result of function 0003h.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub buttons: u8,
    /// Wheel counter since the last read, positive towards the user, 0 without the wheel API.
    pub wheel: i8,
    pub x: i16,
    pub y: i16,
}

#[cfg(test)]
mod tests;
