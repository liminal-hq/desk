// The kernel console:  and  write to the serial port and to the VGA screen.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{serial::Serial, vga::Vga};
use core::fmt::{self, Write};

pub fn init() {
    Vga::clear();
}

pub fn _print(args: fmt::Arguments) {
    let _ = Serial.write_fmt(args);
    let _ = Vga.write_fmt(args);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => { $crate::console::_print(format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\n") };
    ($($arg:tt)*) => { $crate::print!("{}\n", format_args!($($arg)*)) };
}
