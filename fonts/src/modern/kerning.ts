// Pair kerning for Desk Sans and Desk Serif: a small table of letter classes, with each class pair's value measured
// from the outlines (how much open space the pair leaves next to a pair of straight stems) and every member pair
// checked so that no kerned pair comes closer than a floor anywhere along its height.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MGlyph } from './assemble';
import { type P, sampleContour } from './geom';
import type { Style } from './kit';

/**
 * Left classes (their right side faces the pair) and right classes; the first character is the key glyph. Accented
 * letters are kept to the right side, where French and German set them after T, V, W and Y.
 */
const LEFT: Record<string, string> = {
  T: 'T',
  V: 'V',
  W: 'W',
  Y: 'Y',
  A: 'A',
  L: 'L',
  P: 'P',
  F: 'F',
  O: 'ODQ',
  r: 'r',
  f: 'f',
  v: 'v',
  w: 'w',
  y: 'y',
  o: 'obpþ',
  lq: '‘“',
  rq: '’”\'"',
  dot: '.,',
};

const RIGHT: Record<string, string> = {
  o: 'acdegoqàâäçèéêëôöœ',
  n: 'mnpruü',
  s: 's',
  v: 'vwyý',
  A: 'AÀÂÄ',
  O: 'CGOQÇÖ',
  T: 'T',
  V: 'V',
  W: 'W',
  Y: 'YÝ',
  dot: '.,…',
  rq: '’”\'"',
};

/** Which right classes each left class is kerned against. */
const PAIRS: Record<string, string[]> = {
  T: ['o', 'n', 's', 'v', 'A', 'dot'],
  V: ['o', 'n', 'A', 'O', 'dot'],
  W: ['o', 'n', 'A', 'dot'],
  Y: ['o', 'n', 's', 'A', 'O', 'dot'],
  A: ['T', 'V', 'W', 'Y', 'v', 'O', 'rq'],
  L: ['T', 'V', 'W', 'Y', 'v', 'rq'],
  P: ['A', 'o', 'dot'],
  F: ['A', 'o', 'dot'],
  O: ['A', 'T', 'V', 'W', 'Y', 'dot'],
  r: ['o', 'dot'],
  f: ['o', 'dot', 'rq'],
  v: ['o', 'dot'],
  w: ['o', 'dot'],
  y: ['o', 'dot'],
  o: ['v', 'T', 'V', 'W', 'Y'],
  lq: ['A'],
  rq: ['A'],
  dot: ['rq', 'T', 'V', 'W', 'Y'],
};

/** Band height of the profiles, in units. */
const BAND = 10;
const Y0 = -260;
const BANDS = Math.ceil((960 - Y0) / BAND);

/** A glyph's ink extent in each horizontal band: leftmost and rightmost ink, NaN where the band is empty. */
interface Profile {
  l: number[];
  r: number[];
  adv: number;
  y0: number;
  y1: number;
}

export function profile(g: MGlyph): Profile {
  const l = new Array<number>(BANDS).fill(Number.NaN);
  const r = new Array<number>(BANDS).fill(Number.NaN);
  for (const c of g.contours) {
    const pts = sampleContour(c, 16);
    for (let i = 0; i < pts.length; i++) {
      const a = pts[i] as P;
      const b = pts[(i + 1) % pts.length] as P;
      const lo = Math.min(a.y, b.y);
      const hi = Math.max(a.y, b.y);
      // Every band the edge passes through, sampled at the band's top, middle and bottom.
      for (let k = Math.max(0, Math.floor((lo - Y0) / BAND)); k <= Math.min(BANDS - 1, Math.floor((hi - Y0) / BAND)); k++) {
        for (const f of [0, 0.5, 1]) {
          const y = Math.min(hi, Math.max(lo, Y0 + (k + f) * BAND));
          const x = a.y === b.y ? Math.min(a.x, b.x) : a.x + ((y - a.y) / (b.y - a.y)) * (b.x - a.x);
          const x2 = a.y === b.y ? Math.max(a.x, b.x) : x;
          if (!(l[k] as number <= x)) l[k] = x;
          if (!(r[k] as number >= x2)) r[k] = x2;
        }
      }
    }
  }
  return { l, r, adv: g.adv, y0: g.box?.y0 ?? 0, y1: g.box?.y1 ?? 0 };
}

