// Punctuation, symbols and the remaining Latin-1 and code page 1252 characters of the outline faces.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { difference, type P, rect, stroke, superEllipse, union } from './geom';
import type { Kit } from './kit';
import { LETTERS, type Draw } from './letters';

const mid = (k: Kit): number => Math.round(k.xh * 0.5 + 10);
/** Thickness of light strokes in symbols: the stem in Sans and Mono, a medium weight in Serif. */
const sw = (k: Kit): number => (k.serif ? Math.round((k.V + k.H) / 2) : k.V * 0.9);

/** A comma-shaped tail: a dot with a short tail down to the left, its top at y. */
function comma(k: Kit, x: number, y: number): void {
  const d = k.dotSize;
  const cx = x + d / 2;
  const cy = y - d / 2;
  k.dot(cx, cy);
  // The tail leaves the right of the dot and tapers down to the left.
  k.add([
    { x: cx + d * 0.5, y: cy },
    { x: cx + d * 0.42, y: cy - d * 0.55 },
    { x: cx - d * 0.18, y: cy - d * 0.5 - 150 },
    { x: cx - d * 0.42, y: cy - d * 0.5 - 140 },
    { x: cx - d * 0.05, y: cy - d * 0.35 },
  ]);
}

/** A quote: a comma raised, or a straight tick. */
function quote(k: Kit, x: number, y: number, turned: boolean): void {
  const d = k.dotSize;
  if (!turned) {
    comma(k, x, y);
    return;
  }
  // The turned comma: the dot at the bottom, the tail rising to the right.
  const cx = x + d / 2;
  const cy = y - 150 - d * 0.5;
  k.dot(cx, cy);
  k.add([
    { x: cx - d * 0.5, y: cy },
    { x: cx - d * 0.42, y: cy + d * 0.55 },
    { x: cx + d * 0.18, y: cy + d * 0.5 + 150 },
    { x: cx + d * 0.42, y: cy + d * 0.5 + 140 },
    { x: cx + d * 0.05, y: cy + d * 0.35 },
  ]);
}

const arrow = (k: Kit, dir: 'l' | 'r' | 'u' | 'd' | 'lr' | 'ud'): ReturnType<Draw> => {
  const w = k.mono ? 480 : 640;
  const t = sw(k) * 0.9;
  const h = k.mono ? 480 : 560;
  const a = 170;
  const horiz = dir === 'l' || dir === 'r' || dir === 'lr';
  const len = horiz ? w : h;
  const c = horiz ? mid(k) + 40 : w / 2;
  const pts = (end: number, back: number): P[] => [
    { x: back, y: c - a },
    { x: end, y: c },
    { x: back, y: c + a },
  ];
  const shapes: P[][] = [];
  shapes.push(...stroke([{ x: 0, y: c }, { x: len, y: c }], t));
  if (dir === 'r' || dir === 'lr' || dir === 'd' || dir === 'ud') shapes.push(...stroke(pts(len - t * 0.3, len - a), t, 'butt'));
  if (dir === 'l' || dir === 'lr' || dir === 'u' || dir === 'ud') shapes.push(...stroke(pts(t * 0.3, a), t, 'butt'));
  let s = union(...shapes);
  if (!horiz) {
    // Rotate a quarter turn: x along the shaft becomes y, so the shape stands on the baseline.
    const r = (p: P): P => ({ x: p.y - c + w / 2, y: len - p.x + (k.cap - len) / 2 });
    s = union(...s.outer.map((q) => q.map(r)));
  }
  k.add(s);
  return { w: horiz ? len : w, sides: ['o', 'o'] };
};

const triangle = (k: Kit, dir: 'r' | 'l' | 'u' | 'd'): ReturnType<Draw> => {
  const s = k.mono ? 420 : 460;
  const c = mid(k) + 40;
  const pts: Record<string, P[]> = {
    r: [{ x: 0, y: c - s / 2 }, { x: s * 0.86, y: c }, { x: 0, y: c + s / 2 }],
    l: [{ x: s * 0.86, y: c - s / 2 }, { x: s * 0.86, y: c + s / 2 }, { x: 0, y: c }],
    u: [{ x: 0, y: c - s * 0.43 }, { x: s, y: c - s * 0.43 }, { x: s / 2, y: c + s * 0.43 }],
    d: [{ x: 0, y: c + s * 0.43 }, { x: s / 2, y: c - s * 0.43 }, { x: s, y: c + s * 0.43 }],
  };
  k.add(pts[dir] as P[]);
  return { w: dir === 'r' || dir === 'l' ? s * 0.86 : s, sides: ['o', 'o'] };
};

/** A small figure for superscripts and fractions: a figure drawn by the face, scaled and placed. */
function smallFigure(k: Kit, fig: string, x: number, y: number, scale = 0.56): void {
  const fn = LETTERS[fig] as Draw;
  const sub = new (k.constructor as new (s: Kit['s']) => Kit)({ ...k.s, V: k.V * 0.72 / scale, H: k.H * 0.8 / scale });
  fn(sub);
  const sh = sub.shape();
  k.add(union(...sh.outer.map((p) => p.map((q) => ({ x: x + q.x * scale, y: y + q.y * scale }))), ...sh.holes.map((p) => p.map((q) => ({ x: x + q.x * scale, y: y + q.y * scale })))));
}

