// A small reader for the layout tables the outline faces carry, for the tests: it applies GSUB single, ligature and
// chaining context (format 3) substitutions by feature, reads GPOS pair kerning (format 1) and the old `kern` table,
// and names glyphs from the font's own names. It reads only what the build writes; HarfBuzz checks the files in
// `validate.sh`, and this keeps the same checks runnable where there is no Docker.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as ot from 'opentype.js';
import { tables } from './sfnt';

type Lookup = { type: number; subs: number[] };

export class Shaper {
  private readonly dv: DataView;
  private readonly gsubAt: number;
  private readonly lookups: Lookup[] = [];
  private readonly features: { tag: string; lookups: number[] }[] = [];
  readonly names: string[] = [];
  private readonly ids = new Map<string, number>();
  private readonly cmap: ot.Font;

  constructor(readonly file: Uint8Array) {
    const t = tables(file);
    this.dv = t.dv;
    this.cmap = ot.parse(file.slice().buffer);
    for (let i = 0; i < this.cmap.glyphs.length; i++) {
      const n = this.cmap.glyphs.get(i).name as string;
      this.names.push(n);
      this.ids.set(n, i);
    }
    const g = t.dir.find((d) => d.tag === 'GSUB');
    this.gsubAt = g ? g.offset : -1;
    if (g) this.readGsub();
  }

  has(tag: string): boolean {
    return tables(this.file).dir.some((d) => d.tag === tag);
  }

  gid(name: string): number {
    const g = this.ids.get(name);
    if (g === undefined) throw new Error(`no glyph ${name}`);
    return g;
  }

  private u16 = (at: number): number => this.dv.getUint16(at);

  private coverage(at: number): Map<number, number> {
    const m = new Map<number, number>();
    const format = this.u16(at);
    if (format !== 1) throw new Error(`coverage format ${format}`);
    const n = this.u16(at + 2);
    for (let i = 0; i < n; i++) m.set(this.u16(at + 4 + 2 * i), i);
    return m;
  }

  private readGsub(): void {
    const base = this.gsubAt;
    const fl = base + this.u16(base + 6);
    const ll = base + this.u16(base + 8);
    for (let i = 0; i < this.u16(fl); i++) {
      const rec = fl + 2 + 6 * i;
      const tag = String.fromCharCode(...this.file.subarray(rec, rec + 4));
      const f = fl + this.u16(rec + 4);
      const lookups = Array.from({ length: this.u16(f + 2) }, (_, k) => this.u16(f + 4 + 2 * k));
      this.features.push({ tag, lookups });
    }
    for (let i = 0; i < this.u16(ll); i++) {
      const l = ll + this.u16(ll + 2 + 2 * i);
      const subs = Array.from({ length: this.u16(l + 4) }, (_, k) => l + this.u16(l + 6 + 2 * k));
      this.lookups.push({ type: this.u16(l), subs });
    }
  }

  /** The feature tags in GSUB. */
  get featureTags(): string[] {
    return this.features.map((f) => f.tag);
  }

  /** Applies one lookup to a glyph run at position `i`; returns how many glyphs it consumed, or 0 if none matched. */
  private applyAt(lookup: Lookup, run: number[], i: number): number {
    for (const at of lookup.subs) {
      if (lookup.type === 1) {
        const cov = this.coverage(at + this.u16(at + 2));
        const k = cov.get(run[i] as number);
        if (k === undefined) continue;
        if (this.u16(at) !== 2) throw new Error('single substitution format');
        run[i] = this.u16(at + 6 + 2 * k);
        return 1;
      }
      if (lookup.type === 4) {
        const cov = this.coverage(at + this.u16(at + 2));
        const k = cov.get(run[i] as number);
        if (k === undefined) continue;
        const set = at + this.u16(at + 6 + 2 * k);
        for (let j = 0; j < this.u16(set); j++) {
          const lig = set + this.u16(set + 2 + 2 * j);
          const n = this.u16(lig + 2);
          let ok = i + n <= run.length;
          for (let c = 1; ok && c < n; c++) ok = run[i + c] === this.u16(lig + 4 + 2 * (c - 1));
          if (!ok) continue;
          run.splice(i, n, this.u16(lig));
          return 1;
        }
        continue;
      }
      if (lookup.type === 6) {
        if (this.u16(at) !== 3) throw new Error('chaining context format');
        let p = at + 2;
        const read = (): number[] => {
          const n = this.u16(p);
          const offs = Array.from({ length: n }, (_, k) => at + this.u16(p + 2 + 2 * k));
          p += 2 + 2 * n;
          return offs;
        };
        const back = read();
        const input = read();
        const ahead = read();
        const covers = (off: number, g: number | undefined): boolean => g !== undefined && this.coverage(off).has(g);
        if (back.some((off, k) => !covers(off, run[i - 1 - k]))) continue;
        if (input.some((off, k) => !covers(off, run[i + k]))) continue;
        if (ahead.some((off, k) => !covers(off, run[i + input.length + k]))) continue;
        const n = this.u16(p);
        for (let r = 0; r < n; r++) {
          const seq = this.u16(p + 2 + 4 * r);
          const li = this.u16(p + 4 + 4 * r);
          this.applyAt(this.lookups[li] as Lookup, run, i + seq);
        }
        return input.length;
      }
      throw new Error(`lookup type ${lookup.type}`);
    }
    return 0;
  }