/** How the values are measured, per face. */
const TUNE = {
  /** Open space deeper than this does not count (units). */
  depth: 400,
  /** The share of the extra open space taken out. */
  strength: 0.45,
  /** Bands of vertical reach when checking that a kerned pair does not come too close. */
  reach: 2,
  /** The largest tightening, in units. */
  max: 160,
};

/** The gaps between a pair at each band, with the ink of each side spread vertically by `reach` bands. */
export function gaps(a: Profile, b: Profile, reach: number): number[] {
  const out: number[] = [];
  for (let k = 0; k < BANDS; k++) {
    let ra = Number.NaN;
    let lb = Number.NaN;
    for (let j = Math.max(0, k - reach); j <= Math.min(BANDS - 1, k + reach); j++) {
      const x = a.r[j] as number;
      if (!Number.isNaN(x) && !(ra >= x)) ra = x;
      const y = b.l[j] as number;
      if (!Number.isNaN(y) && !(lb <= y)) lb = y;
    }
    out.push(Number.isNaN(ra) || Number.isNaN(lb) ? Number.NaN : a.adv - ra + lb);
  }
  return out;
}

/** The kerning a pair needs to match the open space of two straight stems, before the floor is applied. */
function measure(a: Profile, b: Profile, s: Style): number {
  const g = gaps(a, b, 0);
  const lo = Math.max(a.y0, b.y0, Y0);
  const hi = Math.min(a.y1, b.y1);
  const ref = 2 * s.sb;
  let sum = 0;
  let n = 0;
  for (let k = 0; k < BANDS; k++) {
    const y = Y0 + (k + 0.5) * BAND;
    if (y < lo || y > hi) continue;
    const v = g[k] as number;
    sum += Number.isNaN(v) ? TUNE.depth : Math.min(v, TUNE.depth);
    n++;
  }
  if (!n) return 0;
  return (ref - sum / n) * TUNE.strength;
}

/** The smallest gap of a pair anywhere along its height, with vertical reach. */
function closest(a: Profile, b: Profile): number {
  const g = gaps(a, b, TUNE.reach).filter((v) => !Number.isNaN(v));
  return g.length ? Math.min(...g) : Number.POSITIVE_INFINITY;
}

/** The closest a kerned pair may come: a little under half the straight side bearing. */
const floorOf = (s: Style): number => Math.round(s.sb * 0.45);

const round5 = (v: number): number => Math.round(v / 5) * 5;

/**
 * The kerning pairs of a face, by character: each class pair's value comes from its key glyphs, and each member
 * pair takes that value unless it would bring the pair closer than the floor, when it takes less (or, for a pair
 * that already collides, such as f before a quote, a positive value). Mono has none.
 */
export function kerningPairs(s: Style, glyphs: MGlyph[]): [string, string, number][] {
  if (s.kind === 'mono') return [];
  const byCh = new Map<string, MGlyph>();
  for (const g of glyphs) if (g.cp >= 0 && !byCh.has(g.ch)) byCh.set(g.ch, g);
  const profiles = new Map<string, Profile>();
  const prof = (ch: string): Profile | null => {
    const g = byCh.get(ch);
    if (!g || !g.box) return null;
    if (!profiles.has(ch)) profiles.set(ch, profile(g));
    return profiles.get(ch) as Profile;
  };
  const floor = floorOf(s);
  const out: [string, string, number][] = [];
  for (const [lc, rcs] of Object.entries(PAIRS)) {
    const left = LEFT[lc] as string;
    for (const rc of rcs) {
      const right = RIGHT[rc] as string;
      const ka = prof(left[0] as string);
      const kb = prof(right[0] as string);
      if (!ka || !kb) continue;
      const base = Math.max(-TUNE.max, Math.min(0, measure(ka, kb, s)));
      for (const a of left) {
        for (const b of right) {
          const pa = prof(a);
          const pb = prof(b);
          if (!pa || !pb) continue;
          // Never closer than the floor: less tightening where a mark or an arm would come too near.
          const v = round5(Math.max(base, floor - closest(pa, pb)));
          if (Math.abs(v) >= 10) out.push([a, b, v]);
        }
      }
    }
  }
  return out;
}
