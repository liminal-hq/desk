// Tests for Desk Fixed DOS: code page 437 at its byte values through a symbol cmap, in both styles and both formats.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import * as ot from 'opentype.js';
import { CP437 } from '../src/charset';
import { dosGlyphs, SYMBOL_BASE } from '../src/dos';
import { assemble, type FaceDef } from '../src/face';
import { FACES } from '../src/faces';
import { namesOf } from '../src/names';
import { buildOtf } from '../src/otf';
import { contourArea, outlineOf, type Style } from '../src/round';
import { buildTtf, checksum } from '../src/ttf';
import { cmapLookup, pathArea, tables } from './sfnt';

const fixed = FACES.find((f) => f.id === 'fixed') as FaceDef;
const glyphs = assemble(fixed);
const byChar = new Map(glyphs.map((g) => [g.ch, g]));
const dos = dosGlyphs(fixed, glyphs);

describe('dosGlyphs', () => {
  test('gives 256 glyphs, byte b at 0xF000 + b, drawn like the Unicode glyph of code page 437 byte b', () => {
    expect(dos).toHaveLength(256);
    dos.forEach((g, b) => {
      expect(g.cp).toBe(SYMBOL_BASE + b);
      if (b === 0) expect(g.rows.flat().every((v) => !v)).toBe(true);
      else expect(g.rows).toEqual(byChar.get(CP437[b] as string)?.rows as number[][]);
    });
  });

  test('the control-code positions draw the code page 437 pictures', () => {
    for (let b = 1; b < 0x20; b++) expect(dos[b]?.rows.flat().some((v) => v)).toBe(true);
    expect(dos[0x7f]?.rows.flat().some((v) => v)).toBe(true);
  });
});

for (const style of ['square', 'round'] as Style[]) {
  describe(`Desk Fixed DOS ${style}`, () => {
    const f = buildTtf(fixed, dos, style, 'dos');
    const { dv, dir, get } = tables(f);
    const area = (b: number): number => outlineOf((dos[b] as { rows: number[][] }).rows, fixed.ascent, style).reduce((s, c) => s + contourArea(c), 0);

    test('names stay within 31 characters', () => {
      const n = namesOf(fixed, style, 'dos');
      expect(n.family).toBe(style === 'round' ? 'Desk Fixed DOS Round' : 'Desk Fixed DOS');
      expect(n.family.length).toBeLessThanOrEqual(31);
    });

    test('is a well-formed sfnt', () => {
      const tags = dir.map((d) => d.tag);
      expect([...tags].sort()).toEqual(tags);
      for (const d of dir) expect(d.offset % 4).toBe(0);
      expect(checksum(f)).toBe(0xb1b0afba);
    });

    test('has symbol (3, 0), Macintosh (1, 0) and Unicode (0, 3) cmaps but no Windows Unicode (3, 1)', () => {
      const sym = cmapLookup(f, 3, 0);
      const mac = cmapLookup(f, 1, 0);
      const uni = cmapLookup(f, 0, 3);
      expect([sym.format, mac.format, uni.format]).toEqual([4, 6, 4]);
      expect(() => cmapLookup(f, 3, 1)).toThrow();
      const tt = ot.parse(f.slice().buffer);
      for (let b = 0; b < 256; b++) {
        const gid = sym.glyph(SYMBOL_BASE + b);
        expect(gid).toBeGreaterThan(0);
        expect(mac.glyph(b)).toBe(gid);
        // Every code page 437 character's real code point reaches the same glyph (byte 0, U+0000, is not mapped).
        if (b > 0) expect(uni.glyph(CP437[b]?.codePointAt(0) as number)).toBe(gid);
        expect(-pathArea(tt.glyphs.get(gid).path)).toBeCloseTo(area(b), 6);
        expect(tt.glyphs.get(gid).advanceWidth).toBe(800);
      }
      expect(uni.glyph(0)).toBe(0);
      expect(uni.glyph(SYMBOL_BASE + 0x41)).toBe(0);
    });

    test('OS/2 declares the symbol character set and code page 437, starting at 0xF000', () => {
      const os2 = get('OS/2').offset;
      expect(dv.getUint32(os2 + 78)).toBe(0x80000000);
      expect(dv.getUint32(os2 + 82)).toBe(0x80000000);
      expect(dv.getUint16(os2 + 64)).toBe(SYMBOL_BASE);
      expect(dv.getUint16(os2 + 66)).toBe(SYMBOL_BASE + 0xff);
    });

    test('the OTF reaches the same glyphs at U+F000 to U+F0FF and at the real code points, with the same outlines', () => {
      const cff = ot.parse(buildOtf(fixed, dos, style, 'dos').slice().buffer);
      for (let b = 0; b < 256; b++) {
        const g = cff.charToGlyph(String.fromCharCode(SYMBOL_BASE + b));
        expect(g.index).toBeGreaterThan(0);
        expect(pathArea(g.path)).toBeCloseTo(area(b), 6);
        if (b > 0) expect(cff.charToGlyph(CP437[b] as string).index).toBe(g.index);
      }
    });
  });
}
