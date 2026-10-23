// The drawing kit of the vector emoji: filled primitives (discs, rounded boxes, polygons, round-ended strokes, arcs,
// stars, hearts) built on the outline faces' geometry, in an emoji box of 1000 units, and the colours they are
// painted in.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import ClipperLib from 'clipper-lib';
import { difference, intersection, type P, type Poly, type Shape, stroke, superArc, superEllipse, union } from '../../modern/geom';

/**
 * The vector palette, in one place: the brand colours of the 16-colour Desk set (`docs/PALETTE.md`, the
 * Afterglow accents) with one lighter and one darker tint of each, and the greys of the set.
 */
export const VCOLOURS = {
  ink: '#1b1e28',
  inkSoft: '#333847',
  grey: '#7c8796',
  greyLight: '#b4bfce',
  paper: '#e0e0e0',
  white: '#f4f4f8',
  amber: '#ffaa40',
  amberLight: '#ffc878',
  amberDark: '#d9822b',
  brown: '#7d5422',
  brownLight: '#a8743a',
  rose: '#f43f5e',
  roseLight: '#fb7f93',
  roseDark: '#c0304a',
  purple: '#a78bfa',
  purpleDark: '#7c5fd6',
  cyan: '#22d3ee',
  cyanLight: '#8ae8f6',
  green: '#2ec66a',
  greenDark: '#1f9450',
  blue: '#60a5fa',
  blueDark: '#3b78d0',
} as const;

export type Colour = keyof typeof VCOLOURS;
/** The colours by palette index, in the order of the CPAL table. */
export const COLOUR_ORDER = Object.keys(VCOLOURS) as Colour[];

/**
 * Colours that are paper in the monochrome glyph when they are covered by nothing darker: the light
 * fills, as in the pixel emoji's rule.
 */
export const LIGHT_COLOURS: ReadonlySet<Colour> = new Set(['paper', 'white', 'greyLight', 'amber', 'amberLight', 'cyan', 'cyanLight']);

/**
 * What a layer does in the monochrome glyph: `ink` adds it; `fill` makes it paper with an ink edge
 * (the default for the light colours); `hole` makes it paper with no edge, for a light detail on a
 * dark shape (the keys of a keyboard, a check on a button); `skip` leaves the glyph as it is, for
 * shading.
 */
export type MonoRole = 'ink' | 'fill' | 'hole' | 'skip';

/** One layer of a drawing: a shape painted in one colour. */
export interface Layer {
  colour: Colour;
  shape: Shape;
  /** The layer's part in the monochrome glyph, when it is not the default for its colour. */
  mono?: MonoRole;
}

/**
 * The emoji box: drawings are made in a square of 1000 units, x to the right and y down from the top
 * left, as the pixel grids are; the font places its top 850 units above the baseline.
 */
export const BOX = 1000;
export const TOP = 850;

const at = (x: number, y: number): P => ({ x, y: TOP - y });
const deg = (a: number): number => (a * Math.PI) / 180;

/** A disc, or an ellipse with two radii. */
export const disc = (cx: number, cy: number, rx: number, ry = rx): Shape => union(superEllipse(cx, TOP - cy, rx, ry, 2));

/** A box with rounded corners of radius `r` (a superellipse-free construction, so the sides stay straight). */
export function box(x0: number, y0: number, x1: number, y1: number, r = 0): Shape {
  const rr = Math.min(r, (x1 - x0) / 2, (y1 - y0) / 2);
  const rect = (a: number, b: number, c: number, d: number): Poly => [at(a, b), at(c, b), at(c, d), at(a, d)];
  if (rr <= 0) return union(rect(x0, y0, x1, y1));
  return union(
    rect(x0 + rr, y0, x1 - rr, y1),
    rect(x0, y0 + rr, x1, y1 - rr),
    superEllipse(x0 + rr, TOP - (y0 + rr), rr, rr, 2),
    superEllipse(x1 - rr, TOP - (y0 + rr), rr, rr, 2),
    superEllipse(x0 + rr, TOP - (y1 - rr), rr, rr, 2),
    superEllipse(x1 - rr, TOP - (y1 - rr), rr, rr, 2),
  );
}

/** A filled polygon through the points. */
export const poly = (...pts: [number, number][]): Shape => union(pts.map(([x, y]) => at(x, y)));

/** Points of a circular or elliptical arc from `a0` to `a1` degrees (0 is right, 90 is down), font units. */
function arcPoints(cx: number, cy: number, rx: number, ry: number, a0: number, a1: number): P[] {
  return superArc(cx, TOP - cy, rx, ry, 2, -deg(a0), -deg(a1));
}

/** A stroke of width `w` along the points, with round ends and joins unless `butt`. */
export const line = (pts: [number, number][], w: number, butt = false): Shape =>
  union(stroke(pts.map(([x, y]) => at(x, y)), w, butt ? 'butt' : 'round'));

