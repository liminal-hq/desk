// Tests for the emoji files: the cmaps of the pixel faces' OpenType files (format 12 beyond the BMP) with the faces'
// own glyphs untouched, the TrueType files free of emoji, the COLR and CPAL tables of Desk Emoji Pixel, and the .DEM
// files read back with a decoder written here from docs/FORMATS.md.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import * as ot from 'opentype.js';
import { buildEmoji } from '../src/emoji/build';
import { PLACEMENT, pixelOtfWithEmoji } from '../src/emoji/faces';
import { SIZES, type Size } from '../src/emoji/grid';
import { MANIFEST } from '../src/emoji/manifest';
import { SLOTS } from '../src/emoji/palette';
import { pixelSet } from '../src/emoji/pixels';
import { assemble } from '../src/face';
import { FACES } from '../src/faces';
import { buildOtf } from '../src/otf';
import { buildTtf } from '../src/ttf';
import { cmapLookup, tables } from './sfnt';

const set = pixelSet();
const files = new Map(buildEmoji(set).map((f) => [f.name, f.data]));
const file = (name: string): Uint8Array => {
  const f = files.get(name);
  if (!f) throw new Error(`no ${name}`);
  return f;
};
const hex = (cp: number): string => `U+${cp.toString(16).toUpperCase()}`;

/** The cmap subtables of a font: platform, encoding and format. */
function subtables(f: Uint8Array): [number, number, number][] {
  const { dv, get } = tables(f);
  const c = get('cmap').offset;
  return Array.from({ length: dv.getUint16(c + 2) }, (_, i) => {
    const rec = c + 4 + 8 * i;
    return [dv.getUint16(rec), dv.getUint16(rec + 2), dv.getUint16(c + dv.getUint32(rec + 4))] as [number, number, number];
  });
}

/** A format 12 lookup, read from the bytes. */
function format12(f: Uint8Array, platform: number, encoding: number): (cp: number) => number {
  const { dv, get } = tables(f);
  const c = get('cmap').offset;
  for (let i = 0; i < dv.getUint16(c + 2); i++) {
    const rec = c + 4 + 8 * i;
    if (dv.getUint16(rec) !== platform || dv.getUint16(rec + 2) !== encoding) continue;
    const s = c + dv.getUint32(rec + 4);
    if (dv.getUint16(s) !== 12) throw new Error('not format 12');
    const n = dv.getUint32(s + 12);
    return (cp) => {
      for (let k = 0; k < n; k++) {
        const g = s + 16 + 12 * k;
        const start = dv.getUint32(g);
        if (cp >= start && cp <= dv.getUint32(g + 4)) return dv.getUint32(g + 8) + cp - start;
      }
      return 0;
    };
  }
  throw new Error(`no subtable ${platform}/${encoding}`);
}

