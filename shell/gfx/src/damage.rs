// Damage: the screen rectangles that need repainting or presenting, kept few and disjoint.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::rect::Rect;

/// The most rectangles a `Damage` keeps.
pub const MAX_RECTS: usize = 16;

/// A set of at most `MAX_RECTS` pairwise-disjoint rectangles inside `bounds`.
///
/// The invariants: every rectangle is inside `bounds` and not empty; the rectangles are pairwise disjoint, so a present writes
/// each pixel at most once; and their union covers every rectangle added since the last `clear`. A rectangle that overlaps or
/// shares an edge with an existing one is merged with it into their bounding box, repeatedly, until no two overlap; a rectangle
/// that overlaps nothing is appended; when the list is full, the pair whose bounding box adds the least area is merged.
/// Bounding-box merging overdraws a little and never splits.
#[derive(Clone, Copy, Debug)]
pub struct Damage {
    rects: [Rect; MAX_RECTS],
    len: u8,
    bounds: Rect,
}

/// Two rectangles that overlap, or share part of an edge.
fn mergeable(a: &Rect, b: &Rect) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let x_overlap = a.x0 < b.x1 && b.x0 < a.x1;
    let y_overlap = a.y0 < b.y1 && b.y0 < a.y1;
    let x_touch = a.x0 <= b.x1 && b.x0 <= a.x1;
    let y_touch = a.y0 <= b.y1 && b.y0 <= a.y1;
    (x_overlap && y_touch) || (y_overlap && x_touch)
}

/// The pixels a merge of `a` and `b` adds beyond both.
fn extra_area(a: &Rect, b: &Rect) -> u64 {
    let u = a.union(b);
    let sum = a.area() as u64 + b.area() as u64;
    (u.width() as u64 * u.height() as u64).saturating_sub(sum)
}

impl Damage {
    pub const fn new(bounds: Rect) -> Damage {
        Damage {
            rects: [Rect {
                x0: 0,
                y0: 0,
                x1: 0,
                y1: 0,
            }; MAX_RECTS],
            len: 0,
            bounds,
        }
    }

    /// Adds `r`, clipped to the bounds, and merges as the type's description says.
    pub fn add(&mut self, r: Rect) {
        let r = r.intersect(&self.bounds);
        if r.is_empty() {
            return;
        }
        let mut items = [Rect::default(); MAX_RECTS + 1];
        let n = self.len as usize;
        items[..n].copy_from_slice(&self.rects[..n]);
        items[n] = r;
        self.set(&mut items, n + 1);
    }

    pub fn add_all(&mut self, o: &Damage) {
        for r in o.rects() {
            self.add(*r);
        }
    }

    /// Widens every rectangle to multiples of `a` columns (still inside the bounds), then merges again, because widening can
    /// make neighbours overlap.
    pub fn align_x(&mut self, a: u32) {
        let mut items = [Rect::default(); MAX_RECTS + 1];
        let n = self.len as usize;
        for (slot, r) in items.iter_mut().zip(&self.rects[..n]) {
            *slot = r.align_x(a).intersect(&self.bounds);
        }
        self.set(&mut items, n);
    }

    pub fn rects(&self) -> &[Rect] {
        &self.rects[..self.len as usize]
    }

