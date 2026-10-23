// Tests for the emoji set and the pixel art: the manifest against Unicode, the palette against docs/PALETTE.md, the
// grids and tiers, the monochrome rule and the 12 × 12 derivation.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { derive12, dropRows, trimRows } from '../src/emoji/derive';
import { parseArt, silhouette, slotsUsed } from '../src/emoji/grid';
import { CATEGORIES, MANIFEST } from '../src/emoji/manifest';
import { deriveMono } from '../src/emoji/mono';
import { LIGHT, SHADE, SLOTS, luminance } from '../src/emoji/palette';
import { ART12, ART16, ART8, FIX12, MONO_HAND, MONO_RULES } from '../src/emoji/pixel';
import { pixelSet, sizesFor } from '../src/emoji/pixels';
import { isEmoji } from './emoji-unicode';

const hex = (cp: number): string => `U+${cp.toString(16).toUpperCase()}`;
const set = pixelSet();

describe('manifest', () => {
  test('has more than 300 emoji with unique code points', () => {
    expect(MANIFEST.length).toBeGreaterThan(300);
    expect(new Set(MANIFEST.map((e) => e.cp)).size).toBe(MANIFEST.length);
  });

  test('every entry is one code point with the Unicode Emoji property', () => {
    for (const e of MANIFEST) {
      expect([hex(e.cp), Array.from(e.ch).length, e.ch.codePointAt(0)]).toEqual([hex(e.cp), 1, e.cp]);
      if (!isEmoji(e.cp)) throw new Error(`${hex(e.cp)} ${e.name} is not an emoji in Unicode 17.0`);
    }
  });

  test('no sequences, modifiers, keycap bases, regional indicators or tag characters', () => {
    for (const e of MANIFEST) {
      const cp = e.cp;
      const excluded =
        (cp >= 0x1f3fb && cp <= 0x1f3ff) || // skin tones
        (cp >= 0x1f1e6 && cp <= 0x1f1ff) || // regional indicators
        (cp >= 0x30 && cp <= 0x39) || cp === 0x23 || cp === 0x2a || // keycap bases
        cp === 0x200d || cp === 0xfe0f || (cp >= 0xe0020 && cp <= 0xe007f);
      expect([hex(cp), excluded]).toEqual([hex(cp), false]);
    }
  });

  test('names are CLDR short names in lowercase and categories are known', () => {
    for (const e of MANIFEST) {
      expect(e.name).toMatch(/^[a-z0-9][a-z0-9 :’'.&!-]*$/i);
      expect(Object.keys(CATEGORIES)).toContain(e.category);
    }
  });

  test("has Lt. Fizz's 24 pictures (✓ and ✗ as their emoji forms ✔ and ❌) and the shell's own set", () => {
    const fizz = '🙂😀😉😮😢😠😎❤⭐✔❌👍🍁👽🛸🥤☕💾🎮🔊🔇🔒⚡🏆';
    const shell = '💾💿📁📂📄📝📅⏰⌚📞💡🔑🔒🔓🔔🔇🔊🎵🎮🕹🖥⌨🖱🖨📷✔❌⚠⛔❓❗⬆⬇⬅➡▶⏸⏹🔴🟠🟡🟢🔵🟣⭐✨🔥💧⚡❄☀🌙☁🌈';
    const have = new Map(MANIFEST.map((e) => [e.ch, e]));
    for (const ch of [...fizz, ...shell]) {
      const e = have.get(ch);
      expect([ch, e ? e.tier : 'missing']).not.toEqual([ch, 'missing']);
      expect([ch, e?.tier === 'extended']).toEqual([ch, false]);
    }
    expect(Array.from(fizz)).toHaveLength(24);
  });

  test('about 40 core and 80 hand-drawn 12 × 12 emoji', () => {
    const core = MANIFEST.filter((e) => e.tier === 'core').length;
    const essential = MANIFEST.filter((e) => e.tier === 'essential').length;
    expect(core).toBeGreaterThanOrEqual(36);
    expect(core + essential).toBeGreaterThanOrEqual(76);
  });
});

describe('palette', () => {
  test('is the 16-colour set of docs/PALETTE.md, slot for slot', () => {
    const doc = readFileSync(join(import.meta.dir, '..', '..', 'docs', 'PALETTE.md'), 'utf8');
    // Only this section: later sections hold other slot tables (the slots the first window uses).
    const start = doc.indexOf('## The 16-colour set');
    const end = doc.indexOf('\n## ', start + 1);
    const section = doc.slice(start, end === -1 ? undefined : end);
    const rows = [...section.matchAll(/^\| (\d+) \| `(#[0-9a-f]{6})` \|/gm)].map((m) => [Number(m[1]), m[2]]);
    expect(rows).toEqual(SLOTS.map((s) => [s.slot, s.hex]));
  });

  test('letters are single, unique and not the transparent dot', () => {
    expect(new Set(SLOTS.map((s) => s.letter)).size).toBe(16);
    for (const s of SLOTS) expect(s.letter).toMatch(/^[a-z]$/);
  });

  test('the light slots are white, light grey, amber and cyan, and each shade is darker than its light', () => {
    expect([...LIGHT].sort((a, b) => a - b)).toEqual([6, 7, 8, 13]);
    for (const [light, shade] of SHADE) expect(luminance(shade)).toBeLessThan(luminance(light));
  });
});

describe('pixel art', () => {
  test('every emoji has the hand-drawn sizes its tier needs, and nothing else is drawn', () => {
    const byCh = new Map(MANIFEST.map((e) => [e.ch, e]));
    for (const e of MANIFEST) {
      expect([e.ch, !!ART16[e.ch]]).toEqual([e.ch, true]);
      expect([e.ch, !!ART12[e.ch]]).toEqual([e.ch, e.tier !== 'extended']);
      expect([e.ch, !!ART8[e.ch]]).toEqual([e.ch, e.tier === 'core']);
    }
    for (const ch of [...Object.keys(ART16), ...Object.keys(FIX12), ...Object.keys(MONO_RULES), ...Object.keys(MONO_HAND)]) {
      expect([ch, byCh.has(ch)]).toEqual([ch, true]);
    }
    // A hand fix replaces a derived grid, so only extended emoji have one.
    for (const ch of Object.keys(FIX12)) expect([ch, byCh.get(ch)?.tier]).toEqual([ch, 'extended']);
  });

  test('every grid is square at its size and uses only the 16 slots', () => {
    for (const e of set) {
      for (const size of sizesFor(e)) {
        const d = e.sizes[size];
        if (!d) throw new Error(`${e.ch} has no ${size}`);
        expect(d.colour).toHaveLength(size);
        for (const r of d.colour) {
          expect(r).toHaveLength(size);
          for (const v of r) expect(v >= -1 && v <= 15).toBe(true);
        }
        expect(d.mono).toHaveLength(size);
      }
    }
  });

  test('the parser rejects letters outside the palette and grids of the wrong size', () => {
    expect(() => parseArt('ky/', 2, 't')).toThrow();
    expect(() => parseArt('kk\nky', 2, 't')).toThrow(/unknown letter 'y'/);
    expect(() => parseArt('kk\nkkk', 2, 't')).toThrow(/wide/);
    expect(parseArt('\n  k.\n  .w\n', 2, 't')).toEqual([[0, -1], [-1, 7]]);
  });

  test('no art file draws an emoji twice in one table (an object literal keeps only the last)', () => {
    const dir = join(import.meta.dir, '..', 'src', 'emoji', 'pixel');
    for (const f of readdirSync(dir).filter((n) => n.endsWith('.ts'))) {
      const src = readFileSync(join(dir, f), 'utf8');
      for (const m of src.matchAll(/^export const (\w+)[^=]*= \{\n([\s\S]*?)^\};/gm)) {
        const keys = [...(m[2] as string).matchAll(/^ {2}'([^']+)':/gm)].map((k) => k[1]);
        expect([f, m[1], keys.filter((k, i) => keys.indexOf(k) !== i)]).toEqual([f, m[1], []]);
      }
    }
  });

  test('16 × 16 drawings keep the one-pixel margin on at least two opposite sides', () => {
    const tight: string[] = [];
    for (const e of set) {
      const g = e.sizes[16]?.colour as number[][];
      const rowClear = (y: number): boolean => (g[y] as number[]).every((v) => v < 0);
      const colClear = (x: number): boolean => g.every((r) => (r[x] as number) < 0);
      if (!(rowClear(0) && rowClear(15)) && !(colClear(0) && colClear(15))) tight.push(e.ch);
    }
    expect(tight).toEqual([]);
  });
});

describe('monochrome rule', () => {
  const P = (s: string): number[][] => parseArt(s, s.trim().split('\n').length, 'test');

  test('a light fill inside an outline is paper; the outline and dark detail are ink', () => {
    const g = P(`
      kkkkk
      kookk
      kowok
      kookk
      kkkkk`);
    expect(deriveMono(g)).toEqual([
      [1, 1, 1, 1, 1],
      [1, 0, 0, 1, 1],
      [1, 0, 0, 0, 1],
      [1, 0, 0, 1, 1],
      [1, 1, 1, 1, 1],
    ]);
  });

  test('a light shape with no outline keeps its edge, and a dark shape stays solid', () => {
    expect(deriveMono(P('ooo\nooo\nooo'))).toEqual([[1, 1, 1], [1, 0, 1], [1, 1, 1]]);
    expect(deriveMono(P('rrr\nrrr\nrrr'))).toEqual([[1, 1, 1], [1, 1, 1], [1, 1, 1]]);
    expect(deriveMono(P('...\n.o.\n...'))).toEqual([[0, 0, 0], [0, 1, 0], [0, 0, 0]]);
  });

  test('a shade next to its light colour is fill; away from it, ink', () => {
    expect(deriveMono(P('kkkk\nkonk\nkkkk\nnnnn'))).toEqual([[1, 1, 1, 1], [1, 0, 0, 1], [1, 1, 1, 1], [1, 1, 1, 1]]);
    expect(deriveMono(P('kkkk\nkrnk\nkkkk\nkkkk'))).toEqual([[1, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 1]]);
  });

  test('keep and drop move slots between ink and fill', () => {
    const g = P('kkk\nkok\nkkk');
    expect(deriveMono(g, { keep: [8] })[1]).toEqual([1, 1, 1]);
    expect(deriveMono(P('kkk\nkrk\nkkk'), { drop: [11] })[1]).toEqual([1, 0, 1]);
    // Keeping a light slot keeps its shade too: mid grey next to light grey stays ink.
    expect(deriveMono(P('kkkk\nksmk\nkmmk\nkkkk'), { keep: [6] })[1]).toEqual([1, 1, 1, 1]);
    expect(deriveMono(P('kkkk\nksmk\nkmmk\nkkkk'))[1]).toEqual([1, 0, 0, 1]);
  });

  test('every derived mono glyph is inside the silhouette and not empty', () => {
    for (const e of set) {
      for (const d of Object.values(e.sizes)) {
        const sil = silhouette(d.colour);
        const ink = d.mono.flat().filter((v) => v).length;
        expect([e.ch, d.size, ink > 0]).toEqual([e.ch, d.size, true]);
        if (d.monoDrawn) continue;
        d.mono.forEach((r, y) => r.forEach((v, x) => v && expect(sil[y]?.[x]).toBe(1)));
      }
    }
  });

  test('a face keeps its features: the mono glyph of 🙂 has eyes and a mouth inside the outline', () => {
    const m = set.find((e) => e.ch === '🙂')?.sizes[16]?.mono as number[][];
    const inner = m.slice(4, 12).map((r) => r.slice(4, 12));
    expect(inner.flat().filter((v) => v).length).toBeGreaterThanOrEqual(8);
    expect(inner.flat().filter((v) => !v).length).toBeGreaterThanOrEqual(30);
  });
});

describe('12 × 12 derivation', () => {
  test('removes repeated rows first and keeps one-pixel lines', () => {
    const g = [[0], [1], [1], [1], [-1], [2], [2]].map((r) => r.map((v) => v as number));
    expect(dropRows(g, 2)).toEqual([[0], [1], [-1], [2], [2]]);
    expect(dropRows(g, 3).map((r) => r[0])).toEqual([0, 1, -1, 2]);
  });

  test('removes the blank margin before anything inside the drawing, keeping it centred', () => {
    const g = [[-1], [-1], [-1], [0], [1], [1], [0], [-1]].map((r) => r.map((v) => v as number));
    expect(trimRows(g, 3).map((r) => r[0])).toEqual([0, 1, 1, 0, -1]);
    expect(trimRows(g, 5).map((r) => r[0])).toEqual([0, 1, 0]);
    // A derived drawing loses rows or columns of ink only once its blank margin is used up, so it then
    // reaches all four edges of the 12 × 12.
    const used = (g: number[][]): number => g.filter((r) => r.some((v) => v >= 0)).length;
    const t = (g: number[][]): number[][] => (g[0] ?? []).map((_, x) => g.map((r) => r[x] as number));
    const reaches = (g: number[][]): boolean => (g[0] as number[]).some((v) => v >= 0) && (g[g.length - 1] as number[]).some((v) => v >= 0);
    for (const e of set) {
      const g16 = e.sizes[16]?.colour as number[][];
      for (const [way, g] of [['rows', g16], ['columns', t(g16)]] as const) {
        const cut = trimRows(g, 4);
        if (used(cut) < used(g)) expect([e.ch, way, reaches(cut)]).toEqual([e.ch, way, true]);
      }
    }
  });

  test('gives 12 × 12 grids whose colours all come from the 16 × 16', () => {
    for (const e of set) {
      const d = e.sizes[12];
      if (d?.origin !== 'derived') continue;
      const from = new Set(slotsUsed(e.sizes[16]?.colour as number[][]));
      expect(d.colour).toHaveLength(12);
      for (const s of slotsUsed(d.colour)) expect(from.has(s)).toBe(true);
    }
  });

  test('a symmetric disc stays symmetric', () => {
    const disc = Array.from({ length: 16 }, (_, y) =>
      Array.from({ length: 16 }, (_, x) => ((x - 7.5) ** 2 + (y - 7.5) ** 2 <= 49 ? ((x - 7.5) ** 2 + (y - 7.5) ** 2 > 36 ? 0 : 8) : -1)),
    );
    const d = derive12(disc);
    expect(d.map((r) => [...r].reverse())).toEqual(d);
    expect([...d].reverse()).toEqual(d);
  });

  test('every emoji has its 12 × 12 drawn, derived or fixed as its tier says', () => {
    for (const e of set) {
      const origin = e.sizes[12]?.origin;
      if (e.tier === 'extended') expect([e.ch, origin === 'derived' || origin === 'fixed']).toEqual([e.ch, true]);
      else expect([e.ch, origin]).toEqual([e.ch, 'drawn']);
    }
  });
});
