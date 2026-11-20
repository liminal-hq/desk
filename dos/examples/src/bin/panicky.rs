// A program that hooks IRQ 0 and then panics: the clean-up list must put the vector back before the report.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

rt::entry!(progs::panicky::main);
