// Rectangles: half-open integer boxes with saturating arithmetic.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// A rectangle, half-open: `[x0, x1) × [y0, y1)`. Empty when `x1 <= x0` or `y1 <= y0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

/// A size as a signed length, saturating at `i32::MAX`.
const fn clamp_len(n: u32) -> i32 {
    if n > i32::MAX as u32 {
        i32::MAX
    } else {
        n as i32
    }
}

/// An `i64` back into `i32`, saturating.
fn clamp_i32(v: i64) -> i32 {
    v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

impl Rect {
    /// The rectangle at `(x, y)` that is `w` wide and `h` high. The far edges saturate at `i32::MAX`.
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect {
            x0: x,
            y0: y,
            x1: x.saturating_add(clamp_len(w)),
            y1: y.saturating_add(clamp_len(h)),
        }
    }

    pub fn width(&self) -> u32 {
        if self.is_empty() {
            0
        } else {
            (self.x1 as i64 - self.x0 as i64) as u32
        }
    }

    pub fn height(&self) -> u32 {
        if self.is_empty() {
            0
        } else {
            (self.y1 as i64 - self.y0 as i64) as u32
        }
    }

    /// Width times height, saturating at `u32::MAX`.
    pub fn area(&self) -> u32 {
        (self.width() as u64 * self.height() as u64).min(u32::MAX as u64) as u32
    }

    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }

    /// The part both rectangles cover; `Rect::default()` when there is none.
    pub fn intersect(&self, o: &Rect) -> Rect {
        let r = Rect {
            x0: self.x0.max(o.x0),
            y0: self.y0.max(o.y0),
            x1: self.x1.min(o.x1),
            y1: self.y1.min(o.y1),
        };
        if r.is_empty() {
            Rect::default()
        } else {
            r
        }
    }

    /// The bounding box of both. An empty rectangle contributes nothing.
    pub fn union(&self, o: &Rect) -> Rect {
        if self.is_empty() {
            return if o.is_empty() { Rect::default() } else { *o };
        }
        if o.is_empty() {
            return *self;
        }
        Rect {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }

    /// Whether the two share at least one pixel.
    pub fn overlaps(&self, o: &Rect) -> bool {
        !self.is_empty() && !o.is_empty() && self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }

    /// Grows every side by `by` (shrinks when negative), saturating. An empty rectangle stays empty.
    pub fn inflate(&self, by: i32) -> Rect {
        if self.is_empty() {
            return Rect::default();
        }
        Rect {
            x0: self.x0.saturating_sub(by),
            y0: self.y0.saturating_sub(by),
            x1: self.x1.saturating_add(by),
            y1: self.y1.saturating_add(by),
        }
    }

    /// Moves the rectangle, saturating.
    pub fn offset(&self, dx: i32, dy: i32) -> Rect {
        Rect {
            x0: self.x0.saturating_add(dx),
            y0: self.y0.saturating_add(dy),
            x1: self.x1.saturating_add(dx),
            y1: self.y1.saturating_add(dy),
        }
    }

    /// Widens to `x0` rounded down and `x1` rounded up to multiples of `a` (`a` of 0 or 1 changes nothing). An empty
    /// rectangle stays empty and unchanged.
    pub fn align_x(&self, a: u32) -> Rect {
        if a <= 1 || self.is_empty() {
            return *self;
        }
        let a = a as i64;
        let x0 = (self.x0 as i64).div_euclid(a) * a;
        let x1 = (self.x1 as i64 + a - 1).div_euclid(a) * a;
        Rect {
            x0: clamp_i32(x0),
            x1: clamp_i32(x1),
            ..*self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_open_and_empty() {
        let r = Rect::new(2, 3, 4, 5);
        assert_eq!((r.x0, r.y0, r.x1, r.y1), (2, 3, 6, 8));
        assert_eq!((r.width(), r.height(), r.area()), (4, 5, 20));
        assert!(r.contains(2, 3) && r.contains(5, 7));
        assert!(!r.contains(6, 3) && !r.contains(2, 8) && !r.contains(1, 3));
        assert!(Rect::new(0, 0, 0, 5).is_empty());
        assert!(Rect::new(0, 0, 5, 0).is_empty());
        assert!(Rect::default().is_empty());
        let inverted = Rect {
            x0: 5,
            y0: 5,
            x1: 1,
            y1: 9,
        };
        assert!(inverted.is_empty());
        assert_eq!((inverted.width(), inverted.height(), inverted.area()), (0, 0, 0));
    }

    #[test]
    fn constructor_saturates_at_the_limits() {
        let r = Rect::new(i32::MAX - 1, i32::MAX - 1, 100, u32::MAX);
        assert_eq!((r.x1, r.y1), (i32::MAX, i32::MAX));
        assert_eq!(r.width(), 1);
        let big = Rect::new(i32::MIN, i32::MIN, u32::MAX, u32::MAX);
        assert_eq!((big.x1, big.y1), (-1, -1));
        let full = Rect {
            x0: i32::MIN,
            y0: i32::MIN,
            x1: i32::MAX,
            y1: i32::MAX,
        };
        assert_eq!((full.width(), full.height()), (u32::MAX, u32::MAX));
        assert_eq!(full.area(), u32::MAX);
        let low = Rect::new(i32::MIN, 0, 10, 10);
        assert_eq!(low.x1, i32::MIN + 10);
    }

    #[test]
    fn intersect_union_overlaps() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, 5, 10, 10);
        assert_eq!(a.intersect(&b), Rect::new(5, 5, 5, 5));
        assert_eq!(a.union(&b), Rect::new(0, 0, 15, 15));
        assert!(a.overlaps(&b));
        let touching = Rect::new(10, 0, 5, 5);
        assert!(!a.overlaps(&touching));
        assert!(a.intersect(&touching).is_empty());
        assert_eq!(a.union(&Rect::default()), a);
        assert_eq!(Rect::default().union(&a), a);
        assert!(Rect::default().union(&Rect::default()).is_empty());
        assert!(!a.overlaps(&Rect::default()));
    }

    #[test]
    fn inflate_and_offset() {
        let r = Rect::new(10, 10, 4, 4);
        assert_eq!(r.inflate(2), Rect::new(8, 8, 8, 8));
        assert!(r.inflate(-2).is_empty());
        assert!(Rect::default().inflate(3).is_empty());
        assert!(Rect::new(5, 5, 0, 4).inflate(2).is_empty());
        assert_eq!(r.offset(-10, 5), Rect::new(0, 15, 4, 4));
        let edge = Rect::new(i32::MAX - 2, 0, 2, 2).offset(100, 0);
        assert_eq!(edge.x1, i32::MAX);
        assert_eq!(Rect::new(i32::MIN, 0, 1, 1).inflate(5).x0, i32::MIN);
    }

    #[test]
    fn align_x_widens_to_multiples() {
        let r = Rect::new(3, 1, 10, 2);
        assert_eq!(
            r.align_x(8),
            Rect {
                x0: 0,
                y0: 1,
                x1: 16,
                y1: 3
            }
        );
        assert_eq!(Rect::new(8, 0, 8, 1).align_x(8), Rect::new(8, 0, 8, 1));
        assert_eq!(
            Rect::new(-3, 0, 4, 1).align_x(8),
            Rect {
                x0: -8,
                y0: 0,
                x1: 8,
                y1: 1
            }
        );
        assert_eq!(r.align_x(1), r);
        assert_eq!(r.align_x(0), r);
        assert!(Rect::default().align_x(8).is_empty());
        let top = Rect::new(i32::MAX - 3, 0, 3, 1).align_x(8);
        assert_eq!(top.x1, i32::MAX);
        assert_eq!(top.x0 % 8, 0);
    }
}