  /** Shapes a text: cmap, then every lookup of the features asked for, in lookup order. Returns glyph names. */
  shape(text: string, on: string[]): string[] {
    const run = Array.from(text, (c) => this.cmap.charToGlyph(c).index);
    const wanted = new Set(this.features.filter((f) => on.includes(f.tag)).flatMap((f) => f.lookups));
    for (let li = 0; li < this.lookups.length; li++) {
      if (!wanted.has(li)) continue;
      for (let i = 0; i < run.length; ) i += Math.max(1, this.applyAt(this.lookups[li] as Lookup, run, i));
    }
    return run.map((g) => this.names[g] as string);
  }

  /** The GPOS `kern` pairs, by glyph names: PairPos format 1 with value format 4 (x advance). */
  gposPairs(): Map<string, number> {
    const out = new Map<string, number>();
    const g = tables(this.file).dir.find((d) => d.tag === 'GPOS');
    if (!g) return out;
    const base = g.offset;
    const ll = base + this.u16(base + 8);
    for (let i = 0; i < this.u16(ll); i++) {
      const l = ll + this.u16(ll + 2 + 2 * i);
      if (this.u16(l) !== 2) throw new Error('GPOS lookup type');
      for (let s = 0; s < this.u16(l + 4); s++) {
        const at = l + this.u16(l + 6 + 2 * s);
        if (this.u16(at) !== 1 || this.u16(at + 4) !== 4 || this.u16(at + 6) !== 0) throw new Error('PairPos format');
        const cov = this.coverage(at + this.u16(at + 2));
        for (const [first, k] of cov) {
          const set = at + this.u16(at + 10 + 2 * k);
          for (let j = 0; j < this.u16(set); j++) {
            const second = this.u16(set + 2 + 4 * j);
            out.set(`${this.names[first]} ${this.names[second]}`, this.dv.getInt16(set + 4 + 4 * j));
          }
        }
      }
    }
    return out;
  }

  /** The `kern` table: its header fields and pairs, by glyph names. */
  kernTable(): { version: number; tables: number; format: number; coverage: number; header: number[]; pairs: Map<string, number>; sorted: boolean } | null {
    const k = tables(this.file).dir.find((d) => d.tag === 'kern');
    if (!k) return null;
    const at = k.offset;
    const sub = at + 4;
    const n = this.u16(sub + 6);
    const pairs = new Map<string, number>();
    let sorted = true;
    let prev = -1;
    for (let i = 0; i < n; i++) {
      const rec = sub + 14 + 6 * i;
      const key = this.u16(rec) * 65536 + this.u16(rec + 2);
      if (key <= prev) sorted = false;
      prev = key;
      pairs.set(`${this.names[this.u16(rec)]} ${this.names[this.u16(rec + 2)]}`, this.dv.getInt16(rec + 4));
    }
    return {
      version: this.u16(at),
      tables: this.u16(at + 2),
      format: this.u16(sub + 4) >> 8,
      coverage: this.u16(sub + 4),
      header: [n, this.u16(sub + 8), this.u16(sub + 10), this.u16(sub + 12)],
      pairs,
      sorted,
    };
  }
}
