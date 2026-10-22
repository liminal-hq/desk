// Tests for the outline faces' layout tables, read back from the built files: kerning in GPOS and the old `kern`
// table (present, equal, signs and sizes sane), and that the default cmap forms are unchanged.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import { assembleModern } from '../src/modern/assemble';
import { layoutOf } from '../src/modern/features';
import { buildModernOtf, buildModernTtf } from '../src/modern/files';
import { STYLES } from '../src/modern/kit';
import { glyphName } from '../src/otf';
import { Shaper } from './shaper';

const n = (ch: string): string => glyphName(ch.codePointAt(0) as number);
const MUST_TIGHTEN = ['To', 'Ta', 'AV', 'VA', 'Ty', 'Yo', 'r.', 'LT', 'P.', 'T.', 'fo', 'Av'];

for (const s of STYLES) {
  const name = `${s.family}${s.bold ? ' Bold' : ''}${s.italic ? ' Italic' : ''}`;
  describe(`${name} layout`, () => {
    const glyphs = assembleModern(s);
    const layout = layoutOf(s, glyphs);
    const files = { otf: new Shaper(buildModernOtf(s, glyphs, layout)), ttf: new Shaper(buildModernTtf(s, glyphs, layout)) };

    test('the default cmap forms are unchanged', () => {
      for (const f of Object.values(files)) {
        expect(f.shape('fi0al', [])).toEqual(['f', 'i', '0', 'a', 'l'].map(n));
      }
    });

    test('the build with its layout is reproducible', () => {
      expect(buildModernTtf(s, assembleModern(s), layoutOf(s, assembleModern(s)))).toEqual(files.ttf.file);
    });

    if (s.kind === 'mono') {
      test('no kerning, and every glyph keeps the 600-unit advance', () => {
        expect(layout.kerning).toEqual([]);
        expect(files.ttf.has('kern') || files.ttf.has('GPOS') || files.otf.has('GPOS')).toBe(false);
        expect(glyphs.filter((g) => g.adv !== 600).map((g) => g.name ?? g.ch)).toEqual([]);
      });
      return;
    }

    test('kerning: a format 0 kern table and GPOS with the same pairs, modest and with sane signs', () => {
      const kern = files.ttf.kernTable();
      expect(kern).not.toBeNull();
      const k = kern as NonNullable<typeof kern>;
      expect([k.version, k.tables, k.format, k.coverage & 1]).toEqual([0, 1, 0, 1]);
      const pairs = k.header[0] as number;
      let pow = 1;
      while (pow * 2 <= pairs) pow *= 2;
      expect(k.header).toEqual([pairs, pow * 6, Math.log2(pow), pairs * 6 - pow * 6]);
      expect(k.sorted).toBe(true);
      expect(pairs).toBeGreaterThan(200);
      expect(pairs).toBeLessThan(600);
      expect(files.ttf.gposPairs()).toEqual(k.pairs);
      expect(files.otf.gposPairs()).toEqual(k.pairs);
      expect(files.otf.has('kern')).toBe(false);
      for (const v of k.pairs.values()) {
        expect(v).toBeGreaterThanOrEqual(-160);
        expect(v).toBeLessThanOrEqual(120);
      }
      for (const pair of MUST_TIGHTEN) {
        const v = k.pairs.get(`${n(pair[0] as string)} ${n(pair[1] as string)}`) ?? 0;
        // Desk Serif Bold Italic's T and y meet at the T's drooping bar end, so that pair stays almost unkerned.
        if (pair === 'Ty' && s.kind === 'serif' && s.italic && s.bold) continue;
        expect([pair, v < -20]).toEqual([pair, true]);
      }
      for (const pair of ['HH', 'nn', 'oo', 'Hn']) expect(k.pairs.has(`${n(pair[0] as string)} ${n(pair[1] as string)}`)).toBe(false);
    });
  });
}
