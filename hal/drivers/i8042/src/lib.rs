// The 8042 keyboard controller, the PS/2 keyboard and mouse behind it, and the interrupt services that queue their bytes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Three layers, all over `PortIo` and none with `unsafe`:
//
// - the controller (this file): status and command ports, bounded waits, the configuration byte, self-test, port enables;
// - the devices (this file): the keyboard's reset and enable handshake, the mouse's reset, defaults, wheel probe and streaming;
// - the interrupt services (`irq`) and the drain side (`input`): `KbdIrq` and `MouseIrq` read a byte in an interrupt and push it
//   into a ring; `Ps2Input` pops the rings in the core's loop and decodes.
//
// The set-up runs with the controller's interrupts off and polls the status port, so every wait has a bound and returns
// `Error::Timeout` rather than hanging. Sources and the details that could not be confirmed against a datasheet are in
// `docs/C-INPUT.md`.
#![cfg_attr(not(any(test, feature = "std")), no_std)]

pub mod input;
pub mod irq;

use desk_hal::{HalError, PortIo};

pub use input::Ps2Input;
pub use irq::{IrqStats, KbdIrq, MouseIrq};

/// The data port: bytes to and from the devices, and the controller's own replies.
pub const DATA_PORT: u16 = 0x60;
/// Reads give the status; writes give a controller command.
pub const STATUS_PORT: u16 = 0x64;
pub const COMMAND_PORT: u16 = 0x64;

/// Status register bits.
pub mod status {
    /// The output buffer holds a byte for the CPU.
    pub const OUTPUT_FULL: u8 = 0x01;
    /// The input buffer holds a byte the controller has not taken yet; do not write.
    pub const INPUT_FULL: u8 = 0x02;
    /// The byte in the output buffer came from the auxiliary (mouse) port. Meaningful on PS/2 controllers (the original AT
    /// controller used this bit for a transmit timeout).
    pub const AUX_OUTPUT: u8 = 0x20;
    /// A timeout error on the device link.
    pub const TIMEOUT_ERROR: u8 = 0x40;
    /// A parity error on the device link.
    pub const PARITY_ERROR: u8 = 0x80;
}

/// Controller commands written to `COMMAND_PORT`.
pub mod command {
    pub const READ_CONFIG: u8 = 0x20;
    pub const WRITE_CONFIG: u8 = 0x60;
    pub const DISABLE_AUX: u8 = 0xA7;
    pub const ENABLE_AUX: u8 = 0xA8;
    /// Answers `0x00` for a working auxiliary port.
    pub const TEST_AUX: u8 = 0xA9;
    /// Answers `0x55` for a working controller.
    pub const SELF_TEST: u8 = 0xAA;
    /// Answers `0x00` for a working keyboard port.
    pub const TEST_KBD: u8 = 0xAB;
    pub const DISABLE_KBD: u8 = 0xAD;
    pub const ENABLE_KBD: u8 = 0xAE;
    /// The next byte written to the data port goes to the mouse instead of the keyboard.
    pub const WRITE_AUX: u8 = 0xD4;
}

/// The controller configuration byte (command `0x20` reads it, `0x60` writes it).
pub mod config {
    /// IRQ 1 on a keyboard byte.
    pub const KBD_IRQ: u8 = 0x01;
    /// IRQ 12 on a mouse byte.
    pub const AUX_IRQ: u8 = 0x02;
    /// The system flag, set by the controller's self-test; software leaves it alone.
    pub const SYSTEM: u8 = 0x04;
    /// The keyboard clock is held low: the port is disabled.
    pub const KBD_CLOCK_OFF: u8 = 0x10;
    /// The auxiliary clock is held low: the port is disabled. Reads back as set after `DISABLE_AUX` and clear after `ENABLE_AUX`
    /// on a two-port controller, which is how a single-port controller is told apart.
    pub const AUX_CLOCK_OFF: u8 = 0x20;
    /// The controller translates the keyboard's scancode set 2 into set 1 on the way to the CPU.
    pub const TRANSLATE: u8 = 0x40;
}

/// A device's answers.
pub mod reply {
    pub const ACK: u8 = 0xFA;
    pub const RESEND: u8 = 0xFE;
    pub const SELF_TEST_PASSED: u8 = 0xAA;
    pub const CONTROLLER_TEST_PASSED: u8 = 0x55;
}

/// Device commands (to the keyboard directly, to the mouse after `command::WRITE_AUX`).
pub mod device {
    pub const RESET: u8 = 0xFF;
    pub const SET_DEFAULTS: u8 = 0xF6;
    pub const DISABLE_SCANNING: u8 = 0xF5;
    pub const ENABLE: u8 = 0xF4;
    pub const SET_RATE: u8 = 0xF3;
    pub const GET_ID: u8 = 0xF2;
    pub const SELECT_SET: u8 = 0xF0;
}

