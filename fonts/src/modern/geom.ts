// Geometry for the outline faces: filled primitives as polygons, boolean union and difference, and the fit that turns
// the result back into clean cubic outlines with points at the extrema.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import ClipperLib from 'clipper-lib';

/** A point in font units, y up. */
export interface P {
  x: number;
  y: number;
}

/** A closed polygon in font units. */
export type Poly = P[];

/** One segment of an outline: a line to `p`, or a cubic through `c1` and `c2` to `p`. */
export type Seg = { t: 'L'; p: P } | { t: 'C'; c1: P; c2: P; p: P };

/** A closed outline: a start point and its segments, the last ending back at the start. */
export interface Contour {
  start: P;
  segs: Seg[];
}

/** Clipper works in integers: this many steps to a font unit. */
const SCALE = 16;
/** Curves are flattened into chords no longer than this, in font units. */
const CHORD = 4;

type CPath = { X: number; Y: number }[];
const toC = (p: Poly): CPath => p.map((q) => ({ X: Math.round(q.x * SCALE), Y: Math.round(q.y * SCALE) }));
const fromC = (p: CPath): Poly => p.map((q) => ({ x: q.X / SCALE, y: q.Y / SCALE }));

/** Signed area of a polygon, positive counter-clockwise. */
export const polyArea = (p: Poly): number => {
  let a = 0;
  p.forEach((q, i) => {
    const r = p[(i + 1) % p.length] as P;
    a += q.x * r.y - r.x * q.y;
  });
  return a / 2;
};

// ---- primitives ----

export const rect = (x0: number, y0: number, x1: number, y1: number): Poly => [
  { x: Math.min(x0, x1), y: Math.min(y0, y1) },
  { x: Math.max(x0, x1), y: Math.min(y0, y1) },
  { x: Math.max(x0, x1), y: Math.max(y0, y1) },
  { x: Math.min(x0, x1), y: Math.max(y0, y1) },
];

/** A point on a superellipse of exponent `n` (2 is an ellipse), at parametric angle `a` (radians). */
export function superPoint(cx: number, cy: number, rx: number, ry: number, n: number, a: number): P {
  const c = Math.cos(a);
  const s = Math.sin(a);
  const e = 2 / n;
  return { x: cx + rx * Math.sign(c) * Math.abs(c) ** e, y: cy + ry * Math.sign(s) * Math.abs(s) ** e };
}

/**
 * Points along a superellipse from angle `a0` to `a1` (radians, either direction), spaced evenly by
 * length so that no chord is long enough to be taken for a straight part of the outline.
 */
export function superArc(cx: number, cy: number, rx: number, ry: number, n: number, a0: number, a1: number): P[] {
  const dense = Array.from({ length: 2049 }, (_, i) => superPoint(cx, cy, rx, ry, n, a0 + ((a1 - a0) * i) / 2048));
  const out: P[] = [dense[0] as P];
  let run = 0;
  for (let i = 1; i < dense.length; i++) {
    const a = dense[i - 1] as P;
    const b = dense[i] as P;
    run += Math.hypot(b.x - a.x, b.y - a.y);
    if (run >= CHORD) {
      out.push(b);
      run = 0;
    }
  }
  const last = dense[dense.length - 1] as P;
  const tail = out[out.length - 1] as P;
  if (tail.x !== last.x || tail.y !== last.y) out.push(last);
  return out;
}

/** A whole superellipse as a polygon. */
export const superEllipse = (cx: number, cy: number, rx: number, ry: number, n = 2): Poly =>
  superArc(cx, cy, rx, ry, n, 0, 2 * Math.PI).slice(0, -1);

/** Points along a cubic. */
export function cubicPoints(p0: P, c1: P, c2: P, p3: P): P[] {
  const len = Math.hypot(c1.x - p0.x, c1.y - p0.y) + Math.hypot(c2.x - c1.x, c2.y - c1.y) + Math.hypot(p3.x - c2.x, p3.y - c2.y);
  const steps = Math.max(4, Math.ceil(len / CHORD));
  return Array.from({ length: steps + 1 }, (_, i) => bez(p0, c1, c2, p3, i / steps));
}

