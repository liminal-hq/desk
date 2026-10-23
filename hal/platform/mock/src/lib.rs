// The mock platform: what driver tests run against on the host.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// - `MockPortBus` records every port access in order, answers reads from a strict script, from per-port queues, or from device
//   models attached to port ranges, and returns a floating bus (all ones) for anything else, as an empty ISA slot does.
// - `devices` holds small models of real chips (the 16550, the 8254), just enough to check what a driver programs.
// - `MockIrq` is an interrupt controller and handler registry with a `raise` that delivers an interrupt the way a platform does.
// - `MockClock` and `MockDelay` stand in for time.
//
// `no_std` with `alloc`, so it also builds for the DOS and kernel targets (an on-target self-test could use it); single-threaded,
// with `RefCell` behind the `&self` methods the HAL traits take.
#![cfg_attr(not(any(test, feature = "std")), no_std)]

extern crate alloc;

pub mod bus;
pub mod devices;
pub mod irq;
pub mod time;

pub use bus::{Access, Device, MockPortBus, Width};
pub use irq::MockIrq;
pub use time::{MockClock, MockDelay};
