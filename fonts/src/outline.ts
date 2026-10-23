// Traces the edges of a pixel grid into closed polygons: one contour per shape and one per hole, no overlaps.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Grid } from './grid';

/** A point in pixel units, `x` to the right and `y` up. */
export interface Pt {
  x: number;
  y: number;
}

// Directions in counter-clockwise order (with y up): east, north, west, south. `(d + 1) % 4` turns left.
const DX = [1, 0, -1, 0] as const;
const DY = [0, 1, 0, -1] as const;

const inked = (g: Grid, x: number, y: number): boolean => (g[y]?.[x] ?? 0) === 1;

/**
 * Traces the ink in `grid` as closed contours in pixel units, with the top edge of row `r` at
 * `y = top - r` and column `c` starting at `x = c`.
 *
 * Every boundary edge is directed with the ink on its left, so outer contours run counter-clockwise
 * and holes clockwise: the PostScript (CFF) convention, which fills by the non-zero rule. Where two
 * pixels touch only at a corner the walk turns towards its own ink, so they stay separate contours
 * rather than pinching into a figure eight. Points on a straight run are dropped.
 */
export function trace(grid: Grid, top: number): Pt[][] {
  const edges = new Map<string, number[]>();
  const key = (x: number, y: number): string => `${x},${y}`;
  const add = (x: number, y: number, d: number): void => {
    const k = key(x, y);
    const list = edges.get(k);
    if (list) list.push(d);
    else edges.set(k, [d]);
  };
  grid.forEach((row, r) =>
    row.forEach((v, c) => {
      if (!v) return;
      const xl = c;
      const xr = c + 1;
      const yt = top - r;
      const yb = yt - 1;
      // Ink on the left of each edge: bottom edge runs east, right edge north, top west, left south.
      if (!inked(grid, c, r + 1)) add(xl, yb, 0);
      if (!inked(grid, c + 1, r)) add(xr, yb, 1);
      if (!inked(grid, c, r - 1)) add(xr, yt, 2);
      if (!inked(grid, c - 1, r)) add(xl, yt, 3);
    }),
  );
  // Start each contour at its lowest, leftmost free vertex so the output does not depend on map order.
  const starts = [...edges.keys()]
    .map((k) => k.split(',').map(Number) as [number, number])
    .sort((a, b) => a[1] - b[1] || a[0] - b[0]);
  const contours: Pt[][] = [];
  for (const [sx, sy] of starts) {
    for (;;) {
      const first = edges.get(key(sx, sy));
      if (!first || first.length === 0) break;
      let d = first.shift() as number;
      let x = sx;
      let y = sy;
      const pts: Pt[] = [];
      for (;;) {
        pts.push({ x, y });
        x += DX[d] as number;
        y += DY[d] as number;
        if (x === sx && y === sy) break;
        const next = edges.get(key(x, y)) as number[];
        // Prefer a left turn (towards the ink), then straight on, then right.
        let pick = -1;
        for (const turn of [1, 0, 3]) {
          pick = next.indexOf((d + turn) % 4);
          if (pick >= 0) break;
        }
        d = next.splice(pick, 1)[0] as number;
      }
      contours.push(dropCollinear(pts));
    }
  }
  return contours;
}

/** Removes the points that lie on a straight line between their neighbours. */
function dropCollinear(pts: Pt[]): Pt[] {
  const n = pts.length;
  return pts.filter((b, i) => {
    const a = pts[(i + n - 1) % n] as Pt;
    const c = pts[(i + 1) % n] as Pt;
    return (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x) !== 0;
  });
}

/** The signed area of a contour: positive when it runs counter-clockwise (an outer shape). */
export function area(c: Pt[]): number {
  let a = 0;
  c.forEach((p, i) => {
    const q = c[(i + 1) % c.length] as Pt;
    a += p.x * q.y - q.x * p.y;
  });
  return a / 2;
}
