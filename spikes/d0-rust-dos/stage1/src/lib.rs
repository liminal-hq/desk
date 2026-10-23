// The entry for the stage 1 build: the same `hello` as the ELF build, packaged as a static library of PE-flavoured COFF objects.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]

rt::entry!(progs::hello::main);