export function bez(p0: P, c1: P, c2: P, p3: P, t: number): P {
  const u = 1 - t;
  return {
    x: u * u * u * p0.x + 3 * u * u * t * c1.x + 3 * u * t * t * c2.x + t * t * t * p3.x,
    y: u * u * u * p0.y + 3 * u * u * t * c1.y + 3 * u * t * t * c2.y + t * t * t * p3.y,
  };
}

/**
 * A stroke of width `w` along a polyline (or a flattened curve), with butt or round ends and round
 * joins: the shape a round pen of that diameter would leave.
 */
export function stroke(path: P[], w: number, ends: 'butt' | 'round' = 'butt'): Poly[] {
  const off = new ClipperLib.ClipperOffset(2, 0.25 * SCALE);
  off.AddPath(toC(path), ClipperLib.JoinType.jtRound, ends === 'round' ? ClipperLib.EndType.etOpenRound : ClipperLib.EndType.etOpenButt);
  const out: CPath[] = [];
  off.Execute(out, (w / 2) * SCALE);
  return out.map(fromC);
}

/**
 * A broad-nib stroke: the path swept by an elliptical nib of width `w` and height `h`, rotated by
 * `angle` (radians). A thin nib held at an angle gives thick and thin strokes, as a pen does.
 */
export function nib(path: P[], w: number, h: number, angle: number): Poly[] {
  const pat: CPath = Array.from({ length: 24 }, (_, i) => {
    const a = (i / 24) * 2 * Math.PI;
    const x = (w / 2) * Math.cos(a);
    const y = (h / 2) * Math.sin(a);
    return {
      X: Math.round((x * Math.cos(angle) - y * Math.sin(angle)) * SCALE),
      Y: Math.round((x * Math.sin(angle) + y * Math.cos(angle)) * SCALE),
    };
  });
  return (ClipperLib.Clipper.MinkowskiSum(pat, toC(path), false) as CPath[]).map(fromC);
}

// ---- boolean operations ----

function run(type: number, subject: Poly[], clip: Poly[]): { outer: Poly[]; holes: Poly[] } {
  const c = new ClipperLib.Clipper();
  c.AddPaths(subject.map(toC), ClipperLib.PolyType.ptSubject, true);
  if (clip.length) c.AddPaths(clip.map(toC), ClipperLib.PolyType.ptClip, true);
  const tree = new ClipperLib.PolyTree();
  c.Execute(type, tree, ClipperLib.PolyFillType.pftNonZero, ClipperLib.PolyFillType.pftNonZero);
  const outer: Poly[] = [];
  const holes: Poly[] = [];
  const walk = (node: { Childs(): unknown[]; Contour(): CPath; IsHole(): boolean }): void => {
    for (const child of node.Childs() as (typeof node)[]) {
      const poly = fromC(ClipperLib.Clipper.CleanPolygon(child.Contour(), 0.5) as CPath);
      if (poly.length >= 3) (child.IsHole() ? holes : outer).push(poly);
      walk(child);
    }
  };
  walk(tree as never);
  return { outer, holes };
}

/** A filled shape: outer polygons counter-clockwise and holes clockwise, never overlapping. */
export interface Shape {
  outer: Poly[];
  holes: Poly[];
}

const all = (s: Shape): Poly[] => [...s.outer, ...s.holes];
const ccw = (p: Poly): Poly => (polyArea(p) >= 0 ? p : [...p].reverse());
const cw = (p: Poly): Poly => (polyArea(p) <= 0 ? p : [...p].reverse());
const tidy = (r: { outer: Poly[]; holes: Poly[] }): Shape => ({ outer: r.outer.map(ccw), holes: r.holes.map(cw) });

/** Union of polygons (filled by the non-zero rule) and shapes. */
export function union(...parts: (Poly | Poly[] | Shape)[]): Shape {
  const polys: Poly[] = [];
  for (const p of parts) {
    // A single polygon is a filled region whichever way it was drawn.
    if (Array.isArray(p) && p.length && 'x' in (p[0] as object)) polys.push(ccw(p as Poly));
    else if (Array.isArray(p)) for (const q of p as Poly[]) polys.push(q);
    else polys.push(...all(p as Shape));
  }
  return tidy(run(ClipperLib.ClipType.ctUnion, polys, []));
}

/** `a` with `b` cut away. */
export const difference = (a: Shape, b: Shape): Shape =>
  tidy(run(ClipperLib.ClipType.ctDifference, all(a), all(b)));

