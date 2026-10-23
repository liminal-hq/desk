// Measures a glyph's counters from its outlines: the enclosed holes, how wide each is (the diameter of the largest
// circle inside it), and how wide the narrowest point is between an open counter and the outside (its aperture).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MGlyph } from '../src/modern/assemble';
import { type P, sampleContour } from '../src/modern/geom';

/** Font units per raster pixel. */
const UNIT = 4;

export interface Counters {
  /** Width of each enclosed counter, widest first, in font units. */
  holes: number[];
  /** Width of the narrowest point on the widest way out of the deepest open space, in font units. */
  aperture: number;
}

/** Rasterises a glyph's outlines (non-zero winding) and measures its counters. */
export function counters(g: MGlyph): Counters {
  const box = g.box as NonNullable<MGlyph['box']>;
  const pad = 3;
  const x0 = box.x0 - pad * UNIT;
  const y0 = box.y0 - pad * UNIT;
  const w = Math.ceil((box.x1 - box.x0) / UNIT) + 2 * pad;
  const h = Math.ceil((box.y1 - box.y0) / UNIT) + 2 * pad;
  const ink = new Uint8Array(w * h);
  const edges: [P, P][] = [];
  for (const c of g.contours) {
    const pts = sampleContour(c, 16);
    for (let i = 0; i + 1 < pts.length; i++) edges.push([pts[i] as P, pts[i + 1] as P]);
  }
  for (let r = 0; r < h; r++) {
    const y = y0 + (r + 0.5) * UNIT;
    const xs: [number, number][] = [];
    for (const [a, b] of edges) {
      if ((a.y <= y && b.y > y) || (b.y <= y && a.y > y)) xs.push([a.x + ((y - a.y) / (b.y - a.y)) * (b.x - a.x), a.y < b.y ? 1 : -1]);
    }
    xs.sort((p, q) => p[0] - q[0]);
    let wind = 0;
    let k = 0;
    for (let c = 0; c < w; c++) {
      const x = x0 + (c + 0.5) * UNIT;
      while (k < xs.length && (xs[k] as [number, number])[0] < x) wind += (xs[k++] as [number, number])[1];
      if (wind !== 0) ink[r * w + c] = 1;
    }
  }
  // Chamfer distance (in pixels, x10) from every white pixel to the nearest ink.
  const INF = 1 << 28;
  const d = new Int32Array(w * h).map((_, i) => (ink[i] ? 0 : INF));
  const at = (c: number, r: number): number => (c < 0 || r < 0 || c >= w || r >= h ? INF : (d[r * w + c] as number));
  for (let r = 0; r < h; r++)
    for (let c = 0; c < w; c++) {
      const i = r * w + c;
      d[i] = Math.min(d[i] as number, at(c - 1, r) + 10, at(c, r - 1) + 10, at(c - 1, r - 1) + 14, at(c + 1, r - 1) + 14);
    }
  for (let r = h - 1; r >= 0; r--)
    for (let c = w - 1; c >= 0; c--) {
      const i = r * w + c;
      d[i] = Math.min(d[i] as number, at(c + 1, r) + 10, at(c, r + 1) + 10, at(c + 1, r + 1) + 14, at(c - 1, r + 1) + 14);
    }
  // White regions: those touching the border are outside or open; the rest are holes.
  const label = new Int32Array(w * h).fill(-1);
  const holes: number[] = [];
  const open: boolean[] = [];
  let next = 0;
  for (let start = 0; start < w * h; start++) {
    if (ink[start] || label[start] !== -1) continue;
    const stack = [start];
    label[start] = next;
    let border = false;
    let deepest = 0;
    while (stack.length) {
      const i = stack.pop() as number;
      const c = i % w;
      const r = (i - c) / w;
      if (c === 0 || r === 0 || c === w - 1 || r === h - 1) border = true;
      deepest = Math.max(deepest, d[i] as number);
      for (const [dc, dr] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        const cc = c + (dc as number);
        const rr = r + (dr as number);
        if (cc < 0 || rr < 0 || cc >= w || rr >= h) continue;
        const j = rr * w + cc;
        if (!ink[j] && label[j] === -1) {
          label[j] = next;
          stack.push(j);
        }
      }
    }
    if (!border) holes.push(((deepest / 10) * 2 - 1) * UNIT);
    open.push(border);
    next++;
  }
  // The aperture: from the deepest open white point inside the ink's box, the widest way out (the bottleneck
  // of a maximin path), found by adding white pixels from the widest down until it reaches the border.
  const inside = (i: number): boolean => {
    const c = i % w;
    const r = (i - c) / w;
    return c > pad + 1 && r > pad + 1 && c < w - pad - 2 && r < h - pad - 2;
  };
  let seed = -1;
  for (let i = 0; i < w * h; i++) {
    if (ink[i] || !inside(i)) continue;
    // Only open space counts: a pixel whose region reaches the border.
    if (!open[label[i] as number]) continue;
    if (seed < 0 || (d[i] as number) > (d[seed] as number)) seed = i;
  }
  let aperture = Infinity;
  if (seed >= 0) {
    const order = Array.from({ length: w * h }, (_, i) => i).filter((i) => !ink[i]).sort((a, b) => (d[b] as number) - (d[a] as number));
    const parent = new Int32Array(w * h).fill(-1);
    const find = (i: number): number => {
      while (parent[i] !== i) {
        parent[i] = parent[parent[i] as number] as number;
        i = parent[i] as number;
      }
      return i;
    };
    const onBorder = new Uint8Array(w * h);
    for (const i of order) {
      parent[i] = i;
      const c = i % w;
      const r = (i - c) / w;
      if (c === 0 || r === 0 || c === w - 1 || r === h - 1) onBorder[i] = 1;
      for (const [dc, dr] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        const cc = c + (dc as number);
        const rr = r + (dr as number);
        if (cc < 0 || rr < 0 || cc >= w || rr >= h) continue;
        const j = rr * w + cc;
        if (parent[j] === -1 || ink[j]) continue;
        const a = find(i);
        const b = find(j);
        if (a !== b) {
          parent[a] = b;
          if (onBorder[a]) onBorder[b] = 1;
        }
      }
      if (parent[seed] !== -1 && onBorder[find(seed)]) {
        aperture = (((d[i] as number) / 10) * 2 - 1) * UNIT;
        break;
      }
    }
  }
  holes.sort((a, b) => b - a);
  return { holes, aperture };
}
