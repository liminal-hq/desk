// A single-producer, single-consumer ring: how an interrupt handler hands bytes or events to the core.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Lock-free with nothing but 32-bit loads and stores: the producer alone writes `head`, the consumer alone writes `tail`, and the
// indices run freely and wrap (the capacity is a power of two, so `index & (N - 1)` is the slot and `head - tail` the length even
// across the wrap). No compare-and-swap, which DOSBox's `cputype = 486` rejects, and no lock, which an interrupt could deadlock on.
// On x86 an acquire load and a release store are plain `mov`s; the orderings stop the compiler moving the slot access across the
// index update.
//
// `split` hands out the two ends once, so the single-producer, single-consumer rule is checked by the types: the producer goes
// into an interrupt handler's state, the consumer stays with the core.

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

/// A bounded queue of `N` items of `T` (a power of two), typically in a `static`.
pub struct SpscRing<T, const N: usize> {
    slots: [UnsafeCell<MaybeUninit<T>>; N],
    head: AtomicUsize,
    tail: AtomicUsize,
    dropped: AtomicU32,
    split: AtomicBool,
}

// SAFETY: the producer and consumer are unique (`split` hands each out once), each slot is written only by the producer while it
// is outside `tail..head` and read only by the consumer while it is inside, and the index updates are release stores read with
// acquire loads. Items move between contexts, so `T: Send` is required.
unsafe impl<T: Send, const N: usize> Sync for SpscRing<T, N> {}

impl<T: Copy, const N: usize> SpscRing<T, N> {
    /// An empty ring. `N` must be a power of two of at least 2; in a `static` a wrong `N` fails the build.
    pub const fn new() -> SpscRing<T, N> {
        assert!(N >= 2 && N.is_power_of_two(), "SpscRing capacity must be a power of two");
        SpscRing {
            slots: [const { UnsafeCell::new(MaybeUninit::uninit()) }; N],
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            dropped: AtomicU32::new(0),
            split: AtomicBool::new(false),
        }
    }

    /// The two ends, the first time it is called; `None` after that. `swap` is `xchg`, which every x86 has.
    pub fn split(&self) -> Option<(Producer<'_, T, N>, Consumer<'_, T, N>)> {
        if self.split.swap(true, Ordering::AcqRel) {
            None
        } else {
            Some((Producer { ring: self }, Consumer { ring: self }))
        }
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    /// Items pushed while the ring was full and therefore lost. Only the producer writes it.
    pub fn dropped(&self) -> u32 {
        self.dropped.load(Ordering::Relaxed)
    }

    fn len(&self) -> usize {
        self.head
            .load(Ordering::Acquire)
            .wrapping_sub(self.tail.load(Ordering::Acquire))
    }
}

impl<T: Copy, const N: usize> Default for SpscRing<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

/// The writing end. Lives in an interrupt handler's state.
pub struct Producer<'a, T, const N: usize> {
    ring: &'a SpscRing<T, N>,
}

/// The reading end. Lives with the core.
pub struct Consumer<'a, T, const N: usize> {
    ring: &'a SpscRing<T, N>,
}

impl<T: Copy, const N: usize> Producer<'_, T, N> {
    /// Adds an item. If the ring is full the item is dropped and counted, never blocked on: an interrupt handler must not wait.
    pub fn push(&mut self, item: T) -> Result<(), T> {
        let r = self.ring;
        let head = r.head.load(Ordering::Relaxed);
        let tail = r.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) == N {
            // Single writer, so a load and a store are enough (no read-modify-write instruction).
            r.dropped
                .store(r.dropped.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);
            return Err(item);
        }
        // SAFETY: the slot at `head` is outside `tail..head`, so the consumer is not reading it, and only this unique producer writes.
        unsafe { (*r.slots[head & (N - 1)].get()).write(item) };
        r.head.store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }
}

impl<T: Copy, const N: usize> Consumer<'_, T, N> {
    /// Takes the oldest item, if any.
    pub fn pop(&mut self) -> Option<T> {
        let r = self.ring;
        let tail = r.tail.load(Ordering::Relaxed);
        let head = r.head.load(Ordering::Acquire);
        if head == tail {
            return None;
        }
        // SAFETY: the slot at `tail` is inside `tail..head`, so the producer wrote it (the acquire load of `head` makes the write
        // visible) and will not touch it until `tail` moves past it below.
        let item = unsafe { (*r.slots[tail & (N - 1)].get()).assume_init() };
        r.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some(item)
    }

    pub fn is_empty(&self) -> bool {
        self.ring.len() == 0
    }

    /// Items lost to a full ring so far.
    pub fn dropped(&self) -> u32 {
        self.ring.dropped()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_come_out_in_order_and_a_full_ring_drops() {
        let ring: SpscRing<u8, 4> = SpscRing::new();
        let (mut tx, mut rx) = ring.split().unwrap();
        assert!(ring.split().is_none(), "the ends are handed out once");
        assert!(rx.is_empty());
        for b in 1..=4 {
            tx.push(b).unwrap();
        }
        assert_eq!(tx.push(5), Err(5));
        assert_eq!(rx.dropped(), 1);
        assert_eq!(rx.pop(), Some(1));
        tx.push(6).unwrap();
        assert_eq!(
            [rx.pop(), rx.pop(), rx.pop(), rx.pop(), rx.pop()],
            [Some(2), Some(3), Some(4), Some(6), None]
        );
    }

    #[test]
    fn the_indices_wrap() {
        let ring: SpscRing<u32, 2> = SpscRing::new();
        ring.head.store(usize::MAX, Ordering::Relaxed);
        ring.tail.store(usize::MAX, Ordering::Relaxed);
        let (mut tx, mut rx) = ring.split().unwrap();
        tx.push(7).unwrap();
        tx.push(8).unwrap();
        assert!(tx.push(9).is_err());
        assert_eq!((rx.pop(), rx.pop(), rx.pop()), (Some(7), Some(8), None));
    }

    #[test]
    fn a_producer_thread_and_a_consumer_thread_agree() {
        // Threads stand in for the interrupt and the core: on the host they really do run at the same time.
        static RING: SpscRing<u32, 8> = SpscRing::new();
        let (mut tx, mut rx) = RING.split().unwrap();
        const COUNT: u32 = 100_000;
        let producer = std::thread::spawn(move || {
            for i in 0..COUNT {
                while tx.push(i).is_err() {
                    std::hint::spin_loop();
                }
            }
        });
        let mut expected = 0;
        while expected < COUNT {
            if let Some(v) = rx.pop() {
                assert_eq!(v, expected);
                expected += 1;
            }
        }
        producer.join().unwrap();
        assert!(rx.pop().is_none());
    }
}
