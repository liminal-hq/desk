// Helpers for the font tests: reading an sfnt's table directory and cmap, and the exact area of a parsed path.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type * as ot from 'opentype.js';

export interface Table {
  tag: string;
  sum: number;
  offset: number;
  length: number;
}

export function tables(f: Uint8Array): { dv: DataView; dir: Table[]; get: (t: string) => Table } {
  const dv = new DataView(f.buffer, f.byteOffset, f.byteLength);
  const n = dv.getUint16(4);
  const dir: Table[] = [];
  for (let i = 0; i < n; i++) {
    const at = 12 + 16 * i;
    dir.push({
      tag: String.fromCharCode(...f.subarray(at, at + 4)),
      sum: dv.getUint32(at + 4),
      offset: dv.getUint32(at + 8),
      length: dv.getUint32(at + 12),
    });
  }
  const get = (t: string): Table => {
    const x = dir.find((d) => d.tag === t);
    if (!x) throw new Error(`no ${t}`);
    return x;
  };
  return { dv, dir, get };
}

/** Reads a cmap subtable of the given platform and encoding: its format and a lookup function. */
export function cmapLookup(f: Uint8Array, platform: number, encoding: number): { format: number; glyph: (code: number) => number } {
  const { dv, get } = tables(f);
  const c = get('cmap').offset;
  const n = dv.getUint16(c + 2);
  for (let i = 0; i < n; i++) {
    const rec = c + 4 + 8 * i;
    if (dv.getUint16(rec) !== platform || dv.getUint16(rec + 2) !== encoding) continue;
    const s = c + dv.getUint32(rec + 4);
    const format = dv.getUint16(s);
    if (format === 6) {
      const first = dv.getUint16(s + 6);
      const count = dv.getUint16(s + 8);
      return { format, glyph: (code) => (code >= first && code < first + count ? dv.getUint16(s + 10 + 2 * (code - first)) : 0) };
    }
    if (format === 4) {
      const segs = dv.getUint16(s + 6) / 2;
      return {
        format,
        glyph: (code) => {
          for (let k = 0; k < segs; k++) {
            const end = dv.getUint16(s + 14 + 2 * k);
            if (code > end) continue;
            const start = dv.getUint16(s + 16 + 2 * segs + 2 * k);
            if (code < start) return 0;
            const delta = dv.getUint16(s + 16 + 4 * segs + 2 * k);
            const range = dv.getUint16(s + 16 + 6 * segs + 2 * k);
            if (range !== 0) throw new Error('idRangeOffset not expected');
            return (code + delta) & 0xffff;
          }
          return 0;
        },
      };
    }
    throw new Error(`unexpected cmap format ${format}`);
  }
  throw new Error(`no cmap subtable ${platform}/${encoding}`);
}

/** Area enclosed by a parsed path with lines and quadratic or cubic curves, positive counter-clockwise. */
export function pathArea(path: ot.Path): number {
  type P = { x: number; y: number };
  const cr = (a: P, b: P): number => a.x * b.y - a.y * b.x;
  let twice = 0;
  let start: P = { x: 0, y: 0 };
  let cur: P = { x: 0, y: 0 };
  for (const c of path.commands as { type: string; x: number; y: number; x1: number; y1: number; x2: number; y2: number }[]) {
    if (c.type === 'M') start = cur = { x: c.x, y: c.y };
    else if (c.type === 'L') {
      twice += cr(cur, c);
      cur = { x: c.x, y: c.y };
    } else if (c.type === 'Q') {
      const k = { x: c.x1, y: c.y1 };
      twice += (2 * cr(cur, k) + 2 * cr(k, c) + cr(cur, c)) / 3;
      cur = { x: c.x, y: c.y };
    } else if (c.type === 'C') {
      // Exact signed area of a cubic segment.
      const p0 = cur;
      const p1 = { x: c.x1, y: c.y1 };
      const p2 = { x: c.x2, y: c.y2 };
      const p3 = { x: c.x, y: c.y };
      twice +=
        (6 * cr(p0, p1) + 3 * cr(p0, p2) + cr(p0, p3) + 3 * cr(p1, p2) + 3 * cr(p1, p3) + 6 * cr(p2, p3)) / 10;
      cur = p3;
    } else if (c.type === 'Z') {
      twice += cr(cur, start);
      cur = start;
    }
  }
  return twice / 2;
}

