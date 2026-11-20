// What the stub hands over at entry: registers, selectors, the stub information block, the PSP, the environment, the x87.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

rt::entry!(progs::ctx::main);
