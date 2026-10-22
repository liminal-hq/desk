// Italic forms: Desk Serif's true italic lowercase (cursive, with entry and exit strokes instead of slab serifs) and
// the few cursive forms Desk Mono's italic takes. Drawn upright; the assembler slants them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { rect } from './geom';
import type { Kit } from './kit';
import type { Draw, Drawn } from './letters';
import { LETTERS } from './letters';

/** Radius of the exit stroke's turn. */
const TAIL = 78;

/** A stem from y0 up to y1 that turns out to the right at its foot, as a pen leaving the letter. */
function tailStem(k: Kit, x: number, y0: number, y1: number): void {
  const r = TAIL;
  k.add(rect(x, y0 + r, x + k.V, y1));
  k.arc(x, x + k.V + 2 * r * 1.25, y0 - k.os, y0 + 2 * r - k.os, 180, 320, k.V, k.H);
}

/** A pen's entry at the top of a stem: a short hairline coming in from the upper left. */
function entry(k: Kit, x: number, y: number): void {
  k.add([
    { x: x - k.V * 0.55, y: y - k.V * 0.42 },
    { x: x + k.V * 0.15, y: y - k.H * 0.2 },
    { x: x + k.V, y },
    { x, y },
    { x: x - k.V * 0.6, y: y - k.V * 0.42 + k.H * 1.1 },
  ]);
}

/** The arch of n, m and h, without serifs. */
function arch(k: Kit, x0: number, x1: number, v = k.V): void {
  const ry = Math.min((x1 - x0) * 0.66, k.xh);
  const cy = k.xh + k.os - ry;
  k.arc(x0, x1, cy - ry, k.xh + k.os, 0, 165, v, k.H);
  tailStem(k, x1 - v, 0, cy + 1);
}

const bowl = (k: Kit, x0: number, x1: number): void => {
  k.ring(x0, x1, -k.os, k.xh + k.os);
};

