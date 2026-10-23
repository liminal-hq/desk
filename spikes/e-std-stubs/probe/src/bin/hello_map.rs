// Hello with a `HashMap` and an `Instant`: what the two platform-dependent types add to the size.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_main]
#![cfg_attr(target_os = "stub", feature(restricted_std))]

#[cfg(any(feature = "trusty", feature = "desk"))]
extern crate shim;

rt::entry!(main);

fn main() -> i32 {
    let mut m = std::collections::HashMap::new();
    for i in 0..10u32 {
        m.insert(format!("k{i}"), i * i);
    }
    let mut keys: Vec<_> = m.iter().collect();
    keys.sort();
    println!("hello: {:?}", keys);
    #[cfg(feature = "desk")]
    println!("took {:?}", std::time::Instant::now().elapsed());
    0
}
