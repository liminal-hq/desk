// sfnt helpers for the emoji: a cmap with format 4 and format 12 subtables for code points beyond the BMP, and
// replacing tables in a finished font file with the checksums redone.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { checksum } from '../ttf';

/** A growable big-endian byte buffer. */
export class Bytes {
  private b: number[] = [];
  u8(v: number): this {
    this.b.push(v & 0xff);
    return this;
  }
  u16(v: number): this {
    return this.u8(v >> 8).u8(v);
  }
  i16(v: number): this {
    return this.u16(v < 0 ? v + 0x10000 : v);
  }
  u32(v: number): this {
    return this.u16((v >>> 16) & 0xffff).u16(v & 0xffff);
  }
  raw(d: ArrayLike<number>): this {
    for (let i = 0; i < d.length; i++) this.b.push(d[i] as number);
    return this;
  }
  pad(n: number): this {
    while (this.b.length % n) this.b.push(0);
    return this;
  }
  get length(): number {
    return this.b.length;
  }
  done(): Uint8Array {
    return Uint8Array.from(this.b);
  }
}

/** A format 4 subtable for the BMP code points of `map` (code point to glyph id). */
function format4(map: Map<number, number>): Uint8Array {
  const cps = [...map.keys()].filter((c) => c <= 0xfffe).sort((a, b) => a - b);
  const segs: { start: number; end: number; delta: number }[] = [];
  for (const cp of cps) {
    const gid = map.get(cp) as number;
    const last = segs[segs.length - 1];
    if (last && cp === last.end + 1 && gid - cp === last.delta) last.end = cp;
    else segs.push({ start: cp, end: cp, delta: gid - cp });
  }
  segs.push({ start: 0xffff, end: 0xffff, delta: 1 });
  const n = segs.length;
  let pow = 1;
  while (pow * 2 <= n) pow *= 2;
  const b = new Bytes().u16(4).u16(16 + 8 * n).u16(0).u16(n * 2).u16(pow * 2).u16(Math.log2(pow)).u16(2 * n - 2 * pow);
  for (const s of segs) b.u16(s.end);
  b.u16(0);
  for (const s of segs) b.u16(s.start);
  for (const s of segs) b.u16((s.delta + 0x10000) % 0x10000);
  for (let i = 0; i < n; i++) b.u16(0);
  return b.done();
}

/** A format 12 subtable for every code point of `map`, in runs of consecutive code points and glyph ids. */
function format12(map: Map<number, number>): Uint8Array {
  const cps = [...map.keys()].sort((a, b) => a - b);
  const groups: { start: number; end: number; gid: number }[] = [];
  for (const cp of cps) {
    const gid = map.get(cp) as number;
    const last = groups[groups.length - 1];
    if (last && cp === last.end + 1 && gid === last.gid + (cp - last.start)) last.end = cp;
    else groups.push({ start: cp, end: cp, gid });
  }
  const b = new Bytes().u16(12).u16(0).u32(16 + 12 * groups.length).u32(0).u32(groups.length);
  for (const g of groups) b.u32(g.start).u32(g.end).u32(g.gid);
  return b.done();
}

/**
 * A Unicode cmap: format 4 for the BMP on Unicode BMP (0, 3) and Windows Unicode BMP (3, 1), and, when
 * any code point is beyond the BMP, format 12 for all of them on Unicode full repertoire (0, 4) and
 * Windows Unicode full repertoire (3, 10). Programs that read only format 4 still find the BMP glyphs.
 */
export function unicodeCmap(map: Map<number, number>): Uint8Array {
  const f4 = format4(map);
  const full = [...map.keys()].some((cp) => cp > 0xffff);
  if (!full) {
    return new Bytes().u16(0).u16(2).u16(0).u16(3).u32(20).u16(3).u16(1).u32(20).raw(f4).done();
  }
  const f12 = format12(map);
  const header = 4 + 8 * 4;
  return new Bytes()
    .u16(0)
    .u16(4)
    .u16(0).u16(3).u32(header)
    .u16(0).u16(4).u32(header + f4.length)
    .u16(3).u16(1).u32(header)
    .u16(3).u16(10).u32(header + f4.length)
    .raw(f4)
    .raw(f12)
    .done();
}

/** The tables of an sfnt file, by tag, and its version tag (`0x00010000` or `OTTO`). */
export function readTables(font: Uint8Array): { version: number; tables: Map<string, Uint8Array> } {
  const dv = new DataView(font.buffer, font.byteOffset, font.byteLength);
  const n = dv.getUint16(4);
  const tables = new Map<string, Uint8Array>();
  for (let i = 0; i < n; i++) {
    const rec = 12 + 16 * i;
    const tag = String.fromCharCode(...font.subarray(rec, rec + 4));
    const off = dv.getUint32(rec + 8);
    const len = dv.getUint32(rec + 12);
    tables.set(tag, font.slice(off, off + len));
  }
  return { version: dv.getUint32(0), tables };
}

/**
 * Writes an sfnt from its tables: the directory in tag order, each table 4-byte aligned with its
 * checksum, and `head.checkSumAdjustment` set over the whole file.
 */
export function writeTables(version: number, tables: Map<string, Uint8Array>): Uint8Array {
  const tags = [...tables.keys()].sort();
  const n = tags.length;
  let pow = 1;
  while (pow * 2 <= n) pow *= 2;
  const dir = new Bytes().u32(version).u16(n).u16(pow * 16).u16(Math.log2(pow)).u16(n * 16 - pow * 16);
  let offset = 12 + 16 * n;
  const body = new Bytes();
  for (const tag of tags) {
    const data = Uint8Array.from(tables.get(tag) as Uint8Array);
    if (tag === 'head') new DataView(data.buffer).setUint32(8, 0);
    dir.raw(Array.from(tag, (c) => c.charCodeAt(0))).u32(checksum(data)).u32(offset).u32(data.length);
    body.raw(data).pad(4);
    offset += Math.ceil(data.length / 4) * 4;
  }
  const out = new Bytes().raw(dir.done()).raw(body.done()).done();
  const dv = new DataView(out.buffer);
  const headAt = dv.getUint32(12 + 16 * tags.indexOf('head') + 8);
  dv.setUint32(headAt + 8, (0xb1b0afba - checksum(out)) >>> 0);
  return out;
}

/** A copy of `font` with some tables replaced or added, everything else byte for byte. */
export function replaceTables(font: Uint8Array, replace: Record<string, Uint8Array>): Uint8Array {
  const { version, tables } = readTables(font);
  for (const [tag, data] of Object.entries(replace)) tables.set(tag, data);
  return writeTables(version, tables);
}
