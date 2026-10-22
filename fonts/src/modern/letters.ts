// The skeletons of the outline faces' letters and figures: one drawing per character, parametric in the style, so
// Desk Sans, Desk Mono and Desk Serif share their construction and differ in weight, width, contrast and serifs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { rect } from './geom';
import type { Kit } from './kit';

/** What a glyph function returns: its body width (ink box) and the kind of each side, for spacing. */
export interface Drawn {
  w: number;
  /** Left and right side: straight, round, diagonal, open (an arm or terminal), or a fixed bearing in units. */
  sides: [Side, Side];
  /** A fixed advance with the ink centred, for tabular figures. */
  fixed?: number;
}
export type Side = 's' | 'r' | 'd' | 'o' | number;

export type Draw = (k: Kit) => Drawn;

// ---- shared parts ----

/** An arch like the shoulder of n: from the stem at `x0` over to a right stem at `x1`, which runs down to `y0`. */
function arch(k: Kit, x0: number, x1: number, top: number, y0: number, v = k.V, serifRight = true): void {
  const ry = Math.min((x1 - x0) * 0.62, top - y0);
  const cy = top + k.os - ry;
  k.arc(x0, x1, cy - ry, top + k.os, 0, 162, v, k.H);
  k.stem(x1 - v, y0, cy + 1, { w: v, bottom: serifRight ? 'both' : 'none' });
}

/** The mirror: a cup like the bottom of u, from a left stem down and round to the right stem at x1. */
function cup(k: Kit, x0: number, x1: number, top: number, v = k.V): void {
  const ry = Math.min((x1 - x0) * 0.62, top);
  const cy = -k.os + ry;
  k.arc(x0, x1, -k.os, cy + ry, 180, 360, v, k.H);
  k.stem(x0, cy - 1, top, { w: v, top: 'left', bottom: 'none' });
}

/** A D-shaped bowl on the right of a stem: bars from x0, a half-round on the right, from y0 to y1. */
function dbowl(k: Kit, x0: number, x1: number, y0: number, y1: number, v = k.V, h = k.H): void {
  const r = Math.min((y1 - y0) / 2, (x1 - x0) * 0.9);
  k.arc(x1 - 2 * r, x1, y0, y1, -90, 90, v, h);
  k.add(rect(x0, y0, x1 - r + 1, y0 + h), rect(x0, y1 - h, x1 - r + 1, y1));
}

/** A full bowl for b d p q: a ring of the body's width at the x-height. */
const bowl = (k: Kit, x0: number, x1: number): void => {
  k.ring(x0, x1, -k.os, k.xh + k.os);
};

/** Serif-face foot serifs for a diagonal letter, centred under x. */
function footSerif(k: Kit, x: number, y = 0, half = k.s.serifLen + k.V * 0.45): void {
  if (!k.serif) return;
  k.add(rect(x - half, y, x + half, y + k.s.serifH));
}
function headSerif(k: Kit, x: number, y: number, half = k.s.serifLen + k.V * 0.45): void {
  if (!k.serif) return;
  k.add(rect(x - half, y - k.s.serifH, x + half, y));
}

// ---- lowercase ----