/** A stroke of width `w` along an arc of a circle or ellipse, from `a0` to `a1` degrees (0 right, 90 down). */
export const arc = (cx: number, cy: number, rx: number, ry: number, a0: number, a1: number, w: number, butt = false): Shape =>
  union(stroke(arcPoints(cx, cy, rx, ry, a0, a1), w, butt ? 'butt' : 'round'));

/** A filled pie slice (or a segment of an ellipse closed through its centre). */
export const pie = (cx: number, cy: number, rx: number, ry: number, a0: number, a1: number): Shape =>
  union([{ x: cx, y: TOP - cy }, ...arcPoints(cx, cy, rx, ry, a0, a1)]);

/** A star of `n` points with outer radius `r1` and inner radius `r2`, one point straight up, corners rounded by `round`. */
export function star(cx: number, cy: number, r1: number, r2: number, n = 5, round = 0): Shape {
  const pts: [number, number][] = [];
  for (let i = 0; i < 2 * n; i++) {
    const a = deg(-90 + (180 / n) * i);
    const r = i % 2 ? r2 : r1;
    pts.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
  }
  return round ? rounded(poly(...pts), round) : poly(...pts);
}

/** A heart filling the box from `x0` to `x1` with its top lobes at `y0` and its point at `y1`. */
export function heart(x0: number, y0: number, x1: number, y1: number): Shape {
  const w = x1 - x0;
  const r = w * 0.265;
  const cy = y0 + r;
  const cl = x0 + r;
  const cr = x1 - r;
  const bx = x0 + w / 2;
  // The sides are tangent to each lobe and meet at the point.
  const tangent = (cx: number, side: number): [number, number] => {
    const dx = bx - cx;
    const dy = y1 - cy;
    const d = Math.hypot(dx, dy);
    const a = Math.atan2(dy, dx) + side * Math.acos(r / d);
    return [cx + r * Math.cos(a), cy + r * Math.sin(a)];
  };
  const body = poly([cl, cy], tangent(cl, 1), [bx, y1], tangent(cr, -1), [cr, cy]);
  return rounded(union(disc(cl, cy, r), disc(cr, cy, r), body), w * 0.05);
}

/** A drop of water: a disc below drawn up to a point at `top`. */
export function drop(cx: number, top: number, r: number, bottom: number): Shape {
  const cy = bottom - r;
  const h = cy - top;
  const s = Math.asin(Math.min(1, r / h));
  const tx = r * Math.cos(s);
  const ty = cy - r * Math.sin(s);
  return union(disc(cx, cy, r), poly([cx, top], [cx + tx, ty], [cx, cy], [cx - tx, ty]));
}

/** Grows (positive) or shrinks (negative) a shape by `d` units with round corners. */
export function grow(s: Shape, d: number): Shape {
  const SCALE = 16;
  const off = new ClipperLib.ClipperOffset(2, 0.25 * SCALE);
  for (const p of [...s.outer, ...s.holes]) {
    off.AddPath(
      p.map((q) => ({ X: Math.round(q.x * SCALE), Y: Math.round(q.y * SCALE) })),
      ClipperLib.JoinType.jtRound,
      ClipperLib.EndType.etClosedPolygon,
    );
  }
  const out: { X: number; Y: number }[][] = [];
  off.Execute(out, d * SCALE);
  return union(out.map((p) => p.map((q) => ({ x: q.X / SCALE, y: q.Y / SCALE }))));
}

/** A shape with its convex corners rounded to radius `r` (shrink, then grow). */
export const rounded = (s: Shape, r: number): Shape => grow(grow(s, -r), r);

/** A ring: the band of width `w` inside the edge of a shape. */
export const ring = (s: Shape, w: number): Shape => difference(s, grow(s, -w));

/** A shape turned by `a` degrees (clockwise on the page) about a point. */
export function turn(s: Shape, a: number, cx: number, cy: number): Shape {
  const c = Math.cos(deg(-a));
  const sn = Math.sin(deg(-a));
  const oy = TOP - cy;
  const f = (p: P): P => ({ x: cx + (p.x - cx) * c - (p.y - oy) * sn, y: oy + (p.x - cx) * sn + (p.y - oy) * c });
  return union([...s.outer.map((p) => p.map(f)), ...s.holes.map((p) => p.map(f))]);
}

/** A shape moved by `dx` to the right and `dy` down. */
export const move = (s: Shape, dx: number, dy: number): Shape =>
  union([...s.outer.map((p) => p.map((q) => ({ x: q.x + dx, y: q.y - dy }))), ...s.holes.map((p) => p.map((q) => ({ x: q.x + dx, y: q.y - dy })))]);

/** A shape mirrored left to right about `cx`. */
export const mirror = (s: Shape, cx = BOX / 2): Shape =>
  union([...s.outer.map((p) => p.map((q) => ({ x: 2 * cx - q.x, y: q.y }))), ...s.holes.map((p) => p.map((q) => ({ x: 2 * cx - q.x, y: q.y })))]);

export { difference, intersection, union };
export type { Shape };
