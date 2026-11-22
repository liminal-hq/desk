// The clean-up list: registering a, b and c, forgetting a and registering it again must run them in the order a, c, b at exit, and a
// 17th registration must be refused.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
use rt::{cleanup, println};

fn a() {
    println!("cleanup: run a");
}

fn b() {
    println!("cleanup: run b");
}

fn c() {
    println!("cleanup: run c");
}

/// Does nothing, differently for each `N`, so that no two instances are the same function.
fn filler<const N: usize>() {
    core::hint::black_box(N);
}

pub fn main() -> i32 {
    assert!(cleanup::register(a) && cleanup::register(b) && cleanup::register(c));
    cleanup::forget(a);
    assert!(cleanup::register(a));
    println!(
        "cleanup: registered a, b, c; forgot a; registered a again ({} on the list)",
        cleanup::len()
    );
    // Fill the rest of the list with 13 distinct functions that do nothing (one function is one entry, however often it is registered),
    // then ask for one more.
    let fillers: [fn(); 13] = [
        filler::<0>,
        filler::<1>,
        filler::<2>,
        filler::<3>,
        filler::<4>,
        filler::<5>,
        filler::<6>,
        filler::<7>,
        filler::<8>,
        filler::<9>,
        filler::<10>,
        filler::<11>,
        filler::<12>,
    ];
    for f in fillers {
        assert!(cleanup::register(f));
    }
    println!("cleanup: the list holds {} of {}", cleanup::len(), cleanup::SLOTS);
    println!(
        "cleanup: a 17th registration was refused: {}",
        !cleanup::register(filler::<13>)
    );
    println!("cleanup: exiting");
    0
}
