// The error type the HAL's fallible operations share.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

/// What went wrong below the driver. Small and `Copy`, so it can be returned from anywhere, including a panic path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HalError {
    /// The platform does not offer this operation (for example remapping the PIC under DOS, where the PIC belongs to the BIOS).
    Unsupported,
    /// The device did not answer as expected: a missing UART, a floating bus.
    NoDevice,
    /// The device did not become ready within a bounded wait.
    Timeout,
    /// An argument is out of range for the device (a frequency the PIT cannot produce, an IRQ above 15).
    InvalidArgument,
    /// The resource is already taken (an IRQ line with a handler, a full table).
    Busy,
    /// No memory that meets the constraints (below 16 MiB, no 64 KiB crossing) is left.
    OutOfMemory,
    /// The platform's own error code, passed through: a DPMI error under DOS.
    Platform(u16),
}

impl fmt::Display for HalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HalError::Unsupported => f.write_str("not supported on this platform"),
            HalError::NoDevice => f.write_str("no device"),
            HalError::Timeout => f.write_str("device timed out"),
            HalError::InvalidArgument => f.write_str("argument out of range for the device"),
            HalError::Busy => f.write_str("resource busy"),
            HalError::OutOfMemory => f.write_str("no suitable memory"),
            HalError::Platform(code) => write!(f, "platform error {code:#06x}"),
        }
    }
}

#[cfg(any(test, feature = "std"))]
impl std::error::Error for HalError {}
