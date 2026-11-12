// The Desk loadable image format: parse, validate, write and load `.DMD` modules and `.DKX` applications (docs/LOADER.md).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

extern crate alloc;

mod crc;
mod error;
mod header;
mod module;

pub use crc::crc32;
pub use error::{FormatError, LoadError};
pub use header::{
    parse_header, Header, ABI_I486_FLAT, HEADER_SIZE, KIND_APP, KIND_LIBRARY, KIND_MODULE, LINK_BASE, MAGIC, MAX_IMPORTS,
    MAX_MEMORY, MAX_NAME,
};
pub use module::{Import, Module};
