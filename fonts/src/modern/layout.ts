// Writes the layout tables of the outline faces from glyph names: GSUB (single, ligature and chaining context
// substitutions under their features), GPOS pair kerning, and the old `kern` table that Windows 3.1 reads.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/**
 * A substitution lookup: single (one glyph for another), ligature (a sequence for one glyph), or chaining
 * context (rules tried in order, each matching glyph sets before, in and after the input and applying other
 * lookups to positions of the input; a rule with no lookups matches and does nothing, which is how a context is
 * excluded).
 */
export type SubLookup =
  | { type: 'single'; map: [string, string][] }
  | { type: 'ligature'; ligs: [string[], string][] }
  | { type: 'chain'; rules: ChainRule[] };

/** One chaining context rule: glyph sets for the backtrack (nearest first), the input and the lookahead. */
export interface ChainRule {
  back: string[][];
  input: string[][];
  ahead: string[][];
  subs: { at: number; lookup: SubLookup }[];
}

/** Everything the layout tables say, by glyph name. */
export interface Layout {
  /** GSUB features, each a tag and its lookups, applied in the order given. */
  features: { tag: string; lookups: SubLookup[] }[];
  /** Kerning pairs: left, right, adjustment in font units (negative tightens). */
  kerning: [string, string, number][];
}

class W {
  b: number[] = [];
  u16(v: number): this {
    this.b.push((v >> 8) & 0xff, v & 0xff);
    return this;
  }
  i16(v: number): this {
    return this.u16(v < 0 ? v + 0x10000 : v);
  }
  u32(v: number): this {
    return this.u16((v >>> 16) & 0xffff).u16(v & 0xffff);
  }
  tag(t: string): this {
    for (const c of t.padEnd(4)) this.b.push(c.charCodeAt(0));
    return this;
  }
  raw(d: number[] | Uint8Array): this {
    for (const x of d) this.b.push(x);
    return this;
  }
  get length(): number {
    return this.b.length;
  }
  set16(at: number, v: number): void {
    this.b[at] = (v >> 8) & 0xff;
    this.b[at + 1] = v & 0xff;
  }
  done(): Uint8Array {
    return Uint8Array.from(this.b);
  }
}

/** A coverage table, format 1, for sorted unique glyph ids. */
function coverage(gids: number[]): number[] {
  const w = new W().u16(1).u16(gids.length);
  for (const g of gids) w.u16(g);
  return w.b;
}

/** Serialises child tables after a header, filling the header's 16-bit offsets at the given positions. */
function withChildren(header: W, slots: number[], children: number[][]): number[] {
  const out = [...header.b];
  let at = out.length;
  slots.forEach((slot, i) => {
    out[slot] = (at >> 8) & 0xff;
    out[slot + 1] = at & 0xff;
    out.push(...(children[i] as number[]));
    at += (children[i] as number[]).length;
  });
  return out;
}

function singleSubtable(map: [number, number][]): number[] {
  const sorted = [...map].sort((a, b) => a[0] - b[0]);
  const h = new W().u16(2).u16(0).u16(sorted.length);
  for (const [, to] of sorted) h.u16(to);
  return withChildren(h, [2], [coverage(sorted.map(([f]) => f))]);
}

function ligatureSubtable(ligs: [number[], number][]): number[] {
  const byFirst = new Map<number, [number[], number][]>();
  for (const l of ligs) {
    const f = l[0][0] as number;
    if (!byFirst.has(f)) byFirst.set(f, []);
    (byFirst.get(f) as [number[], number][]).push(l);
  }
  const firsts = [...byFirst.keys()].sort((a, b) => a - b);
  const h = new W().u16(1).u16(0).u16(firsts.length);
  const slots = [2];
  for (let i = 0; i < firsts.length; i++) {
    slots.push(h.length);
    h.u16(0);
  }
  const sets = firsts.map((f) => {
    // Longer ligatures first, so the longest match wins.
    const list = [...(byFirst.get(f) as [number[], number][])].sort((a, b) => b[0].length - a[0].length);
    const sh = new W().u16(list.length);
    const ss: number[] = [];
    for (let i = 0; i < list.length; i++) {
      ss.push(sh.length);
      sh.u16(0);
    }
    const ligTables = list.map(([seq, to]) => {
      const lw = new W().u16(to).u16(seq.length);
      for (const g of seq.slice(1)) lw.u16(g);
      return lw.b;
    });
    return withChildren(sh, ss, ligTables);
  });
  return withChildren(h, slots, [coverage(firsts), ...sets]);
}