const L: Record<string, Draw> = {
  a: (k) => {
    if (k.bold && k.s.kind === 'sans') return sansBoldA(k);
    const w = k.w(470, 420);
    const top = k.xh;
    // The stem, the shoulder and the bowl.
    const ry = w * 0.6;
    const cy = top + k.os - ry;
    k.arc(0, w, cy - ry, top + k.os, 0, 152, k.V, k.H);
    k.stem(w - k.V, 0, cy + 1, { bottom: k.serif ? 'right' : 'none' });
    const bt = Math.round(k.xh * (k.bold ? 0.62 : 0.58));
    k.ring(0, w - k.V * 0.4, -k.os, bt + k.H * 0.5);
    k.add(rect(w * 0.5, bt + k.H * 0.5 - k.H, w - k.V * 0.5, bt + k.H * 0.5));
    return { w, sides: ['r', 's'] };
  },
  b: (k) => {
    const w = k.w(510, 440);
    k.stem(0, 0, k.asc, { top: 'left', bottom: k.serif ? 'left' : 'none' });
    bowl(k, 0, w);
    return { w, sides: ['s', 'r'] };
  },
  c: (k) => {
    const w = k.w(460, 420);
    k.arc(0, w, -k.os, k.xh + k.os, 42, 318);
    k.spur(0, w, -k.os, k.xh + k.os, 42, -1, k.xh * 0.2);
    return { w, sides: ['r', 'o'] };
  },
  d: (k) => {
    const w = k.w(510, 440);
    bowl(k, 0, w);
    k.stem(w - k.V, 0, k.asc, { top: 'left', bottom: k.serif ? 'right' : 'none' });
    return { w, sides: ['r', 's'] };
  },
  e: (k) => {
    const sansBold = k.bold && k.s.kind === 'sans';
    const w = k.w(sansBold ? 500 : 484, 432);
    // Sans Bold lifts the bar so the eye stays open above it.
    const cy = Math.round((k.xh + k.os - k.os) / 2) + (sansBold ? 12 : 6);
    k.arc(0, w, -k.os, k.xh + k.os, 0, 330);
    k.add(rect(k.V * 0.5, cy - k.H / 2, w, cy + k.H / 2));
    return { w, sides: ['r', 'r'] };
  },
  f: (k) => {
    const w = k.w(300, 380);
    const x = k.mono ? Math.round(w * 0.36) : Math.round(w * 0.2);
    const top = k.asc + k.os;
    const ry = w * 0.6;
    k.arc(x, x + (w - x) * 2 - k.V * 0.2, top - 2 * ry, top, 52, 180, k.V, k.H);
    k.stem(x, 0, top - ry + 1, { bottom: k.serif ? 'both' : 'none' });
    k.add(rect(k.mono ? x - w * 0.3 : 0, k.xh - k.H, Math.min(w, x + k.V + w * 0.45), k.xh));
    return { w, sides: ['o', k.mono ? 'o' : -24] };
  },
  g: (k) => {
    const w = k.w(510, 440);
    bowl(k, 0, w);
    const ry = k.mono ? 150 : 160;
    k.stem(w - k.V, k.desc + ry - k.os, k.xh, { top: k.serif ? 'right' : 'none', bottom: 'none' });
    k.arc(0, w, k.desc - k.os, k.desc - k.os + 2 * ry, 200, 360, k.V, k.H);
    return { w, sides: ['r', 's'] };
  },
  h: (k) => {
    const w = k.w(470, 432);
    k.stem(0, 0, k.asc, { top: 'left' });
    arch(k, 0, w, k.xh, 0);
    return { w, sides: ['s', 's'] };
  },
  i: (k) => {
    if (k.mono) {
      // Slabs at the head and the foot keep the mono i as wide as its neighbours.
      const w = 400;
      const x = Math.round((w - k.V) / 2) + 10;
      k.stem(x, 0, k.xh);
      k.add(rect(x - 120, k.xh - k.H, x + k.V, k.xh), rect(0, 0, w, k.H));
      k.dot(x + k.V / 2, k.xh + (k.asc - k.xh) * 0.55 + 10);
      return { w, sides: ['o', 'o'] };
    }
    const w = k.V;
    k.stem(0, 0, k.xh, { top: 'left' });
    k.dot(k.V / 2, k.xh + (k.asc - k.xh) * 0.55 + 10);
    return { w, sides: ['s', 's'] };
  },
  j: (k) => {
    const w = k.mono ? 360 : k.V + 70;
    const x = w - k.V;
    const ry = 150;
    k.stem(x, k.desc + ry - k.os, k.xh, { top: 'left', bottom: 'none' });
    k.arc(x - 2 * (w - k.V * 0.3), w, k.desc - k.os, k.desc - k.os + 2 * ry, 270, 360, k.V, k.H);
    if (k.mono) k.add(rect(x - 150, k.xh - k.H, x + k.V, k.xh));
    k.dot(x + k.V / 2, k.xh + (k.asc - k.xh) * 0.55 + 10);
    return { w, sides: ['o', 's'] };
  },
  k: (k) => {
    const w = k.w(450, 430);
    k.stem(0, 0, k.asc, { top: 'left' });
    const jy = k.xh * 0.38;
    k.diag(k.V * 0.6, jy - 20, w - k.V * 0.35, k.xh, k.V * 0.92);
    k.diag(w - k.V * 0.35, 0, k.V + (w - k.V) * 0.22, k.xh * 0.52, k.V * 0.98);
    if (k.serif) {
      headSerif(k, w - k.V * 0.4, k.xh + k.s.serifH, k.s.serifLen + 10);
      footSerif(k, w - k.V * 0.35, 0, k.s.serifLen + 14);
    }
    return { w, sides: ['s', 'd'] };
  },
  l: (k) => {
    if (k.mono) {
      // A tailed l with a head serif, so it never reads as I or 1.
      const w = 420;
      const x = Math.round(w / 2 - k.V / 2) - 20;
      k.add(rect(x - 110, k.asc - k.H, x + k.V, k.asc));
      const ry = 130;
      k.stem(x, ry - k.os, k.asc);
      k.arc(x, x + 2 * (w - x) - k.V * 0.2, -k.os, 2 * ry - k.os, 180, 290, k.V, k.H);
      return { w, sides: ['o', 'o'] };
    }
    k.stem(0, 0, k.asc, { top: 'left' });
    return { w: k.V, sides: ['s', 's'] };
  },
  m: (k) => {
    const v = k.mono ? Math.round(k.V * 0.86) : k.V;
    const w = k.w(740, 500);
    const mid = Math.round((w - v) / 2);
    k.stem(0, 0, k.xh, { top: 'left', w: v });
    arch(k, 0, mid + v, k.xh, 0, v);
    arch(k, mid, w, k.xh, 0, v);
    return { w, sides: ['s', 's'] };
  },
  n: (k) => {
    const w = k.w(470, 432);
    k.stem(0, 0, k.xh, { top: 'left' });
    arch(k, 0, w, k.xh, 0);
    return { w, sides: ['s', 's'] };
  },
  o: (k) => {
    const w = k.w(510, 446);
    k.ring(0, w, -k.os, k.xh + k.os);
    return { w, sides: ['r', 'r'] };
  },
  p: (k) => {
    const w = k.w(510, 440);
    k.stem(0, k.desc, k.xh, { top: 'left', bottom: 'both' });
    bowl(k, 0, w);
    return { w, sides: ['s', 'r'] };
  },
  q: (k) => {
    const w = k.w(510, 440);
    bowl(k, 0, w);
    k.stem(w - k.V, k.desc, k.xh, { top: k.serif ? 'right' : 'none', bottom: 'both' });
    return { w, sides: ['r', 's'] };
  },
  r: (k) => {
    const w = k.w(320, 400);
    const x = k.mono ? 60 : 0;
    k.stem(x, 0, k.xh, { top: 'left', bottom: k.serif ? 'both' : 'none' });
    const ry = k.xh * 0.6;
    k.arc(x, x + (w - x) * 1.7, k.xh + k.os - 2 * ry, k.xh + k.os, 70, 162, k.V, k.H);
    k.spur(x, x + (w - x) * 1.7, k.xh + k.os - 2 * ry, k.xh + k.os, 70, -1, k.xh * 0.16);
    return { w, sides: [k.mono ? 'o' : 's', 'o'] };
  },
  s: (k) => {
    const w = k.w(420, 400);
    sCurve(k, w, k.xh);
    return { w, sides: ['r', 'r'] };
  },
  t: (k) => {
    const w = k.w(300, 380);
    const x = k.mono ? Math.round(w * 0.3) : Math.round(w * 0.16);
    const ry = 130;
    k.stem(x, ry - k.os, k.xh + Math.round((k.asc - k.xh) * 0.62), { bottom: 'none' });
    k.arc(x, x + 2 * (w - x) - k.V * 0.1, -k.os, 2 * ry - k.os, 180, 300, k.V, k.H);
    k.add(rect(k.mono ? x - w * 0.24 : 0, k.xh - k.H, Math.min(w, x + k.V + w * 0.5), k.xh));
    return { w, sides: ['o', k.mono ? 'o' : 14] };
  },
  u: (k) => {
    const w = k.w(470, 432);
    cup(k, 0, w, k.xh);
    k.stem(w - k.V, 0, k.xh, { top: 'left', bottom: k.serif ? 'right' : 'none' });
    return { w, sides: ['s', 's'] };
  },
  v: (k) => {
    const w = k.w(470, 450);
    const d = k.V * 0.94;
    k.diag(w / 2, 0, d / 2, k.xh, d);
    k.diag(w / 2, 0, w - d / 2, k.xh, d * 0.92);
    headSerif(k, d / 2, k.xh, k.s.serifLen + 14);
    headSerif(k, w - d / 2, k.xh, k.s.serifLen + 14);
    return { w, sides: ['d', 'd'] };
  },
  w: (k) => {
    const w = k.w(700, 520);
    const d = k.mono ? k.V * 0.8 : k.V * 0.9;
    const q = w / 4;
    k.diag(q, 0, d / 2, k.xh, d);
    k.diag(q, 0, w / 2, k.xh * 0.86, d * 0.9);
    k.diag(3 * q, 0, w / 2, k.xh * 0.86, d * 0.9);
    k.diag(3 * q, 0, w - d / 2, k.xh, d);
    headSerif(k, d / 2, k.xh, k.s.serifLen + 10);
    headSerif(k, w - d / 2, k.xh, k.s.serifLen + 10);
    return { w, sides: ['d', 'd'] };
  },
  x: (k) => {
    const w = k.w(450, 440);
    const d = k.V * 0.94;
    k.diag(d / 2, 0, w - d / 2, k.xh, d);
    k.diag(w - d / 2, 0, d / 2, k.xh, d * 0.92);
    footSerif(k, d / 2, 0, k.s.serifLen + 12);
    footSerif(k, w - d / 2, 0, k.s.serifLen + 12);
    headSerif(k, d / 2, k.xh, k.s.serifLen + 12);
    headSerif(k, w - d / 2, k.xh, k.s.serifLen + 12);
    return { w, sides: ['d', 'd'] };
  },
  y: (k) => {
    const w = k.w(470, 450);
    const d = k.V * 0.94;
    const bx = w * 0.5;
    k.diag(bx, 0, d / 2, k.xh, d);
    // The right arm runs on, straight, into the descender.
    const slope = (w - d / 2 - bx) / k.xh;
    const tailX = bx + slope * (k.desc + 20);
    k.diag(tailX, k.desc + 20, w - d / 2, k.xh, d * 0.92);
    k.add(rect(tailX - (k.mono ? 120 : 80), k.desc, tailX + d * 0.4, k.desc + k.H));
    headSerif(k, d / 2, k.xh, k.s.serifLen + 14);
    headSerif(k, w - d / 2, k.xh, k.s.serifLen + 14);
    return { w, sides: ['d', 'd'] };
  },
  z: (k) => {
    const w = k.w(420, 400);
    k.add(rect(k.serif ? 20 : 10, k.xh - k.H, w, k.xh), rect(0, 0, w, k.H));
    k.diag(k.V * 0.55, k.H * 0.5, w - k.V * 0.55, k.xh - k.H * 0.5, k.V * 0.98);
    if (k.serif) k.add(rect(w - k.V * 0.35, 0, w, k.H + 70), rect(20, k.xh - k.H - 60, 20 + k.V * 0.35, k.xh));
    return { w, sides: ['d', 'd'] };
  },
  ı: (k) => {
    if (k.mono) {
      const w = 400;
      const x = Math.round((w - k.V) / 2) + 10;
      k.stem(x, 0, k.xh);
      k.add(rect(x - 120, k.xh - k.H, x + k.V, k.xh), rect(0, 0, w, k.H));
      return { w, sides: ['o', 'o'] };
    }
    k.stem(0, 0, k.xh, { top: 'left' });
    return { w: k.V, sides: ['s', 's'] };
  },
};

