// The glyphs and layout of the outline faces' OpenType features: the f ligatures of Desk Sans and Desk Serif, their
// figure styles and alternates, and the kerning and substitutions the GSUB, GPOS and `kern` tables are written from.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ACCENTS } from '../face';
import { glyphName } from '../otf';
import { compose, draw, type MGlyph, type Raw, shapeBox, sideBearing } from './assemble';
import { codeLigatureGlyphs, codeLigatureLookup } from './code';
import { type P, type Poly, rect, type Shape, stroke, transform, union } from './geom';
import { tailStem } from './italics';
import { type Draw, LETTERS } from './letters';
import { fraction } from './symbols';
import { kerningPairs } from './kerning';
import { bowlPoint, Kit, type Style } from './kit';
import type { Layout, SubLookup } from './layout';

/** An unencoded glyph before placing: the character it stands for (for its slant) and its drawing. */
export interface Extra {
  ch: string;
  raw: Raw;
}

const moved = (sh: Shape, dx: number, dy = 0): Shape => transform(sh, (p) => ({ x: p.x + dx, y: p.y + dy }));

/** Points along every edge of a shape, about `step` units apart, for measuring distances. */
function edgePoints(sh: Shape, step = 4): P[] {
  const out: P[] = [];
  for (const poly of [...sh.outer, ...sh.holes] as Poly[]) {
    for (let i = 0; i < poly.length; i++) {
      const a = poly[i] as P;
      const b = poly[(i + 1) % poly.length] as P;
      const n = Math.max(1, Math.ceil(Math.hypot(b.x - a.x, b.y - a.y) / step));
      for (let j = 0; j < n; j++) out.push({ x: a.x + ((b.x - a.x) * j) / n, y: a.y + ((b.y - a.y) * j) / n });
    }
  }
  return out;
}

/** The smallest distance between the edges of two shapes (0 if they overlap). */
function distance(a: Shape, b: Shape): number {
  if (union(a, b).outer.length < a.outer.length + b.outer.length) return 0;
  const pa = edgePoints(a);
  const pb = edgePoints(b).sort((p, q) => p.x - q.x);
  let best = Number.POSITIVE_INFINITY;
  for (const p of pa) {
    // Only the points of b within the best distance so far along x can be closer.
    let lo = 0;
    let hi = pb.length;
    while (lo < hi) {
      const m = (lo + hi) >> 1;
      if ((pb[m] as P).x < p.x - best) lo = m + 1;
      else hi = m;
    }
    for (let i = lo; i < pb.length && (pb[i] as P).x <= p.x + best; i++) {
      const q = pb[i] as P;
      best = Math.min(best, Math.hypot(p.x - q.x, p.y - q.y));
    }
  }
  return best;
}

// ---- the f ligatures ----

/** Where an f's parts are, in its own drawing's units, and how to draw it with a different hood. */
interface FGeom {
  stemX: number;
  barL: number;
  barR: number;
  /** The hood's box and the angle its end is cut at (degrees, 0 is the right, 90 the top). */
  hood: { x0: number; x1: number; y0: number; y1: number; end: number; v: number };
  /** Draws the f at offset `dx` without its hood. */
  body: (k: Kit, dx: number) => void;
  /** The cursive f's descender hook from angle `from` round to the stem, or null for an f on the baseline. */
  tail: ((k: Kit, dx: number, from: number) => void) | null;
}

