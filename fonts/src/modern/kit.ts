// The design system of the outline faces: one set of parameters per face and the drawing kit every glyph is built
// with (stems, bars, bowls, arcs, diagonals, dots, serifs and brackets), so Sans, Mono and Serif share their skeletons.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { difference, nib, type P, type Poly, rect, type Shape, stroke, superArc, superEllipse, superPoint, union } from './geom';

/** One outline face: its metrics and the weights and widths its glyphs are drawn with. */
export interface Style {
  /** `sans`, `mono` or `serif`. */
  kind: 'sans' | 'mono' | 'serif';
  bold: boolean;
  /** Italic: the drawing is slanted by `slant` degrees after it is spaced, with per-glyph italic forms. */
  italic?: boolean;
  slant?: number;
  family: string;
  file: string;
  /** Capital, x-height, ascender and descender, in units of a 1000-unit em. */
  cap: number;
  xh: number;
  asc: number;
  desc: number;
  /** How far round letters reach past the baseline, x-height and cap height. */
  os: number;
  /** Thickness of vertical stems and of horizontal strokes. */
  V: number;
  H: number;
  /** Superellipse exponent of the bowls: 2 is an ellipse, larger is squarer. */
  n: number;
  /** Width factor for the proportional letters. */
  wide: number;
  /** Side bearing of a straight side; round, diagonal and open sides are fractions of it. */
  sb: number;
  /** Mono only: the advance of every glyph. */
  advance?: number;
  /** Serif only: serif length beyond the stem, thickness and bracket radius. */
  serifLen: number;
  serifH: number;
  bracket: number;
}

const base = { cap: 700, xh: 528, asc: 742, desc: -208, os: 11 };

/**
 * Desk Serif's contrast: how much thinner its horizontals, hairlines and serifs are than its stems, as a
 * fraction of the stem (0 is monoline). Bold keeps more of its hairlines so it holds up at 16 px.
 */
export const SERIF_CONTRAST = { regular: 0.64, bold: 0.58 };

function serif(bold: boolean): Style {
  const V = bold ? 162 : 104;
  const H = Math.round(V * (1 - (bold ? SERIF_CONTRAST.bold : SERIF_CONTRAST.regular)));
  return {
    ...base,
    cap: 690,
    xh: 498,
    asc: 735,
    desc: -222,
    kind: 'serif',
    bold,
    family: 'Desk Serif',
    file: bold ? 'desk-serif-bold' : 'desk-serif',
    V,
    H,
    n: 2.05,
    wide: bold ? 1.08 : 1.02,
    sb: bold ? 30 : 34,
    serifLen: bold ? 50 : 54,
    serifH: Math.round(H * 0.86),
    bracket: bold ? 24 : 28,
  };
}

/** Slant of each family's italic, in degrees. */
export const SLANT = { sans: 9.5, mono: 9, serif: 12 };

/**
 * The italic of an upright style: the same weights, slanted by its family's angle; Serif's is narrower,
 * as a true italic is.
 */
function italic(s: Style): Style {
  return {
    ...s,
    italic: true,
    slant: SLANT[s.kind],
    file: `${s.file}-italic`,
    wide: s.kind === 'serif' ? s.wide * 0.9 : s.kind === 'sans' ? s.wide * 0.97 : s.wide,
    // Sans bowls a little rounder, so slanting does not make them look boxy.
    n: s.kind === 'sans' ? s.n - 0.1 : s.n,
    sb: s.kind === 'serif' ? Math.round(s.sb * 1.1) : s.sb,
  };
}

const UPRIGHT: Style[] = [
  { ...base, kind: 'sans', bold: false, family: 'Desk Sans', file: 'desk-sans', V: 86, H: 76, n: 2.35, wide: 1, sb: 62, serifLen: 0, serifH: 0, bracket: 0 },
  { ...base, kind: 'sans', bold: true, family: 'Desk Sans', file: 'desk-sans-bold', V: 144, H: 104, n: 2.2, wide: 1.1, sb: 52, serifLen: 0, serifH: 0, bracket: 0 },
  { ...base, xh: 536, kind: 'mono', bold: false, family: 'Desk Mono', file: 'desk-mono', V: 82, H: 74, n: 2.4, wide: 0.86, sb: 60, advance: 600, serifLen: 0, serifH: 0, bracket: 0 },
  { ...base, xh: 536, kind: 'mono', bold: true, family: 'Desk Mono', file: 'desk-mono-bold', V: 140, H: 120, n: 2.3, wide: 0.86, sb: 50, advance: 600, serifLen: 0, serifH: 0, bracket: 0 },
  serif(false),
  serif(true),
];

/** Every outline face: the uprights, then their italics. */
export const STYLES: Style[] = [...UPRIGHT, ...UPRIGHT.map(italic)];

/** Where a stem gets serifs, in the Serif face: none, both sides, or one side. */
export type SerifAt = 'none' | 'both' | 'left' | 'right';