/** Desk Serif Italic's lowercase. */
export const SERIF_ITALIC: Record<string, Draw> = {
  a: (k) => {
    const w = k.w(470);
    bowl(k, 0, w - k.V * 0.3);
    tailStem(k, w - k.V, 0, k.xh + k.os);
    return { w: w + TAIL, sides: ['r', 6] };
  },
  b: (k) => {
    const w = k.w(480);
    k.add(rect(0, 0, k.V, k.asc));
    entry(k, 0, k.asc);
    bowl(k, 0, w);
    return { w, sides: ['o', 'r'] };
  },
  c: (k) => {
    const w = k.w(430);
    k.arc(0, w, -k.os, k.xh + k.os, 48, 320);
    k.dot(w - k.dotSize * 0.4, k.xh - k.dotSize * 0.25, k.dotSize * 0.82);
    return { w, sides: ['r', 6] };
  },
  d: (k) => {
    const w = k.w(470);
    bowl(k, 0, w - k.V * 0.3);
    tailStem(k, w - k.V, 0, k.asc);
    entry(k, w - k.V, k.asc);
    return { w: w + TAIL, sides: ['r', 6] };
  },
  e: (k) => {
    const w = k.w(440);
    const cy = Math.round(k.xh * 0.5) + 14;
    k.arc(0, w, -k.os, k.xh + k.os, 10, 320);
    // A slanted crossbar, rising to the right, as a pen draws the eye.
    k.add([
      { x: k.V * 0.5, y: cy - k.H * 0.6 },
      { x: w - k.V * 0.3, y: cy + k.H * 0.2 + 30 },
      { x: w - k.V * 0.3, y: cy + k.H * 1.2 + 30 },
      { x: k.V * 0.5, y: cy + k.H * 0.4 },
    ]);
    return { w, sides: ['r', 'r'] };
  },
  f: (k) => {
    // The italic f runs through the descender, hooked at both ends.
    const w = k.w(360);
    const x = Math.round(w * 0.42);
    const r = 120;
    k.add(rect(x, k.desc + r, x + k.V * 0.9, k.asc - r + 4));
    k.arc(x, x + 2 * (w - x), k.asc + k.os - 2 * r, k.asc + k.os, 40, 180, k.V * 0.9, k.H);
    k.arc(x + k.V * 0.9 - 2 * (x + k.V * 0.9), x + k.V * 0.9, k.desc - k.os, k.desc + 2 * r - k.os, 220, 360, k.V * 0.9, k.H);
    k.add(rect(x - w * 0.3, k.xh - k.H, x + k.V + w * 0.32, k.xh));
    return { w, sides: [-10, -10] };
  },
  g: (k) => {
    const w = k.w(470);
    bowl(k, 0, w - k.V * 0.2);
    const ry = 150;
    k.add(rect(w - k.V, k.desc + ry - k.os, w, k.xh + k.os));
    k.arc(0, w, k.desc - k.os, k.desc - k.os + 2 * ry, 200, 360, k.V, k.H);
    return { w, sides: ['r', 's'] };
  },
  h: (k) => {
    const w = k.w(460);
    k.add(rect(0, 0, k.V, k.asc));
    entry(k, 0, k.asc);
    arch(k, 0, w);
    return { w: w + TAIL, sides: ['o', 6] };
  },
  i: (k) => {
    tailStem(k, 0, 0, k.xh);
    entry(k, 0, k.xh);
    k.dot(k.V / 2 + 14, k.xh + (k.asc - k.xh) * 0.55 + 10);
    return { w: k.V + TAIL, sides: ['o', 6] };
  },
  ı: (k) => {
    tailStem(k, 0, 0, k.xh);
    entry(k, 0, k.xh);
    return { w: k.V + TAIL, sides: ['o', 6] };
  },
  j: (k) => {
    const w = k.V + 70;
    const x = w - k.V;
    const ry = 150;
    k.add(rect(x, k.desc + ry - k.os, x + k.V, k.xh));
    entry(k, x, k.xh);
    k.arc(x - 2 * (w - k.V * 0.3), w, k.desc - k.os, k.desc - k.os + 2 * ry, 250, 360, k.V, k.H);
    k.dot(x + k.V / 2 + 14, k.xh + (k.asc - k.xh) * 0.55 + 10);
    return { w, sides: ['o', 's'] };
  },
  k: (k) => {
    const w = k.w(430);
    k.add(rect(0, 0, k.V, k.asc));
    entry(k, 0, k.asc);
    // A looped arm: a small bowl off the stem, then the leg leaving to the right.
    k.arc(k.V * 0.4, w - k.V * 0.2, k.xh * 0.3, k.xh + k.os, -60, 175, k.V * 0.9, k.H);
    k.diag(k.V + (w - k.V) * 0.36, k.xh * 0.36, w - k.V * 0.3, 0, k.V * 0.95);
    return { w, sides: ['o', 'd'] };
  },
  l: (k) => {
    tailStem(k, 0, 0, k.asc);
    entry(k, 0, k.asc);
    return { w: k.V + TAIL, sides: ['o', 6] };
  },
  m: (k) => {
    const w = k.w(700);
    const mid = Math.round((w - k.V) / 2);
    k.add(rect(0, 0, k.V, k.xh));
    entry(k, 0, k.xh);
    const ry = Math.min((mid + k.V) * 0.66, k.xh);
    k.arc(0, mid + k.V, k.xh + k.os - 2 * ry, k.xh + k.os, 0, 165, k.V, k.H);
    k.add(rect(mid, 0, mid + k.V, k.xh + k.os - ry + 1));
    arch(k, mid, w);
    return { w: w + TAIL, sides: ['o', 6] };
  },
  n: (k) => {
    const w = k.w(460);
    k.add(rect(0, 0, k.V, k.xh));
    entry(k, 0, k.xh);
    arch(k, 0, w);
    return { w: w + TAIL, sides: ['o', 6] };
  },
  o: (k) => {
    const w = k.w(470);
    bowl(k, 0, w);
    return { w, sides: ['r', 'r'] };
  },
  p: (k) => {
    const w = k.w(480);
    k.add(rect(0, k.desc, k.V, k.xh));
    entry(k, 0, k.xh);
    k.serifs(0, k.V, k.desc, 'both', 'bottom');
    bowl(k, 0, w);
    return { w, sides: ['o', 'r'] };
  },
  q: (k) => {
    const w = k.w(470);
    bowl(k, 0, w);
    k.add(rect(w - k.V, k.desc, w, k.xh + k.os));
    k.serifs(w - k.V, k.V, k.desc, 'both', 'bottom');
    return { w, sides: ['r', 's'] };
  },
  r: (k) => {
    const w = k.w(330);
    k.add(rect(0, 0, k.V, k.xh));
    entry(k, 0, k.xh);
    const ry = k.xh * 0.6;
    k.arc(0, w * 1.6, k.xh + k.os - 2 * ry, k.xh + k.os, 72, 165, k.V, k.H);
    k.dot(w - k.dotSize * 0.2, k.xh - k.dotSize * 0.35, k.dotSize * 0.8);
    return { w, sides: ['o', 6] };
  },
  t: (k) => {
    const w = k.w(300);
    const x = Math.round(w * 0.2);
    tailStem(k, x, 0, k.xh + Math.round((k.asc - k.xh) * 0.55));
    k.add(rect(0, k.xh - k.H, Math.min(w, x + k.V + w * 0.45), k.xh));
    return { w: w + TAIL * 0.6, sides: ['o', 'o'] };
  },
  u: (k) => {
    const w = k.w(460);
    const ry = Math.min(w * 0.62, k.xh);
    const cy = -k.os + ry;
    k.arc(0, w, -k.os, cy + ry, 180, 360, k.V, k.H);
    k.add(rect(0, cy - 1, k.V, k.xh));
    entry(k, 0, k.xh);
    tailStem(k, w - k.V, 0, k.xh);
    entry(k, w - k.V, k.xh);
    return { w: w + TAIL, sides: ['o', 6] };
  },
  v: (k) => {
    // A round-bottomed v: down the left, round the bottom and up the right.
    const w = k.w(440);
    const ry = k.xh * 0.55;
    k.arc(0, w, -k.os, 2 * ry - k.os, 180, 360, k.V * 0.92, k.H);
    k.add(rect(0, ry - k.os - 1, k.V * 0.92, k.xh), rect(w - k.V * 0.8, ry - k.os - 1, w, k.xh * 0.86));
    entry(k, 0, k.xh);
    k.dot(w - k.V * 0.4, k.xh * 0.86, k.V * 0.9);
    return { w, sides: ['o', 'r'] };
  },
  w: (k) => {
    const w = k.w(680);
    const half = Math.round(w / 2);
    const ry = k.xh * 0.5;
    k.arc(0, half + k.V * 0.45, -k.os, 2 * ry - k.os, 180, 360, k.V * 0.92, k.H);
    k.arc(half - k.V * 0.45, w, -k.os, 2 * ry - k.os, 180, 360, k.V * 0.92, k.H);
    k.add(rect(0, ry - k.os - 1, k.V * 0.92, k.xh), rect(half - k.V * 0.45, ry - k.os - 1, half + k.V * 0.45, k.xh * 0.8));
    k.add(rect(w - k.V * 0.8, ry - k.os - 1, w, k.xh * 0.86));
    entry(k, 0, k.xh);
    k.dot(w - k.V * 0.4, k.xh * 0.86, k.V * 0.9);
    return { w, sides: ['o', 'r'] };
  },
  y: (k) => {
    // The u's shape, with the right stem running on into a hook in the descender.
    const w = k.w(460);
    const ry = Math.min(w * 0.62, k.xh);
    const cy = -k.os + ry;
    k.arc(0, w, -k.os, cy + ry, 180, 360, k.V, k.H);
    k.add(rect(0, cy - 1, k.V, k.xh));
    entry(k, 0, k.xh);
    const hr = 150;
    k.add(rect(w - k.V, k.desc + hr - k.os, w, k.xh));
    entry(k, w - k.V, k.xh);
    k.arc(0, w, k.desc - k.os, k.desc - k.os + 2 * hr, 200, 360, k.V, k.H);
    return { w, sides: ['o', 's'] };
  },
  z: (k) => {
    const w = k.w(400);
    k.add(rect(10, k.xh - k.H, w, k.xh), rect(0, 0, w, k.H));
    k.diag(k.V * 0.55, k.H * 0.5, w - k.V * 0.55, k.xh - k.H * 0.5, k.V * 0.98);
    return { w, sides: ['d', 'd'] };
  },
  // The rest of the lowercase keeps the upright drawing, without its slab serifs.
  s: (k) => noSerifs(k, 's'),
  x: (k) => noSerifs(k, 'x'),
};

/** An upright letter drawn as a sans: the same skeleton with the Serif's weights but no slab serifs. */
function noSerifs(k: Kit, ch: string): Drawn {
  const sans = new (k.constructor as new (s: Kit['s']) => Kit)({ ...k.s, kind: 'sans' });
  const d = (LETTERS[ch] as Draw)(sans);
  k.add(sans.shape());
  return d;
}

/** Desk Mono Italic's cursive forms: only the f, which drops into the descender and keeps the cell. */
export const MONO_ITALIC: Record<string, Draw> = {
  f: (k) => {
    const w = 400;
    const x = Math.round(w * 0.42);
    const r = 110;
    k.add(rect(x, k.desc + r, x + k.V, k.asc - r + 4));
    k.arc(x, x + 2 * (w - x), k.asc + k.os - 2 * r, k.asc + k.os, 45, 180, k.V, k.H);
    k.arc(x + k.V - 2 * (x + k.V) * 0.9, x + k.V, k.desc - k.os, k.desc + 2 * r - k.os, 215, 360, k.V, k.H);
    k.add(rect(x - w * 0.28, k.xh - k.H, x + k.V + w * 0.3, k.xh));
    return { w, sides: ['o', 'o'] };
  },
};