/// Status polls before a wait gives up: about 100 ms of ISA reads at 1 µs each. For an ordinary command and its reply.
pub const POLLS: u32 = 100_000;
/// Status polls for the slow replies: a keyboard's reset and self-test take up to a second on real parts.
pub const SLOW_POLLS: u32 = 1_000_000;
/// Times a command is sent again after the device answers `0xFE`, before giving up.
pub const RESEND_LIMIT: u32 = 3;
/// Bytes read and dropped while flushing the output buffer, so a device that streams forever cannot trap the set-up.
pub const FLUSH_LIMIT: u32 = 64;

/// What went wrong talking to the controller or a device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// A status bit never changed, or a device never replied, within the bounded wait.
    Timeout,
    /// Nothing answered the device's first command: the keyboard or mouse is not plugged in (or is a device that never
    /// acknowledges, which is the same to this driver).
    NoDevice,
    /// The device asked for the command again (`0xFE`) more than `RESEND_LIMIT` times.
    Resend,
    /// A reply that is not the one the sequence allows.
    Unexpected(u8),
    /// The controller's self-test answered something other than `0x55`.
    ControllerSelfTest(u8),
    /// A port test answered a failure code (clock or data line stuck).
    PortTest { aux: bool, code: u8 },
    /// The keyboard's basic assurance test after reset failed (`0xFC` and `0xFD` are the usual codes).
    DeviceSelfTest(u8),
}

impl From<Error> for HalError {
    fn from(e: Error) -> HalError {
        match e {
            Error::Timeout => HalError::Timeout,
            Error::NoDevice => HalError::NoDevice,
            Error::Resend
            | Error::Unexpected(_)
            | Error::ControllerSelfTest(_)
            | Error::PortTest { .. }
            | Error::DeviceSelfTest(_) => HalError::NoDevice,
        }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Timeout => f.write_str("timed out"),
            Error::NoDevice => f.write_str("no device"),
            Error::Resend => f.write_str("device kept asking for a resend"),
            Error::Unexpected(b) => write!(f, "unexpected reply {b:#04x}"),
            Error::ControllerSelfTest(b) => write!(f, "controller self-test answered {b:#04x}"),
            Error::PortTest { aux, code } => write!(
                f,
                "{} port test answered {code:#04x}",
                if *aux { "mouse" } else { "keyboard" }
            ),
            Error::DeviceSelfTest(b) => write!(f, "device self-test answered {b:#04x}"),
        }
    }
}

/// What the controller set-up found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControllerInfo {
    /// The controller has an auxiliary port (a second channel for the mouse).
    pub dual_channel: bool,
    /// The configuration byte reads back with translation on: the CPU sees scancode set 1.
    pub translation: bool,
}

/// What the keyboard set-up found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardInfo {
    /// The scancodes the CPU reads are set 1: the controller translates the keyboard's set 2, or the keyboard was switched to
    /// set 1 itself. `false` means set 1 could not be arranged and the decoder would be fed another set.
    pub set1: bool,
    /// The controller translates (as opposed to the keyboard having been switched).
    pub translated: bool,
}

/// What the mouse set-up found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseInfo {
    /// The device ID after the set-up: 0 for a standard mouse, 3 for an IntelliMouse with a wheel.
    pub id: u8,
}

impl MouseInfo {
    /// Bytes per packet: 4 with a wheel, otherwise 3.
    pub const fn packet_len(&self) -> usize {
        if self.id == 3 {
            4
        } else {
            3
        }
    }
}

/// The 8042, reached through `ports`.
#[derive(Clone, Copy, Debug)]
pub struct I8042<P> {
    ports: P,
}

impl<P: PortIo> I8042<P> {
    /// Names the controller; touches no hardware.
    pub const fn new(ports: P) -> Self {
        I8042 { ports }
    }

    /// The status register.
    pub fn status(&self) -> u8 {
        self.ports.inb(STATUS_PORT)
    }

    fn wait_input_empty(&self, polls: u32) -> Result<(), Error> {
        for _ in 0..polls {
            if self.status() & status::INPUT_FULL == 0 {
                return Ok(());
            }
        }
        Err(Error::Timeout)
    }