/**
 * Sans Bold's a: a shallower shoulder and a lower, wider bowl than the Regular's, so that with heavy strokes
 * both counters stay open, and the bowl meets the stem with a notch rather than clotting.
 */
function sansBoldA(k: Kit): Drawn {
  const w = k.w(486, 420);
  const bowlTop = Math.round(k.xh * 0.54);
  // The shoulder: the top of a bowl whose bottom sits on the lower bowl's top.
  const yb = bowlTop - k.H * 0.6;
  k.arc(0, w, yb, k.xh + k.os, 0, 150, k.V, k.H);
  const cy = (yb + k.xh + k.os) / 2;
  k.stem(w - k.V, 0, cy + 1);
  // The bowl stops short of the stem's right edge, so where its top curves away from the stem it leaves a
  // notch open to the upper counter instead of a pinhole.
  k.ring(0, w - k.V * 0.55, -k.os, bowlTop + k.H * 0.4, k.V * 0.96, k.H);
  return { w, sides: ['r', 's'] };
}

/** The s-curve of s and S: two arcs that share the spine, with level terminals. */
function sCurve(k: Kit, w: number, h: number): void {
  const mid = Math.round(h * 0.52);
  const inset = w * 0.04;
  k.arc(inset, w - inset, mid - k.H / 2, h + k.os, 28, 270, k.V * 0.92, k.H);
  k.arc(0, w, -k.os, mid + k.H / 2, 208, 450, k.V * 0.96, k.H);
  k.spur(inset, w - inset, mid - k.H / 2, h + k.os, 28, -1, h * 0.17, k.V * 0.92);
  k.spur(0, w, -k.os, mid + k.H / 2, 208, 1, h * 0.17, k.V * 0.96);
}