function fGeom(k: Kit): FGeom {
  if (k.italic && k.serif) {
    // Desk Serif Italic's f runs through the descender (`SERIF_ITALIC.f`).
    const w = k.w(360);
    const x = Math.round(w * 0.42);
    const r = 120;
    const v = k.V * 0.9;
    return {
      stemX: x,
      barL: x - w * 0.3,
      barR: x + k.V + w * 0.32,
      hood: { x0: x, x1: x + 2 * (w - x), y0: k.asc + k.os - 2 * r, y1: k.asc + k.os, end: 40, v },
      body: (kk, dx) => {
        kk.add([
          { x: dx + x, y: k.desc + r - k.os - 1 },
          { x: dx + x + v, y: k.desc + r - k.os - 1 },
          { x: dx + x + v, y: k.asc + k.os - r + 1 },
          { x: dx + x, y: k.asc + k.os - r + 1 },
        ]);
        kk.bar(dx + x - w * 0.3, dx + x + k.V + w * 0.32, k.xh - k.H);
      },
      tail: (kk, dx, from) => kk.arc(dx + x + v - 2 * (x + v), dx + x + v, k.desc - k.os, k.desc + 2 * r - k.os, from, 360, v, k.H),
    };
  }
  // The upright f (`LETTERS.f`), which Desk Sans Italic slants.
  const w = k.w(300, 380);
  const x = Math.round(w * 0.2);
  const top = k.asc + k.os;
  const ry = w * 0.6;
  const barR = Math.min(w, x + k.V + w * 0.45);
  return {
    stemX: x,
    barL: 0,
    barR,
    hood: { x0: x, x1: x + (w - x) * 2 - k.V * 0.2, y0: top - 2 * ry, y1: top, end: 52, v: k.V },
    body: (kk, dx) => {
      kk.stem(dx + x, 0, top - ry + 1, { bottom: k.serif ? 'both' : 'none' });
      kk.bar(dx, dx + barR, k.xh - k.H);
    },
    tail: null,
  };
}

/** A hood ending at `end` degrees in a box reaching to `x1`, drawn from the stem at `dx + stemX`. */
function hood(k: Kit, g: FGeom, dx: number, x1: number, end = g.hood.end): Shape {
  const kk = new Kit(k.s);
  kk.arc(dx + g.hood.x0, x1, g.hood.y0, g.hood.y1, end, 180, g.hood.v, k.H);
  return kk.shape();
}

/** The middle of a hood's end, for setting it over the stem of the i. */
function hoodEnd(k: Kit, g: FGeom, dx: number, x1: number): number {
  const { x0, y0, y1, end, v } = g.hood;
  const o = bowlPoint(dx + x0, x1, y0, y1, end, k.s.n);
  const i = bowlPoint(dx + x0 + v, x1 - v, y0 + k.H, y1 - k.H, end, k.s.n);
  return (o.x + i.x) / 2;
}

/** Solves for a box edge between `lo` and `hi` where `f` (increasing) reaches `target`. */
function solve(lo: number, hi: number, f: (x: number) => number, target: number): number {
  for (let i = 0; i < 20; i++) {
    const m = (lo + hi) / 2;
    if (f(m) < target) lo = m;
    else hi = m;
  }
  return (lo + hi) / 2;
}

/**
 * One f ligature: ff, fi, fl, ffi or ffl. The f's are drawn as the face draws them; a second f follows the first's
 * crossbar so the two bars run together, and the first f's hood is shortened to clear the second. Before an i the
 * last hood reaches over the dotless i and stands for its dot; before an l it arches over into the l's stem. In Desk
 * Serif the i's head serif meets the crossbar. Spaced like the f on the left and like the i or l on the right.
 */