describe('pixel faces with emoji', () => {
  for (const face of FACES) {
    const at = PLACEMENT[face.id];
    if (!at) throw new Error(`no placement for ${face.id}`);
    const glyphs = assemble(face);
    const own = new Set(glyphs.map((g) => g.cp));
    const otf = pixelOtfWithEmoji(face, glyphs, 'square');
    const want = set.filter((e) => e.sizes[at.size]);

    test(`${face.family}: format 4 for the BMP and format 12 for every code point`, () => {
      expect(subtables(otf)).toEqual([
        [0, 3, 4],
        [0, 4, 12],
        [3, 1, 4],
        [3, 10, 12],
      ]);
      const full = format12(otf, 3, 10);
      const uni = format12(otf, 0, 4);
      const bmp = cmapLookup(otf, 3, 1).glyph;
      for (const e of want) {
        expect([hex(e.cp), full(e.cp) > 0, uni(e.cp) === full(e.cp)]).toEqual([hex(e.cp), true, true]);
        if (e.cp <= 0xffff) expect([hex(e.cp), bmp(e.cp)]).toEqual([hex(e.cp), full(e.cp)]);
      }
      for (const g of glyphs) expect([g.ch, full(g.cp)]).toEqual([g.ch, g.cp <= 0xffff ? bmp(g.cp) : full(g.cp) || -1]);
    });

    test(`${face.family}: the emoji glyph is the monochrome grid in the cell, and the face's own glyphs are unchanged`, () => {
      const font = ot.parse(otf.slice().buffer);
      const plain = ot.parse(buildOtf(face, glyphs, 'square').slice().buffer);
      for (const g of glyphs) {
        const a = font.charToGlyph(g.ch);
        const b = plain.charToGlyph(g.ch);
        expect([g.ch, a.advanceWidth, a.path.toPathData(0)]).toEqual([g.ch, b.advanceWidth, b.path.toPathData(0)]);
      }
      const full = format12(otf, 3, 10);
      for (const e of want) {
        if (own.has(e.cp)) continue;
        const g = font.glyphs.get(full(e.cp));
        expect([e.ch, g.advanceWidth]).toEqual([e.ch, at.adv * 100]);
        const ink = (e.sizes[at.size]?.mono ?? []).flat().filter((v) => v).length;
        const box = g.getBoundingBox();
        expect([e.ch, ink > 0, box.x1 >= at.left * 100, box.x2 <= (at.left + at.size) * 100]).toEqual([e.ch, true, true, true]);
      }
    });

    test(`${face.family}: the TrueType file carries no emoji the face does not draw itself`, () => {
      const ttf = buildTtf(face, glyphs, 'square');
      expect(subtables(ttf).some(([, , f]) => f === 12)).toBe(false);
      const bmp = cmapLookup(ttf, 3, 1).glyph;
      for (const e of MANIFEST) if (e.cp <= 0xffff && !own.has(e.cp)) expect([e.ch, bmp(e.cp)]).toEqual([e.ch, 0]);
    });
  }

  test('Desk Small carries only the core emoji, the others every emoji', () => {
    const core = set.filter((e) => e.tier === 'core').length;
    expect(set.filter((e) => e.sizes[8]).length).toBe(core);
    expect(set.filter((e) => e.sizes[12]).length).toBe(MANIFEST.length);
  });
});

/** COLR version 0 read from the bytes: base glyph id to its layers. */
function readColr(f: Uint8Array): Map<number, { glyph: number; palette: number }[]> {
  const { dv, get } = tables(f);
  const c = get('COLR').offset;
  expect(dv.getUint16(c)).toBe(0);
  const n = dv.getUint16(c + 2);
  const base = c + dv.getUint32(c + 4);
  const layers = c + dv.getUint32(c + 8);
  const out = new Map<number, { glyph: number; palette: number }[]>();
  let last = -1;
  for (let i = 0; i < n; i++) {
    const r = base + 6 * i;
    const gid = dv.getUint16(r);
    expect(gid).toBeGreaterThan(last);
    last = gid;
    const first = dv.getUint16(r + 2);
    out.set(
      gid,
      Array.from({ length: dv.getUint16(r + 4) }, (_, k) => ({
        glyph: dv.getUint16(layers + 4 * (first + k)),
        palette: dv.getUint16(layers + 4 * (first + k) + 2),
      })),
    );
  }
  return out;
}