// ---- capitals ----

const U: Record<string, Draw> = {
  A: (k) => {
    const w = k.w(640, 470);
    const d = k.V * 0.98;
    k.diag(d / 2, 0, w / 2, k.cap, d);
    k.diag(w - d / 2, 0, w / 2, k.cap, d * 0.92);
    const by = Math.round(k.cap * 0.28);
    k.add(rect(w * 0.2, by, w * 0.8, by + k.H));
    footSerif(k, d / 2, 0, k.s.serifLen + 16);
    footSerif(k, w - d / 2, 0, k.s.serifLen + 16);
    return { w, sides: ['d', 'd'] };
  },
  B: (k) => {
    const w = k.w(540, 430);
    k.stem(0, 0, k.cap, { top: 'right', bottom: 'right' });
    const mid = Math.round(k.cap * 0.53);
    dbowl(k, 0, w - w * 0.06, mid - k.H / 2, k.cap);
    dbowl(k, 0, w, 0, mid + k.H / 2);
    if (k.serif) k.add(rect(-k.s.serifLen, 0, 0, k.s.serifH), rect(-k.s.serifLen, k.cap - k.s.serifH, 0, k.cap));
    return { w, sides: ['s', 'r'] };
  },
  C: (k) => {
    const w = k.w(640, 450);
    k.arc(0, w, -k.os, k.cap + k.os, 40, 320, k.V * 1.02, k.H);
    k.spur(0, w, -k.os, k.cap + k.os, 40, -1, k.cap * 0.2, k.V * 1.02);
    return { w, sides: ['r', 'o'] };
  },
  D: (k) => {
    const w = k.w(640, 450);
    k.stem(0, 0, k.cap, { top: 'right', bottom: 'right' });
    dbowl(k, 0, w, 0, k.cap, k.V * 1.04);
    if (k.serif) k.add(rect(-k.s.serifLen, 0, 0, k.s.serifH), rect(-k.s.serifLen, k.cap - k.s.serifH, 0, k.cap));
    return { w, sides: ['s', 'r'] };
  },
  E: (k) => {
    const w = k.w(490, 420);
    k.stem(0, 0, k.cap, { top: 'left', bottom: 'left' });
    k.add(rect(0, k.cap - k.H, w, k.cap), rect(0, 0, w + 6, k.H), rect(0, k.cap * 0.5 - k.H / 2 + 6, w * 0.88, k.cap * 0.5 + k.H / 2 + 6));
    if (k.serif) k.add(rect(w - k.V * 0.32, k.cap - 150, w, k.cap), rect(w + 6 - k.V * 0.32, 0, w + 6, 150), rect(w * 0.88 - k.V * 0.3, k.cap * 0.5 - 70, w * 0.88, k.cap * 0.5 + 80));
    return { w: w + 6, sides: ['s', 'o'] };
  },
  F: (k) => {
    const w = k.w(470, 420);
    k.stem(0, 0, k.cap, { top: 'left', bottom: 'both' });
    k.add(rect(0, k.cap - k.H, w, k.cap), rect(0, k.cap * 0.48 - k.H / 2, w * 0.86, k.cap * 0.48 + k.H / 2));
    if (k.serif) k.add(rect(w - k.V * 0.32, k.cap - 150, w, k.cap), rect(w * 0.86 - k.V * 0.3, k.cap * 0.48 - 70, w * 0.86, k.cap * 0.48 + 80));
    return { w, sides: ['s', 'o'] };
  },
  G: (k) => {
    const w = k.w(660, 456);
    k.arc(0, w, -k.os, k.cap + k.os, 40, 360, k.V * 1.02, k.H);
    const gy = Math.round(k.cap * 0.44);
    k.add(rect(w - k.V * 1.02, 2 * k.os, w, gy));
    k.add(rect(w * 0.5, gy - k.H, w, gy));
    k.spur(0, w, -k.os, k.cap + k.os, 40, -1, k.cap * 0.18, k.V * 1.02);
    return { w, sides: ['r', 's'] };
  },
  H: (k) => {
    const w = k.w(620, 430);
    k.stem(0, 0, k.cap, { top: 'both', bottom: 'both' });
    k.stem(w - k.V, 0, k.cap, { top: 'both', bottom: 'both' });
    k.add(rect(0, k.cap * 0.5 - k.H / 2, w, k.cap * 0.5 + k.H / 2));
    return { w, sides: ['s', 's'] };
  },
  I: (k) => {
    if (k.mono || k.serif) {
      const w = k.mono ? 400 : k.V + 2 * k.s.serifLen;
      const x = (w - k.V) / 2;
      k.stem(x, 0, k.cap, { top: 'both', bottom: 'both' });
      if (k.mono) k.add(rect(0, 0, w, k.H), rect(0, k.cap - k.H, w, k.cap));
      return { w, sides: k.mono ? ['o', 'o'] : [k.s.sb - k.s.serifLen + 10, k.s.sb - k.s.serifLen + 10] };
    }
    k.stem(0, 0, k.cap);
    return { w: k.V, sides: ['s', 's'] };
  },
  J: (k) => {
    const w = k.w(440, 420);
    const ry = 170;
    k.stem(w - k.V, ry - k.os, k.cap, { top: 'both', bottom: 'none' });
    k.arc(0, w, -k.os, 2 * ry - k.os, 180, 360, k.V, k.H);
    k.add(rect(0, ry - k.os - 40, k.V * 0.92, ry - k.os + 10));
    return { w, sides: ['o', 's'] };
  },
  K: (k) => {
    const w = k.w(580, 450);
    k.stem(0, 0, k.cap, { top: 'both', bottom: 'both' });
    const jy = k.cap * 0.36;
    k.diag(k.V * 0.6, jy - 30, w - k.V * 0.4, k.cap, k.V * 0.92);
    k.diag(w - k.V * 0.4, 0, k.V + (w - k.V) * 0.26, k.cap * 0.56, k.V);
    headSerif(k, w - k.V * 0.45, k.cap, k.s.serifLen + 12);
    footSerif(k, w - k.V * 0.4, 0, k.s.serifLen + 14);
    return { w, sides: ['s', 'd'] };
  },
  L: (k) => {
    const w = k.w(460, 410);
    k.stem(0, 0, k.cap, { top: 'both', bottom: 'left' });
    k.add(rect(0, 0, w, k.H));
    if (k.serif) k.add(rect(w - k.V * 0.32, 0, w, 160));
    return { w, sides: ['s', 'o'] };
  },
  M: (k) => {
    const w = k.w(760, 480);
    const v = k.mono ? k.V * 0.86 : k.V;
    k.stem(0, 0, k.cap, { top: k.serif ? 'left' : 'none', bottom: 'both', w: v });
    k.stem(w - v, 0, k.cap, { top: k.serif ? 'right' : 'none', bottom: 'both', w: v });
    const vy = k.mono ? k.cap * 0.32 : 0;
    k.diag(v / 2 + 4, k.cap, w / 2, vy, v * 0.9);
    k.diag(w - v / 2 - 4, k.cap, w / 2, vy, v * 0.9);
    if (!k.serif) k.within(0, 0, w, k.cap);
    return { w, sides: ['s', 's'] };
  },
  N: (k) => {
    const w = k.w(630, 440);
    const v = k.mono ? k.V * 0.92 : k.V;
    k.stem(0, 0, k.cap, { top: k.serif ? 'left' : 'none', bottom: 'both', w: v * (k.serif ? 0.5 : 1) });
    k.stem(w - v * (k.serif ? 0.5 : 1), 0, k.cap, { top: 'both', bottom: 'none', w: v * (k.serif ? 0.5 : 1) });
    k.diag(v / 2, k.cap, w - v / 2, 0, v * 1.04);
    if (!k.serif) k.within(0, 0, w, k.cap);
    else k.within(-200, 0, w + 200, k.cap);
    return { w, sides: ['s', 's'] };
  },
  O: (k) => {
    const w = k.w(690, 456);
    k.ring(0, w, -k.os, k.cap + k.os, k.V * 1.04, k.H * 1.02);
    return { w, sides: ['r', 'r'] };
  },
  P: (k) => {
    const w = k.w(530, 430);
    k.stem(0, 0, k.cap, { top: 'right', bottom: 'both' });
    dbowl(k, 0, w, Math.round(k.cap * 0.44), k.cap);
    if (k.serif) k.add(rect(-k.s.serifLen, k.cap - k.s.serifH, 0, k.cap));
    return { w, sides: ['s', 'r'] };
  },
  Q: (k) => {
    const w = k.w(690, 456);
    k.ring(0, w, -k.os, k.cap + k.os, k.V * 1.04, k.H * 1.02);
    k.diag(w * 0.92, -k.cap * 0.1, w * 0.62, k.cap * 0.24, k.V);
    return { w, sides: ['r', 'r'] };
  },
  R: (k) => {
    const w = k.w(560, 446);
    k.stem(0, 0, k.cap, { top: 'right', bottom: 'both' });
    const by = Math.round(k.cap * 0.42);
    dbowl(k, 0, w - w * 0.03, by, k.cap);
    k.diag(w - k.V * 0.45, 0, w * 0.42, by + k.H * 0.5, k.V);
    footSerif(k, w - k.V * 0.4, 0, k.s.serifLen + 10);
    if (k.serif) k.add(rect(-k.s.serifLen, k.cap - k.s.serifH, 0, k.cap));
    return { w, sides: ['s', 'd'] };
  },
  S: (k) => {
    const w = k.w(540, 440);
    sCurve(k, w, k.cap);
    return { w, sides: ['r', 'r'] };
  },
  T: (k) => {
    const w = k.w(560, 460);
    k.add(rect(0, k.cap - k.H, w, k.cap));
    k.stem((w - k.V) / 2, 0, k.cap, { bottom: 'both' });
    if (k.serif) k.add(rect(0, k.cap - 150, k.V * 0.32, k.cap), rect(w - k.V * 0.32, k.cap - 150, w, k.cap));
    return { w, sides: ['o', 'o'] };
  },
  U: (k) => {
    const w = k.w(620, 440);
    const ry = Math.round(w * 0.5);
    k.arc(0, w, -k.os, 2 * ry - k.os, 180, 360, k.V, k.H);
    k.stem(0, ry - k.os - 1, k.cap, { top: 'both', bottom: 'none' });
    k.stem(w - k.V, ry - k.os - 1, k.cap, { top: 'both', bottom: 'none' });
    return { w, sides: ['s', 's'] };
  },
  V: (k) => {
    const w = k.w(620, 470);
    const d = k.V * 0.98;
    k.diag(w / 2, 0, d / 2, k.cap, d);
    k.diag(w / 2, 0, w - d / 2, k.cap, d * 0.9);
    headSerif(k, d / 2, k.cap, k.s.serifLen + 16);
    headSerif(k, w - d / 2, k.cap, k.s.serifLen + 16);
    return { w, sides: ['d', 'd'] };
  },
  W: (k) => {
    const w = k.w(900, 540);
    const d = k.mono ? k.V * 0.78 : k.V * 0.94;
    const q = w / 4;
    k.diag(q * 0.95, 0, d / 2, k.cap, d);
    k.diag(q * 0.95, 0, w / 2, k.cap, d * 0.88);
    k.diag(w - q * 0.95, 0, w / 2, k.cap, d * 0.88);
    k.diag(w - q * 0.95, 0, w - d / 2, k.cap, d);
    headSerif(k, d / 2, k.cap, k.s.serifLen + 10);
    headSerif(k, w - d / 2, k.cap, k.s.serifLen + 10);
    return { w, sides: ['d', 'd'] };
  },
  X: (k) => {
    const w = k.w(600, 460);
    const d = k.V * 0.98;
    k.diag(d / 2, 0, w - d / 2, k.cap, d);
    k.diag(w - d / 2, 0, d / 2, k.cap, d * 0.9);
    for (const x of [d / 2, w - d / 2]) {
      footSerif(k, x, 0, k.s.serifLen + 14);
      headSerif(k, x, k.cap, k.s.serifLen + 14);
    }
    return { w, sides: ['d', 'd'] };
  },
  Y: (k) => {
    const w = k.w(600, 470);
    const d = k.V * 0.98;
    const jy = Math.round(k.cap * 0.42);
    k.diag(w / 2, jy, d / 2, k.cap, d);
    k.diag(w / 2, jy, w - d / 2, k.cap, d * 0.9);
    k.stem((w - k.V) / 2, 0, jy + 10, { bottom: 'both' });
    headSerif(k, d / 2, k.cap, k.s.serifLen + 14);
    headSerif(k, w - d / 2, k.cap, k.s.serifLen + 14);
    return { w, sides: ['d', 'd'] };
  },
  Z: (k) => {
    const w = k.w(560, 440);
    k.add(rect(k.serif ? 10 : 20, k.cap - k.H, w, k.cap), rect(0, 0, w, k.H));
    k.diag(k.V * 0.6, k.H * 0.6, w - k.V * 0.6, k.cap - k.H * 0.6, k.V * 1.02);
    if (k.serif) k.add(rect(w - k.V * 0.32, 0, w, 160), rect(10, k.cap - 150, 10 + k.V * 0.32, k.cap));
    return { w, sides: ['d', 'd'] };
  },
};