/** A ScriptList with DFLT and latn, both listing every feature in their default language system. */
function scriptList(featureCount: number): number[] {
  const langSys = new W().u16(0).u16(0xffff).u16(featureCount);
  for (let i = 0; i < featureCount; i++) langSys.u16(i);
  const script = withChildren(new W().u16(0).u16(0), [0], [langSys.b]);
  const h = new W().u16(2).tag('DFLT').u16(0).tag('latn').u16(0);
  return withChildren(h, [6, 12], [script, script]);
}

function featureList(features: { tag: string; lookups: number[] }[]): number[] {
  const h = new W().u16(features.length);
  const slots: number[] = [];
  for (const f of features) {
    h.tag(f.tag);
    slots.push(h.length);
    h.u16(0);
  }
  const tables = features.map((f) => {
    const w = new W().u16(0).u16(f.lookups.length);
    for (const l of f.lookups) w.u16(l);
    return w.b;
  });
  return withChildren(h, slots, tables);
}

function lookupList(lookups: { type: number; subs: number[][] }[]): number[] {
  const h = new W().u16(lookups.length);
  const slots: number[] = [];
  for (let i = 0; i < lookups.length; i++) {
    slots.push(h.length);
    h.u16(0);
  }
  const tables = lookups.map((l) => {
    const t = new W().u16(l.type).u16(0).u16(l.subs.length);
    const subSlots = l.subs.map(() => {
      const at = t.length;
      t.u16(0);
      return at;
    });
    return withChildren(t, subSlots, l.subs);
  });
  return withChildren(h, slots, tables);
}

/** A chaining context subtable, format 3: one coverage per position and the lookups to apply. */
function chainSubtable(back: number[][], input: number[][], ahead: number[][], subs: [number, number][]): number[] {
  const h = new W().u16(3);
  const slots: number[] = [];
  const covs: number[][] = [];
  for (const group of [back, input, ahead]) {
    h.u16(group.length);
    for (const g of group) {
      slots.push(h.length);
      h.u16(0);
      covs.push(coverage([...new Set(g)].sort((a, b) => a - b)));
    }
  }
  h.u16(subs.length);
  for (const [at, lookup] of subs) h.u16(at).u16(lookup);
  return withChildren(h, slots, covs);
}

function header(scripts: number[], features: number[], lookups: number[]): Uint8Array {
  const h = new W().u32(0x00010000).u16(0).u16(0).u16(0);
  return Uint8Array.from(withChildren(h, [4, 6, 8], [scripts, features, lookups]));
}