function fLigature(s: Style, raws: Map<string, Raw>, seq: string): Raw {
  const k = new Kit(s);
  const g = fGeom(k);
  const italicSerif = k.italic && k.serif;
  const gap = Math.round(s.sb * 0.9);
  const fs = seq.replace(/[il]$/, '');
  const last = seq.endsWith('i') ? 'i' : seq.endsWith('l') ? 'l' : '';
  // The f's, each following the last one's crossbar.
  const at: number[] = [0];
  for (let i = 1; i < fs.length; i++) at.push((at[i - 1] as number) + g.barR + gap - g.stemX);
  const lastAt = at[at.length - 1] as number;
  for (const dx of at) g.body(k, dx);
  // The cursive f's descender hooks: the first keeps its own, and each later one is cut short to clear the f before.
  if (g.tail) {
    const tail = g.tail;
    g.tail(k, 0, 220);
    for (let i = 1; i < at.length; i++) {
      const before = new Kit(s);
      g.body(before, at[i - 1] as number);
      tail(before, at[i - 1] as number, 220);
      const prev = before.shape();
      const from = solve(220, 330, (a) => {
        const t = new Kit(s);
        tail(t, at[i] as number, a);
        return distance(t.shape(), prev);
      }, gap * 0.7);
      tail(k, at[i] as number, from);
    }
  }
  // The stem of the i or l after the last crossbar: in the upright Serif the i's head serif reaches the bar, and in
  // the italic the bar runs on into the i's stem in place of its entry stroke.
  const ser = s.serifLen * 1.15;
  const stemX = lastAt + g.barR + (last === 'i' && k.serif && !k.italic ? ser - 6 : gap);
  let right = 0;
  let rightSide: Raw['sides'][1] = 's';
  const hoods: Shape[] = [];
  // A hood before another f is shortened to keep clear of it by about a side bearing.
  for (let i = 0; i + 1 < at.length; i++) {
    const dx = at[i] as number;
    const next = new Kit(s);
    g.body(next, at[i + 1] as number);
    next.add(hood(k, g, at[i + 1] as number, (at[i + 1] as number) + g.hood.x1));
    const nextShape = next.shape();
    const x1 = solve(dx + g.stemX + g.hood.v * 2.2, dx + g.hood.x1, (x) => -distance(hood(k, g, dx, x), nextShape), -gap);
    hoods.push(hood(k, g, dx, x1));
  }
  if (last === 'i') {
    const i = raws.get('ı') as Raw;
    if (italicSerif) {
      tailStem(k, stemX, 0, k.xh);
      k.bar(lastAt + g.barR - 2, stemX + k.V * 0.5, k.xh - k.H);
    } else k.add(moved(i.shape, stemX));
    const target = stemX + k.V / 2;
    const x1 = solve(lastAt + g.stemX + g.hood.v * 2, lastAt + g.hood.x1 * 2, (x) => hoodEnd(k, g, lastAt, x), target);
    hoods.push(hood(k, g, lastAt, x1));
    right = stemX + (shapeBox(i.shape)?.x1 ?? k.V);
    rightSide = i.sides[1];
  } else if (last === 'l') {
    // The hood becomes an arch that lands on the l's stem.
    const x1 = stemX + k.V;
    const cy = (g.hood.y0 + g.hood.y1) / 2;
    const arch = new Kit(s);
    arch.arc(lastAt + g.hood.x0, x1, g.hood.y0, g.hood.y1, 0, 180, k.V, k.H);
    hoods.push(arch.shape());
    if (italicSerif) tailStem(k, stemX, 0, cy + 1);
    else k.stem(stemX, 0, cy + 1, { bottom: k.serif ? 'both' : 'none' });
    const l = raws.get('l') as Raw;
    right = stemX + (shapeBox(l.shape)?.x1 ?? k.V);
    rightSide = l.sides[1];
  } else {
    hoods.push(hood(k, g, lastAt, lastAt + g.hood.x1));
    const f = raws.get('f') as Raw;
    right = lastAt + (shapeBox(f.shape)?.x1 ?? 0);
    rightSide = f.sides[1];
  }
  const shape = union(k.shape(), ...hoods);
  // Placed like the f: its left side bearing from the f's own side kind and ink.
  const f = raws.get('f') as Raw;
  const dx = sideBearing(s, f.sides[0]) - (shapeBox(f.shape)?.x0 ?? 0);
  return { shape: moved(shape, dx), sides: ['o', 'o'], w: 0, preset: Math.round(right + dx + sideBearing(s, rightSide)) };
}

const F_LIGATURES: [string, string][] = [
  ['ffi', 'f_f_i'],
  ['ffl', 'f_f_l'],
  ['ff', 'f_f'],
  ['fi', 'f_i'],
  ['fl', 'f_l'],
];

// ---- figures ----

const FIGURES = Array.from('0123456789');
const FIGURE_NAMES = ['zero', 'one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine'];
const figName = (fig: string, suffix: string): string => `${FIGURE_NAMES[Number(fig)]}${suffix}`;

/**
 * Desk Serif's old-style figures: the same drawings at other heights, with the text's stroke weights. 0, 1 and 2 stand
 * on the x-height, 6 and 8 rise nearly to the capitals, and 3, 4, 5, 7 and 9 hang below the baseline.
 */