/** The part of `a` inside `b`. */
export const intersection = (a: Shape, b: Shape): Shape =>
  tidy(run(ClipperLib.ClipType.ctIntersection, all(a), all(b)));

/** A shape moved and scaled about the origin. */
export const transform = (s: Shape, f: (p: P) => P): Shape => tidy({ outer: s.outer.map((p) => p.map(f)), holes: s.holes.map((p) => p.map(f)) });

/** Grows (positive) or shrinks (negative) a shape by `d` units, keeping its corners sharp. */
function offset(s: Shape, d: number): Shape {
  const off = new ClipperLib.ClipperOffset(8, 0.25 * SCALE);
  off.AddPaths(all(s).map(toC), ClipperLib.JoinType.jtMiter, ClipperLib.EndType.etClosedPolygon);
  const out = new ClipperLib.PolyTree();
  off.Execute(out, d * SCALE);
  const outer: Poly[] = [];
  const holes: Poly[] = [];
  const walk = (node: { Childs(): unknown[]; Contour(): CPath; IsHole(): boolean }): void => {
    for (const child of node.Childs() as (typeof node)[]) {
      const poly = fromC(child.Contour());
      if (poly.length >= 3) (child.IsHole() ? holes : outer).push(poly);
      walk(child);
    }
  };
  walk(out as never);
  return tidy({ outer, holes });
}

/**
 * Tidies a shape for fitting: fills notches and closes gaps narrower than `2d` (closing), then removes
 * slivers of ink thinner than that (opening). Where strokes meet at a shallow angle a boolean union
 * leaves hairline notches and specks that would fit as crossing curves.
 */
export const tidyShape = (s: Shape, d = 2): Shape => offset(offset(offset(offset(s, d), -2 * d), d * 2), -d);

// ---- fitting polygons back to curves ----

const sub = (a: P, b: P): P => ({ x: a.x - b.x, y: a.y - b.y });
const add = (a: P, b: P): P => ({ x: a.x + b.x, y: a.y + b.y });
const mul = (a: P, k: number): P => ({ x: a.x * k, y: a.y * k });
const dot = (a: P, b: P): number => a.x * b.x + a.y * b.y;
const len = (a: P): number => Math.hypot(a.x, a.y);
const unit = (a: P): P => {
  const l = len(a) || 1;
  return { x: a.x / l, y: a.y / l };
};

/** A segment this long or longer is a straight part of the outline, not a chord of a curve. */
const LINE = 14;
/** A turn sharper than this, in degrees, is a corner. */
let CORNER = 32;
/** Curve fitting tolerance, in font units. */
let TOL = 0.5;

/** Fits a cubic to points with given end tangents (Schneider, "An Algorithm for Automatically Fitting Digitized Curves"). */
function fitCubic(pts: P[], t1: P, t2: P, out: Seg[]): void {
  const n = pts.length;
  const first = pts[0] as P;
  const last = pts[n - 1] as P;
  if (n === 2) {
    const d = len(sub(last, first)) / 3;
    out.push({ t: 'C', c1: add(first, mul(t1, d)), c2: add(last, mul(t2, d)), p: last });
    return;
  }
  let u = chordParams(pts);
  let best = generate(pts, u, t1, t2);
  let [err, split] = maxError(pts, best, u);
  if (err < TOL) {
    out.push(best);
    return;
  }
  if (err < TOL * 16) {
    for (let i = 0; i < 12; i++) {
      u = reparam(pts, u, best);
      best = generate(pts, u, t1, t2);
      [err, split] = maxError(pts, best, u);
      if (err < TOL) {
        out.push(best);
        return;
      }
    }
  }
  const centre = unit(sub(pts[split - 1] as P, pts[split + 1] as P));
  fitCubic(pts.slice(0, split + 1), t1, centre, out);
  fitCubic(pts.slice(split), mul(centre, -1), t2, out);
}

function chordParams(pts: P[]): number[] {
  const u = [0];
  for (let i = 1; i < pts.length; i++) u.push((u[i - 1] as number) + len(sub(pts[i] as P, pts[i - 1] as P)));
  const total = u[u.length - 1] as number;
  return u.map((v) => v / total);
}

