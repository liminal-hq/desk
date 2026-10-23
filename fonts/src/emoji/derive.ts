// Derives a 12 × 12 emoji from its 16 × 16 drawing by removing whole rows and columns, for the emoji that have no
// hand-drawn 12 × 12.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Pixels } from './grid';

/** How many pixels two lines differ in. */
const diff = (a: number[], b: number[]): number => a.reduce((n, v, i) => n + (v === b[i] ? 0 : 1), 0);

/**
 * Removes `count` rows, one at a time. Each step removes the row that differs least from a neighbour
 * (the number of pixels it differs from the more similar of the rows above and below), so runs of
 * repeated rows (the middle of a disc, the side of a box) shrink first and one-pixel outlines and
 * details survive. Ties go to the row nearest the middle, then to the upper one, so a symmetric
 * drawing loses its rows in the same places on both sides.
 */
export function dropRows(p: Pixels, count: number): Pixels {
  let rows = p.map((r) => r.slice());
  for (let k = 0; k < count; k++) {
    const n = rows.length;
    const mid = (n - 1) / 2;
    let best = -1;
    let bestCost = Infinity;
    for (let i = 0; i < n; i++) {
      const row = rows[i] as number[];
      const up = i > 0 ? diff(row, rows[i - 1] as number[]) : Infinity;
      const down = i < n - 1 ? diff(row, rows[i + 1] as number[]) : Infinity;
      const cost = Math.min(up, down);
      const better =
        cost < bestCost || (cost === bestCost && Math.abs(i - mid) < Math.abs(best - mid));
      if (better) {
        best = i;
        bestCost = cost;
      }
    }
    rows = rows.filter((_, i) => i !== best);
  }
  return rows;
}

const transpose = (p: Pixels): Pixels => (p[0] ?? []).map((_, x) => p.map((r) => r[x] as number));

/**
 * Removes `count` rows: first blank rows at the top and bottom (the drawing's margin), from whichever
 * side has more of them so the drawing stays centred, then, when the margin runs out, rows inside the
 * drawing by `dropRows`.
 */
export function trimRows(p: Pixels, count: number): Pixels {
  const blank = (r: number[] | undefined): boolean => !!r && r.every((v) => v < 0);
  let rows = p.map((r) => r.slice());
  let k = count;
  let top = 0;
  while (blank(rows[top])) top++;
  let bottom = 0;
  while (blank(rows[rows.length - 1 - bottom])) bottom++;
  while (k > 0 && (top > 0 || bottom > 0)) {
    if (top >= bottom) {
      rows = rows.slice(1);
      top--;
    } else {
      rows = rows.slice(0, -1);
      bottom--;
    }
    k--;
  }
  return dropRows(rows, k);
}

/**
 * The 12 × 12 derivation: four rows removed by `trimRows`, then four columns the same way. The blank
 * margin goes first, so the drawing fills the smaller grid as a hand-drawn one does; only then are rows
 * and columns inside it removed. Nothing is resampled, so every pixel that remains is one the artist
 * drew, in its own colour.
 */
export function derive12(p: Pixels): Pixels {
  if (p.length !== 16 || p.some((r) => r.length !== 16)) throw new Error('derive12 needs a 16 × 16 grid');
  return transpose(trimRows(transpose(trimRows(p, 4)), 4));
}
