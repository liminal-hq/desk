// The compositor: repaints damaged scene rectangles into the back buffer, then presents the damage with the cursor on top.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::damage::Damage;
use crate::digest::FrameDigest;
use crate::rect::Rect;
use crate::sprite::{Sprite, ARROW};
use crate::surface::Surface;
use desk_hal::{HalError, Rgb6};

use crate::present::Present;

/// The widest surface the compositor takes: the size of the row buffer it keeps on the stack (as
/// `desk_vga::canvas::MAX_ROW`).
pub const MAX_WIDTH: u32 = 1280;

/// The cursor, drawn over the back buffer as each row is presented and never stored in it.
#[derive(Clone, Copy)]
pub struct CursorLayer {
    /// Where the sprite's hot spot is. May be partly or wholly off the screen.
    pub x: i32,
    pub y: i32,
    pub visible: bool,
    pub sprite: &'static Sprite,
    /// Palette indices for the sprite's outline and fill.
    pub inks: [u8; 2],
}

impl CursorLayer {
    /// The screen rectangle the sprite covers now.
    pub fn rect(&self) -> Rect {
        self.sprite.rect_at(self.x, self.y)
    }

    /// Draws the cursor's pixels that fall in row `y`, columns `x0..x0 + row.len()`, over `row`.
    fn overlay(&self, y: i32, x0: i32, row: &mut [u8]) {
        if !self.visible {
            return;
        }
        let r = self.rect();
        if y < r.y0 || y >= r.y1 {
            return;
        }
        let sprite_row = (y as i64 - r.y0 as i64) as usize;
        let from = (r.x0 as i64).max(x0 as i64);
        let to = (r.x1 as i64).min(x0 as i64 + row.len() as i64);
        for x in from..to {
            if let Some(ink) = self.sprite.ink((x - r.x0 as i64) as usize, sprite_row) {
                row[(x - x0 as i64) as usize] = self.inks[ink];
            }
        }
    }
}

/// What one `frame` did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    /// Pixels repainted into the back buffer (the scene damage's area).
    pub repainted: u32,
    /// Pixels handed to the presenter (the area of the aligned scene and overlay damage together).
    pub presented: u32,
    /// Rectangles presented.
    pub rects: u8,
}

pub struct Compositor {
    back: Surface,
    scene: Damage,
    overlay: Damage,
    cursor: CursorLayer,
}

impl Compositor {
    /// A compositor for a `width × height` screen with the back buffer filled with `background`. The first frame damages the whole
    /// screen. Refused with `InvalidArgument`: a size of zero, or a width above `MAX_WIDTH`.
    pub fn new(width: u32, height: u32, background: u8) -> Result<Compositor, HalError> {
        if width == 0 || height == 0 || width > MAX_WIDTH {
            return Err(HalError::InvalidArgument);
        }
        let back = Surface::new(width, height, background);
        let bounds = back.bounds();
        let mut scene = Damage::new(bounds);
        scene.add(bounds);
        Ok(Compositor {
            back,
            scene,
            overlay: Damage::new(bounds),
            cursor: CursorLayer {
                x: 0,
                y: 0,
                visible: true,
                sprite: &ARROW,
                inks: [0, 1],
            },
        })
    }

    /// Marks `r` as needing a repaint by the next frame's `paint`.
    pub fn damage(&mut self, r: Rect) {
        self.scene.add(r);
    }

    pub fn cursor(&self) -> &CursorLayer {
        &self.cursor
    }

    /// Moves the cursor's hot spot. Only the overlay is damaged (the old and new sprite rectangles): nothing is repainted.
    pub fn move_cursor(&mut self, x: i32, y: i32) {
        if (x, y) == (self.cursor.x, self.cursor.y) {
            return;
        }
        if self.cursor.visible {
            self.overlay.add(self.cursor.rect());
        }
        self.cursor.x = x;
        self.cursor.y = y;
        if self.cursor.visible {
            self.overlay.add(self.cursor.rect());
        }
    }

    pub fn set_cursor_inks(&mut self, inks: [u8; 2]) {
        if inks != self.cursor.inks {
            self.cursor.inks = inks;
            if self.cursor.visible {
                self.overlay.add(self.cursor.rect());
            }
        }
    }