    /// The sum of the rectangles' areas: the pixels a present writes.
    pub fn area(&self) -> u32 {
        self.rects().iter().fold(0u32, |acc, r| acc.saturating_add(r.area()))
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Normalises `items[..n]` (merging, then bringing the count down to `MAX_RECTS`) and stores the result. Each merge removes
    /// one rectangle, so the loop ends.
    fn set(&mut self, items: &mut [Rect; MAX_RECTS + 1], mut n: usize) {
        'again: loop {
            let mut i = 0;
            while i < n {
                let mut j = i + 1;
                while j < n {
                    if mergeable(&items[i], &items[j]) {
                        items[i] = items[i].union(&items[j]);
                        n -= 1;
                        items[j] = items[n];
                        continue 'again;
                    }
                    j += 1;
                }
                i += 1;
            }
            if n > MAX_RECTS {
                let (mut bi, mut bj, mut best) = (0, 1, u64::MAX);
                for i in 0..n {
                    for j in i + 1..n {
                        let e = extra_area(&items[i], &items[j]);
                        if e < best {
                            (bi, bj, best) = (i, j, e);
                        }
                    }
                }
                items[bi] = items[bi].union(&items[bj]);
                n -= 1;
                items[bj] = items[n];
                continue;
            }
            break;
        }
        self.rects[..n].copy_from_slice(&items[..n]);
        self.len = n as u8;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    /// A fixed-seed integer generator (xorshift32); the tests use no clocks and no randomness.
    pub(crate) struct Rng(pub u32);

    impl Rng {
        pub(crate) fn next(&mut self) -> u32 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            self.0 = x;
            x
        }

        pub(crate) fn below(&mut self, n: u32) -> u32 {
            self.next() % n
        }
    }

    const BOUNDS: Rect = Rect {
        x0: 0,
        y0: 0,
        x1: 64,
        y1: 48,
    };

    fn assert_invariants(d: &Damage) {
        assert!(d.rects().len() <= MAX_RECTS);
        for (i, r) in d.rects().iter().enumerate() {
            assert!(!r.is_empty(), "empty rect in the list");
            assert_eq!(r.intersect(&BOUNDS), *r, "{r:?} outside the bounds");
            for o in &d.rects()[i + 1..] {
                assert!(!r.overlaps(o), "{r:?} overlaps {o:?}");
            }
        }
    }

    #[test]
    fn invariants_hold_over_ten_thousand_random_adds() {
        let mut rng = Rng(0x2545_F491);
        let mut d = Damage::new(BOUNDS);
        let mut covered = vec![false; 64 * 48];
        for step in 0..10_000 {
            // Rectangles from tiny to larger than the screen, some off its edges, some empty.
            let x = rng.below(100) as i32 - 20;
            let y = rng.below(80) as i32 - 15;
            let (w, h) = (rng.below(14), rng.below(10));
            let r = Rect::new(x, y, w, h);
            d.add(r);
            for py in 0..48 {
                for px in 0..64 {
                    if r.contains(px, py) {
                        covered[(py * 64 + px) as usize] = true;
                    }
                }
            }
            assert_invariants(&d);
            // Covering: every pixel of every add since the last clear is inside some rect.
            for py in 0..48 {
                for px in 0..64 {
                    if covered[(py * 64 + px) as usize] {
                        assert!(d.rects().iter().any(|q| q.contains(px, py)), "step {step}: ({px}, {py}) lost");
                    }
                }
            }
            if step % 700 == 699 {
                d.clear();
                assert!(d.is_empty() && d.area() == 0);
                covered.fill(false);
            }
        }
    }

    #[test]
    fn the_list_fills_to_sixteen_before_merging_the_cheapest_pair() {
        let mut d = Damage::new(Rect::new(0, 0, 400, 400));
        // 16 separate 4 × 4 squares, 10 apart on a diagonal-free grid.
        for i in 0..16 {
            d.add(Rect::new((i % 4) * 100, (i / 4) * 100, 4, 4));
        }
        assert_eq!(d.rects().len(), 16);
        assert_eq!(d.area(), 16 * 16);
        // A 17th next to one of them is cheapest to merge with that neighbour (the gap is 6 columns).
        d.add(Rect::new(10, 0, 4, 4));
        assert_eq!(d.rects().len(), 16);
        assert!(d.rects().contains(&Rect::new(0, 0, 14, 4)));
        assert_eq!(d.area(), 15 * 16 + 14 * 4);
    }

