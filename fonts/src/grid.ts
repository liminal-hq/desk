// Pixel grids: parsing the `#` and `.` row strings the faces are drawn in, and small pure helpers on them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** Rows of 0 (blank) and 1 (ink), top row first. Rows may differ in length; missing pixels are blank. */
export type Grid = number[][];

/** Parses rows of `#` (ink) and `.` (blank) joined with `/`. Any other character is an error. */
export function parse(s: string): Grid {
  return s.split('/').map((row) =>
    Array.from(row, (c) => {
      if (c === '#') return 1;
      if (c === '.') return 0;
      throw new Error(`bad pixel '${c}' in '${s}'`);
    }),
  );
}

/** Writes a grid back as `/`-joined rows, the form the faces are drawn in. */
export const format = (g: Grid): string =>
  g.map((r) => r.map((v) => (v ? '#' : '.')).join('')).join('/');

/** A grid of zeros. */
export const blank = (w: number, h: number): Grid =>
  Array.from({ length: h }, () => new Array<number>(w).fill(0));

/** The width of the widest row. */
export const width = (g: Grid): number => g.reduce((m, r) => Math.max(m, r.length), 0);

/** The number of ink pixels. */
export const ink = (g: Grid): number => g.reduce((n, r) => n + r.reduce((a, v) => a + (v ? 1 : 0), 0), 0);

/** A copy with every row padded to `w` columns and the grid padded to `h` rows. */
export function sized(g: Grid, w: number, h: number): Grid {
  const out = blank(w, h);
  g.forEach((r, y) => r.forEach((v, x) => stamp(out, x, y, v)));
  return out;
}

/** Sets one pixel if it falls inside the grid. */
function stamp(g: Grid, x: number, y: number, v: number): void {
  const row = g[y];
  if (row && x >= 0 && x < row.length && v) row[x] = 1;
}

/** Ink of `src` drawn onto `dst` (in place) with its top-left corner at `(x, y)`. */
export function overlay(dst: Grid, src: Grid, x: number, y: number): Grid {
  src.forEach((r, sy) => r.forEach((v, sx) => stamp(dst, x + sx, y + sy, v)));
  return dst;
}

/** The leftmost and rightmost inked columns, or `null` for a blank grid. */
export function inkSpan(g: Grid): [number, number] | null {
  let lo = Infinity;
  let hi = -1;
  g.forEach((r) =>
    r.forEach((v, x) => {
      if (!v) return;
      lo = Math.min(lo, x);
      hi = Math.max(hi, x);
    }),
  );
  return hi < 0 ? null : [lo, hi];
}

/** The topmost and bottommost inked rows, or `null` for a blank grid. */
export function inkRows(g: Grid): [number, number] | null {
  let lo = -1;
  let hi = -1;
  g.forEach((r, y) => {
    if (!r.some((v) => v)) return;
    if (lo < 0) lo = y;
    hi = y;
  });
  return hi < 0 ? null : [lo, hi];
}

/** Top-to-bottom mirror. */
export const flipV = (g: Grid): Grid => g.map((r) => r.slice()).reverse();

/** Left-to-right mirror, about the grid's full width. */
export const flipH = (g: Grid): Grid => {
  const w = width(g);
  return sized(g, w, g.length).map((r) => r.reverse());
};

/**
 * Gap-keeping bold: each ink pixel also inks the pixel to its right. Where that would close a
 * one-pixel gap between two strokes, it inks the pixel to its left instead, if that closes no gap
 * either; otherwise the stroke stays one pixel wide. The result is one column wider.
 */
export function embolden(g: Grid): Grid {
  return g.map((r) => {
    const o = new Array<number>(r.length + 1).fill(0);
    r.forEach((v, x) => {
      if (!v) return;
      o[x] = 1;
      if (!r[x + 2] || r[x + 1]) o[x + 1] = 1;
      else if (x > 0 && !r[x - 1] && !r[x - 2]) o[x - 1] = 1;
    });
    return o;
  });
}