    pub fn set_cursor_visible(&mut self, visible: bool) {
        if visible != self.cursor.visible {
            self.overlay.add(self.cursor.rect());
            self.cursor.visible = visible;
        }
    }

    /// Repaints every scene-damage rectangle through `paint` (with the surface clipped to it), then presents the union of the scene
    /// and overlay damage, aligned to the presenter, compositing the cursor into each row on the way. An idle frame (no damage)
    /// calls nothing and returns zeros.
    ///
    /// Refused with `InvalidArgument`, before anything is painted, when the presenter's size differs from the compositor's or its
    /// alignment does not divide the width. If the presenter fails, the damage that was being presented is kept for the next frame.
    pub fn frame<P: Present>(&mut self, out: &mut P, mut paint: impl FnMut(&mut Surface, Rect)) -> Result<FrameStats, HalError> {
        if self.scene.is_empty() && self.overlay.is_empty() {
            return Ok(FrameStats::default());
        }
        let align = out.align().max(1);
        if out.width() != self.back.width() || out.height() != self.back.height() || !self.back.width().is_multiple_of(align) {
            return Err(HalError::InvalidArgument);
        }
        let mut stats = FrameStats {
            repainted: self.scene.area(),
            ..FrameStats::default()
        };
        for r in self.scene.rects() {
            self.back.set_clip(*r);
            paint(&mut self.back, *r);
        }
        self.back.set_clip(self.back.bounds());
        let mut present = self.scene;
        present.add_all(&self.overlay);
        present.align_x(align);
        self.scene.clear();
        self.overlay.clear();
        if let Err(e) = self.present(out, &present) {
            self.scene.add_all(&present);
            return Err(e);
        }
        stats.presented = present.area();
        stats.rects = present.rects().len() as u8;
        Ok(stats)
    }

    fn present<P: Present>(&self, out: &mut P, damage: &Damage) -> Result<(), HalError> {
        let mut buf = [0u8; MAX_WIDTH as usize];
        out.begin()?;
        for r in damage.rects() {
            let (x0, x1) = (r.x0 as usize, r.x1 as usize);
            let n = x1 - x0;
            for y in r.y0..r.y1 {
                buf[..n].copy_from_slice(&self.back.row(y as u32)[x0..x1]);
                self.cursor.overlay(y, r.x0, &mut buf[..n]);
                out.span(r.x0 as u32, y as u32, &buf[..n])?;
            }
        }
        out.end()
    }

    /// The back buffer: the scene without the cursor.
    pub fn back(&self) -> &Surface {
        &self.back
    }