const fraction = (k: Kit, num: string, den: string): ReturnType<Draw> => {
  const w = k.mono ? 520 : 760;
  const sc = 0.46;
  smallFigure(k, num, 0, k.cap * 0.5, sc);
  smallFigure(k, den, w - 500 * k.s.wide * sc, 0, sc);
  k.diag(w * 0.16, 0, w * 0.84, k.cap, sw(k) * 0.7);
  return { w, sides: ['o', 'o'] };
};

const S: Record<string, Draw> = {
  '!': (k) => {
    const x = k.dotSize / 2;
    k.add([
      { x: x - k.V * 0.5, y: k.cap },
      { x: x + k.V * 0.5, y: k.cap },
      { x: x + k.V * 0.36, y: k.cap * 0.27 },
      { x: x - k.V * 0.36, y: k.cap * 0.27 },
    ]);
    k.dot(x, k.dotSize / 2);
    return { w: k.dotSize, sides: ['s', 's'] };
  },
  '"': (k) => {
    const g = k.V * 1.9;
    for (const x of [0, g]) k.add([
      { x, y: k.cap + k.os },
      { x: x + k.V, y: k.cap + k.os },
      { x: x + k.V * 0.8, y: k.cap - 230 },
      { x: x + k.V * 0.2, y: k.cap - 230 },
    ]);
    return { w: g + k.V, sides: ['s', 's'] };
  },
  "'": (k) => {
    k.add([
      { x: 0, y: k.cap + k.os },
      { x: k.V, y: k.cap + k.os },
      { x: k.V * 0.8, y: k.cap - 230 },
      { x: k.V * 0.2, y: k.cap - 230 },
    ]);
    return { w: k.V, sides: ['s', 's'] };
  },
  '#': (k) => {
    const w = k.w(560, 460);
    const t = sw(k) * 0.85;
    const y0 = k.cap * 0.33;
    const y1 = k.cap * 0.67;
    k.add(rect(0, y0 - t / 2, w, y0 + t / 2), rect(0, y1 - t / 2, w, y1 + t / 2));
    k.diag(w * 0.28, 0, w * 0.4, k.cap, t);
    k.diag(w * 0.6, 0, w * 0.72, k.cap, t);
    return { w, sides: ['o', 'o'] };
  },
  $: (k) => {
    const s = LETTERS.S as Draw;
    const d = s(k);
    const x = d.w / 2 - k.V * 0.4;
    k.add(rect(x, -100, x + k.V * 0.8, k.cap + 100));
    // Clear the bar where it crosses the counters, keeping it on the strokes only.
    return d;
  },
  '%': (k) => {
    const w = k.w(720, 500);
    const r = k.mono ? 105 : 130;
    const t = Math.min(sw(k) * 0.82, r * 0.5);
    k.ring(0, 2 * r, k.cap - 2 * r * 1.15, k.cap, t, t * 0.9);
    k.ring(w - 2 * r, w, 0, 2 * r * 1.15, t, t * 0.9);
    k.diag(w * 0.22, 0, w * 0.78, k.cap, t * 0.9);
    return { w, sides: ['r', 'r'] };
  },
  '&': (k) => {
    const w = k.w(640, 470);
    const t = k.V;
    // Upper loop, lower bowl and the leg, as one pen line.
    const pts: P[] = [
      { x: w * 0.98, y: 0 },
      { x: w * 0.44, y: k.cap * 0.56 },
      { x: w * 0.3, y: k.cap * 0.68 },
      { x: w * 0.26, y: k.cap * 0.82 },
      { x: w * 0.34, y: k.cap * 0.94 },
      { x: w * 0.47, y: k.cap * 0.97 },
      { x: w * 0.6, y: k.cap * 0.92 },
      { x: w * 0.66, y: k.cap * 0.8 },
      { x: w * 0.6, y: k.cap * 0.66 },
      { x: w * 0.42, y: k.cap * 0.53 },
      { x: w * 0.16, y: k.cap * 0.38 },
      { x: w * 0.06, y: k.cap * 0.22 },
      { x: w * 0.12, y: k.cap * 0.06 },
      { x: w * 0.32, y: k.cap * 0.0 },
      { x: w * 0.54, y: k.cap * 0.04 },
      { x: w * 0.74, y: k.cap * 0.2 },
      { x: w * 0.86, y: k.cap * 0.42 },
    ];
    k.path(smooth(pts), t * 0.9, 'butt');
    return { w, sides: ['r', 'd'] };
  },
  '(': (k) => {
    const w = k.mono ? 260 : 230;
    k.arc(0, w * 2.6, k.desc + 40, k.asc + 40, 112, 248, k.V * 0.92, k.H);
    return { w, sides: ['r', 'o'] };
  },
  ')': (k) => {
    const w = k.mono ? 260 : 230;
    k.arc(w - w * 2.6, w, k.desc + 40, k.asc + 40, -68, 68, k.V * 0.92, k.H);
    return { w, sides: ['o', 'r'] };
  },
  '*': (k) => {
    const w = k.mono ? 400 : 420;
    const c = { x: w / 2, y: k.cap - w * 0.45 };
    const t = sw(k) * 0.82;
    for (const a of [90, 162, 234, 306, 18]) {
      const r = (a * Math.PI) / 180;
      k.path([c, { x: c.x + Math.cos(r) * w * 0.5, y: c.y + Math.sin(r) * w * 0.5 }], t, 'butt');
    }
    return { w, sides: ['o', 'o'] };
  },
  '+': (k) => {
    const w = k.mono ? 460 : 520;
    const t = sw(k) * 0.9;
    const c = mid(k) + 40;
    k.add(rect(0, c - t / 2, w, c + t / 2), rect(w / 2 - t / 2, c - w / 2, w / 2 + t / 2, c + w / 2));
    return { w, sides: ['o', 'o'] };
  },
  ',': (k) => {
    comma(k, 0, k.dotSize);
    return { w: k.dotSize, sides: ['s', 's'] };
  },
  '-': (k) => {
    const w = k.mono ? 360 : 300;
    k.hbar(0, w, mid(k) + 30, sw(k) * 0.95);
    return { w, sides: ['o', 'o'] };
  },
  '.': (k) => {
    k.dot(k.dotSize / 2, k.dotSize / 2);
    return { w: k.dotSize, sides: ['s', 's'] };
  },
  '/': (k) => {
    const w = k.mono ? 400 : 380;
    k.diag(0 + k.V * 0.45, k.desc + 40, w - k.V * 0.45, k.asc + 40, sw(k) * 0.92);
    return { w, sides: ['d', 'd'] };
  },
  ':': (k) => {
    k.dot(k.dotSize / 2, k.dotSize / 2);
    k.dot(k.dotSize / 2, k.xh - k.dotSize / 2);
    return { w: k.dotSize, sides: ['s', 's'] };
  },
  ';': (k) => {
    comma(k, 0, k.dotSize);
    k.dot(k.dotSize / 2, k.xh - k.dotSize / 2);
    return { w: k.dotSize, sides: ['s', 's'] };
  },
  '<': (k) => {
    const w = k.mono ? 440 : 480;
    const c = mid(k) + 40;
    k.path([{ x: w, y: c + w * 0.42 }, { x: k.V * 0.3, y: c }, { x: w, y: c - w * 0.42 }], sw(k) * 0.9, 'butt');
    return { w, sides: ['o', 'o'] };
  },
  '=': (k) => {
    const w = k.mono ? 460 : 520;
    const c = mid(k) + 40;
    const t = sw(k) * 0.88;
    k.add(rect(0, c + 70, w, c + 70 + t), rect(0, c - 70 - t, w, c - 70));
    return { w, sides: ['o', 'o'] };
  },
  '>': (k) => {
    const w = k.mono ? 440 : 480;
    const c = mid(k) + 40;
    k.path([{ x: 0, y: c + w * 0.42 }, { x: w - k.V * 0.3, y: c }, { x: 0, y: c - w * 0.42 }], sw(k) * 0.9, 'butt');
    return { w, sides: ['o', 'o'] };
  },
  '?': (k) => {
    const w = k.w(440, 420);
    const ry = k.cap * 0.3;
    k.arc(0, w, k.cap + k.os - 2 * ry, k.cap + k.os, -78, 165, k.V, k.H);
    const x = w * 0.46;
    k.add(rect(x - k.V / 2, k.cap * 0.26, x + k.V / 2, k.cap + k.os - 2 * ry + k.H * 0.6));
    k.dot(x, k.dotSize / 2);
    return { w, sides: ['r', 'r'] };
  },
  '@': (k) => {
    const w = k.w(820, 540);
    const t = sw(k) * 0.82;
    const R = w / 2;
    const cy = k.cap * 0.4;
    // The outer round, open at the lower right where the a's tail runs into it.
    k.arc(0, w, cy - R, cy + R, -18, 292, t, t * 0.94);
    const ri = R * 0.44;
    const cx = R - R * 0.06;
    k.ring(cx - ri, cx + ri, cy - ri, cy + ri, t, t * 0.94);
    k.add(rect(cx + ri - t, cy - ri, cx + ri, cy + ri + 1));
    k.path(smooth([{ x: cx + ri - t / 2, y: cy - ri + t * 0.6 }, { x: cx + ri + R * 0.12, y: cy - ri - t * 0.1 }, { x: w - R * 0.16, y: cy - R * 0.3 }, { x: w - t / 2, y: cy - R * 0.02 }]), t);
    return { w, sides: ['r', 'r'] };
  },
  '[': (k) => {
    const w = k.mono ? 240 : 220;
    k.add(rect(0, k.desc + 40, k.V, k.asc + 40), rect(0, k.desc + 40, w, k.desc + 40 + k.H), rect(0, k.asc + 40 - k.H, w, k.asc + 40));
    return { w, sides: ['s', 'o'] };
  },
  '\\': (k) => {
    const w = k.mono ? 400 : 380;
    k.diag(w - k.V * 0.45, k.desc + 40, k.V * 0.45, k.asc + 40, sw(k) * 0.92);
    return { w, sides: ['d', 'd'] };
  },
  ']': (k) => {
    const w = k.mono ? 240 : 220;
    k.add(rect(w - k.V, k.desc + 40, w, k.asc + 40), rect(0, k.desc + 40, w, k.desc + 40 + k.H), rect(0, k.asc + 40 - k.H, w, k.asc + 40));
    return { w, sides: ['o', 's'] };
  },
  '^': (k) => {
    const w = k.mono ? 420 : 440;
    k.path([{ x: 0, y: k.cap * 0.45 }, { x: w / 2, y: k.cap }, { x: w, y: k.cap * 0.45 }], sw(k) * 0.82, 'butt');
    return { w, sides: ['o', 'o'] };
  },
  _: (k) => {
    const w = k.mono ? 600 : 500;
    k.add(rect(0, k.desc + 40, w, k.desc + 40 + k.H));
    return { w, sides: [k.mono ? 0 : 'o', k.mono ? 0 : 'o'] };
  },
  '`': (k) => {
    k.diag(130, k.xh + 70, 10, k.xh + 230, k.V * 0.8);
    return { w: 140, sides: ['o', 'o'] };
  },
  '{': (k) => {
    const w = k.mono ? 280 : 260;
    const t = k.V * 0.86;
    const c = (k.asc + k.desc) / 2 + 40;
    // The foot stays inside the line's descender however heavy the stroke.
    const foot = Math.max(k.desc + 40, -250 + t / 2 + 6);
    k.path(smooth([{ x: w, y: k.asc + 40 }, { x: w * 0.5, y: k.asc }, { x: w * 0.45, y: c + 160 }, { x: w * 0.35, y: c + 30 }, { x: 0, y: c }]), t);
    k.path(smooth([{ x: 0, y: c }, { x: w * 0.35, y: c - 30 }, { x: w * 0.45, y: c - 160 }, { x: w * 0.5, y: k.desc + 80 }, { x: w, y: foot }]), t);
    return { w, sides: ['o', 'o'] };
  },
  '|': (k) => {
    k.add(rect(0, k.desc + 20, k.V * 0.92, k.asc + 60));
    return { w: k.V * 0.92, sides: ['s', 's'] };
  },
  '}': (k) => {
    const w = k.mono ? 280 : 260;
    const t = k.V * 0.86;
    const c = (k.asc + k.desc) / 2 + 40;
    // The foot stays inside the line's descender however heavy the stroke.
    const foot = Math.max(k.desc + 40, -250 + t / 2 + 6);
    k.path(smooth([{ x: 0, y: k.asc + 40 }, { x: w * 0.5, y: k.asc }, { x: w * 0.55, y: c + 160 }, { x: w * 0.65, y: c + 30 }, { x: w, y: c }]), t);
    k.path(smooth([{ x: w, y: c }, { x: w * 0.65, y: c - 30 }, { x: w * 0.55, y: c - 160 }, { x: w * 0.5, y: k.desc + 80 }, { x: 0, y: foot }]), t);
    return { w, sides: ['o', 'o'] };
  },
  '~': (k) => {
    const w = k.mono ? 460 : 500;
    const c = mid(k) + 40;
    const pts = Array.from({ length: 41 }, (_, i) => ({ x: (i / 40) * w, y: c + 60 * Math.sin((i / 40) * 2 * Math.PI + Math.PI * 0.8) }));
    k.path(pts, sw(k) * 0.86);
    return { w, sides: ['o', 'o'] };
  },

  // Latin-1
  '¡': (k) => {
    const x = k.dotSize / 2;
    k.add([
      { x: x - k.V * 0.36, y: k.xh - k.cap * 0.27 - 10 },
      { x: x + k.V * 0.36, y: k.xh - k.cap * 0.27 - 10 },
      { x: x + k.V * 0.5, y: k.xh - k.cap - 10 },
      { x: x - k.V * 0.5, y: k.xh - k.cap - 10 },
    ]);
    k.dot(x, k.xh - k.dotSize / 2);
    return { w: k.dotSize, sides: ['s', 's'] };
  },
  '¿': (k) => {
    const w = k.w(440, 420);
    const ry = k.cap * 0.3;
    const base = k.xh - k.cap - 10;
    k.arc(0, w, base - k.os, base - k.os + 2 * ry, 102, 345, k.V, k.H);
    const x = w * 0.54;
    k.add(rect(x - k.V / 2, base + 2 * ry - k.H * 0.6 - k.os, x + k.V / 2, k.xh - k.cap * 0.26));
    k.dot(x, k.xh - k.dotSize / 2);
    return { w, sides: ['r', 'r'] };
  },
  '¢': (k) => {
    const c = LETTERS.c as Draw;
    const d = c(k);
    k.add(rect(d.w * 0.48 - k.V * 0.35, -90, d.w * 0.48 + k.V * 0.35, k.xh + 90));
    return d;
  },
  '£': (k) => {
    const w = k.w(520, 440);
    const x = w * 0.18;
    const ry = k.cap * 0.28;
    k.arc(x, x + (w - x) * 1.25, k.cap + k.os - 2 * ry, k.cap + k.os, 20, 180, k.V, k.H);
    k.add(rect(x, k.H * 0.6, x + k.V, k.cap + k.os - ry + 1));
    k.add(rect(0, k.cap * 0.42, w * 0.7, k.cap * 0.42 + k.H), rect(0, 0, w, k.H));
    return { w, sides: ['o', 'o'] };
  },
  '¤': (k) => {
    const w = k.mono ? 440 : 480;
    const c = mid(k) + 40;
    const t = sw(k) * 0.8;
    const r = w * 0.3;
    k.ring(w / 2 - r, w / 2 + r, c - r, c + r, t, t);
    for (const [sx, sy] of [[-1, -1], [1, -1], [1, 1], [-1, 1]] as const) {
      k.path([{ x: w / 2 + sx * r * 0.6, y: c + sy * r * 0.6 }, { x: w / 2 + sx * w * 0.5, y: c + sy * w * 0.5 }], t);
    }
    return { w, sides: ['o', 'o'] };
  },
  '¥': (k) => {
    const y = LETTERS.Y as Draw;
    const d = y(k);
    k.add(rect(d.w * 0.15, k.cap * 0.3, d.w * 0.85, k.cap * 0.3 + k.H), rect(d.w * 0.15, k.cap * 0.12, d.w * 0.85, k.cap * 0.12 + k.H));
    return d;
  },
  '¦': (k) => {
    const t = k.V * 0.92;
    k.add(rect(0, k.desc + 20, t, (k.desc + k.asc) / 2 - 40), rect(0, (k.desc + k.asc) / 2 + 80, t, k.asc + 60));
    return { w: t, sides: ['s', 's'] };
  },
  '§': (k) => {
    const w = k.w(440, 400);
    const t = k.V * 0.88;
    k.arc(0, w, k.cap * 0.5, k.cap + k.os, 30, 270, t, k.H);
    k.arc(0, w, k.cap * 0.18, k.cap * 0.72, -90, 90, t, k.H);
    k.arc(0, w, k.cap * 0.18, k.cap * 0.72, 90, 270, t, k.H);
    k.arc(0, w, -k.os - k.cap * 0.1, k.cap * 0.4, 210, 450, t, k.H);
    return { w, sides: ['r', 'r'] };
  },
  '©': (k) => circled(k, 'C'),
  '®': (k) => circled(k, 'R'),
  ª: (k) => {
    smallFigureLetter(k, 'a', 0, k.cap * 0.42, 0.6);
    k.add(rect(0, k.cap * 0.3, 0.6 * k.w(470, 420), k.cap * 0.3 + k.H * 0.9));
    return { w: 0.6 * k.w(470, 420), sides: ['o', 'o'] };
  },
  º: (k) => {
    smallFigureLetter(k, 'o', 0, k.cap * 0.42, 0.6);
    k.add(rect(0, k.cap * 0.3, 0.6 * k.w(510, 446), k.cap * 0.3 + k.H * 0.9));
    return { w: 0.6 * k.w(510, 446), sides: ['o', 'o'] };
  },
  '«': (k) => guillemets(k, true, 2),
  '»': (k) => guillemets(k, false, 2),
  '‹': (k) => guillemets(k, true, 1),
  '›': (k) => guillemets(k, false, 1),
  '¬': (k) => {
    const w = k.mono ? 460 : 520;
    const c = mid(k) + 40;
    const t = sw(k) * 0.88;
    k.add(rect(0, c, w, c + t), rect(w - t, c - 160, w, c + t));
    return { w, sides: ['o', 'o'] };
  },
  '°': (k) => {
    const r = 105;
    const t = Math.min(sw(k) * 0.7, r * 0.42);
    k.ring(0, 2 * r, k.cap - 2 * r + k.os, k.cap + k.os, t, t);
    return { w: 2 * r, sides: ['o', 'o'] };
  },
  '±': (k) => {
    const w = k.mono ? 460 : 520;
    const t = sw(k) * 0.9;
    const c = mid(k) + 110;
    k.add(rect(0, c - t / 2, w, c + t / 2), rect(w / 2 - t / 2, c - w * 0.42, w / 2 + t / 2, c + w * 0.42), rect(0, 0, w, t));
    return { w, sides: ['o', 'o'] };
  },
  '²': (k) => sup(k, '2'),
  '³': (k) => sup(k, '3'),
  '¹': (k) => sup(k, '1'),
  µ: (k) => {
    const u = LETTERS.u as Draw;
    const d = u(k);
    k.add(rect(0, k.desc, k.V, k.xh));
    return d;
  },
  '¶': (k) => {
    const w = k.w(520, 440);
    const t = k.V * 0.9;
    k.add(superEllipse(w * 0.32, k.cap * 0.68, w * 0.32, k.cap * 0.32, 2.2));
    k.add(rect(w * 0.32, k.cap - k.H, w, k.cap), rect(w * 0.5, k.desc + 40, w * 0.5 + t, k.cap), rect(w - t, k.desc + 40, w, k.cap));
    return { w, sides: ['r', 's'] };
  },
  '·': (k) => {
    k.dot(k.dotSize / 2, mid(k) + 40);
    return { w: k.dotSize, sides: ['s', 's'] };
  },
  '¼': (k) => fraction(k, '1', '4'),
  '½': (k) => fraction(k, '1', '2'),
  '¾': (k) => fraction(k, '3', '4'),
  '×': (k) => {
    const w = k.mono ? 400 : 440;
    const c = mid(k) + 40;
    const t = sw(k) * 0.9;
    k.path([{ x: 0, y: c - w / 2 }, { x: w, y: c + w / 2 }], t);
    k.path([{ x: 0, y: c + w / 2 }, { x: w, y: c - w / 2 }], t);
    return { w, sides: ['o', 'o'] };
  },
  '÷': (k) => {
    const w = k.mono ? 460 : 520;
    const c = mid(k) + 40;
    k.add(rect(0, c - sw(k) * 0.45, w, c + sw(k) * 0.45));
    k.dot(w / 2, c + 180);
    k.dot(w / 2, c - 180);
    return { w, sides: ['o', 'o'] };
  },
  Æ: (k) => {
    const w = k.w(880, 520);
    const d = k.V * 0.98;
    const sx = w * 0.46;
    k.diag(d / 2, 0, sx + 30, k.cap, d);
    k.stem(sx, 0, k.cap, { top: 'none', bottom: 'left' });
    k.add(rect(sx, k.cap - k.H, w, k.cap), rect(sx, 0, w, k.H), rect(sx, k.cap * 0.5 - k.H / 2, w * 0.92, k.cap * 0.5 + k.H / 2));
    k.add(rect(w * 0.16, k.cap * 0.28, sx + 4, k.cap * 0.28 + k.H));
    return { w, sides: ['d', 'o'] };
  },
  æ: (k) => {
    const w = k.w(760, 520);
    const half = Math.round(w * 0.52);
    // The a's bowl and shoulder on the left, the e on the right, sharing the middle stroke.
    const ry = half * 0.62;
    k.arc(0, half, k.xh + k.os - 2 * ry, k.xh + k.os, 0, 152, k.V * 0.9, k.H);
    k.ring(0, half, -k.os, k.xh * 0.58);
    k.add(rect(half - k.V * 0.9, k.xh * 0.2, half, k.xh * 0.75));
    const cy = Math.round(k.xh / 2) + 6;
    k.arc(half - k.V * 0.9, w, -k.os, k.xh + k.os, 0, 330, k.V * 0.9, k.H);
    k.add(rect(half - k.V * 0.5, cy - k.H / 2, w, cy + k.H / 2));
    return { w, sides: ['r', 'r'] };
  },
  Ð: (k) => {
    const d = (LETTERS.D as Draw)(k);
    k.add(rect(-k.V * 0.9, k.cap * 0.5 - k.H / 2, k.V * 2.4, k.cap * 0.5 + k.H / 2));
    return { w: d.w, sides: ['o', 'r'] };
  },
  ð: (k) => {
    const w = k.w(500, 440);
    k.ring(0, w, -k.os, k.xh + k.os);
    k.path(smooth([{ x: w - k.V / 2, y: k.xh * 0.5 }, { x: w - k.V * 0.6, y: k.xh * 0.95 }, { x: w * 0.72, y: k.asc * 0.9 }, { x: w * 0.4, y: k.asc + k.os }]), k.V);
    k.diag(w * 0.36, k.asc - 260, w * 0.86, k.asc - 90, k.H * 1.1);
    return { w, sides: ['r', 'r'] };
  },
  Ø: (k) => {
    const d = (LETTERS.O as Draw)(k);
    k.diag(d.w * 0.06, -k.os - 30, d.w * 0.94, k.cap + k.os + 30, k.V * 0.8);
    return d;
  },
  ø: (k) => {
    const d = (LETTERS.o as Draw)(k);
    k.diag(d.w * 0.06, -k.os - 30, d.w * 0.94, k.xh + k.os + 30, k.V * 0.8);
    return d;
  },
  Þ: (k) => {
    const w = k.w(530, 430);
    k.stem(0, 0, k.cap, { top: 'both', bottom: 'both' });
    const y0 = Math.round(k.cap * 0.2);
    const y1 = Math.round(k.cap * 0.78);
    const r = (y1 - y0) / 2;
    k.arc(w - 2 * r, w, y0, y1, -90, 90);
    k.add(rect(0, y0, w - r + 1, y0 + k.H), rect(0, y1 - k.H, w - r + 1, y1));
    return { w, sides: ['s', 'r'] };
  },
  þ: (k) => {
    const d = (LETTERS.p as Draw)(k);
    k.add(rect(0, k.xh - 10, k.V, k.asc));
    return d;
  },
  ß: (k) => {
    // Sans Bold and Serif take the open form; Sans and Mono Regular keep the first drawing.
    if ((k.bold && k.s.kind === 'sans') || k.serif) return sansBoldSharpS(k);
    const w = k.w(520, 440);
    const t = k.V;
    const ry = 170;
    const top = k.asc + k.os;
    const xr = Math.round(w * 0.86);
    const mid = Math.round(k.asc * 0.62);
    k.stem(0, 0, top - ry + 20, { top: 'none' });
    k.arc(0, xr, top - 2 * ry, top, 0, 180, t, k.H);
    k.add(rect(xr - t, mid - k.H, xr, top - ry + 20));
    k.arc(w * 0.08, w, -k.os, mid, -112, 90, t, k.H);
    k.add(rect(w * 0.42, mid - k.H, xr, mid));
    return { w, sides: ['s', 'r'] };
  },
  Œ: (k) => {
    const w = k.w(940, 540);
    const ow = Math.round(w * 0.62);
    // The left half of an O, level with the bars of the E it shares its stem with.
    k.arc(0, ow, 0, k.cap, 90, 270, k.V, k.H);
    const sx = Math.round(ow / 2 - k.V * 0.2);
    k.add(rect(sx, 0, sx + k.V, k.cap));
    k.add(rect(ow / 2 - 6, k.cap - k.H, w, k.cap), rect(ow / 2 - 6, 0, w, k.H), rect(sx, k.cap * 0.5 - k.H / 2, w * 0.9, k.cap * 0.5 + k.H / 2));
    return { w, sides: ['r', 'o'] };
  },
  œ: (k) => {
    const w = k.w(800, 530);
    const half = Math.round(w * 0.52);
    k.ring(0, half, -k.os, k.xh + k.os, k.V * 0.92, k.H);
    const cy = Math.round(k.xh / 2) + 6;
    k.arc(half - k.V * 0.92, w, -k.os, k.xh + k.os, 0, 330, k.V * 0.92, k.H);
    k.add(rect(half - k.V * 0.5, cy - k.H / 2, w, cy + k.H / 2));
    return { w, sides: ['r', 'r'] };
  },

  // Code page 1252 and typography
  '€': (k) => {
    const w = k.w(600, 460);
    k.arc(w * 0.12, w, -k.os, k.cap + k.os, 40, 320, k.V, k.H);
    k.add(rect(0, k.cap * 0.56, w * 0.7, k.cap * 0.56 + k.H * 0.9), rect(0, k.cap * 0.36, w * 0.66, k.cap * 0.36 + k.H * 0.9));
    return { w, sides: ['o', 'o'] };
  },
  '‚': (k) => {
    comma(k, 0, k.dotSize);
    return { w: k.dotSize, sides: ['s', 's'] };
  },
  ƒ: (k) => {
    const w = k.w(420, 420);
    const x = w * 0.42;
    const ry = 150;
    k.arc(x, x + (w - x) * 2, k.asc + k.os - 2 * ry, k.asc + k.os, 52, 180, k.V, k.H);
    k.add(rect(x, k.desc + ry, x + k.V, k.asc + k.os - ry + 1));
    k.arc(x + k.V - (x + k.V) * 2, x + k.V, k.desc - k.os, k.desc - k.os + 2 * ry, 232, 360, k.V, k.H);
    k.add(rect(x - w * 0.3, k.xh - k.H, x + k.V + w * 0.3, k.xh));
    return { w, sides: ['o', 'o'] };
  },
  '„': (k) => {
    const g = k.dotSize * 1.6;
    comma(k, 0, k.dotSize);
    comma(k, g, k.dotSize);
    return { w: g + k.dotSize, sides: ['s', 's'] };
  },
  '…': (k) => {
    const g = k.mono ? 190 : k.dotSize * 2.5;
    for (const i of [0, 1, 2]) k.dot(k.dotSize / 2 + i * g, k.dotSize / 2);
    return { w: k.dotSize + 2 * g, sides: ['s', 's'] };
  },
  '†': (k) => {
    const w = k.mono ? 440 : 440;
    const t = sw(k);
    k.add(rect(w / 2 - t / 2, k.desc + 60, w / 2 + t / 2, k.cap), rect(0, k.cap * 0.62, w, k.cap * 0.62 + k.H * 1.1));
    return { w, sides: ['o', 'o'] };
  },
  '‡': (k) => {
    const w = 440;
    const t = sw(k);
    k.add(rect(w / 2 - t / 2, k.desc + 60, w / 2 + t / 2, k.cap), rect(0, k.cap * 0.62, w, k.cap * 0.62 + k.H * 1.1), rect(0, k.cap * 0.05, w, k.cap * 0.05 + k.H * 1.1));
    return { w, sides: ['o', 'o'] };
  },
  '‰': (k) => {
    const w = k.mono ? 560 : 960;
    const r = k.mono ? 72 : 120;
    const t = Math.min(sw(k) * 0.72, r * 0.48);
    k.ring(0, 2 * r, k.cap - 2.3 * r, k.cap, t, t * 0.9);
    const x1 = k.mono ? w * 0.36 : w * 0.46;
    k.ring(x1, x1 + 2 * r, 0, 2.3 * r, t, t * 0.9);
    k.ring(w - 2 * r, w, 0, 2.3 * r, t, t * 0.9);
    k.diag(r * 1.4, 0, x1 + r * 0.6, k.cap, t * 0.9);
    return { w, sides: ['r', 'r'] };
  },
  '‘': (k) => {
    quote(k, 0, k.cap + k.os, true);
    return { w: k.dotSize * 1.25, sides: ['s', 's'] };
  },
  '’': (k) => {
    comma(k, 0, k.cap + k.os);
    return { w: k.dotSize, sides: ['s', 's'] };
  },
  '“': (k) => {
    const g = k.dotSize * 1.6;
    quote(k, 0, k.cap + k.os, true);
    quote(k, g, k.cap + k.os, true);
    return { w: g + k.dotSize * 1.25, sides: ['s', 's'] };
  },
  '”': (k) => {
    const g = k.dotSize * 1.6;
    comma(k, 0, k.cap + k.os);
    comma(k, g, k.cap + k.os);
    return { w: g + k.dotSize, sides: ['s', 's'] };
  },
  '•': (k) => {
    const d = k.dotSize * 1.9;
    k.dot(d / 2, mid(k) + 40, d);
    return { w: d, sides: ['o', 'o'] };
  },
  '–': (k) => {
    const w = k.mono ? 520 : 500;
    k.hbar(0, w, mid(k) + 30, sw(k) * 0.95);
    return { w, sides: k.mono ? ['o', 'o'] : [10, 10] };
  },
  '—': (k) => {
    const w = k.mono ? 600 : 1000;
    k.hbar(0, w, mid(k) + 30, sw(k) * 0.95);
    return { w, sides: [0, 0] };
  },
  '™': (k) => {
    const w = k.mono ? 560 : 860;
    const t = Math.max(k.H, k.V * 0.7);
    const h = k.cap * 0.42;
    const y0 = k.cap - h;
    const tw = w * 0.4;
    k.add(rect(0, k.cap - t, tw, k.cap), rect(tw / 2 - t / 2, y0, tw / 2 + t / 2, k.cap));
    const mx = tw + w * 0.07;
    k.add(rect(mx, y0, mx + t, k.cap), rect(w - t, y0, w, k.cap));
    k.path([{ x: mx + t / 2, y: k.cap - t / 2 }, { x: (mx + w) / 2, y: y0 + h * 0.3 }, { x: w - t / 2, y: k.cap - t / 2 }], t * 0.9);
    return { w, sides: ['o', 'o'] };
  },

  // Arrows and shapes
  '←': (k) => arrow(k, 'l'),
  '→': (k) => arrow(k, 'r'),
  '↑': (k) => arrow(k, 'u'),
  '↓': (k) => arrow(k, 'd'),
  '↔': (k) => arrow(k, 'lr'),
  '↕': (k) => arrow(k, 'ud'),
  '►': (k) => triangle(k, 'r'),
  '◄': (k) => triangle(k, 'l'),
  '▲': (k) => triangle(k, 'u'),
  '▼': (k) => triangle(k, 'd'),
  '●': (k) => {
    const d = k.mono ? 460 : 520;
    k.add(superEllipse(d / 2, mid(k) + 40, d / 2, d / 2, 2));
    return { w: d, sides: ['o', 'o'] };
  },
  '○': (k) => {
    const d = k.mono ? 460 : 520;
    k.ring(0, d, mid(k) + 40 - d / 2, mid(k) + 40 + d / 2, sw(k) * 0.8, sw(k) * 0.8, 2);
    return { w: d, sides: ['o', 'o'] };
  },
  '■': (k) => {
    const d = k.mono ? 420 : 460;
    k.add(rect(0, mid(k) + 40 - d / 2, d, mid(k) + 40 + d / 2));
    return { w: d, sides: ['o', 'o'] };
  },
  '□': (k) => {
    const d = k.mono ? 420 : 460;
    const t = sw(k) * 0.8;
    const c = mid(k) + 40;
    k.add(difference(union(rect(0, c - d / 2, d, c + d / 2)), union(rect(t, c - d / 2 + t, d - t, c + d / 2 - t))));
    return { w: d, sides: ['o', 'o'] };
  },
  '✓': (k) => {
    const w = k.mono ? 480 : 560;
    k.path([{ x: 0, y: k.xh * 0.5 }, { x: w * 0.36, y: k.V * 0.4 }, { x: w, y: k.cap }], sw(k), 'butt');
    return { w, sides: ['o', 'o'] };
  },
  '✗': (k) => {
    const w = k.mono ? 440 : 500;
    const t = sw(k) * 1.05;
    k.path([{ x: 0, y: 0 }, { x: w, y: k.cap * 0.92 }], t);
    k.path([{ x: 0, y: k.cap * 0.92 }, { x: w, y: 0 }], t);
    return { w, sides: ['o', 'o'] };
  },
};