function oldStyle(s: Style, fig: string): Raw {
  const desc = '34579'.includes(fig);
  const h = '012'.includes(fig) ? s.xh + 6 : desc ? Math.round(s.xh * 1.3) : Math.round(s.cap * 0.96);
  const raw = draw({ ...s, cap: h }, LETTERS[fig] as Draw);
  const dy = desc ? s.xh - h : 0;
  return { ...raw, shape: transform(raw.shape, (p) => ({ x: p.x, y: p.y + dy })) };
}

/** Fractions beyond Latin-1's ½ ¼ ¾, reached through `frac`. */
const FRACTIONS: [string, string, string][] = [
  ['1', '3', 'onethird'],
  ['2', '3', 'twothirds'],
  ['1', '8', 'oneeighth'],
  ['3', '8', 'threeeighths'],
  ['5', '8', 'fiveeighths'],
  ['7', '8', 'seveneighths'],
];

// ---- Desk Sans stylistic set 1 ----

const SS01: Record<string, Draw> = {
  // A single-storey a: a bowl and a stem, as d without its ascender.
  a: (k) => {
    const w = k.w(500);
    k.ring(0, w, -k.os, k.xh + k.os);
    k.stem(w - k.V, 0, k.xh + k.os);
    return { w, sides: ['r', 's'] };
  },
  // An l with a tail turning out to the right, so it never reads as I or 1.
  l: (k) => {
    const ry = 120;
    const w = k.V + 110;
    k.stem(0, ry - k.os - 1, k.asc);
    k.arc(0, 2 * w - k.V * 0.2, -k.os, 2 * ry - k.os, 180, 290, k.V, k.H);
    return { w, sides: ['s', 'o'] };
  },
  // A zero with a slash inside its counter, apart from O and o.
  '0': (k) => {
    const d = (LETTERS['0'] as Draw)(k);
    const w = d.w;
    k.diag(w * 0.3, k.cap * 0.16, w * 0.7, k.cap * 0.84, k.V * 0.82);
    return d;
  },
};

// ---- Desk Serif small capitals ----

/** The small capitals' style: capitals a little taller than the x-height, narrower, with the text's weights. */
const smallCapStyle = (s: Style): Style => ({ ...s, cap: Math.round(s.xh * 1.1), wide: s.wide * 0.88 });

/** The small capital of a letter, by its lowercase character. */
const scName = (lower: string): string => (/^[a-z]$/.test(lower) ? `${lower}.sc` : `${nameOf(lower)}.sc`);

/** Letters with small capitals: A to Z and the accented capitals of Latin-1 (with their lowercase). */
function smallCapLetters(): [string, string][] {
  const out: [string, string][] = [];
  for (const up of 'ABCDEFGHIJKLMNOPQRSTUVWXYZ') out.push([up, up.toLowerCase()]);
  for (const [ch] of ACCENTS) if (ch !== ch.toLowerCase() && ch.toLowerCase() !== ch) out.push([ch, ch.toLowerCase()]);
  return out;
}

// ---- Desk Serif discretionary ligatures ----

/**
 * The historical ct and st: the c or s and the t set as usual, joined by a hairline that leaves the top terminal of
 * the round letter, loops up towards the ascender and comes down into the head of the t.
 */