export interface StemOpts {
  w?: number;
  top?: SerifAt;
  bottom?: SerifAt;
}

/** The drawing kit handed to every glyph. Coordinates are font units, y up, x from the glyph's left ink edge. */
export class Kit {
  readonly s: Style;
  private adds: (Poly | Poly[] | Shape)[] = [];
  private cuts: (Poly | Poly[] | Shape)[] = [];

  constructor(s: Style) {
    this.s = s;
  }

  get V(): number {
    return this.s.V;
  }
  get H(): number {
    return this.s.H;
  }
  get cap(): number {
    return this.s.cap;
  }
  get xh(): number {
    return this.s.xh;
  }
  get asc(): number {
    return this.s.asc;
  }
  get desc(): number {
    return this.s.desc;
  }
  get os(): number {
    return this.s.os;
  }
  get serif(): boolean {
    return this.s.kind === 'serif';
  }
  get mono(): boolean {
    return this.s.kind === 'mono';
  }
  get italic(): boolean {
    return !!this.s.italic;
  }
  get bold(): boolean {
    return this.s.bold;
  }

  /** A body width: the proportional width scaled for the face, or the mono width when given. */
  w(prop: number, mono?: number): number {
    if (this.mono) return Math.round(mono ?? prop * this.s.wide);
    return Math.round(prop * this.s.wide);
  }

  add(...parts: (Poly | Poly[] | Shape)[]): this {
    this.adds.push(...parts);
    return this;
  }

  cut(...parts: (Poly | Poly[] | Shape)[]): this {
    this.cuts.push(...parts);
    return this;
  }

  /** Cuts away everything outside a box, for diagonals whose level ends poke past a stem or the cap height. */
  within(x0: number, y0: number, x1: number, y1: number): this {
    const B = 3000;
    return this.cut(rect(-B, -B, x0, B), rect(x1, -B, B, B), rect(-B, -B, B, y0), rect(-B, y1, B, B));
  }

  /** The finished shape: everything added, unioned, with the cuts removed. */
  shape(): Shape {
    const u = union(...this.adds);
    return this.cuts.length ? difference(u, union(...this.cuts)) : u;
  }

  /** A vertical stem with its left edge at `x`, from `y0` to `y1`, with serifs as asked in the Serif face. */
  stem(x: number, y0: number, y1: number, o: StemOpts = {}): this {
    const w = o.w ?? this.V;
    this.add(rect(x, y0, x + w, y1));
    if (this.serif) {
      this.serifs(x, w, y0, o.bottom ?? 'both', 'bottom');
      this.serifs(x, w, y1, o.top ?? 'none', 'top');
    }
    return this;
  }

  /** Serifs at one end of a stem: slabs with brackets on the chosen sides. */
  serifs(x: number, w: number, y: number, at: SerifAt, end: 'top' | 'bottom'): this {
    if (!this.serif || at === 'none') return this;
    const { serifLen: L, serifH: h, bracket: r } = this.s;
    const dy = end === 'bottom' ? 1 : -1;
    const yIn = y + dy * h;
    const l = at === 'both' || at === 'left';
    const rt = at === 'both' || at === 'right';
    // A head serif on one side only is longer and slightly tapered, like a pen's entry stroke.
    const lenL = at === 'left' && end === 'top' ? L * 1.15 : L;
    const x0 = l ? x - lenL : x;
    const x1 = rt ? x + w + L : x + w;
    this.add(rect(x0, y, x1, yIn));
    if (l) this.add(fillet({ x, y: yIn }, -1, dy, r));
    if (rt) this.add(fillet({ x: x + w, y: yIn }, 1, dy, r));
    return this;
  }

  /** A horizontal bar from `x0` to `x1` with its bottom at `y`. */
  bar(x0: number, x1: number, y: number, h = this.H): this {
    return this.add(rect(x0, y, x1, y + h));
  }

  /** A bar centred on `y`. */
  hbar(x0: number, x1: number, y: number, h = this.H): this {
    return this.add(rect(x0, y - h / 2, x1, y + h / 2));
  }

  /** A whole bowl filling the box, sides `v` thick and top and bottom `h` thick. */
  ring(x0: number, x1: number, y0: number, y1: number, v = this.V, h = this.H, n = this.s.n): this {
    const cx = (x0 + x1) / 2;
    const cy = (y0 + y1) / 2;
    const rx = (x1 - x0) / 2;
    const ry = (y1 - y0) / 2;
    return this.add(difference(union(superEllipse(cx, cy, rx, ry, n)), union(superEllipse(cx, cy, rx - v, ry - h, n))));
  }

  /** The filled inside of a bowl box (for counters and cuts). */
  oval(x0: number, x1: number, y0: number, y1: number, n = this.s.n): Poly {
    return superEllipse((x0 + x1) / 2, (y0 + y1) / 2, (x1 - x0) / 2, (y1 - y0) / 2, n);
  }

