// The test program: output, arguments, environment, allocation, floating point, files, DPMI and the exit status.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

rt::entry!(progs::hello::main);
