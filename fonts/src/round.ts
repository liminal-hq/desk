// Glyph outlines in font units for both styles: Square (the traced pixel edges) and Round (convex corners softened
// with quadratic curves that stay inside the pixels).
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Grid } from './grid';
import { trace, type Pt } from './outline';

/** Font units in one pixel. */
export const PIXEL = 100;

/** The two outline styles. Round is derived from the same grids and keeps every metric. */
export type Style = 'square' | 'round';

/**
 * How far a rounded corner reaches along each edge, in pixels. It must stay below half a pixel so
 * the two corners of a one-pixel end never meet; at 0.45 such an end becomes a round cap with a
 * sliver of flat edge, and a lone pixel a dot. Keep `ROUND_RADIUS × 100` a multiple of 3: the CFF
 * files store each quadratic as a cubic whose control points sit two thirds of the way along, and a
 * multiple of 3 keeps them on whole font units, so the OTF and TTF outlines are identical.
 */
export const ROUND_RADIUS = 0.45;

/** A point of an outline in font units: on the curve, or the control point of a quadratic segment. */
export interface OPt {
  x: number;
  y: number;
  on: boolean;
}

const sub = (a: Pt, b: Pt): Pt => ({ x: a.x - b.x, y: a.y - b.y });
const len = (v: Pt): number => Math.abs(v.x) + Math.abs(v.y);
const cross = (a: Pt, b: Pt): number => a.x * b.y - a.y * b.x;

/** Square outlines: the traced pixel edges, counter-clockwise outside and clockwise around holes. */
export function squareOutline(rows: Grid, ascent: number): OPt[][] {
  return trace(rows, ascent).map((c) => c.map((p) => ({ x: p.x * PIXEL, y: p.y * PIXEL, on: true })));
}

/**
 * Round outlines, a pure function of the grid. The tracer puts the ink on the left of every edge,
 * so a left turn is a convex (outer) corner and a right turn a concave (inner) one. Each convex
 * corner is replaced by a quadratic curve whose control point is the corner itself and whose ends
 * lie on the two edges; concave corners stay sharp. The curve lies inside the triangle formed by
 * the corner and its two ends, so rounding only removes ink and never reaches outside the pixels.
 */
export function roundOutline(rows: Grid, ascent: number, radius = ROUND_RADIUS): OPt[][] {
  return trace(rows, ascent).map((c) => {
    const n = c.length;
    const convex = c.map((b, i) => {
      const a = c[(i + n - 1) % n] as Pt;
      const d = c[(i + 1) % n] as Pt;
      return cross(sub(b, a), sub(d, b)) > 0;
    });
    // How far a corner may reach along an edge, given whether the corner at its other end is rounded too.
    const reach = (edge: number, otherConvex: boolean): number => Math.min(radius, otherConvex ? edge / 2 : edge);
    const out: OPt[] = [];
    const push = (p: OPt): void => {
      const last = out[out.length - 1];
      if (last && last.on && p.on && last.x === p.x && last.y === p.y) return;
      out.push(p);
    };
    c.forEach((b, i) => {
      const at = (p: Pt): OPt => ({ x: Math.round(p.x * PIXEL), y: Math.round(p.y * PIXEL), on: true });
      if (!convex[i]) {
        push(at(b));
        return;
      }
      const a = c[(i + n - 1) % n] as Pt;
      const d = c[(i + 1) % n] as Pt;
      const vin = sub(b, a);
      const vout = sub(d, b);
      const lin = len(vin);
      const lout = len(vout);
      const rin = reach(lin, convex[(i + n - 1) % n] as boolean);
      const rout = reach(lout, convex[(i + 1) % n] as boolean);
      push(at({ x: b.x - (vin.x / lin) * rin, y: b.y - (vin.y / lin) * rin }));
      out.push({ x: b.x * PIXEL, y: b.y * PIXEL, on: false });
      push(at({ x: b.x + (vout.x / lout) * rout, y: b.y + (vout.y / lout) * rout }));
    });
    // A closing duplicate of the first point is dropped.
    const first = out[0] as OPt;
    const last = out[out.length - 1] as OPt;
    if (out.length > 1 && last.on && first.on && last.x === first.x && last.y === first.y) out.pop();
    return out;
  });
}

/** The outline of a glyph in the given style. */
export const outlineOf = (rows: Grid, ascent: number, style: Style): OPt[][] =>
  style === 'round' ? roundOutline(rows, ascent) : squareOutline(rows, ascent);

/**
 * The segments of a closed contour: lines `[p, q]` and quadratics `[p, control, q]`. The contour
 * never has two control points in a row.
 */
export function segments(c: OPt[]): OPt[][] {
  const start = c.findIndex((p) => p.on);
  if (start < 0) throw new Error('contour has no on-curve point');
  const pts = [...c.slice(start), ...c.slice(0, start)];
  const out: OPt[][] = [];
  for (let i = 0; i < pts.length; ) {
    const p = pts[i] as OPt;
    const q = pts[(i + 1) % pts.length] as OPt;
    if (q.on) {
      out.push([p, q]);
      i += 1;
    } else {
      const r = pts[(i + 2) % pts.length] as OPt;
      if (!r.on) throw new Error('two control points in a row');
      out.push([p, q, r]);
      i += 2;
    }
  }
  return out;
}

/** The signed area of a contour (positive counter-clockwise), exact for lines and quadratics. */
export function contourArea(c: OPt[]): number {
  const x = (a: OPt, b: OPt): number => a.x * b.y - a.y * b.x;
  let twice = 0;
  for (const s of segments(c)) {
    if (s.length === 2) twice += x(s[0] as OPt, s[1] as OPt);
    else {
      const [p, k, q] = s as [OPt, OPt, OPt];
      twice += (2 * x(p, k) + 2 * x(k, q) + x(p, q)) / 3;
    }
  }
  return twice / 2;
}
