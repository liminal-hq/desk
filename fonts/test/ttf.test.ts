// Tests for the TrueType files: the sfnt structure Windows 3.1 needs, cmap and name records, and outlines that match
// the OpenType (CFF) files.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import * as ot from 'opentype.js';
import { CP1252_PRINTABLE } from '../src/charset';
import { assemble } from '../src/face';
import { FACES } from '../src/faces';
import { buildOtf } from '../src/otf';
import { outlineOf, contourArea, PIXEL, type Style } from '../src/round';
import { checksum, MAC_ROMAN_HIGH, buildTtf } from '../src/ttf';
import { cmapLookup, pathArea, tables } from './sfnt';

describe('sfnt checksum and Mac Roman table', () => {
  test('Mac Roman has 128 high characters', () => expect(MAC_ROMAN_HIGH).toHaveLength(128));
});

for (const face of FACES) {
  const glyphs = assemble(face);
  for (const style of ['square', 'round'] as Style[]) {
    describe(`${face.family} ${style} TrueType`, () => {
      const f = buildTtf(face, glyphs, style);
      const { dv, dir, get } = tables(f);

      test('has the tables Windows 3.1 needs, in tag order, 4-byte aligned, with correct checksums', () => {
        const tags = dir.map((d) => d.tag);
        expect([...tags].sort()).toEqual(tags);
        for (const t of ['head', 'hhea', 'maxp', 'hmtx', 'loca', 'glyf', 'cmap', 'name', 'post', 'OS/2']) expect(tags).toContain(t);
        for (const d of dir) {
          expect(d.offset % 4).toBe(0);
          const sum = d.tag === 'head' ? checksum(Uint8Array.from([...f.subarray(d.offset, d.offset + 8), 0, 0, 0, 0, ...f.subarray(d.offset + 12, d.offset + d.length)])) : checksum(f, d.offset, d.length);
          expect([d.tag, sum]).toEqual([d.tag, d.sum]);
        }
        expect(checksum(f)).toBe(0xb1b0afba);
      });

      test('head, maxp, OS/2 and post use the era-safe versions and flags', () => {
        const head = get('head').offset;
        expect(dv.getUint16(head + 18)).toBe(face.size * PIXEL);
        expect(dv.getUint16(head + 16) & 0b1000).toBe(0b1000);
        expect(dv.getUint16(head + 46)).toBe(face.size);
        expect(dv.getUint32(get('maxp').offset)).toBe(0x00010000);
        expect(get('maxp').length).toBe(32);
        const os2 = get('OS/2').offset;
        expect(dv.getUint16(os2)).toBe(1);
        expect(dv.getUint16(os2 + 8)).toBe(0);
        expect(dv.getUint32(os2 + 78) & 1).toBe(1);
        expect(dv.getUint32(get('post').offset)).toBe(0x00020000);
      });

      test('maxp, loca, glyf and hmtx agree, and no glyph is a composite', () => {
        const n = dv.getUint16(get('maxp').offset + 4);
        expect(dv.getUint16(get('hhea').offset + 34)).toBe(n);
        expect(get('hmtx').length).toBe(4 * n);
        expect(dv.getInt16(get('head').offset + 50)).toBe(1);
        expect(get('loca').length).toBe(4 * (n + 1));
        const loca = get('loca').offset;
        const glyf = get('glyf').offset;
        let maxPoints = 0;
        for (let i = 0; i < n; i++) {
          const a = dv.getUint32(loca + 4 * i);
          const b = dv.getUint32(loca + 4 * i + 4);
          expect(b).toBeGreaterThanOrEqual(a);
          if (b === a) continue;
          const contours = dv.getInt16(glyf + a);
          expect(contours).toBeGreaterThan(0);
          const points = dv.getUint16(glyf + a + 10 + 2 * (contours - 1)) + 1;
          maxPoints = Math.max(maxPoints, points);
        }
        expect(dv.getUint32(loca + 4 * n)).toBe(get('glyf').length);
        expect(dv.getUint16(get('maxp').offset + 6)).toBe(maxPoints);
      });

      test('every code page 1252 character maps through the Windows Unicode cmap, and Mac Roman agrees', () => {
        const win = cmapLookup(f, 3, 1);
        const mac = cmapLookup(f, 1, 0);
        expect(win.format).toBe(4);
        expect(mac.format).toBe(6);
        expect(cmapLookup(f, 0, 3).format).toBe(4);
        expect(CP1252_PRINTABLE.filter((ch) => win.glyph(ch.codePointAt(0) as number) === 0)).toEqual([]);
        for (let b = 0x80; b < 0x100; b++) {
          const cp = (MAC_ROMAN_HIGH[b - 0x80] as string).codePointAt(0) as number;
          expect(mac.glyph(b)).toBe(win.glyph(cp));
        }
        expect(mac.glyph(0x41)).toBe(win.glyph(0x41));
      });

      test('names: Windows (0x409) and Macintosh records for IDs 0-6, 13 and 14; regular full name is the family', () => {
        const name = get('name').offset;
        const count = dv.getUint16(name + 2);
        const store = name + dv.getUint16(name + 4);
        const recs = new Map<string, string>();
        let prev = -1;
        for (let i = 0; i < count; i++) {
          const r = name + 6 + 12 * i;
          const [pl, enc, lang, id, len, off] = [0, 2, 4, 6, 8, 10].map((k) => dv.getUint16(r + k)) as number[];
          const key = (((pl as number) * 65536 + (enc as number)) * 65536 + (lang as number)) * 65536 + (id as number);
          expect(key).toBeGreaterThan(prev);
          prev = key;
          const bytes = f.subarray(store + (off as number), store + (off as number) + (len as number));
          const text = pl === 3 ? String.fromCharCode(...Array.from({ length: (len as number) / 2 }, (_, k) => (bytes[2 * k] as number) * 256 + (bytes[2 * k + 1] as number))) : String.fromCharCode(...bytes);
          recs.set(`${pl}/${enc}/${lang}/${id}`, text);
        }
        for (const id of [0, 1, 2, 3, 4, 5, 6, 13, 14]) {
          expect(recs.has(`3/1/1033/${id}`)).toBe(true);
          expect(recs.get(`1/0/0/${id}`)).toBe(recs.get(`3/1/1033/${id}`));
        }
        const family = recs.get('3/1/1033/1') as string;
        expect(family.length).toBeLessThanOrEqual(31);
        expect(family).toBe(style === 'round' ? `${face.family} Round` : face.family);
        expect(recs.get('3/1/1033/4')).toBe(face.style === 'Regular' ? family : `${family} Bold`);
      });

      test('every glyph has the same advance and outline area as in the OpenType file', () => {
        const tt = ot.parse(f.slice().buffer);
        const cff = ot.parse(buildOtf(face, glyphs, style).slice().buffer);
        for (const g of glyphs) {
          if (g.cp > 0xffff) continue;
          const a = tt.charToGlyph(g.ch);
          const b = cff.charToGlyph(g.ch);
          expect(a.advanceWidth).toBe(b.advanceWidth as number);
          // TrueType winds outer contours clockwise, CFF counter-clockwise; CFF stores each quadratic as a cubic.
          const ta = -pathArea(a.path);
          const ca = pathArea(b.path);
          const want = outlineOf(g.rows, face.ascent, style).reduce((s, c) => s + contourArea(c), 0);
          expect(Math.abs(ta - want)).toBeLessThan(1e-6);
          if (Math.abs(ca - want) > 1e-6) throw new Error(`${face.id} ${style} ${g.ch}: CFF area ${ca}, TrueType ${ta}`);
        }
      });
    });
  }
}