/** The GSUB table, or null when there are no substitutions. */
export function gsub(layout: Layout, gid: (name: string) => number | undefined): Uint8Array | null {
  const lookups: { type: number; subs: number[][] }[] = [];
  const index = new Map<SubLookup, number>();
  const ids = (names: string[]): number[] => names.map(gid).filter((g) => g !== undefined) as number[];
  /** Adds a lookup (once, however many rules refer to it) and returns its index, or -1 if it would be empty. */
  const add = (l: SubLookup): number => {
    const known = index.get(l);
    if (known !== undefined) return known;
    let entry: { type: number; subs: number[][] } | null = null;
    if (l.type === 'single') {
      const map = l.map.map(([a, b]) => [gid(a), gid(b)]).filter((p) => p[0] !== undefined && p[1] !== undefined) as [number, number][];
      if (map.length) entry = { type: 1, subs: [singleSubtable(map)] };
    } else if (l.type === 'ligature') {
      const ligs = l.ligs
        .map(([seq, to]) => [seq.map(gid), gid(to)])
        .filter(([seq, to]) => to !== undefined && (seq as (number | undefined)[]).every((g) => g !== undefined)) as [number[], number][];
      if (ligs.length) entry = { type: 4, subs: [ligatureSubtable(ligs)] };
    } else {
      // The chain's own index comes first, so the lookups it applies follow it in the list.
      const at = lookups.push({ type: 6, subs: [] }) - 1;
      index.set(l, at);
      const subs: number[][] = [];
      for (const r of l.rules) {
        const sets = [r.back, r.input, r.ahead].map((group) => group.map(ids));
        if (sets.some((group) => group.some((g) => g.length === 0))) continue;
        const recs = r.subs.map((x) => [x.at, add(x.lookup)] as [number, number]).filter((x) => x[1] >= 0);
        subs.push(chainSubtable(sets[0] as number[][], sets[1] as number[][], sets[2] as number[][], recs));
      }
      (lookups[at] as { subs: number[][] }).subs = subs;
      return subs.length ? at : -1;
    }
    if (!entry) return -1;
    const at = lookups.push(entry) - 1;
    index.set(l, at);
    return at;
  };
  const feats: { tag: string; lookups: number[] }[] = [];
  for (const f of layout.features) {
    const idx = f.lookups.map(add).filter((i) => i >= 0);
    if (idx.length) feats.push({ tag: f.tag, lookups: [...new Set(idx)].sort((a, b) => a - b) });
  }
  if (!feats.length) return null;
  // Features must be listed in tag order.
  const sorted = [...feats].sort((a, b) => (a.tag < b.tag ? -1 : a.tag > b.tag ? 1 : 0));
  return header(scriptList(sorted.length), featureList(sorted), lookupList(lookups));
}

function resolvePairs(layout: Layout, gid: (name: string) => number | undefined): [number, number, number][] {
  const seen = new Set<string>();
  const out: [number, number, number][] = [];
  for (const [a, b, v] of layout.kerning) {
    const l = gid(a);
    const r = gid(b);
    if (l === undefined || r === undefined || v === 0) continue;
    const key = `${l},${r}`;
    if (seen.has(key)) continue;
    seen.add(key);
    out.push([l, r, v]);
  }
  return out.sort((p, q) => p[0] - q[0] || p[1] - q[1]);
}

/** The GPOS table with pair kerning under `kern`, or null without pairs. */
export function gpos(layout: Layout, gid: (name: string) => number | undefined): Uint8Array | null {
  const pairs = resolvePairs(layout, gid);
  if (!pairs.length) return null;
  const firsts = [...new Set(pairs.map((p) => p[0]))];
  // PairPos format 1, with the first glyph's x advance adjusted (value format 4).
  const h = new W().u16(1).u16(0).u16(4).u16(0).u16(firsts.length);
  const slots = [2];
  for (let i = 0; i < firsts.length; i++) {
    slots.push(h.length);
    h.u16(0);
  }
  const sets = firsts.map((f) => {
    const mine = pairs.filter((p) => p[0] === f);
    const w = new W().u16(mine.length);
    for (const [, r, v] of mine) w.u16(r).i16(v);
    return w.b;
  });
  const sub = withChildren(h, slots, [coverage(firsts), ...sets]);
  const feats = featureList([{ tag: 'kern', lookups: [0] }]);
  return header(scriptList(1), feats, lookupList([{ type: 2, subs: [sub] }]));
}

/** The `kern` table, version 0 with one format 0 subtable, as Windows 3.1 reads it. */
export function kernTable(layout: Layout, gid: (name: string) => number | undefined): Uint8Array | null {
  const pairs = resolvePairs(layout, gid);
  if (!pairs.length) return null;
  const n = pairs.length;
  let pow = 1;
  while (pow * 2 <= n) pow *= 2;
  const w = new W().u16(0).u16(1);
  w.u16(0).u16(14 + 6 * n).u16(0x0001);
  w.u16(n).u16(pow * 6).u16(Math.log2(pow)).u16(n * 6 - pow * 6);
  for (const [l, r, v] of pairs) w.u16(l).u16(r).i16(v);
  return w.done();
}
