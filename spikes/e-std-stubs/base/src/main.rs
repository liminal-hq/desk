// The baseline for the size comparison: the same hello program as `probe`'s `hello_std`, on `core` and `alloc` with the d0 runtime.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]
#![no_main]

extern crate alloc;

use alloc::format;
use alloc::vec::Vec;

rt::entry!(main);

fn main() -> i32 {
    let v: Vec<u32> = (1..=10).map(|n| n * n).collect();
    let s = format!("hello: squares={} sum={}", v.len(), v.iter().sum::<u32>());
    rt::println!("{}", s);
    0
}