function linkLigature(s: Style, raws: Map<string, Raw>, first: string): Raw {
  const a = raws.get(first) as Raw;
  const t = raws.get('t') as Raw;
  const ba = shapeBox(a.shape) as NonNullable<ReturnType<typeof shapeBox>>;
  const bt = shapeBox(t.shape) as NonNullable<ReturnType<typeof shapeBox>>;
  const dxa = Math.round(sideBearing(s, a.sides[0]) - ba.x0);
  const adv = Math.round(ba.x1 + dxa + sideBearing(s, a.sides[1]));
  const dxt = adv + Math.round(sideBearing(s, t.sides[0]) - bt.x0);
  const k = new Kit(s);
  k.add(moved(a.shape, dxa), moved(t.shape, dxt));
  // The hairline leaves the top of the round letter's bowl, heading right, so it reads as the letter's own stroke.
  const start = { x: dxa + (ba.x0 + ba.x1) / 2, y: s.xh + s.os - k.H / 2 };
  // The t's stem: its top, from the drawing of the t.
  const tx = s.italic ? Math.round(k.w(300) * 0.2) : Math.round(k.w(300) * 0.16);
  const tTop = s.xh + Math.round((s.asc - s.xh) * (s.italic ? 0.55 : 0.62));
  const end = { x: dxt + tx + k.V / 2, y: tTop - 4 };
  const top = s.asc + 10;
  const span = end.x - start.x;
  // An arch: up out of the bowl, over, and down into the t, both ends nearly upright.
  const c1 = { x: start.x + span * 0.12, y: top + 70 };
  const c2 = { x: end.x + span * 0.02, y: top + 70 };
  const curve: P[] = [];
  for (let i = 0; i <= 48; i++) {
    const u = i / 48;
    const v = 1 - u;
    curve.push({
      x: v * v * v * start.x + 3 * v * v * u * c1.x + 3 * v * u * u * c2.x + u * u * u * end.x,
      y: v * v * v * start.y + 3 * v * v * u * c1.y + 3 * v * u * u * c2.y + u * u * u * end.y,
    });
  }
  k.add(...stroke(curve, k.H * 1.05, 'butt'));
  // The t's head grows a little to take the hairline.
  k.add(rect(end.x - k.V / 2, tTop - 30, end.x + k.V / 2, tTop));
  const tAdv = Math.round(bt.x1 - bt.x0 + sideBearing(s, t.sides[0]) + sideBearing(s, t.sides[1]));
  return { shape: k.shape(), sides: ['o', 'o'], w: 0, preset: adv + tAdv };
}

// ---- the glyphs and the layout ----


/** Every unencoded glyph of a face, by name. */
export function extraGlyphs(s: Style, raws: Map<string, Raw>): [string, Extra][] {
  const out: [string, Extra][] = [];
  if (s.kind === 'mono') {
    for (const [name, raw] of codeLigatureGlyphs(s, raws)) out.push([name, { ch: '-', raw }]);
    return out;
  }
  for (const [seq, name] of F_LIGATURES) out.push([name, { ch: 'f', raw: fLigature(s, raws, seq) }]);
  // Proportional figures, and in Desk Serif old-style figures, tabular and proportional.
  for (const fig of FIGURES) {
    const raw = raws.get(fig) as Raw;
    // The tabular 1 is set wide in its cell; the proportional one sits close, like any narrow letter.
    const sides: Raw['sides'] = fig === '1' ? [Math.round(s.sb * 0.6), Math.round(s.sb * 0.6)] : raw.sides;
    out.push([figName(fig, '.pnum'), { ch: fig, raw: { ...raw, sides, fixed: undefined } }]);
    if (s.kind === 'serif') {
      const os = oldStyle(s, fig);
      out.push([figName(fig, '.tosf'), { ch: fig, raw: { ...os, fixed: raw.fixed } }]);
      out.push([figName(fig, '.osf'), { ch: fig, raw: { ...os, sides } }]);
    }
  }
  for (const [num, den, name] of FRACTIONS) {
    const kk = new Kit(s);
    const d = fraction(kk, num, den);
    out.push([name, { ch: '½', raw: { shape: kk.shape(), sides: d.sides, w: d.w } }]);
  }
  if (s.kind === 'sans') {
    const a = draw(s, SS01.a as Draw);
    out.push(['a.ss01', { ch: 'a', raw: a }]);
    for (const [ch, base, mark] of ACCENTS) if (base === 'a') out.push([`${nameOf(ch)}.ss01`, { ch, raw: compose(s, a, mark, false, false) }]);
    out.push(['l.ss01', { ch: 'l', raw: draw(s, SS01.l as Draw) }]);
    const zero = draw(s, SS01['0'] as Draw);
    out.push(['zero.ss01', { ch: '0', raw: zero }]);
    out.push(['zero.pnum.ss01', { ch: '0', raw: { ...zero, fixed: undefined } }]);
  }
  if (s.kind === 'serif') {
    const sc = smallCapStyle(s);
    const bases = new Map<string, Raw>();
    for (const [up, low] of smallCapLetters()) {
      const accent = ACCENTS.find(([ch]) => ch === up);
      let raw: Raw;
      if (accent) {
        const [, base, mark] = accent;
        if (!bases.has(base)) bases.set(base, draw(sc, LETTERS[base] as Draw));
        raw = compose(sc, bases.get(base) as Raw, mark, true, false);
      } else {
        if (!bases.has(up)) bases.set(up, draw(sc, LETTERS[up] as Draw));
        raw = bases.get(up) as Raw;
      }
      out.push([scName(low), { ch: up, raw }]);
    }
    out.push(['c_t', { ch: 'c', raw: linkLigature(s, raws, 'c') }]);
    out.push(['s_t', { ch: 's', raw: linkLigature(s, raws, 's') }]);
  }
  return out;
}