/** Sans Bold's ß: wider than the Regular's, with a round top and a lower bowl that hangs from the right side. */
function sansBoldSharpS(k: Kit): ReturnType<Draw> {
  const w = k.w(540, 440);
  const t = k.V;
  const top = k.asc + k.os;
  const xr = Math.round(w * 0.8);
  const ry = Math.round(xr / 2);
  const mid = Math.round(k.asc * 0.6);
  k.stem(0, 0, top - ry + 20, { top: 'none' });
  k.arc(0, xr, top - 2 * ry, top, 0, 180, t, k.H);
  k.add(rect(xr - t, mid - k.H, xr, top - ry + 20));
  k.arc(w * 0.26, w, -k.os, mid + k.H * 0.5, -125, 90, t, k.H);
  return { w, sides: ['s', 'r'] };
}

function smooth(pts: P[]): P[] {
  // Catmull-Rom through the points, for pen paths.
  const out: P[] = [];
  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = pts[Math.max(0, i - 1)] as P;
    const p1 = pts[i] as P;
    const p2 = pts[i + 1] as P;
    const p3 = pts[Math.min(pts.length - 1, i + 2)] as P;
    for (let j = 0; j < 12; j++) {
      const t = j / 12;
      const t2 = t * t;
      const t3 = t2 * t;
      out.push({
        x: 0.5 * (2 * p1.x + (-p0.x + p2.x) * t + (2 * p0.x - 5 * p1.x + 4 * p2.x - p3.x) * t2 + (-p0.x + 3 * p1.x - 3 * p2.x + p3.x) * t3),
        y: 0.5 * (2 * p1.y + (-p0.y + p2.y) * t + (2 * p0.y - 5 * p1.y + 4 * p2.y - p3.y) * t2 + (-p0.y + 3 * p1.y - 3 * p2.y + p3.y) * t3),
      });
    }
  }
  out.push(pts[pts.length - 1] as P);
  return out;
}

