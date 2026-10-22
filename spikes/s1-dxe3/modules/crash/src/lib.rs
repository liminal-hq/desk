// A module that fails on demand: a panic, a null write, a stack overflow, a divide error, or a panic while holding heap blocks.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
#![no_std]

extern crate alloc;

use alloc::format;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use rt::{desk_app, log, DeskEvent};

const PANIC: u32 = 100;
const NULL_WRITE: u32 = 101;
const STACK_OVERFLOW: u32 = 102;
const DIVIDE: u32 = 103;
const PANIC_WITH_HEAP: u32 = 104;

static EVENTS: AtomicU32 = AtomicU32::new(0);

fn start(app: u32, _args: &str) -> i32 {
    log(&format!("crash: start app={}", app));
    0
}

#[inline(never)]
#[allow(unconditional_recursion)]
fn recurse(depth: u32) -> u32 {
    let pad = [depth as u8; 256];
    // The volatile read keeps the frame and the recursion; the sum stops the compiler turning it into a loop.
    let back = recurse(depth + 1);
    back + u32::from(unsafe { core::ptr::read_volatile(&pad[(depth % 256) as usize]) })
}

fn event(_app: u32, e: &DeskEvent) {
    let n = EVENTS.fetch_add(1, Ordering::Relaxed) + 1;
    match e.kind {
        PANIC => panic!("deliberate panic number {}", n),
        NULL_WRITE => {
            log("crash: writing to address 0");
            // SAFETY: not sound, on purpose: the spike checks what a real CPU fault does.
            unsafe { core::ptr::write_volatile(core::ptr::null_mut::<u32>(), 0xDEAD) };
        }
        STACK_OVERFLOW => {
            log("crash: recursing without end");
            log(&format!("crash: unreachable {}", recurse(0)));
        }
        DIVIDE => {
            log("crash: dividing by zero");
            let zero = e.data;
            let mut quotient: u32 = 1;
            // SAFETY: not sound, on purpose: `div` by zero raises the CPU's divide error.
            unsafe { core::arch::asm!("xor edx, edx", "div {z}", z = in(reg) zero, inout("eax") quotient, out("edx") _) };
            log(&format!("crash: unreachable {}", quotient));
        }
        PANIC_WITH_HEAP => {
            let blocks: Vec<Vec<u8>> = (1..=5).map(|i| Vec::with_capacity(i * 100)).collect();
            log(&format!("crash: holding {} blocks, then panicking", blocks.len()));
            panic!("panic with heap");
        }
        _ => log(&format!("crash: ordinary event {} (count {})", e.kind, n)),
    }
}

fn stop(app: u32) {
    log(&format!("crash: stop app={}", app));
}

desk_app!(start, event, stop);
