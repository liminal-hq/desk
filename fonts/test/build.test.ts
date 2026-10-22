// Tests for the written files: OpenType metrics, cmap and outlines read back through a parser, the .DFN
// round trip, reproducible builds, and that the committed files in generated/ are current.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import * as ot from 'opentype.js';
import { buildAll } from '../build';
import { decodeDfn, encodeDfn } from '../src/dfn';
import { assemble } from '../src/face';
import { FACES } from '../src/faces';
import { ink } from '../src/grid';
import { buildOtf, metrics, PIXEL } from '../src/otf';

/** Area enclosed by a parsed glyph path (straight lines only), positive for counter-clockwise contours. */
function pathArea(path: ot.Path): number {
  let total = 0;
  let pts: { x: number; y: number }[] = [];
  const flush = (): void => {
    pts.forEach((p, i) => {
      const q = pts[(i + 1) % pts.length] as { x: number; y: number };
      total += p.x * q.y - q.x * p.y;
    });
    pts = [];
  };
  for (const c of path.commands) {
    if (c.type === 'M') {
      flush();
      pts.push({ x: c.x as number, y: c.y as number });
    } else if (c.type === 'L') pts.push({ x: c.x as number, y: c.y as number });
    else if (c.type === 'Z') flush();
    else throw new Error(`unexpected path command ${c.type}`);
  }
  flush();
  return total / 2;
}

for (const face of FACES) {
  describe(`${face.family} OpenType`, () => {
    const glyphs = assemble(face);
    const bytes = buildOtf(face, glyphs);
    const font = ot.parse(bytes.slice().buffer);
    const m = metrics(face);

    test('one pixel is 100 units and the em is the face size', () => {
      expect(font.unitsPerEm).toBe(face.size * PIXEL);
      expect(font.ascender).toBe(m.ascender);
      expect(font.descender).toBe(m.descender);
      expect(m.ascender - m.descender).toBe(face.height * PIXEL);
      const os2 = font.tables.os2 as Record<string, number>;
      expect(os2.usWinAscent).toBe(m.ascender);
      expect(os2.usWinDescent).toBe(-m.descender);
      expect(os2.sTypoLineGap).toBe(0);
    });

    test('names carry the family, style and the SIL Open Font License', () => {
      const names = font.names as Record<string, Record<string, Record<string, string>>>;
      const win = names.windows as Record<string, Record<string, string>>;
      expect(win.fontFamily?.en).toBe(face.family);
      expect(win.fontSubfamily?.en).toBe(face.style);
      expect(win.license?.en).toContain('SIL Open Font License, Version 1.1');
      expect(win.licenseURL?.en).toBe('https://openfontlicense.org');
    });

    test('every glyph is in the cmap with its advance, and its outline covers exactly its ink', () => {
      for (const g of glyphs) {
        const og = font.charToGlyph(g.ch);
        if (og.index === 0) throw new Error(`${face.id}: ${g.ch} is not in the cmap`);
        expect(og.advanceWidth).toBe(g.adv * PIXEL);
        const a = pathArea(og.path);
        if (a !== ink(g.rows) * PIXEL * PIXEL) throw new Error(`${face.id} ${g.ch}: area ${a}, ink ${ink(g.rows)}`);
      }
    });

    test('a fixed face says so in post and panose', () => {
      expect((font.tables.post as Record<string, number>).isFixedPitch).toBe(face.cell === undefined ? 0 : 1);
    });

    test('the build is reproducible', () => {
      expect(buildOtf(face, assemble(face))).toEqual(bytes);
    });
  });

  describe(`${face.family} .DFN`, () => {
    const glyphs = assemble(face);
    const dfn = decodeDfn(encodeDfn(face, glyphs));

    test('the header carries the height and baseline', () => {
      expect(dfn.height).toBe(face.height);
      expect(dfn.baseline).toBe(face.ascent);
    });

    test('decodes to the same glyphs, rows and widths', () => {
      expect(dfn.glyphs.map((g) => [g.cp, g.width, g.rows])).toEqual(glyphs.map((g) => [g.cp, g.adv, g.rows]));
    });
  });
}

describe('generated/', () => {
  test('the committed files match a fresh build (run `bun build.ts` after changing a grid)', () => {
    for (const f of buildAll()) {
      const committed = new Uint8Array(readFileSync(join(import.meta.dir, '..', 'generated', f.name)));
      if (committed.length !== f.data.length || !committed.every((v, i) => v === f.data[i])) {
        throw new Error(`generated/${f.name} is stale`);
      }
    }
    // A whole build, the emoji included, takes several seconds.
  }, 30_000);

  test('the licence sits beside the fonts', () => {
    const ofl = readFileSync(join(import.meta.dir, '..', 'generated', 'OFL.txt'), 'utf8');
    expect(ofl).toContain('Copyright (c) 2026 Liminal HQ, Scott Morris');
    expect(ofl).toContain('SIL OPEN FONT LICENSE Version 1.1');
  });
});