    #[test]
    fn overlapping_adds_merge_into_the_bounding_box() {
        let mut d = Damage::new(BOUNDS);
        d.add(Rect::new(0, 0, 10, 10));
        d.add(Rect::new(5, 5, 10, 10));
        assert_eq!(d.rects(), &[Rect::new(0, 0, 15, 15)]);
        d.add(Rect::new(40, 40, 4, 4));
        assert_eq!(d.rects().len(), 2);
        // A rectangle bridging both merges all three.
        d.add(Rect::new(10, 10, 32, 32));
        assert_eq!(d.rects(), &[Rect::new(0, 0, 44, 44)]);
    }

    #[test]
    fn touching_edges_merge_but_corners_do_not() {
        let mut d = Damage::new(BOUNDS);
        d.add(Rect::new(0, 0, 10, 10));
        d.add(Rect::new(10, 2, 5, 5));
        assert_eq!(d.rects(), &[Rect::new(0, 0, 15, 10)]);
        let mut c = Damage::new(BOUNDS);
        c.add(Rect::new(0, 0, 10, 10));
        c.add(Rect::new(10, 10, 5, 5));
        assert_eq!(c.rects().len(), 2);
        let mut s = Damage::new(BOUNDS);
        s.add(Rect::new(0, 0, 10, 10));
        s.add(Rect::new(0, 12, 10, 4));
        assert_eq!(s.rects().len(), 2);
    }

    #[test]
    fn adds_are_clipped_and_empty_adds_ignored() {
        let mut d = Damage::new(BOUNDS);
        d.add(Rect::default());
        d.add(Rect::new(100, 100, 5, 5));
        d.add(Rect::new(-5, -5, 3, 3));
        assert!(d.is_empty());
        d.add(Rect::new(-5, -5, 10, 10));
        assert_eq!(d.rects(), &[Rect::new(0, 0, 5, 5)]);
        d.add(Rect {
            x0: i32::MIN,
            y0: i32::MIN,
            x1: i32::MAX,
            y1: i32::MAX,
        });
        assert_eq!(d.rects(), &[BOUNDS]);
        assert_eq!(d.area(), 64 * 48);
    }

    #[test]
    fn add_all_unions_two_lists() {
        let mut a = Damage::new(BOUNDS);
        let mut b = Damage::new(BOUNDS);
        a.add(Rect::new(0, 0, 4, 4));
        b.add(Rect::new(30, 30, 4, 4));
        b.add(Rect::new(2, 2, 4, 4));
        a.add_all(&b);
        assert_invariants(&a);
        assert_eq!(a.rects().len(), 2);
        assert!(a.rects().contains(&Rect::new(0, 0, 6, 6)));
    }

    #[test]
    fn align_x_widens_and_keeps_the_invariants() {
        let mut d = Damage::new(BOUNDS);
        d.add(Rect::new(3, 0, 2, 4));
        d.add(Rect::new(10, 1, 2, 4));
        assert_eq!(d.rects().len(), 2);
        d.align_x(8);
        // [3, 5) widens to [0, 8) and [10, 12) to [8, 16): they now share an edge and merge.
        assert_eq!(d.rects(), &[Rect::new(0, 0, 16, 5)]);
        // A random list stays valid, on multiples of 8 (the bounds' own edges are), and still covers what it did.
        let mut rng = Rng(7);
        for _ in 0..500 {
            let mut e = Damage::new(BOUNDS);
            let mut added: Vec<Rect> = Vec::new();
            for _ in 0..rng.below(20) {
                let r = Rect::new(
                    rng.below(70) as i32 - 3,
                    rng.below(50) as i32 - 3,
                    rng.below(12),
                    rng.below(9),
                );
                e.add(r);
                added.push(r.intersect(&BOUNDS));
            }
            e.align_x(8);
            assert_invariants(&e);
            for r in e.rects() {
                assert_eq!((r.x0 % 8, r.x1 % 8), (0, 0), "{r:?}");
            }
            for r in added {
                for y in r.y0..r.y1 {
                    for x in r.x0..r.x1 {
                        assert!(e.rects().iter().any(|q| q.contains(x, y)));
                    }
                }
            }
        }
    }
}