function circled(k: Kit, letter: 'C' | 'R'): ReturnType<Draw> {
  const d = k.mono ? 560 : 800;
  const t = sw(k) * 0.7;
  const c = k.cap / 2;
  k.ring(0, d, c - d / 2, c + d / 2, t, t, 2);
  const sc = 0.5;
  smallFigureLetter(k, letter, d / 2 - (letter === 'C' ? 300 : 270) * k.s.wide * sc, c - k.cap * sc / 2, sc);
  return { w: d, sides: ['r', 'r'] };
}

function smallFigureLetter(k: Kit, ch: string, x: number, y: number, scale: number): void {
  smallFigure(k, ch, x, y, scale);
}

function sup(k: Kit, fig: string): ReturnType<Draw> {
  const sc = 0.56;
  smallFigure(k, fig, 0, k.cap * 0.44, sc);
  return { w: (k.mono ? 440 : 500 * k.s.wide) * sc, sides: ['o', 'o'] };
}

function guillemets(k: Kit, left: boolean, n: 1 | 2): ReturnType<Draw> {
  const w = 190;
  const h = 330;
  const c = mid(k) + 30;
  const t = sw(k) * 0.86;
  for (let i = 0; i < n; i++) {
    const x0 = i * (w * 0.95);
    const pts = left
      ? [{ x: x0 + w, y: c + h / 2 }, { x: x0, y: c }, { x: x0 + w, y: c - h / 2 }]
      : [{ x: x0, y: c + h / 2 }, { x: x0 + w, y: c }, { x: x0, y: c - h / 2 }];
    k.path(pts, t, 'butt');
  }
  return { w: w + (n - 1) * w * 0.95, sides: ['o', 'o'] };
}

export const SYMBOLS: Record<string, Draw> = S;