// ---- figures: tabular, one width for all ten ----

/** The advance of every figure: tabular by default. */
const figAdv = (k: Kit): number => (k.mono ? (k.s.advance as number) : figW(k) + 2 * Math.round(k.s.sb * 0.72));

function figW(k: Kit): number {
  return k.mono ? 440 : Math.round(500 * k.s.wide);
}

const F: Record<string, Draw> = {
  '0': (k) => {
    const w = figW(k);
    k.ring(0, w, -k.os, k.cap + k.os, k.V, k.H);
    if (k.mono) k.dot(w / 2, k.cap / 2, k.dotSize * 1.05);
    return { w, sides: ['r', 'r'], fixed: figAdv(k) };
  },
  '1': (k) => {
    const w = figW(k);
    const x = Math.round(w * 0.52 - k.V / 2);
    k.stem(x, 0, k.cap, { bottom: 'both' });
    k.diag(x + k.V * 0.2 - (w * 0.3), k.cap - 170, x + k.V * 0.5, k.cap, k.V * 0.86);
    if (k.mono || k.serif) k.add(rect(w * 0.12, 0, w * 0.9, k.H));
    return { w, sides: [80, 80], fixed: figAdv(k) };
  },
  '2': (k) => {
    const w = figW(k);
    const ry = k.cap * 0.29;
    k.arc(w * 0.02, w, k.cap + k.os - 2 * ry, k.cap + k.os, -40, 164, k.V, k.H);
    const e = { x: w * 0.83, y: k.cap + k.os - ry * 1.62 };
    k.add([
      { x: 0, y: k.H - 2 },
      { x: k.V * 1.25, y: k.H - 2 },
      { x: e.x + k.V * 0.25, y: e.y + k.V * 0.42 },
      { x: e.x - k.V * 0.7, y: e.y - k.V * 0.15 },
    ]);
    k.add(rect(0, 0, w, k.H));
    return { w, sides: ['r', 'r'], fixed: figAdv(k) };
  },
  '3': (k) => {
    const w = figW(k);
    const mid = Math.round(k.cap * 0.55);
    k.arc(w * 0.06, w * 0.96, mid - k.H / 2, k.cap + k.os, -90, 160, k.V * 0.94, k.H);
    k.arc(0, w, -k.os, mid + k.H / 2, -160, 90, k.V, k.H);
    k.add(rect(w * 0.36, mid - k.H / 2, w * 0.55, mid + k.H / 2));
    return { w, sides: ['r', 'r'], fixed: figAdv(k) };
  },
  '4': (k) => {
    const w = figW(k);
    const x = Math.round(w * 0.66);
    const by = Math.round(k.cap * 0.26);
    k.stem(x, 0, k.cap, { bottom: 'both' });
    k.add(rect(0, by, w, by + k.H));
    k.add([
      { x: 0, y: by },
      { x: k.V * 1.1, y: by },
      { x: x + k.V, y: k.cap },
      { x: x + k.V - k.V * 1.25, y: k.cap },
      { x: 0, y: by + k.H + 4 },
    ]);
    return { w, sides: ['d', 's'], fixed: figAdv(k) };
  },
  '5': (k) => {
    const w = figW(k);
    const top = k.cap;
    const sx = w * 0.06;
    k.add(rect(sx, top - k.H, w * 0.92, top));
    const bt = k.cap * 0.62;
    if (k.bold && k.s.kind === 'sans') {
      // Sans Bold: a plain upright stem, so its foot lines up with the bowl's bar without a step.
      k.add(rect(sx + 6, bt - k.H, sx + 6 + k.V, top));
    } else
      k.add([
        { x: sx, y: top },
        { x: sx + k.V, y: top },
        { x: sx + k.V * 0.92, y: k.cap * 0.58 },
        { x: sx - k.V * 0.08 + 6, y: k.cap * 0.55 },
      ]);
    k.arc(0, w, -k.os, bt, -150, 120, k.V, k.H);
    k.add(rect(sx + 6, bt - k.H, w * 0.56, bt));
    return { w, sides: ['r', 'r'], fixed: figAdv(k) };
  },
  '6': (k) => {
    const w = figW(k);
    const bt = k.cap * 0.62;
    k.ring(0, w, -k.os, bt);
    k.arc(0, w * 2.3, -k.os, k.cap + k.os, 112, 180, k.V, k.H);
    k.add(rect(0, k.cap * 0.25, k.V, k.cap * 0.62));
    return { w, sides: ['r', 'r'], fixed: figAdv(k) };
  },
  '7': (k) => {
    const w = figW(k);
    k.add(rect(0, k.cap - k.H, w, k.cap));
    k.diag(w * 0.32, 0, w - k.V * 0.5, k.cap - k.H * 0.5, k.V);
    return { w, sides: ['o', 'd'], fixed: figAdv(k) };
  },
  '8': (k) => {
    const w = figW(k);
    const mid = Math.round(k.cap * 0.56);
    k.ring(w * 0.05, w * 0.95, mid - k.H / 2, k.cap + k.os, k.V * 0.94, k.H);
    k.ring(0, w, -k.os, mid + k.H / 2, k.V, k.H);
    return { w, sides: ['r', 'r'], fixed: figAdv(k) };
  },
  '9': (k) => {
    const w = figW(k);
    const bb = k.cap * 0.38;
    k.ring(0, w, bb, k.cap + k.os);
    k.arc(w - w * 2.3, w, -k.os, k.cap + k.os, -68, 0, k.V, k.H);
    k.add(rect(w - k.V, k.cap * 0.38, w, k.cap * 0.75));
    return { w, sides: ['r', 'r'], fixed: figAdv(k) };
  },
};

export const LETTERS: Record<string, Draw> = { ...L, ...U, ...F };
