// The d0-rust-dos test programs as a library, so one body of code builds as an ELF executable and, for the stage 1 experiment, as a
// static library for DJGPP's linker.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]

extern crate alloc;

pub mod ctx;
pub mod hello;
pub mod hw;
pub mod panicky;