function generate(pts: P[], u: number[], t1: P, t2: P): Seg & { t: 'C' } {
  const first = pts[0] as P;
  const last = pts[pts.length - 1] as P;
  let c00 = 0;
  let c01 = 0;
  let c11 = 0;
  let x0 = 0;
  let x1 = 0;
  pts.forEach((p, i) => {
    const t = u[i] as number;
    const b0 = (1 - t) ** 3;
    const b1 = 3 * t * (1 - t) ** 2;
    const b2 = 3 * t * t * (1 - t);
    const b3 = t ** 3;
    const a1 = mul(t1, b1);
    const a2 = mul(t2, b2);
    c00 += dot(a1, a1);
    c01 += dot(a1, a2);
    c11 += dot(a2, a2);
    const tmp = sub(p, add(mul(first, b0 + b1), mul(last, b2 + b3)));
    x0 += dot(a1, tmp);
    x1 += dot(a2, tmp);
  });
  const det = c00 * c11 - c01 * c01;
  let al = det ? (x0 * c11 - x1 * c01) / det : 0;
  let ar = det ? (c00 * x1 - c01 * x0) / det : 0;
  const seg = len(sub(last, first));
  if (al < 1e-6 * seg || ar < 1e-6 * seg || al > seg * 2 || ar > seg * 2) al = ar = seg / 3;
  return { t: 'C', c1: add(first, mul(t1, al)), c2: add(last, mul(t2, ar)), p: last };
}

function maxError(pts: P[], c: Seg & { t: 'C' }, u: number[]): [number, number] {
  let max = 0;
  let at = Math.floor(pts.length / 2);
  const p0 = pts[0] as P;
  for (let i = 1; i < pts.length - 1; i++) {
    const d = len(sub(bez(p0, c.c1, c.c2, c.p, u[i] as number), pts[i] as P));
    if (d > max) {
      max = d;
      at = i;
    }
  }
  return [max, at];
}

function reparam(pts: P[], u: number[], c: Seg & { t: 'C' }): number[] {
  const p0 = pts[0] as P;
  const q1 = [mul(sub(c.c1, p0), 3), mul(sub(c.c2, c.c1), 3), mul(sub(c.p, c.c2), 3)];
  const q2 = [mul(sub(q1[1] as P, q1[0] as P), 2), mul(sub(q1[2] as P, q1[1] as P), 2)];
  return u.map((t, i) => {
    const p = bez(p0, c.c1, c.c2, c.p, t);
    const s = 1 - t;
    const d1 = add(add(mul(q1[0] as P, s * s), mul(q1[1] as P, 2 * s * t)), mul(q1[2] as P, t * t));
    const d2 = add(mul(q2[0] as P, s), mul(q2[1] as P, t));
    const diff = sub(p, pts[i] as P);
    const num = dot(diff, d1);
    const den = dot(d1, d1) + dot(diff, d2);
    const nt = den ? t - num / den : t;
    return Math.min(1, Math.max(0, nt));
  });
}

/** Splits a cubic at parameter t. */
function splitCubic(p0: P, c: Seg & { t: 'C' }, t: number): [Seg & { t: 'C' }, Seg & { t: 'C' }] {
  const lerp = (a: P, b: P): P => add(a, mul(sub(b, a), t));
  const a = lerp(p0, c.c1);
  const b = lerp(c.c1, c.c2);
  const d = lerp(c.c2, c.p);
  const e = lerp(a, b);
  const f = lerp(b, d);
  const m = lerp(e, f);
  return [
    { t: 'C', c1: a, c2: e, p: m },
    { t: 'C', c1: f, c2: d, p: c.p },
  ];
}

/** Parameters in (0, 1) where a cubic's x or y derivative is zero. */
export function extremaParams(p0: P, c: Seg & { t: 'C' }): number[] {
  const ts: number[] = [];
  for (const k of ['x', 'y'] as const) {
    const a = -p0[k] + 3 * c.c1[k] - 3 * c.c2[k] + c.p[k];
    const b = 2 * (p0[k] - 2 * c.c1[k] + c.c2[k]);
    const d = c.c1[k] - p0[k];
    const roots: number[] = [];
    if (Math.abs(a) < 1e-9) {
      if (Math.abs(b) > 1e-9) roots.push(-d / b);
    } else {
      const disc = b * b - 4 * a * d;
      if (disc >= 0) roots.push((-b + Math.sqrt(disc)) / (2 * a), (-b - Math.sqrt(disc)) / (2 * a));
    }
    for (const t of roots) if (t > 0.02 && t < 0.98) ts.push(t);
  }
  return ts.sort((x, y) => x - y);
}