    /// Waits for a byte from the wanted source and reads it. A byte from the other device (a key pressed during the mouse's
    /// set-up) is read and dropped, so it cannot be taken for a reply.
    fn read_from(&self, aux: bool, polls: u32) -> Result<u8, Error> {
        for _ in 0..polls {
            let s = self.status();
            if s & status::OUTPUT_FULL != 0 {
                let byte = self.ports.inb(DATA_PORT);
                if (s & status::AUX_OUTPUT != 0) == aux {
                    return Ok(byte);
                }
            }
        }
        Err(Error::Timeout)
    }

    /// Waits for any byte in the output buffer (the controller's own replies).
    fn read_any(&self, polls: u32) -> Result<u8, Error> {
        for _ in 0..polls {
            if self.status() & status::OUTPUT_FULL != 0 {
                return Ok(self.ports.inb(DATA_PORT));
            }
        }
        Err(Error::Timeout)
    }

    /// Sends a controller command. `Timeout` if the input buffer never empties.
    pub fn write_command(&self, cmd: u8) -> Result<(), Error> {
        self.wait_input_empty(POLLS)?;
        self.ports.outb(COMMAND_PORT, cmd);
        Ok(())
    }

    /// Writes a byte to the data port: a command's argument, or a byte for the keyboard.
    pub fn write_data(&self, byte: u8) -> Result<(), Error> {
        self.wait_input_empty(POLLS)?;
        self.ports.outb(DATA_PORT, byte);
        Ok(())
    }

    /// Reads and drops what is in the output buffer, at most `FLUSH_LIMIT` bytes. Returns how many it dropped.
    pub fn flush(&self) -> u32 {
        let mut dropped = 0;
        while dropped < FLUSH_LIMIT && self.status() & status::OUTPUT_FULL != 0 {
            let _ = self.ports.inb(DATA_PORT);
            dropped += 1;
        }
        dropped
    }

    pub fn read_config(&self) -> Result<u8, Error> {
        self.write_command(command::READ_CONFIG)?;
        self.read_any(POLLS)
    }

    pub fn write_config(&self, value: u8) -> Result<(), Error> {
        self.write_command(command::WRITE_CONFIG)?;
        self.write_data(value)
    }

    /// Turns the controller's interrupt generation for the keyboard (IRQ 1) and the mouse (IRQ 12) on or off, leaving the other
    /// configuration bits alone. Do this last, after the devices are set up, and after the interrupt handlers are registered.
    pub fn set_interrupts(&self, keyboard: bool, mouse: bool) -> Result<(), Error> {
        let mut c = self.read_config()? & !(config::KBD_IRQ | config::AUX_IRQ);
        if keyboard {
            c |= config::KBD_IRQ;
        }
        if mouse {
            c |= config::AUX_IRQ;
        }
        self.write_config(c)
    }

    /// Brings the controller to a known state with its interrupts off: both ports disabled, the output buffer flushed, the
    /// configuration byte set (interrupts off, translation on, clocks running), the self-test run and the configuration byte
    /// written again (some controllers reset it), the auxiliary port probed, the port tests run, both ports enabled. The
    /// output port (and with it the A20 gate, bit 1) is never touched.
    ///
    /// A missing auxiliary port is not an error: `dual_channel` comes back `false`.
    pub fn init(&self) -> Result<ControllerInfo, Error> {
        self.write_command(command::DISABLE_KBD)?;
        self.write_command(command::DISABLE_AUX)?;
        self.flush();

        // Interrupts off, translation on, both clocks running (the enables below clear the clock bits too).
        let start = self.read_config()?;
        let wanted = (start & !(config::KBD_IRQ | config::AUX_IRQ)) | config::TRANSLATE;
        self.write_config(wanted)?;

        self.write_command(command::SELF_TEST)?;
        let result = self.read_any(SLOW_POLLS)?;
        if result != reply::CONTROLLER_TEST_PASSED {
            return Err(Error::ControllerSelfTest(result));
        }
        self.write_config(wanted)?;

        // A two-port controller shows the auxiliary clock enabled (bit 5 clear) after `ENABLE_AUX`.
        self.write_command(command::ENABLE_AUX)?;
        let dual_channel = self.read_config()? & config::AUX_CLOCK_OFF == 0;
        if dual_channel {
            self.write_command(command::DISABLE_AUX)?;
        }

        self.write_command(command::TEST_KBD)?;
        let code = self.read_any(POLLS)?;
        if code != 0 {
            return Err(Error::PortTest { aux: false, code });
        }
        if dual_channel {
            self.write_command(command::TEST_AUX)?;
            let code = self.read_any(POLLS)?;
            if code != 0 {
                return Err(Error::PortTest { aux: true, code });
            }
        }

        self.write_command(command::ENABLE_KBD)?;
        if dual_channel {
            self.write_command(command::ENABLE_AUX)?;
        }
        self.flush();
        let translation = self.read_config()? & config::TRANSLATE != 0;
        Ok(ControllerInfo {
            dual_channel,
            translation,
        })
    }

