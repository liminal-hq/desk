// `print!` and `println!` over standard output (handle 1), and `eprint!` over handle 2.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use core::fmt;

/// A `core::fmt::Write` over a DOS handle.
pub struct Handle(pub u16);

pub const STDOUT: Handle = Handle(1);
pub const STDERR: Handle = Handle(2);

impl fmt::Write for Handle {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        crate::dos::write_all(self.0, s.as_bytes()).map_err(|_| fmt::Error)
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    let _ = fmt::Write::write_fmt(&mut Handle(STDOUT.0), args);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => { $crate::io::_print(core::format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! println {
    () => { $crate::print!("\r\n") };
    ($($arg:tt)*) => {{ $crate::io::_print(core::format_args!($($arg)*)); $crate::print!("\r\n") }};
}