/** Fits a polygon (from a boolean operation) with lines and cubics, splitting curves at their extrema. */
export function fitPolygon(poly: Poly): Contour {
  // Drop duplicate points and spikes (a vertex where the outline doubles back on itself).
  let pts = poly.filter((p, i) => len(sub(p, poly[(i + poly.length - 1) % poly.length] as P)) > 1e-6);
  for (let changed = true; changed && pts.length > 3; ) {
    changed = false;
    for (let i = 0; i < pts.length; i++) {
      const a = pts[(i + pts.length - 1) % pts.length] as P;
      const b = pts[i] as P;
      const c = pts[(i + 1) % pts.length] as P;
      const u = unit(sub(b, a));
      const v = unit(sub(c, b));
      if (dot(u, v) < -0.985 || len(sub(c, a)) < 1e-6) {
        pts = pts.filter((_, k) => k !== i);
        changed = true;
        break;
      }
    }
  }
  const n = pts.length;
  const at = (i: number): P => pts[((i % n) + n) % n] as P;
  const segLen = (i: number): number => len(sub(at(i + 1), at(i)));
  // A vertex is a break where the outline turns sharply or where a straight run starts or ends.
  const isBreak = (i: number): boolean => {
    const a = unit(sub(at(i), at(i - 1)));
    const b = unit(sub(at(i + 1), at(i)));
    const turn = (Math.acos(Math.max(-1, Math.min(1, dot(a, b)))) * 180) / Math.PI;
    return turn > CORNER || segLen(i) >= LINE || segLen(i - 1) >= LINE;
  };
  const breaks: number[] = [];
  for (let i = 0; i < n; i++) if (isBreak(i)) breaks.push(i);
  if (breaks.length === 0) {
    // A smooth closed curve: break at the leftmost point and the rightmost.
    let lo = 0;
    let hi = 0;
    pts.forEach((p, i) => {
      if (p.x < (pts[lo] as P).x) lo = i;
      if (p.x > (pts[hi] as P).x) hi = i;
    });
    breaks.push(...[lo, hi].sort((a, b) => a - b));
  }
  const segs: Seg[] = [];
  const startIndex = breaks[0] as number;
  for (let k = 0; k < breaks.length; k++) {
    const i0 = breaks[k] as number;
    const i1 = k + 1 < breaks.length ? (breaks[k + 1] as number) : startIndex + n;
    const run: P[] = [];
    for (let i = i0; i <= i1; i++) run.push(at(i));
    if (run.length === 2) {
      segs.push({ t: 'L', p: run[1] as P });
      continue;
    }
    // Lines inside the run (long chords) are kept as lines; the curved stretches between them are fitted.
    let s = 0;
    for (let i = 0; i < run.length - 1; i++) {
      if (len(sub(run[i + 1] as P, run[i] as P)) >= LINE) {
        if (i > s) fitRun(run.slice(s, i + 1), segs);
        segs.push({ t: 'L', p: run[i + 1] as P });
        s = i + 1;
      }
    }
    if (s < run.length - 1) fitRun(run.slice(s), segs);
  }
  return splitAtExtrema({ start: at(startIndex), segs });
}

function fitRun(run: P[], out: Seg[]): void {
  if (run.length === 2) {
    out.push({ t: 'L', p: run[1] as P });
    return;
  }
  // A run that is almost straight stays a line.
  const a = run[0] as P;
  const b = run[run.length - 1] as P;
  const d = unit(sub(b, a));
  const off = Math.max(...run.map((p) => Math.abs((p.x - a.x) * d.y - (p.y - a.y) * d.x)));
  if (off < TOL) {
    out.push({ t: 'L', p: b });
    return;
  }
  const k = Math.min(3, run.length - 1);
  const t1 = unit(sub(run[k] as P, a));
  const t2 = unit(sub(run[run.length - 1 - k] as P, b));
  fitCubic(run, t1, t2, out);
}

function splitAtExtrema(c: Contour): Contour {
  const segs: Seg[] = [];
  let p0 = c.start;
  for (const s of c.segs) {
    if (s.t === 'L') {
      segs.push(s);
      p0 = s.p;
      continue;
    }
    let rest: Seg & { t: 'C' } = s;
    let from = p0;
    let consumed = 0;
    for (const t of extremaParams(p0, s)) {
      const local = (t - consumed) / (1 - consumed);
      const [a, b] = splitCubic(from, rest, local);
      if (len(sub(a.p, from)) < 0.5 || len(sub(b.p, a.p)) < 0.5) continue;
      segs.push(a);
      from = a.p;
      rest = b;
      consumed = t;
    }
    segs.push(rest);
    p0 = s.p;
  }
  return { start: c.start, segs };
}

