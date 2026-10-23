// A model of the 8042 keyboard controller with a PS/2 keyboard and mouse behind it, for the input drivers' host tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Enough of the chip to run the real set-up sequences and interrupt services: the status register, the command and data ports,
// the configuration byte, the self-test and port tests, the output buffer with its keyboard-or-mouse source bit, and the two
// devices' command sets (reset, defaults, enable and disable, the keyboard's scancode-set and LED commands, the mouse's
// sample-rate magic for the wheel). Test knobs make the awkward cases happen: a device that is absent, that answers `0xFE` (resend),
// that never acknowledges, a controller whose status never shows the input buffer empty. The bytes the model queues are what the
// CPU would read from port 0x60, that is after the controller's set 2 to set 1 translation, so a test feeds it set 1 directly.
//
// Not modelled: timing (the buffers fill and empty instantly), the keyboard's typematic repeat and LEDs, the output port beyond
// storing it, parity errors.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::bus::{Device, Width};

pub const DATA: u16 = 0x60;
pub const STATUS: u16 = 0x64;

const OBF: u8 = 0x01;
const IBF: u8 = 0x02;
const SYSTEM: u8 = 0x04;
const COMMAND_WRITTEN: u8 = 0x08;
const AUX_OBF: u8 = 0x20;

const ACK: u8 = 0xFA;
const RESEND: u8 = 0xFE;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    Config,
    Aux,
    OutputPort,
}

/// One byte waiting in the output buffer, and which device it came from (status bit 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Queued {
    pub byte: u8,
    pub aux: bool,
}

/// A device behind the controller: the behaviour knobs both devices share.
#[derive(Clone, Debug, Default)]
pub struct DeviceKnobs {
    /// Nothing is plugged in: the device never answers.
    pub absent: bool,
    /// Answer the next this-many bytes written with `0xFE` (resend) instead of acting on them.
    pub resend_next: u8,
    /// Act on commands but never acknowledge them.
    pub silent: bool,
    /// Every command written to the device, in order, including parameters.
    pub received: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct KeyboardModel {
    pub knobs: DeviceKnobs,
    /// The basic assurance test result after a reset: `0xAA` for a pass.
    pub bat_result: u8,
    pub scanning: bool,
    /// The scancode set the keyboard is in (1 to 3).
    pub set: u8,
    expecting: Option<u8>,
}

#[derive(Clone, Debug)]
pub struct MouseModel {
    pub knobs: DeviceKnobs,
    /// A mouse that answers the IntelliMouse sample-rate sequence (200, 100, 80) with device ID 3.
    pub wheel_capable: bool,
    pub id: u8,
    pub streaming: bool,
    pub sample_rate: u8,
    rates: [u8; 3],
    expecting: Option<u8>,
}

/// The controller and its two devices, mapped over ports `0x60..=0x64`.
#[derive(Clone, Debug)]
pub struct I8042Model {
    out: VecDeque<Queued>,
    pending: Option<Pending>,
    last_was_command: bool,
    pub config: u8,
    pub output_port: u8,
    pub kbd_port_enabled: bool,
    pub aux_port_enabled: bool,
    pub keyboard: KeyboardModel,
    pub mouse: MouseModel,
    /// The reply to the controller self-test (`0x55` for a pass).
    pub self_test_result: u8,
    pub kbd_port_test_result: u8,
    pub aux_port_test_result: u8,
    /// A single-channel controller: no auxiliary port, so config bit 5 (aux clock disabled) always reads back set and `0xA9` does not answer.
    pub single_channel: bool,
    /// A controller that cannot translate: config bit 6 reads back as zero.
    pub no_translation: bool,
    /// The self-test resets the configuration byte, as some controllers do.
    pub self_test_resets_config: bool,
    /// Status reads show the input buffer full for this many more reads (a slow controller); `u32::MAX` is a hung one.
    pub input_busy_reads: u32,
    /// Controller commands received (`port 0x64` writes), in order.
    pub commands: Vec<u8>,
    /// How many times the pulse-reset command `0xFE` was received.
    pub resets: u32,
}

impl Default for I8042Model {
    fn default() -> Self {
        I8042Model::new()
    }
}

impl I8042Model {
    /// A controller as a BIOS leaves it: both ports enabled, interrupts and translation on, a keyboard and a plain mouse present.
    pub fn new() -> I8042Model {
        I8042Model {
            out: VecDeque::new(),
            pending: None,
            last_was_command: false,
            config: 0x47,
            output_port: 0xCF,
            kbd_port_enabled: true,
            aux_port_enabled: true,
            keyboard: KeyboardModel {
                knobs: DeviceKnobs::default(),
                bat_result: 0xAA,
                scanning: true,
                set: 2,
                expecting: None,
            },
            mouse: MouseModel {
                knobs: DeviceKnobs::default(),
                wheel_capable: false,
                id: 0,
                streaming: false,
                sample_rate: 100,
                rates: [0; 3],
                expecting: None,
            },
            self_test_result: 0x55,
            kbd_port_test_result: 0x00,
            aux_port_test_result: 0x00,
            single_channel: false,
            no_translation: false,
            self_test_resets_config: false,
            input_busy_reads: 0,
            commands: Vec::new(),
            resets: 0,
        }
    }