    /// Sends a byte to a device and waits for its `0xFA`; sends it again on `0xFE`, up to `RESEND_LIMIT` times.
    fn device_byte(&self, aux: bool, byte: u8) -> Result<(), Error> {
        for _ in 0..=RESEND_LIMIT {
            if aux {
                self.write_command(command::WRITE_AUX)?;
            }
            self.write_data(byte)?;
            match self.read_from(aux, POLLS)? {
                reply::ACK => return Ok(()),
                reply::RESEND => continue,
                other => return Err(Error::Unexpected(other)),
            }
        }
        Err(Error::Resend)
    }

    /// A command with one argument byte: each byte is acknowledged on its own.
    fn device_command_with(&self, aux: bool, cmd: u8, arg: u8) -> Result<(), Error> {
        self.device_byte(aux, cmd)?;
        self.device_byte(aux, arg)
    }

    /// Sends the reset and waits for the self-test result. The usual reply is `0xFA` then `0xAA`; a device that sends `0xAA`
    /// without the acknowledgement is accepted too. `NoDevice` if nothing answers, `DeviceSelfTest` for any other result.
    fn reset_device(&self, aux: bool) -> Result<(), Error> {
        for _ in 0..=RESEND_LIMIT {
            if aux {
                self.write_command(command::WRITE_AUX)?;
            }
            self.write_data(device::RESET)?;
            let first = self.read_from(aux, POLLS).map_err(no_device_on_timeout)?;
            let result = match first {
                reply::ACK => self.read_from(aux, SLOW_POLLS)?,
                reply::RESEND => continue,
                other => other,
            };
            return if result == reply::SELF_TEST_PASSED {
                Ok(())
            } else {
                Err(Error::DeviceSelfTest(result))
            };
        }
        Err(Error::Resend)
    }

    /// Resets and enables the keyboard: scanning off, reset, wait for its self-test (`0xAA`), make sure the CPU will see
    /// scancode set 1, scanning on. The keyboard's own typematic rate and delay are left at their defaults (the BIOS's choice
    /// is lost on reset; the defaults are 500 ms and 10.9 characters a second).
    ///
    /// `NoDevice` if the first command is not answered at all.
    pub fn init_keyboard(&self, controller: ControllerInfo) -> Result<KeyboardInfo, Error> {
        self.device_byte(false, device::DISABLE_SCANNING)
            .map_err(no_device_on_timeout)?;
        self.reset_device(false)?;
        self.flush();
        let mut set1 = controller.translation;
        if !set1 {
            // The controller cannot translate: ask the keyboard for set 1 and check by asking which set it is in.
            self.device_command_with(false, device::SELECT_SET, 1)?;
            self.device_command_with(false, device::SELECT_SET, 0)?;
            set1 = matches!(self.read_from(false, POLLS)?, 1 | 0x43);
        }
        self.device_byte(false, device::ENABLE)?;
        Ok(KeyboardInfo {
            set1,
            translated: controller.translation,
        })
    }

    /// Resets and enables the mouse: reset (acknowledge, `0xAA`, ID `0x00`), defaults, the IntelliMouse probe when `wheel` is
    /// set (sample rates 200, 100, 80, then read the ID: 3 means a wheel), the sample rate back to 100, streaming on. A failed
    /// probe is not an error: the mouse stays a plain one.
    ///
    /// `NoDevice` if the reset is not answered at all.
    pub fn init_mouse(&self, wheel: bool) -> Result<MouseInfo, Error> {
        self.reset_device(true)?;
        let _id = self.read_from(true, POLLS)?;
        self.device_byte(true, device::SET_DEFAULTS)?;
        let mut id = 0;
        if wheel && self.probe_wheel().is_ok() {
            id = self.mouse_id()?;
            if id != 3 {
                id = 0;
            }
        }
        if id == 3 {
            self.device_command_with(true, device::SET_RATE, 100)?;
        }
        self.device_byte(true, device::ENABLE)?;
        Ok(MouseInfo { id })
    }

    fn probe_wheel(&self) -> Result<(), Error> {
        for rate in [200, 100, 80] {
            self.device_command_with(true, device::SET_RATE, rate)?;
        }
        Ok(())
    }

    fn mouse_id(&self) -> Result<u8, Error> {
        self.device_byte(true, device::GET_ID)?;
        self.read_from(true, POLLS)
    }
}

fn no_device_on_timeout(e: Error) -> Error {
    if e == Error::Timeout {
        Error::NoDevice
    } else {
        e
    }
}

#[cfg(test)]
mod tests;