const nameOf = (ch: string): string => glyphName(ch.codePointAt(0) as number);

/** The layout of a face: its GSUB features and its kerning, by glyph name. */
export function layoutOf(s: Style, glyphs: MGlyph[]): Layout {
  const features: Layout['features'] = [];
  if (s.kind === 'mono') {
    features.push({ tag: 'calt', lookups: [codeLigatureLookup()] });
  } else {
    const serif = s.kind === 'serif';
    // Lookups apply in the order they are listed: fractions before the figure styles, which they would hide.
    const frac: SubLookup = {
      type: 'ligature',
      ligs: [
        [['1', '/', '2'].map(nameOf), nameOf('½')],
        [['1', '/', '4'].map(nameOf), nameOf('¼')],
        [['3', '/', '4'].map(nameOf), nameOf('¾')],
        ...FRACTIONS.map(([n, d, name]) => [[n, '/', d].map(nameOf), name] as [string[], string]),
      ],
    };
    features.push({ tag: 'frac', lookups: [frac] });
    const map = (from: string, to: string): [string, string][] => FIGURES.map((f) => [from ? figName(f, from) : nameOf(f), to ? figName(f, to) : nameOf(f)]);
    features.push({ tag: 'pnum', lookups: [{ type: 'single', map: [...map('', '.pnum'), ...(serif ? map('.tosf', '.osf') : [])] }] });
    features.push({ tag: 'tnum', lookups: [{ type: 'single', map: [...map('.pnum', ''), ...(serif ? map('.osf', '.tosf') : [])] }] });
    if (serif) {
      features.push({ tag: 'onum', lookups: [{ type: 'single', map: [...map('', '.tosf'), ...map('.pnum', '.osf')] }] });
      features.push({ tag: 'lnum', lookups: [{ type: 'single', map: [...map('.tosf', ''), ...map('.osf', '.pnum')] }] });
      const letters = smallCapLetters();
      features.push({ tag: 'smcp', lookups: [{ type: 'single', map: letters.map(([, low]) => [nameOf(low), scName(low)]) }] });
      features.push({ tag: 'c2sc', lookups: [{ type: 'single', map: letters.map(([up, low]) => [nameOf(up), scName(low)]) }] });
    } else {
      const ss01: [string, string][] = [
        [nameOf('a'), 'a.ss01'],
        ...ACCENTS.filter(([, base]) => base === 'a').map(([ch]) => [nameOf(ch), `${nameOf(ch)}.ss01`] as [string, string]),
        [nameOf('l'), 'l.ss01'],
        [nameOf('0'), 'zero.ss01'],
        ['zero.pnum', 'zero.pnum.ss01'],
      ];
      features.push({ tag: 'ss01', lookups: [{ type: 'single', map: ss01 }] });
    }
    const liga: SubLookup = { type: 'ligature', ligs: F_LIGATURES.map(([seq, name]) => [Array.from(seq, nameOf), name]) };
    features.push({ tag: 'liga', lookups: [liga] });
    if (serif) features.push({ tag: 'dlig', lookups: [{ type: 'ligature', ligs: [[['c', 't'].map(nameOf), 'c_t'], [['s', 't'].map(nameOf), 's_t']] }] });
  }
  const kerning = kerningPairs(s, glyphs).map(([a, b, v]) => [nameOf(a), nameOf(b), v] as [string, string, number]);
  return { features, kerning };
}