    /// A controller with nothing behind it, as a machine without PS/2 ports has.
    pub fn without_devices() -> I8042Model {
        let mut m = I8042Model::new();
        m.keyboard.knobs.absent = true;
        m.mouse.knobs.absent = true;
        m
    }

    /// What is waiting in the output buffer, front first.
    pub fn output(&self) -> Vec<Queued> {
        self.out.iter().copied().collect()
    }

    pub fn clear_output(&mut self) {
        self.out.clear();
    }

    /// The keyboard sends a scancode byte, as a key press does. Dropped if the keyboard is absent or disabled or scanning is off.
    pub fn press_scancode(&mut self, byte: u8) -> bool {
        if self.keyboard.knobs.absent || !self.kbd_port_enabled || !self.keyboard.scanning {
            return false;
        }
        self.out.push_back(Queued { byte, aux: false });
        true
    }

    /// The mouse sends one packet's bytes. Dropped if the mouse is absent or disabled or not streaming.
    pub fn send_mouse_bytes(&mut self, bytes: &[u8]) -> bool {
        if self.mouse.knobs.absent || !self.aux_port_enabled || !self.mouse.streaming {
            return false;
        }
        self.out.extend(bytes.iter().map(|&byte| Queued { byte, aux: true }));
        true
    }

    /// The interrupt line the front byte of the output buffer would raise (1 or 12), if the configuration enables it.
    pub fn pending_irq(&self) -> Option<u8> {
        let front = self.out.front()?;
        if front.aux {
            (self.config & 0x02 != 0).then_some(12)
        } else {
            (self.config & 0x01 != 0).then_some(1)
        }
    }

    fn status(&mut self) -> u8 {
        let mut s = 0;
        if let Some(front) = self.out.front() {
            s |= OBF;
            if front.aux {
                s |= AUX_OBF;
            }
        }
        if self.input_busy_reads > 0 {
            if self.input_busy_reads != u32::MAX {
                self.input_busy_reads -= 1;
            }
            s |= IBF;
        }
        if self.config & 0x04 != 0 {
            s |= SYSTEM;
        }
        if self.last_was_command {
            s |= COMMAND_WRITTEN;
        }
        s
    }

    fn command(&mut self, cmd: u8) {
        self.commands.push(cmd);
        self.last_was_command = true;
        match cmd {
            0x20 => self.out.push_back(Queued {
                byte: self.config,
                aux: false,
            }),
            0x60 => self.pending = Some(Pending::Config),
            0xA7 => {
                self.aux_port_enabled = false;
                self.config |= 0x20;
            }
            0xA8 => {
                self.aux_port_enabled = true;
                if !self.single_channel {
                    self.config &= !0x20;
                }
            }
            0xA9 if !self.single_channel => {
                self.out.push_back(Queued {
                    byte: self.aux_port_test_result,
                    aux: false,
                });
            }
            0xAA => {
                self.out.push_back(Queued {
                    byte: self.self_test_result,
                    aux: false,
                });
                if self.self_test_resets_config {
                    self.config = 0x45 & !0x03;
                }
            }
            0xAB => self.out.push_back(Queued {
                byte: self.kbd_port_test_result,
                aux: false,
            }),
            0xAD => {
                self.kbd_port_enabled = false;
                self.config |= 0x10;
            }
            0xAE => {
                self.kbd_port_enabled = true;
                self.config &= !0x10;
            }
            0xD0 => self.out.push_back(Queued {
                byte: self.output_port,
                aux: false,
            }),
            0xD1 => self.pending = Some(Pending::OutputPort),
            0xD4 => self.pending = Some(Pending::Aux),
            0xFE => self.resets += 1,
            _ => {}
        }
    }

