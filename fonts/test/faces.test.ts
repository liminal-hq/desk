// Tests for the assembled faces: coverage, cell sizes, advance rules, joining box drawing and exact outlines.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import { boxCell, GENERATED } from '../src/boxes';
import { BOX, CP437, CP437_PRINTABLE, REQUIRED } from '../src/charset';
import { ACCENTS, assemble, type Glyph } from '../src/face';
import { FACES } from '../src/faces';
import { embolden, ink, inkSpan, parse, width } from '../src/grid';
import { area, trace } from '../src/outline';

const hex = (ch: string): string => `U+${(ch.codePointAt(0) as number).toString(16).toUpperCase().padStart(4, '0')}`;

describe('charset', () => {
  test('code page 437 has 256 entries and no repeats', () => {
    expect(CP437).toHaveLength(256);
    expect(new Set(CP437).size).toBe(256);
  });
});

describe('embolden', () => {
  test('widens strokes to the right and keeps one-pixel gaps open', () => {
    expect(embolden(parse('#...#'))).toEqual(parse('##..##'));
    expect(embolden(parse('#..#..#'))).toEqual(parse('##.##.##'));
  });

  test('widens to the left where widening right would close a gap', () => {
    expect(embolden(parse('..#.#..'))).toEqual(parse('.##.##..'));
  });
});

for (const face of FACES) {
  describe(face.family, () => {
    const glyphs = assemble(face);
    const byChar = new Map(glyphs.map((g) => [g.ch, g]));

    test('covers ASCII, Latin-1, the French extras, box drawing, blocks and the shell symbols', () => {
      const missing = REQUIRED.filter((ch) => !byChar.has(ch)).map(hex);
      expect(missing).toEqual([]);
    });

    test('every accented letter is drawn or composed', () => {
      expect(ACCENTS.filter(([ch]) => !byChar.has(ch)).map(([ch]) => ch)).toEqual([]);
    });

    test('every glyph fills exactly its cell: the face height and its advance', () => {
      for (const g of glyphs) {
        expect(g.rows).toHaveLength(face.height);
        for (const r of g.rows) expect(r).toHaveLength(g.adv);
      }
    });

    test('the em is a multiple of 4 px, so the face is exact at a whole point size at 96 dpi', () => {
      expect(face.size % 4).toBe(0);
      expect((face.size * 3) / 4).toBe(Math.round((face.size * 3) / 4));
    });

    test('code points are unique and sorted', () => {
      for (let i = 1; i < glyphs.length; i++) {
        expect((glyphs[i] as Glyph).cp).toBeGreaterThan((glyphs[i - 1] as Glyph).cp);
      }
    });

    if (face.cell !== undefined) {
      const cell = face.cell;
      test(`every advance is the ${cell}-pixel cell`, () => {
        expect(glyphs.filter((g) => g.adv !== cell).map((g) => g.ch)).toEqual([]);
      });
    } else {
      test('advance is the drawn width plus the gap, and the last column of a letter is blank', () => {
        for (const g of glyphs) {
          if (g.kind === 'generated' || g.kind === 'space') continue;
          const last = g.rows.map((r) => r[g.adv - 1]);
          expect(last.every((v) => !v)).toBe(true);
          const span = inkSpan(g.rows);
          if (span) expect(span[1]).toBeLessThan(g.adv - face.gap);
        }
      });

      test(`box drawing and blocks are ${face.boxWidth} pixels wide with no gap`, () => {
        for (const ch of GENERATED) expect(byChar.get(ch)?.adv).toBe(face.boxWidth);
      });

      test('the space is the face space', () => {
        expect(byChar.get(' ')?.adv).toBe(face.space);
        expect(byChar.get(' ')?.adv).toBe(face.space);
      });
    }

    test('box-drawing arms reach the cell edge so neighbours join', () => {
      const { w, h, cx, cy } = boxCell(face.cell ?? face.boxWidth, face.height);
      const row = (g: Glyph, y: number): number[] => g.rows[y] as number[];
      const col = (g: Glyph, x: number): number[] => g.rows.map((r) => r[x] as number);
      for (const ch of BOX) {
        const g = byChar.get(ch) as Glyph;
        const left = row(g, cy)[0] || row(g, cy - 1)[0] || row(g, cy + 1)[0];
        const right = row(g, cy)[w - 1] || row(g, cy - 1)[w - 1] || row(g, cy + 1)[w - 1];
        const top = row(g, 0)[cx] || row(g, 0)[cx - 1] || row(g, 0)[cx + 1];
        const bottom = row(g, h - 1)[cx] || row(g, h - 1)[cx - 1] || row(g, h - 1)[cx + 1];
        expect([ch, !!left, !!right, !!top, !!bottom]).toEqual([
          ch,
          '─┐┘┤┬┴┼═╕╖╗╛╜╝╡╢╣╤╥╦╧╨╩╪╫╬╮╯'.includes(ch),
          '─┌└├┬┴┼═╒╓╔╘╙╚╞╟╠╤╥╦╧╨╩╪╫╬╭╰'.includes(ch),
          '│└┘├┤┴┼║╘╙╚╛╜╝╞╟╠╡╢╣╧╨╩╪╫╬╯╰'.includes(ch),
          '│┌┐├┤┬┼║╒╓╔╕╖╗╞╟╠╡╢╣╤╥╦╪╫╬╭╮'.includes(ch),
        ]);
      }
      const full = byChar.get('█') as Glyph;
      expect(ink(full.rows)).toBe(w * h);
      expect(col(byChar.get('▌') as Glyph, 0).every((v) => v)).toBe(true);
    });

    test('the traced outline of every glyph covers exactly its ink', () => {
      for (const g of glyphs) {
        const traced = trace(g.rows, face.ascent).reduce((a, c) => a + area(c), 0);
        if (traced !== ink(g.rows)) throw new Error(`${face.id} ${g.ch}: traced ${traced}, ink ${ink(g.rows)}`);
      }
    });

    test('capitals and digits share one cap height and sit on the baseline', () => {
      for (const ch of 'EFHILT0123456789') {
        const g = byChar.get(ch) as Glyph;
        const rows = g.rows.map((r) => r.some((v) => v));
        expect([ch, rows.indexOf(true), rows.lastIndexOf(true)]).toEqual([ch, face.capTop, face.ascent - 1]);
      }
    });

    test('lowercase without ascenders or descenders fills the x-height', () => {
      for (const ch of 'acemnorsuvwxz') {
        const g = byChar.get(ch) as Glyph;
        const rows = g.rows.map((r) => r.some((v) => v));
        expect([ch, rows.indexOf(true), rows.lastIndexOf(true)]).toEqual([ch, face.xTop, face.ascent - 1]);
      }
    });

    test('drawn widths stay inside a fixed cell', () => {
      if (face.cell === undefined) return;
      for (const g of glyphs) expect(width(g.rows)).toBeLessThanOrEqual(face.cell);
    });
  });
}

describe('Desk Fixed', () => {
  const fixed = FACES.find((f) => f.id === 'fixed');
  test('covers every printable character of code page 437', () => {
    const have = new Set(assemble(fixed as never).map((g) => g.ch));
    expect(CP437_PRINTABLE.filter((ch) => !have.has(ch)).map(hex)).toEqual([]);
  });
});
