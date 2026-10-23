// The same hello program as `base`, on std: `println!` instead of the runtime's macro.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_main]
#![cfg_attr(target_os = "stub", feature(restricted_std))]

#[cfg(any(feature = "trusty", feature = "desk"))]
extern crate shim;

rt::entry!(main);

fn main() -> i32 {
    let v: Vec<u32> = (1..=10).map(|n| n * n).collect();
    let s = format!("hello: squares={} sum={}", v.len(), v.iter().sum::<u32>());
    println!("{}", s);
    0
}