/** Rounds every point of a contour to whole font units, dropping segments that collapse. */
export function roundContour(c: Contour): Contour {
  const r = (p: P): P => ({ x: Math.round(p.x), y: Math.round(p.y) });
  const start = r(c.start);
  const segs: Seg[] = [];
  let last = start;
  for (const s of c.segs) {
    const p = r(s.p);
    if (p.x === last.x && p.y === last.y) continue;
    // A curve of a unit or two is a line at any size the font is used at.
    const tiny = Math.abs(p.x - last.x) + Math.abs(p.y - last.y) <= 2;
    segs.push(s.t === 'L' || tiny ? { t: 'L', p } : { t: 'C', c1: r(s.c1), c2: r(s.c2), p });
    last = p;
  }
  // The final segment must end at the start.
  const tail = segs[segs.length - 1];
  if (tail && (tail.p.x !== start.x || tail.p.y !== start.y)) segs.push({ t: 'L', p: start });
  // Merge consecutive lines that run straight on.
  const merged: Seg[] = [];
  let prev = start;
  for (const s of segs) {
    const m = merged[merged.length - 1];
    if (s.t === 'L' && m && m.t === 'L') {
      const before = merged.length > 1 ? (merged[merged.length - 2] as Seg).p : start;
      if ((m.p.x - before.x) * (s.p.y - m.p.y) - (m.p.y - before.y) * (s.p.x - m.p.x) === 0) {
        merged[merged.length - 1] = s;
        prev = s.p;
        continue;
      }
    }
    merged.push(s);
    prev = s.p;
  }
  return { start, segs: merged.length >= 2 ? merged : segs, ...(prev ? {} : {}) };
}

/** Outline contours of a shape, fitted and rounded: outer contours counter-clockwise, holes clockwise. */
export function outline(s: Shape): Contour[] {
  return [...s.outer, ...s.holes]
    .filter((p) => Math.abs(polyArea(p)) >= 150)
    .map(fitSafely)
    .filter((c) => c.segs.length >= 2);
}

