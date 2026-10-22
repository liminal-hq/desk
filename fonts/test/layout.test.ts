// Tests for the outline faces' layout tables, read back from the built files: kerning in GPOS and the old `kern`
// table (present, equal, signs and sizes sane), the f ligatures, the figure styles, alternates, small capitals and
// fractions, Desk Mono's code ligatures (every advance kept, plain when switched off or next to another operator), and
// that the default cmap forms are unchanged. `validate.sh` checks the same with HarfBuzz.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import { assembleModern } from '../src/modern/assemble';
import { CODE_LIGATURES, SPACER } from '../src/modern/code';
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

    test('the default cmap forms are unchanged: no ligature or alternate has a code point', () => {
      for (const f of Object.values(files)) {
        for (const g of glyphs.filter((x) => x.cp < 0)) {
          expect(f.shape(g.ch, [])).toEqual([n(g.ch)]);
        }
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

      test('code ligatures: spacers and one drawing, under calt only', () => {
        for (const f of Object.values(files)) {
          expect(f.featureTags).toEqual(['calt']);
          for (const seq of CODE_LIGATURES) {
            const got = f.shape(`a${seq}b`, ['calt']);
            expect(got.length).toBe(seq.length + 2);
            expect(got.slice(1, -2)).toEqual(Array(seq.length - 1).fill(SPACER));
            expect(got[got.length - 2]).toMatch(/\.liga$/);
            // Switched off, and typed one at a time, they stay plain.
            expect(f.shape(seq, [])).toEqual(Array.from(seq, n));
            for (const ch of seq) expect(f.shape(ch, ['calt'])).toEqual([n(ch)]);
          }
          for (const text of ['<!--', '-->', '====', '///', '/**', '<<=', '->>']) expect([text, f.shape(text, ['calt'])]).toEqual([text, Array.from(text, n)]);
        }
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

    test('the f ligatures start where the f does', () => {
      // The ligatures redraw the f (`fGeom` in features.ts); its left edge and descender must match the face's own f.
      const f = glyphs.find((g) => g.ch === 'f' && g.cp >= 0) as (typeof glyphs)[number];
      for (const name of ['f_f', 'f_i', 'f_l', 'f_f_i', 'f_f_l']) {
        const lig = glyphs.find((g) => g.name === name) as (typeof glyphs)[number];
        expect([name, Math.abs((lig.box?.x0 ?? 0) - (f.box?.x0 ?? 0)) <= 2, Math.abs((lig.box?.y0 ?? 0) - (f.box?.y0 ?? 0)) <= 2]).toEqual([name, true, true]);
      }
    });

    test('f ligatures under liga, and plain without it', () => {
      for (const f of Object.values(files)) {
        for (const [seq, lig] of [['fi', 'f_i'], ['fl', 'f_l'], ['ff', 'f_f'], ['ffi', 'f_f_i'], ['ffl', 'f_f_l']] as const) {
          expect(f.shape(seq, ['liga'])).toEqual([lig]);
          expect(f.shape(seq, [])).toEqual(Array.from(seq, n));
        }
        expect(f.shape('office', ['liga'])).toEqual([n('o'), 'f_f_i', n('c'), n('e')]);
      }
    });

    test('figures: tabular by default, proportional and (Serif) old-style on request, and fractions', () => {
      for (const f of Object.values(files)) {
        expect(f.shape('10', ['pnum'])).toEqual(['one.pnum', 'zero.pnum']);
        expect(f.shape('10', ['pnum', 'tnum'])).toEqual(['one', 'zero'].map((x) => (x === 'one' ? n('1') : n('0'))));
        expect(f.shape('1/2 3/4 1/4 2/3 7/8', ['frac'])).toEqual([n('½'), n(' '), n('¾'), n(' '), n('¼'), n(' '), 'twothirds', n(' '), 'seveneighths']);
        if (s.kind === 'serif') {
          expect(f.shape('39', ['onum'])).toEqual(['three.tosf', 'nine.tosf']);
          expect(f.shape('39', ['onum', 'pnum'])).toEqual(['three.osf', 'nine.osf']);
          expect(f.shape('39', ['onum', 'lnum'])).toEqual([n('3'), n('9')]);
          expect(f.shape('Aé', ['smcp', 'c2sc'])).toEqual(['a.sc', 'uni00E9.sc']);
          expect(f.shape('ct st', ['dlig'])).toEqual(['c_t', n(' '), 's_t']);
          expect(f.featureTags).toEqual(['c2sc', 'dlig', 'frac', 'liga', 'lnum', 'onum', 'pnum', 'smcp', 'tnum']);
        } else {
          expect(f.shape('a0lâ', ['ss01'])).toEqual(['a.ss01', 'zero.ss01', 'l.ss01', 'uni00E2.ss01']);
          expect(f.shape('0', ['pnum', 'ss01'])).toEqual(['zero.pnum.ss01']);
          expect(f.featureTags).toEqual(['frac', 'liga', 'pnum', 'ss01', 'tnum']);
        }
      }
      // Old-style and proportional figures keep their advances: tabular ones share one.
      const adv = (nm: string): number => (glyphs.find((g) => (g.name ?? '') === nm) as { adv: number }).adv;
      if (s.kind === 'serif') expect(new Set(['zero', 'one', 'three', 'eight'].map((x) => adv(`${x}.tosf`))).size).toBe(1);
      expect(adv('one.pnum')).toBeLessThan(adv('zero.pnum'));
    });
  });
}
