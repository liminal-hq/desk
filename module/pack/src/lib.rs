// The module packer's library: a strict reader of the ELF files `rust-lld` links for Desk modules and applications.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![forbid(unsafe_code)]

pub mod elf;

pub use elf::{read, ElfError, Extract};

/// The kind of a module (`.DMD`): `app_start`, with `app_event` and `app_stop` optional.
pub const KIND_MODULE: u16 = 1;
/// The kind of an application (`.DKX`): `desk_main` alone.
pub const KIND_APP: u16 = 2;
/// The default link base of an image (docs/LOADER.md, "Address-space assumptions").
pub const LINK_BASE: u32 = 0x0040_0000;
/// The most memory an image may ask for, the same limit as the container's.
pub const MAX_MEMORY: u32 = 16 << 20;
/// The most imports an image may have, the same limit as the container's.
pub const MAX_IMPORTS: usize = 256;
/// The longest import name, the same limit as the container's.
pub const MAX_NAME: usize = 31;