    /// The digests of what is on the screen now (the back buffer with the cursor over it), without a second buffer. This is what
    /// the screen shows once the last frame has been presented.
    pub fn digest(&self, palette: &[Rgb6]) -> FrameDigest {
        FrameDigest::from_rows(self.back.width(), self.back.height(), palette, |y, buf| {
            buf.copy_from_slice(self.back.row(y));
            self.cursor.overlay(y as i32, 0, buf);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::present::{MemoryPresent, PresentStats};
    use alloc::vec;
    use alloc::vec::Vec;

    const W: u32 = 64;
    const H: u32 = 40;
    const INKS: [u8; 2] = [250, 251];

    /// A presenter that also counts calls to `begin`, `span` and `end`.
    struct Counting {
        inner: MemoryPresent,
        begins: u32,
        spans: u32,
        ends: u32,
        fail_after: Option<u32>,
    }

    impl Counting {
        fn new(align: u32) -> Counting {
            Counting {
                inner: MemoryPresent::new(W, H, align),
                begins: 0,
                spans: 0,
                ends: 0,
                fail_after: None,
            }
        }
    }

    impl Present for Counting {
        fn width(&self) -> u32 {
            W
        }
        fn height(&self) -> u32 {
            H
        }
        fn align(&self) -> u32 {
            self.inner.align()
        }
        fn begin(&mut self) -> Result<(), HalError> {
            self.begins += 1;
            self.inner.begin()
        }
        fn span(&mut self, x: u32, y: u32, pixels: &[u8]) -> Result<(), HalError> {
            if self.fail_after == Some(self.spans) {
                return Err(HalError::Timeout);
            }
            self.spans += 1;
            self.inner.span(x, y, pixels)
        }
        fn end(&mut self) -> Result<(), HalError> {
            self.ends += 1;
            self.inner.end()
        }
    }

    /// The scene: a `W × H` array of indices the paint callback copies from, so a full redraw is known.
    struct World {
        pixels: Vec<u8>,
        paints: u32,
    }

    impl World {
        fn new() -> World {
            World {
                pixels: (0..W * H).map(|i| (i % 200) as u8).collect(),
                paints: 0,
            }
        }

        fn paint(&mut self, s: &mut Surface, r: Rect) {
            self.paints += 1;
            s.blit(
                r.x0,
                r.y0,
                r.width(),
                r.height(),
                &self.pixels[(r.y0 as u32 * W + r.x0 as u32) as usize..],
                W as usize,
            );
        }

        fn set(&mut self, r: Rect, v: u8) {
            let r = r.intersect(&Rect::new(0, 0, W, H));
            for y in r.y0..r.y1 {
                for x in r.x0..r.x1 {
                    self.pixels[(y as u32 * W + x as u32) as usize] = v;
                }
            }
        }

        /// What the screen should show: the scene with the cursor over it.
        fn screen(&self, c: &Compositor) -> Vec<u8> {
            let mut out = self.pixels.clone();
            for y in 0..H {
                let row = &mut out[(y * W) as usize..((y + 1) * W) as usize];
                c.cursor().overlay(y as i32, 0, row);
            }
            out
        }
    }

    fn frame(c: &mut Compositor, out: &mut Counting, w: &mut World) -> FrameStats {
        c.frame(out, |s, r| w.paint(s, r)).unwrap()
    }

    fn setup(align: u32) -> (Compositor, Counting, World) {
        let mut c = Compositor::new(W, H, 0).unwrap();
        c.set_cursor_inks(INKS);
        (c, Counting::new(align), World::new())
    }

    #[test]
    fn new_refuses_a_bad_size() {
        assert_eq!(Compositor::new(0, 10, 0).err(), Some(HalError::InvalidArgument));
        assert_eq!(Compositor::new(10, 0, 0).err(), Some(HalError::InvalidArgument));
        assert_eq!(Compositor::new(MAX_WIDTH + 1, 10, 0).err(), Some(HalError::InvalidArgument));
        assert!(Compositor::new(MAX_WIDTH, 2, 0).is_ok());
    }

    #[test]
    fn the_first_frame_presents_every_pixel_once() {
        for align in [1, 8] {
            let (mut c, mut out, mut w) = setup(align);
            let s = frame(&mut c, &mut out, &mut w);
            assert_eq!((s.repainted, s.presented, s.rects), (W * H, W * H, 1));
            assert_eq!(
                out.inner.stats,
                PresentStats {
                    frames: 1,
                    spans: H,
                    pixels: W * H
                }
            );
            assert_eq!((out.begins, out.ends), (1, 1));
            assert_eq!(out.inner.pixels, w.screen(&c));
            assert_eq!(c.back().pixels(), &w.pixels[..], "the cursor is not in the back buffer");
        }
    }

    #[test]
    fn an_idle_frame_presents_nothing_and_calls_nothing() {
        let (mut c, mut out, mut w) = setup(8);
        frame(&mut c, &mut out, &mut w);
        let (b, sp, e, paints) = (out.begins, out.spans, out.ends, w.paints);
        let s = frame(&mut c, &mut out, &mut w);
        assert_eq!(s, FrameStats::default());
        assert_eq!((out.begins, out.spans, out.ends, w.paints), (b, sp, e, paints));
        // A move to where the cursor already is is idle as well.
        c.move_cursor(0, 0);
        assert_eq!(frame(&mut c, &mut out, &mut w), FrameStats::default());
        assert_eq!(out.begins, b);
    }

    #[test]
    fn a_cursor_move_presents_only_the_two_sprite_rectangles_and_paints_nothing() {
        let (mut c, mut out, mut w) = setup(1);
        frame(&mut c, &mut out, &mut w);
        let paints = w.paints;
        let before = out.inner.stats.pixels;
        // Far apart: the sum of the two 12 × 19 rectangles.
        c.move_cursor(30, 10);
        let s = frame(&mut c, &mut out, &mut w);
        assert_eq!(s.repainted, 0);
        assert_eq!(s.presented, 2 * 12 * 19);
        assert_eq!(out.inner.stats.pixels - before, s.presented);
        assert_eq!(w.paints, paints);
        let before = out.inner.stats.pixels;
        c.move_cursor(50, 20);
        let s = frame(&mut c, &mut out, &mut w);
        assert_eq!(s.presented, 2 * 12 * 19);
        assert_eq!(s.rects, 2);
        assert_eq!(out.inner.stats.pixels - before, 2 * 12 * 19);
        // Overlapping: at most their bounding box.
        let before = out.inner.stats.pixels;
        c.move_cursor(53, 23);
        let s = frame(&mut c, &mut out, &mut w);
        assert_eq!(
            s.presented,
            Rect::new(50, 20, 12, 19)
                .union(&Rect::new(53, 23, 12, 19))
                .intersect(&Rect::new(0, 0, W, H))
                .area()
        );
        assert!(s.presented <= 2 * 12 * 19);
        assert_eq!(out.inner.stats.pixels - before, s.presented);
        assert_eq!(w.paints, paints);
        assert_eq!(out.inner.pixels, w.screen(&c));
        assert_eq!(c.back().pixels(), &w.pixels[..]);
    }

    #[test]
    fn at_depth_four_a_move_presents_the_rectangles_widened_to_eight_columns() {
        let (mut c, mut out, mut w) = setup(8);
        frame(&mut c, &mut out, &mut w);
        c.move_cursor(27, 10);
        let s = frame(&mut c, &mut out, &mut w);
        // The old rect [0, 12) widens to [0, 16); the new [27, 39) to [24, 40).
        assert_eq!(s.presented, (16 + 16) * 19);
        assert_eq!(out.inner.pixels, w.screen(&c));
    }

    #[test]
    fn the_cursor_is_clipped_half_off_every_edge() {
        for align in [1, 8] {
            for (x, y) in [
                (-6, 10),
                (W as i32 - 6, 10),
                (20, -9),
                (20, H as i32 - 9),
                (-6, -9),
                (W as i32 - 6, H as i32 - 9),
                (-500, 5),
                (5, 900),
            ] {
                let (mut c, mut out, mut w) = setup(align);
                frame(&mut c, &mut out, &mut w);
                c.move_cursor(x, y);
                frame(&mut c, &mut out, &mut w);
                assert_eq!(out.inner.pixels, w.screen(&c), "cursor at ({x}, {y}) align {align}");
                assert_eq!(c.back().pixels(), &w.pixels[..]);
                // Move it back on: the screen is whole again.
                c.move_cursor(40, 12);
                frame(&mut c, &mut out, &mut w);
                assert_eq!(out.inner.pixels, w.screen(&c));
            }
        }
        // Half off the left edge really shows the right half of the arrow, in the ink colours.
        let (mut c, mut out, mut w) = setup(1);
        frame(&mut c, &mut out, &mut w);
        c.move_cursor(-1, 0);
        frame(&mut c, &mut out, &mut w);
        assert_eq!(
            out.inner.pixels[W as usize], INKS[0],
            "column 1 of row 1 of the arrow is an outline pixel"
        );
        assert_eq!(out.inner.pixels[2 * W as usize], INKS[1], "column 1 of row 2 is fill");
    }

    #[test]
    fn hiding_the_cursor_restores_what_it_covered() {
        let (mut c, mut out, mut w) = setup(1);
        frame(&mut c, &mut out, &mut w);
        c.move_cursor(10, 10);
        frame(&mut c, &mut out, &mut w);
        assert_ne!(out.inner.pixels, w.pixels);
        c.set_cursor_visible(false);
        frame(&mut c, &mut out, &mut w);
        assert_eq!(out.inner.pixels, w.pixels);
        c.move_cursor(20, 20);
        assert_eq!(frame(&mut c, &mut out, &mut w), FrameStats::default());
        c.set_cursor_visible(true);
        frame(&mut c, &mut out, &mut w);
        assert_eq!(out.inner.pixels, w.screen(&c));
    }

    #[test]
    fn scene_damage_repaints_under_the_cursor_and_keeps_it_on_top() {
        let (mut c, mut out, mut w) = setup(1);
        frame(&mut c, &mut out, &mut w);
        c.move_cursor(10, 10);
        frame(&mut c, &mut out, &mut w);
        let r = Rect::new(8, 8, 8, 8);
        w.set(r, 77);
        c.damage(r);
        let s = frame(&mut c, &mut out, &mut w);
        assert_eq!((s.repainted, s.presented), (64, 64));
        assert_eq!(out.inner.pixels, w.screen(&c));
    }

    #[test]
    fn presented_frames_equal_a_full_redraw_after_every_step_of_a_random_sequence() {
        for (align, seed) in [(1, 0x1234_5678), (8, 0x0BAD_CAFE)] {
            let (mut c, mut out, mut w) = setup(align);
            let mut rng = crate::damage::tests::Rng(seed);
            frame(&mut c, &mut out, &mut w);
            for step in 0..600 {
                for _ in 0..rng.below(4) {
                    let r = Rect::new(
                        rng.below(80) as i32 - 8,
                        rng.below(56) as i32 - 8,
                        rng.below(20),
                        rng.below(14),
                    );
                    w.set(r, rng.below(240) as u8);
                    c.damage(r);
                }
                for _ in 0..rng.below(3) {
                    c.move_cursor(rng.below(110) as i32 - 25, rng.below(80) as i32 - 20);
                }
                if rng.below(40) == 0 {
                    c.set_cursor_visible(rng.below(2) == 0);
                }
                let s = frame(&mut c, &mut out, &mut w);
                assert!(s.rects as usize <= crate::damage::MAX_RECTS);
                assert_eq!(out.inner.pixels, w.screen(&c), "step {step} align {align}");
                assert_eq!(
                    c.back().pixels(),
                    &w.pixels[..],
                    "step {step}: the cursor leaked into the back buffer"
                );
                let palette: Vec<Rgb6> = (0..=255u8).map(|i| Rgb6::new(i, i >> 2, i >> 4)).collect();
                let want = FrameDigest::from_rows(W, H, &palette, |y, b| {
                    b.copy_from_slice(&out.inner.pixels[(y * W) as usize..((y + 1) * W) as usize])
                });
                assert_eq!(c.digest(&palette), want, "step {step}");
            }
        }
    }

    #[test]
    fn a_mismatched_presenter_is_refused_before_anything_is_painted() {
        let (mut c, _, mut w) = setup(1);
        let mut small = MemoryPresent::new(W - 8, H, 1);
        assert_eq!(c.frame(&mut small, |s, r| w.paint(s, r)), Err(HalError::InvalidArgument));
        assert_eq!(w.paints, 0);
        let mut odd = MemoryPresent::new(W, H, 24);
        assert_eq!(c.frame(&mut odd, |s, r| w.paint(s, r)), Err(HalError::InvalidArgument));
        assert_eq!(w.paints, 0);
        // The damage is still there for a presenter that fits.
        let mut ok = Counting::new(1);
        let s = frame(&mut c, &mut ok, &mut w);
        assert_eq!(s.presented, W * H);
    }

    #[test]
    fn a_failing_presenter_leaves_the_damage_for_the_next_frame() {
        let (mut c, mut out, mut w) = setup(1);
        out.fail_after = Some(5);
        assert_eq!(c.frame(&mut out, |s, r| w.paint(s, r)), Err(HalError::Timeout));
        out.fail_after = None;
        let s = frame(&mut c, &mut out, &mut w);
        assert_eq!(s.presented, W * H);
        assert_eq!(out.inner.pixels, w.screen(&c));
    }

    #[test]
    fn digest_includes_the_cursor_but_back_does_not() {
        let (mut c, mut out, mut w) = setup(1);
        frame(&mut c, &mut out, &mut w);
        let palette: Vec<Rgb6> = vec![Rgb6::new(9, 9, 9); 256];
        let d0 = c.digest(&palette);
        let bare = FrameDigest::of(c.back(), &palette);
        // The palette is flat, so only the index digest can tell the cursor from the scene.
        assert_ne!(d0.index, bare.index);
        c.set_cursor_visible(false);
        assert_eq!(c.digest(&palette).index, bare.index);
    }
}
