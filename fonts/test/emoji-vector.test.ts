// Tests for the vector emoji: the set against the manifest, the palette, the drawings and their monochrome forms,
// Desk Emoji (COLR and CPAL), and the emoji in the OpenType files of Desk Sans, Mono and Serif.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import * as ot from 'opentype.js';
import { colourTtf } from '../src/emoji/colourfont';
import { BY_CHAR } from '../src/emoji/manifest';
import { COLOUR_ORDER, VCOLOURS } from '../src/emoji/vector/kit';
import { MONO_EDGE, monoOf, modernOtfWithEmoji, vectorFontSpec, vectorSet } from '../src/emoji/vector/font';
import { assembleModern, LINE } from '../src/modern/assemble';
import { buildModernOtf } from '../src/modern/files';
import { outline, contourArea } from '../src/modern/geom';
import { STYLES } from '../src/modern/kit';
import { cmapLookup, tables } from './sfnt';

const set = vectorSet();
const area = (s: Parameters<typeof outline>[0]): number => outline(s).reduce((a, c) => a + contourArea(c), 0);

describe('vector set', () => {
  test('every vector emoji is in the manifest, once', () => {
    expect(new Set(set.map((e) => e.cp)).size).toBe(set.length);
    for (const e of set) expect([e.ch, BY_CHAR.get(e.ch)?.cp]).toEqual([e.ch, e.cp]);
  });

  test("has Lt. Fizz's 24 and the shell's pictures", () => {
    const have = new Set(set.map((e) => e.ch));
    const want = '🙂😀😉😮😢😠😎❤⭐✔❌👍🍁👽🛸🥤☕💾🎮🔊🔇🔒⚡🏆💿📁📂📄📝📅⏰⌚📞💡🔑🔓🔔🎵🕹🖥⌨🖱🖨📷⚠⛔❓❗⬆⬇⬅➡▶⏸⏹🔴🟠🟡🟢🔵🟣✨🔥💧❄☀🌙☁🌈';
    for (const ch of want) expect([ch, have.has(ch)]).toEqual([ch, true]);
  });

  test('the palette is defined once, as opaque #rrggbb colours, and every layer uses it', () => {
    for (const c of COLOUR_ORDER) expect(VCOLOURS[c]).toMatch(/^#[0-9a-f]{6}$/);
    expect(COLOUR_ORDER.length).toBeLessThanOrEqual(24);
    for (const e of set) for (const l of e.layers) expect(COLOUR_ORDER).toContain(l.colour);
  });

  test('every layer has ink, inside the line, and every emoji has a monochrome glyph', () => {
    for (const e of set) {
      for (const l of e.layers) expect([e.ch, area(l.shape) > 0]).toEqual([e.ch, true]);
      expect([e.ch, area(e.mono) > 2000]).toEqual([e.ch, true]);
      for (const l of e.layers) {
        for (const p of [...l.shape.outer, ...l.shape.holes].flat()) {
          expect(p.y).toBeLessThanOrEqual(LINE.ascender);
          expect(p.y).toBeGreaterThanOrEqual(LINE.descender);
          expect(p.x).toBeGreaterThanOrEqual(0);
          expect(p.x).toBeLessThanOrEqual(1000);
        }
      }
    }
  });

  test('the monochrome rule: a light layer is paper with an ink edge, a dark one ink, a hole cuts', () => {
    const face = set.find((e) => e.ch === '🙂');
    const heart = set.find((e) => e.ch === '❤');
    const key = set.find((e) => e.ch === '⌨');
    if (!face || !heart || !key) throw new Error('missing');
    // The face is its outline, eyes and mouth: much less ink than the disc.
    expect(area(face.mono)).toBeLessThan(area(face.layers[0]?.shape ?? face.mono) * 0.5);
    // The heart is solid.
    expect(Math.abs(area(heart.mono) - area(heart.layers[0]?.shape ?? heart.mono))).toBeLessThan(area(heart.mono) * 0.05);
    // The keyboard's keys are cut out of its body.
    expect(area(key.mono)).toBeLessThan(area(key.layers[0]?.shape ?? key.mono) * 0.85);
    // A heavier edge gives more ink.
    expect(area(monoOf(face.layers, 120))).toBeGreaterThan(area(monoOf(face.layers, MONO_EDGE)));
  });
});

describe('Desk Emoji', () => {
  const f = colourTtf(vectorFontSpec(set));

  test('COLR version 0 and a CPAL of the vector palette', () => {
    const { dv, get } = tables(f);
    const colr = get('COLR').offset;
    expect(dv.getUint16(colr)).toBe(0);
    expect(dv.getUint16(colr + 2)).toBe(set.length);
    const c = get('CPAL').offset;
    expect(dv.getUint16(c + 2)).toBe(COLOUR_ORDER.length);
    const rec = c + dv.getUint32(c + 8);
    COLOUR_ORDER.forEach((name, i) => {
      const hex = `#${[2, 1, 0].map((k) => dv.getUint8(rec + 4 * i + k).toString(16).padStart(2, '0')).join('')}`;
      expect(hex).toBe(VCOLOURS[name]);
    });
  });

  test('every emoji is reached and has its layers, with the name and line of the family', () => {
    const font = ot.parse(f.slice().buffer);
    const family = (font.names as unknown as Record<string, Record<string, Record<string, string>>>).windows?.fontFamily?.en;
    expect(family).toBe('Desk Emoji');
    expect(font.unitsPerEm).toBe(1000);
    const { dv, get } = tables(f);
    const colr = get('COLR').offset;
    const base = colr + dv.getUint32(colr + 4);
    const records = new Map<number, number>();
    for (let i = 0; i < set.length; i++) records.set(dv.getUint16(base + 6 * i), dv.getUint16(base + 6 * i + 4));
    for (const e of set) {
      const gid = font.charToGlyphIndex(e.ch);
      expect([e.ch, records.get(gid)]).toEqual([e.ch, e.layers.length]);
    }
  });
});

describe('vector emoji in the outline faces', () => {
  for (const s of STYLES) {
    test(`${s.file}.otf: every emoji through format 12, the face's own glyphs unchanged`, () => {
      const glyphs = assembleModern(s);
      const with_ = modernOtfWithEmoji(s, glyphs, set);
      const font = ot.parse(with_.slice().buffer);
      const plain = ot.parse(buildModernOtf(s, glyphs).slice().buffer);
      for (const g of glyphs) {
        const a = font.charToGlyph(g.ch);
        const b = plain.charToGlyph(g.ch);
        expect([g.ch, a.advanceWidth, a.path.toPathData(0)]).toEqual([g.ch, b.advanceWidth, b.path.toPathData(0)]);
      }
      const bmp = cmapLookup(with_, 3, 1).glyph;
      for (const e of set) {
        const g = font.charToGlyph(e.ch);
        expect([e.ch, g.index > 0, g.advanceWidth]).toEqual([e.ch, true, s.advance ? 2 * s.advance : 1000]);
        if (e.cp <= 0xffff) expect([e.ch, bmp(e.cp)]).toEqual([e.ch, g.index]);
      }
    });
  }
});