  /**
   * Part of a bowl, counter-clockwise from angle `a0` to `a1` (degrees; 0 is the right, 90 the top),
   * ends cut along the stroke's own normal.
   */
  arc(x0: number, x1: number, y0: number, y1: number, a0: number, a1: number, v = this.V, h = this.H, n = this.s.n): this {
    this.add(arcPoly(x0, x1, y0, y1, a0, a1, v, h, n));
    return this;
  }

  /** A diagonal from (xb, yb) to (xt, yt), `w` thick across, its ends cut level. */
  diag(xb: number, yb: number, xt: number, yt: number, w = this.V * 0.94): this {
    const dx = xt - xb;
    const dy = yt - yb;
    const hw = (w * Math.hypot(dx, dy)) / Math.abs(dy) / 2;
    return this.add([
      { x: xb - hw, y: yb },
      { x: xb + hw, y: yb },
      { x: xt + hw, y: yt },
      { x: xt - hw, y: yt },
    ]);
  }

  /** A straight stroke with square ends. */
  line(x0: number, y0: number, x1: number, y1: number, w = this.V, ends: 'butt' | 'round' = 'butt'): this {
    return this.add(stroke([{ x: x0, y: y0 }, { x: x1, y: y1 }], w, ends));
  }

  /** A stroke along any path. */
  path(pts: P[], w = this.V, ends: 'butt' | 'round' = 'butt'): this {
    return this.add(stroke(pts, w, ends));
  }

  /** A broad-nib stroke along a path, for curved details that need thick and thin. */
  pen(pts: P[], w: number, h: number, angle: number): this {
    return this.add(nib(pts, w, h, angle));
  }

  /** A dot (i, j, punctuation) centred at (cx, cy). */
  dot(cx: number, cy: number, d = this.dotSize): this {
    return this.add(superEllipse(cx, cy, d / 2, d / 2, this.serif ? 2 : 2.6));
  }

  get dotSize(): number {
    return Math.round(this.V * (this.bold ? 1.08 : 1.24));
  }

  /**
   * A serif-face terminal on the end of a bowl at angle `a` (degrees): the end thickened by a short
   * vertical spur, hanging down at a top terminal (`dir` -1) or rising at a bottom one (`dir` 1).
   */
  spur(x0: number, x1: number, y0: number, y1: number, a: number, dir: 1 | -1, len: number, v = this.V, h = this.H, n = this.s.n): this {
    if (!this.serif) return this;
    const o = bowlPoint(x0, x1, y0, y1, a, n);
    const i = bowlPoint(x0 + v, x1 - v, y0 + h, y1 - h, a, n);
    const side = o.x > (x0 + x1) / 2 ? 1 : -1;
    return this.add([o, { x: o.x, y: o.y + dir * len }, { x: o.x - side * v * 0.62, y: o.y + dir * len * 0.78 }, i]);
  }

}

/** A quarter-round fillet in the corner at `c`, reaching `r` along x (direction `dx`) and along y (`dy`). */
export function fillet(c: P, dx: number, dy: number, r: number): Poly {
  if (r <= 0) return [];
  const centre = { x: c.x + dx * r, y: c.y + dy * r };
  const pts: P[] = [c, { x: c.x + dx * r, y: c.y }];
  const steps = 16;
  // From straight below/above the centre (on the slab) round to the stem side.
  const a0 = Math.atan2(-dy, 0);
  const a1 = Math.atan2(0, -dx);
  let d = a1 - a0;
  if (d > Math.PI) d -= 2 * Math.PI;
  if (d < -Math.PI) d += 2 * Math.PI;
  for (let i = 0; i <= steps; i++) {
    const a = a0 + (d * i) / steps;
    pts.push({ x: centre.x + r * Math.cos(a), y: centre.y + r * Math.sin(a) });
  }
  return pts;
}

/** A sector of a bowl as a polygon. */
export function arcPoly(x0: number, x1: number, y0: number, y1: number, a0: number, a1: number, v: number, h: number, n: number): Poly {
  const cx = (x0 + x1) / 2;
  const cy = (y0 + y1) / 2;
  const rx = (x1 - x0) / 2;
  const ry = (y1 - y0) / 2;
  const r0 = (a0 * Math.PI) / 180;
  const r1 = (a1 * Math.PI) / 180;
  const outer = superArc(cx, cy, rx, ry, n, r0, r1);
  const inner = superArc(cx, cy, rx - v, ry - h, n, r1, r0);
  return [...outer, ...inner];
}

/** A point on a bowl's outer edge, for placing joins. */
export const bowlPoint = (x0: number, x1: number, y0: number, y1: number, a: number, n: number): P =>
  superPoint((x0 + x1) / 2, (y0 + y1) / 2, (x1 - x0) / 2, (y1 - y0) / 2, n, (a * Math.PI) / 180);