/** Whether a fitted contour crosses itself anywhere. */
function selfCrosses(c: Contour): boolean {
  const pts = sampleContour(c, 6);
  const n = pts.length - 1;
  const o = (p: P, q: P, r: P): number => (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
  for (let i = 0; i < n; i++) {
    const a = pts[i] as P;
    const b = pts[i + 1] as P;
    for (let j = i + 2; j < n; j++) {
      if (i === 0 && j === n - 1) continue;
      const c2 = pts[j] as P;
      const d = pts[j + 1] as P;
      if (Math.max(a.x, b.x) < Math.min(c2.x, d.x) || Math.max(c2.x, d.x) < Math.min(a.x, b.x)) continue;
      if (Math.max(a.y, b.y) < Math.min(c2.y, d.y) || Math.max(c2.y, d.y) < Math.min(a.y, b.y)) continue;
      const d1 = o(c2, d, a);
      const d2 = o(c2, d, b);
      const d3 = o(a, b, c2);
      const d4 = o(a, b, d);
      if (((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0))) return true;
    }
  }
  return false;
}

/**
 * Fits a polygon, and where the fit loops on itself (a tight turn the corner rule missed), fits it again
 * with more corners and a tighter tolerance, and in the last resort keeps the polygon's own lines.
 */
function fitSafely(p: Poly): Contour {
  for (const [corner, tol] of [
    [32, 0.5],
    [12, 0.3],
    [6, 0.15],
  ] as const) {
    CORNER = corner;
    TOL = tol;
    const c = roundContour(fitPolygon(p));
    CORNER = 32;
    TOL = 0.5;
    if (!selfCrosses(c)) return c;
  }
  // The polygon itself, rounded, with any loop that rounding made cut out.
  let pts = p.map((q) => ({ x: Math.round(q.x), y: Math.round(q.y) }));
  for (let pass = 0; pass < 20; pass++) {
    const loop = firstLoop(pts);
    if (!loop) break;
    const [i, j, x] = loop;
    pts = [...pts.slice(0, i + 1), x, ...pts.slice(j + 1)];
  }
  const start = pts[0] as P;
  return roundContour({ start, segs: [...pts.slice(1), start].map((q) => ({ t: 'L', p: q }) as Seg) });
}

/** The first pair of edges of a closed polyline that cross, and where (rounded), or null. */
function firstLoop(pts: P[]): [number, number, P] | null {
  const n = pts.length;
  for (let i = 0; i < n; i++) {
    const a = pts[i] as P;
    const b = pts[(i + 1) % n] as P;
    for (let j = i + 2; j < n; j++) {
      if (i === 0 && j === n - 1) continue;
      const c = pts[j] as P;
      const d = pts[(j + 1) % n] as P;
      const den = (b.x - a.x) * (d.y - c.y) - (b.y - a.y) * (d.x - c.x);
      if (den === 0) continue;
      const t = ((c.x - a.x) * (d.y - c.y) - (c.y - a.y) * (d.x - c.x)) / den;
      const u = ((c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x)) / den;
      if (t > 0 && t < 1 && u > 0 && u < 1) return [i, j, { x: Math.round(a.x + t * (b.x - a.x)), y: Math.round(a.y + t * (b.y - a.y)) }];
    }
  }
  return null;
}

/** Signed area of a contour (exact for cubics), positive counter-clockwise. */
export function contourArea(c: Contour): number {
  let a = 0;
  let p0 = c.start;
  const x = (p: P, q: P): number => p.x * q.y - q.x * p.y;
  for (const s of c.segs) {
    if (s.t === 'L') a += x(p0, s.p);
    else a += (6 * x(p0, s.c1) + 3 * x(p0, s.c2) + x(p0, s.p) + 3 * x(s.c1, s.c2) + 3 * x(s.c1, s.p) + 6 * x(s.c2, s.p)) / 10;
    p0 = s.p;
  }
  return a / 2;
}

/** Points along a contour, for tests and rendering checks. */
export function sampleContour(c: Contour, steps = 8): P[] {
  const out: P[] = [c.start];
  let p0 = c.start;
  for (const s of c.segs) {
    if (s.t === 'L') out.push(s.p);
    else for (let i = 1; i <= steps; i++) out.push(bez(p0, s.c1, s.c2, s.p, i / steps));
    p0 = s.p;
  }
  return out;
}

/**
 * TrueType quadratics for a contour: each cubic is split into as few quadratics as keep within a
 * quarter of a unit before rounding. Returns points with on-curve flags, in the contour's direction.
 */
export function toQuadratic(c: Contour): { x: number; y: number; on: boolean }[] {
  const out: { x: number; y: number; on: boolean }[] = [{ ...c.start, on: true }];
  let p0 = c.start;
  for (const s of c.segs) {
    if (s.t === 'L') {
      out.push({ ...s.p, on: true });
      p0 = s.p;
      continue;
    }
    for (let n = 1; n <= 16; n++) {
      const pieces: { q: P; p: P }[] = [];
      let ok = true;
      let from = p0;
      let rest: Seg & { t: 'C' } = s;
      for (let k = 0; k < n; k++) {
        const [a, b] = k < n - 1 ? splitCubic(from, rest, 1 / (n - k)) : [rest, rest];
        const q = mul(sub(add(mul(add(a.c1, a.c2), 3), mul(add(from, a.p), -1)), { x: 0, y: 0 }), 0.25);
        for (let i = 1; i < 10; i++) {
          const t = i / 10;
          const cb = bez(from, a.c1, a.c2, a.p, t);
          const u = 1 - t;
          const qb = { x: u * u * from.x + 2 * u * t * q.x + t * t * a.p.x, y: u * u * from.y + 2 * u * t * q.y + t * t * a.p.y };
          if (len(sub(cb, qb)) > 0.25) ok = false;
        }
        pieces.push({ q, p: a.p });
        from = a.p;
        rest = b;
        if (!ok) break;
      }
      if (ok || n === 16) {
        for (const pc of pieces) out.push({ ...pc.q, on: false }, { ...pc.p, on: true });
        break;
      }
    }
    p0 = s.p;
  }
  out.pop(); // the last point repeats the start
  return out.map((p) => ({ x: Math.round(p.x), y: Math.round(p.y), on: p.on }));
}