describe('Desk Emoji Pixel', () => {
  for (const name of ['emoji/desk-emoji-pixel.ttf', 'emoji/desk-emoji-pixel.otf']) {
    const f = file(name);
    test(`${name}: CPAL is one palette of the 16 Desk colours`, () => {
      const { dv, get } = tables(f);
      const c = get('CPAL').offset;
      expect([dv.getUint16(c), dv.getUint16(c + 2), dv.getUint16(c + 4), dv.getUint16(c + 6)]).toEqual([0, 16, 1, 16]);
      const rec = c + dv.getUint32(c + 8) + 4 * dv.getUint16(c + 12);
      const colours = Array.from({ length: 16 }, (_, i) => {
        const [b, g, r, a] = [0, 1, 2, 3].map((k) => dv.getUint8(rec + 4 * i + k)) as [number, number, number, number];
        expect(a).toBe(255);
        return `#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
      });
      expect(colours).toEqual(SLOTS.map((s) => s.hex));
    });

    test(`${name}: every emoji maps to a base glyph with one layer per colour, back to front`, () => {
      const colr = readColr(f);
      const { dv, get } = tables(f);
      const numGlyphs = dv.getUint16(get('maxp').offset + 4);
      expect(subtables(f)).toEqual([
        [0, 3, 4],
        [0, 4, 12],
        [3, 1, 4],
        [3, 10, 12],
      ]);
      const full = format12(f, 3, 10);
      for (const e of set) {
        const gid = full(e.cp);
        const layers = colr.get(gid);
        if (!layers) throw new Error(`${e.ch}: no COLR record`);
        const colour = e.sizes[16]?.colour ?? [];
        const used = new Set(colour.flat().filter((v) => v >= 0));
        expect([e.ch, layers.map((l) => l.palette).sort((a, b) => a - b)]).toEqual([e.ch, [...used].sort((a, b) => a - b)]);
        // Black, the outline, is painted first; the rest cover it.
        if (used.has(0)) expect([e.ch, layers[0]?.palette]).toEqual([e.ch, 0]);
        for (const l of layers) {
          expect(l.glyph).toBeLessThan(numGlyphs);
          expect(l.palette < 16 || l.palette === 0xffff).toBe(true);
        }
      }
      for (const cp of [0x200d, 0xfe0f]) expect(full(cp)).toBeGreaterThan(0);
    });

    test(`${name}: the em is the 16-pixel grid and the family name fits in 31 characters`, () => {
      const font = ot.parse(f.slice().buffer);
      expect(font.unitsPerEm).toBe(1600);
      const family = (font.names as unknown as Record<string, Record<string, Record<string, string>>>).windows?.fontFamily?.en;
      expect(family).toBe('Desk Emoji Pixel');
      expect((family ?? '').length).toBeLessThanOrEqual(31);
    });
  }
});

/** The .DEM layout of docs/FORMATS.md, decoded independently of the encoder. */
function decodeDem(d: Uint8Array): { size: number; emoji: Map<number, { layers: { slot: number; rows: number[][] }[]; mono: number[][] }> } {
  const dv = new DataView(d.buffer, d.byteOffset, d.byteLength);
  expect(String.fromCharCode(...d.subarray(0, 4))).toBe('DEM1');
  const size = d[4] as number;
  expect(d[5]).toBe(0);
  const count = dv.getUint16(6, true);
  const stride = Math.ceil(size / 8);
  const bitmap = (at: number): number[][] =>
    Array.from({ length: size }, (_, y) => Array.from({ length: size }, (_, x) => ((d[at + y * stride + (x >> 3)] as number) >> (7 - (x & 7))) & 1));
  const emoji = new Map<number, { layers: { slot: number; rows: number[][] }[]; mono: number[][] }>();
  let prev = -1;
  let end = 8 + 8 * count;
  for (let i = 0; i < count; i++) {
    const cp = dv.getUint32(8 + 8 * i, true);
    const off = dv.getUint32(12 + 8 * i, true);
    expect(cp).toBeGreaterThan(prev);
    expect(off).toBe(end);
    prev = cp;
    let at = off + 1;
    const layers = Array.from({ length: d[off] as number }, () => {
      const slot = d[at] as number;
      const rows = bitmap(at + 1);
      at += 1 + size * stride;
      return { slot, rows };
    });
    emoji.set(cp, { layers, mono: bitmap(at) });
    end = at + size * stride;
  }
  expect(end).toBe(d.length);
  return { size, emoji };
}

describe('.DEM files', () => {
  for (const size of SIZES) {
    test(`emoji${size}.dem decodes to every drawing of ${size} pixels, colour and mono`, () => {
      const { size: s, emoji } = decodeDem(file(`emoji/emoji${size}.dem`));
      expect(s).toBe(size);
      const want = set.filter((e) => e.sizes[size as Size]);
      expect(emoji.size).toBe(want.length);
      for (const e of want) {
        const d = e.sizes[size as Size];
        const got = emoji.get(e.cp);
        if (!d || !got) throw new Error(`${e.ch}: missing at ${size}`);
        const colour = Array.from({ length: size }, () => new Array<number>(size).fill(-1));
        for (const l of got.layers) {
          expect(l.slot).toBeLessThan(16);
          l.rows.forEach((r, y) => r.forEach((v, x) => {
            if (!v) return;
            // Layers hold only their own colour, so none overlap.
            expect((colour[y] as number[])[x]).toBe(-1);
            (colour[y] as number[])[x] = l.slot;
          }));
        }
        expect([e.ch, colour]).toEqual([e.ch, d.colour]);
        expect([e.ch, got.mono]).toEqual([e.ch, d.mono]);
      }
    });
  }
});

test('the emoji files are reproducible', () => {
  const again = buildEmoji(pixelSet());
  for (const f of again) expect([f.name, Buffer.from(f.data).equals(Buffer.from(file(f.name)))]).toEqual([f.name, true]);
});
