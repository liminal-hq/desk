// Accent marks for the outline faces, drawn once per style and placed over base letters by the assembler.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { type P, type Shape, stroke, superEllipse, union, difference } from './geom';
import type { Kit } from './kit';

type Mark = (k: Kit, upper: boolean) => Shape;

const mw = (k: Kit): number => (k.serif ? Math.max(k.H * 1.3, k.V * 0.62) : k.V * 0.82);

const diag = (k: Kit, x0: number, y0: number, x1: number, y1: number, w: number): Shape => {
  // A wedge: wider at the top, as an accent written with a pen.
  const dx = x1 - x0;
  const dy = y1 - y0;
  const l = Math.hypot(dx, dy);
  const nx = -dy / l;
  const ny = dx / l;
  const w0 = w * (k.serif ? 0.55 : 0.9);
  const w1 = w * (k.serif ? 1.15 : 1);
  return union([
    { x: x0 + (nx * w0) / 2, y: y0 + (ny * w0) / 2 },
    { x: x0 - (nx * w0) / 2, y: y0 - (ny * w0) / 2 },
    { x: x1 - (nx * w1) / 2, y: y1 - (ny * w1) / 2 },
    { x: x1 + (nx * w1) / 2, y: y1 + (ny * w1) / 2 },
  ]);
};

const dot = (k: Kit, cx: number, cy: number): P[] => superEllipse(cx, cy, k.dotSize / 2, k.dotSize / 2, k.serif ? 2 : 2.6);

export const MARKS: Record<string, Mark> = {
  acute: (k, up) => diag(k, 0, 0, 90 + k.V * 0.3, (up ? 120 : 150) + k.V * 0.3, mw(k)),
  grave: (k, up) => diag(k, 90 + k.V * 0.3, 0, 0, (up ? 120 : 150) + k.V * 0.3, mw(k)),
  circ: (k, up) => {
    const h = up ? 120 : 140;
    const w = 220 + k.V * 0.6;
    return union(stroke([{ x: 0, y: 0 }, { x: w / 2, y: h }, { x: w, y: 0 }], mw(k) * 0.92, 'butt'));
  },
  caron: (k, up) => {
    const h = up ? 120 : 140;
    const w = 220 + k.V * 0.6;
    return union(stroke([{ x: 0, y: h }, { x: w / 2, y: 0 }, { x: w, y: h }], mw(k) * 0.92, 'butt'));
  },
  tilde: (k, up) => {
    const w = 250 + k.V * 0.5;
    const a = up ? 34 : 42;
    const pts = Array.from({ length: 41 }, (_, i) => {
      const t = i / 40;
      return { x: t * w, y: a + a * Math.sin(t * 2 * Math.PI + Math.PI * 0.75) * 0.9 };
    });
    return union(stroke(pts, mw(k) * 0.85, 'butt'));
  },
  diaer: (k) => {
    const gap = Math.max(170, k.dotSize * 1.9);
    return union(dot(k, 0, 0), dot(k, gap, 0));
  },
  ring: (k, up) => {
    const r = up ? 84 : 96;
    const t = Math.max(k.H * 0.9, k.V * 0.5);
    return difference(union(superEllipse(0, 0, r, r, 2)), union(superEllipse(0, 0, r - t, r - t, 2)));
  },
  cedilla: (k) => {
    const t = Math.max(k.H * 1.1, k.V * 0.6);
    return union(
      stroke(
        [
          { x: 0, y: 40 },
          { x: 0, y: -60 },
          { x: 50, y: -70 },
          { x: 80, y: -110 },
          { x: 60, y: -160 },
          { x: -40, y: -175 },
        ],
        t,
        'butt',
      ),
    );
  },
  macron: (k) => union([
    { x: 0, y: 0 },
    { x: 240, y: 0 },
    { x: 240, y: Math.max(k.H, 60) },
    { x: 0, y: Math.max(k.H, 60) },
  ]),
};

/** Spacing accents that stand alone, drawn from the marks. */
export const SPACING_MARKS: [string, string][] = [
  ['´', 'acute'],
  ['¨', 'diaer'],
  ['¸', 'cedilla'],
  ['¯', 'macron'],
  ['ˆ', 'circ'],
  ['˜', 'tilde'],
];