    fn data(&mut self, byte: u8) {
        self.last_was_command = false;
        match self.pending.take() {
            Some(Pending::Config) => {
                self.config = if self.no_translation { byte & !0x40 } else { byte };
                if self.single_channel {
                    self.config = (self.config | 0x20) & !0x02;
                }
            }
            Some(Pending::OutputPort) => self.output_port = byte,
            Some(Pending::Aux) => {
                if self.aux_port_enabled {
                    self.mouse_byte(byte);
                }
            }
            None => {
                if self.kbd_port_enabled {
                    self.keyboard_byte(byte);
                }
            }
        }
    }

    fn reply(&mut self, aux: bool, bytes: &[u8]) {
        self.out.extend(bytes.iter().map(|&byte| Queued { byte, aux }));
    }

    /// The knobs every device checks first; `true` when the byte was consumed (absent, or answered with a resend).
    fn gate(&mut self, aux: bool, byte: u8) -> bool {
        let knobs = if aux {
            &mut self.mouse.knobs
        } else {
            &mut self.keyboard.knobs
        };
        if knobs.absent {
            return true;
        }
        knobs.received.push(byte);
        if knobs.resend_next > 0 {
            knobs.resend_next -= 1;
            self.reply(aux, &[RESEND]);
            return true;
        }
        false
    }

    fn ack(&mut self, aux: bool) {
        let silent = if aux {
            self.mouse.knobs.silent
        } else {
            self.keyboard.knobs.silent
        };
        if !silent {
            self.reply(aux, &[ACK]);
        }
    }

    fn keyboard_byte(&mut self, byte: u8) {
        if self.gate(false, byte) {
            return;
        }
        if let Some(cmd) = self.keyboard.expecting.take() {
            self.ack(false);
            match (cmd, byte) {
                (0xF0, 0) => {
                    let set = self.keyboard.set;
                    self.reply(false, &[set]);
                }
                (0xF0, 1..=3) => self.keyboard.set = byte,
                _ => {}
            }
            return;
        }
        match byte {
            0xFF => {
                self.ack(false);
                let bat = self.keyboard.bat_result;
                self.keyboard.set = 2;
                self.keyboard.scanning = true;
                self.reply(false, &[bat]);
            }
            0xF4 => {
                self.keyboard.scanning = true;
                self.ack(false);
            }
            0xF5 | 0xF6 => {
                self.keyboard.scanning = byte == 0xF6;
                self.ack(false);
            }
            0xF0 | 0xF3 | 0xED => {
                self.keyboard.expecting = Some(byte);
                self.ack(false);
            }
            0xEE => self.reply(false, &[0xEE]),
            0xF2 => {
                self.ack(false);
                self.reply(false, &[0xAB, 0x83]);
            }
            _ => self.reply(false, &[RESEND]),
        }
    }

    fn mouse_byte(&mut self, byte: u8) {
        if self.gate(true, byte) {
            return;
        }
        if let Some(cmd) = self.mouse.expecting.take() {
            self.ack(true);
            if cmd == 0xF3 {
                self.mouse.sample_rate = byte;
                self.mouse.rates = [self.mouse.rates[1], self.mouse.rates[2], byte];
            }
            return;
        }
        match byte {
            0xFF => {
                self.ack(true);
                self.mouse.id = 0;
                self.mouse.streaming = false;
                self.mouse.rates = [0; 3];
                self.reply(true, &[0xAA, 0x00]);
            }
            0xF6 => {
                self.mouse.streaming = false;
                self.mouse.sample_rate = 100;
                self.ack(true);
            }
            0xF4 | 0xF5 => {
                self.mouse.streaming = byte == 0xF4;
                self.ack(true);
            }
            0xF3 | 0xE8 => {
                self.mouse.expecting = Some(byte);
                self.ack(true);
            }
            0xF2 => {
                if self.mouse.wheel_capable && self.mouse.rates == [200, 100, 80] {
                    self.mouse.id = 3;
                }
                self.ack(true);
                let id = self.mouse.id;
                self.reply(true, &[id]);
            }
            0xE6 | 0xE7 | 0xEA => self.ack(true),
            _ => self.reply(true, &[RESEND]),
        }
    }
}

impl Device for I8042Model {
    fn read(&mut self, port: u16, _: Width) -> u32 {
        match port {
            DATA => self.out.pop_front().map_or(0, |q| q.byte).into(),
            STATUS => self.status().into(),
            _ => 0xFF,
        }
    }

    fn write(&mut self, port: u16, _: Width, value: u32) {
        let v = value as u8;
        match port {
            DATA => self.data(v),
            STATUS => self.command(v),
            _ => {}
        }
    }
}
