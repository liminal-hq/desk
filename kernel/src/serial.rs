// COM1 through the shared 16550 driver, as the kernel's text sink.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::hal::SERIAL;
use core::fmt;

/// Programs the UART: 115200 baud, 8N1, FIFOs on, interrupts off.
pub fn init() {
    SERIAL.init(115_200).expect("115200 baud is a valid rate");
}

pub struct Serial;

impl fmt::Write for Serial {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        s.bytes().for_each(|b| SERIAL.write_byte(b));
        Ok(())
    }
}
