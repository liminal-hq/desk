// Tests for the outline faces: coverage, clean contours (closed, wound, unioned, points at extrema), bounds, advances,
// spacing rhythm, and OTF and TTF files with the same shapes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import * as ot from 'opentype.js';
import { BLOCKS, BOX, REQUIRED } from '../src/charset';
import { assembleModern, LINE, type MGlyph } from '../src/modern/assemble';
import { buildModernOtf, buildModernTtf, modernGlyphName } from '../src/modern/files';
import { type Contour, contourArea, extremaParams, type P, sampleContour } from '../src/modern/geom';
import { STYLES } from '../src/modern/kit';
import { cmapLookup, pathArea } from './sfnt';

const hex = (ch: string): string => `U+${(ch.codePointAt(0) as number).toString(16).toUpperCase().padStart(4, '0')}`;

/** Whether two segments properly cross (touching at shared end points does not count). */
function crosses(a: P, b: P, c: P, d: P): boolean {
  const o = (p: P, q: P, r: P): number => (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
  const d1 = o(c, d, a);
  const d2 = o(c, d, b);
  const d3 = o(a, b, c);
  const d4 = o(a, b, d);
  return ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0));
}

/** Winding number of a point against a closed polyline. */
function windingOf(poly: P[], p: P): number {
  let w = 0;
  for (let i = 0; i < poly.length; i++) {
    const a = poly[i] as P;
    const b = poly[(i + 1) % poly.length] as P;
    if (a.y <= p.y && b.y > p.y && (b.x - a.x) * (p.y - a.y) - (p.x - a.x) * (b.y - a.y) > 0) w++;
    else if (a.y > p.y && b.y <= p.y && (b.x - a.x) * (p.y - a.y) - (p.x - a.x) * (b.y - a.y) < 0) w--;
  }
  return w;
}

/** A point just inside a contour, on the ink side (the left) of its longest sampled edge. */
function insidePoint(c: Contour): P {
  const pts = sampleContour(c, 64);
  let best = 0;
  let at = 0;
  for (let i = 0; i + 1 < pts.length; i++) {
    const l = Math.hypot((pts[i + 1] as P).x - (pts[i] as P).x, (pts[i + 1] as P).y - (pts[i] as P).y);
    if (l > best) {
      best = l;
      at = i;
    }
  }
  const a = pts[at] as P;
  const b = pts[at + 1] as P;
  return { x: (a.x + b.x) / 2 - ((b.y - a.y) / best) * 0.5, y: (a.y + b.y) / 2 + ((b.x - a.x) / best) * 0.5 };
}

for (const s of STYLES) {
  const name = `${s.family}${s.bold ? ' Bold' : ''}${s.italic ? ' Italic' : ''}`;
  describe(name, () => {
    const glyphs = assembleModern(s);
    // The encoded glyphs by character; ligatures and alternates (no code point) are reached by name.
    const by = new Map(glyphs.filter((g) => g.cp >= 0).map((g) => [g.ch, g]));

    test('covers code page 1252, French, the shell symbols and (Mono) box drawing and blocks', () => {
      const want = s.kind === 'mono' ? REQUIRED : REQUIRED.filter((c) => !BOX.includes(c) && !BLOCKS.includes(c));
      expect(want.filter((c) => !by.has(c)).map(hex)).toEqual([]);
    });

    test('every contour is closed, has no repeated points and no slivers', () => {
      for (const g of glyphs) {
        for (const c of g.contours) {
          const last = c.segs[c.segs.length - 1] as { p: P };
          if (last.p.x !== c.start.x || last.p.y !== c.start.y) throw new Error(`${g.ch}: contour not closed`);
          let prev = c.start;
          for (const seg of c.segs) {
            if (seg.p.x === prev.x && seg.p.y === prev.y) throw new Error(`${g.ch}: repeated point`);
            prev = seg.p;
          }
          if (Math.abs(contourArea(c)) < 150) throw new Error(`${g.ch}: sliver contour of area ${contourArea(c)}`);
        }
      }
    });

    test('outer contours run counter-clockwise and holes clockwise, nested as they should be', () => {
      for (const g of glyphs) {
        const polys = g.contours.map((c) => sampleContour(c, 64));
        g.contours.forEach((c, i) => {
          const p = insidePoint(c);
          // The ink side of every edge is filled exactly once.
          const total = polys.reduce((w, poly) => w + windingOf(poly, p), 0);
          if (total !== 1) throw new Error(`${g.ch}: contour ${i} has winding ${total} on its ink side`);
        });
        const area = g.contours.reduce((a, c) => a + contourArea(c), 0);
        if (g.contours.length && area <= 0) throw new Error(`${g.ch}: negative total area`);
      }
    });

    test('contours do not overlap or cross themselves or each other', () => {
      for (const g of glyphs) {
        const segs: [P, P, number, number, number][] = [];
        g.contours.forEach((c, ci) => {
          const pts = sampleContour(c, 6);
          for (let i = 0; i + 1 < pts.length; i++) segs.push([pts[i] as P, pts[i + 1] as P, ci, i, pts.length - 1]);
        });
        for (let i = 0; i < segs.length; i++) {
          const [a, b, ca, ia, na] = segs[i] as [P, P, number, number, number];
          for (let j = i + 1; j < segs.length; j++) {
            const [c, d, cb, ib] = segs[j] as [P, P, number, number, number];
            // Neighbouring segments of one contour share an end point.
            if (ca === cb && (ib === ia + 1 || (ia === 0 && ib === na - 1))) continue;
            if (Math.max(a.x, b.x) < Math.min(c.x, d.x) || Math.max(c.x, d.x) < Math.min(a.x, b.x)) continue;
            if (Math.max(a.y, b.y) < Math.min(c.y, d.y) || Math.max(c.y, d.y) < Math.min(a.y, b.y)) continue;
            if (crosses(a, b, c, d)) throw new Error(`${g.ch}: outline crosses itself near ${Math.round(a.x)},${Math.round(a.y)}`);
          }
        }
      }
    });

    test('curves have points at their extrema', () => {
      for (const g of glyphs) {
        for (const c of g.contours) {
          let p0 = c.start;
          for (const seg of c.segs) {
            if (seg.t === 'C') {
              for (const t of extremaParams(p0, seg)) {
                const u = 1 - t;
                const x = u * u * u * p0.x + 3 * u * u * t * seg.c1.x + 3 * u * t * t * seg.c2.x + t * t * t * seg.p.x;
                const y = u * u * u * p0.y + 3 * u * u * t * seg.c1.y + 3 * u * t * t * seg.c2.y + t * t * t * seg.p.y;
                const beyond = Math.max(
                  Math.min(p0.x, seg.p.x) - x,
                  x - Math.max(p0.x, seg.p.x),
                  Math.min(p0.y, seg.p.y) - y,
                  y - Math.max(p0.y, seg.p.y),
                );
                if (beyond > 1) throw new Error(`${g.ch}: a curve bulges ${beyond.toFixed(1)} units past its end points`);
              }
            }
            p0 = seg.p;
          }
        }
      }
    });

    test('every glyph stays inside the line: ascender 950, descender -250', () => {
      for (const g of glyphs) {
        if (!g.box) continue;
        if (g.box.y1 > LINE.ascender || g.box.y0 < LINE.descender) throw new Error(`${g.ch}: reaches ${g.box.y0} to ${g.box.y1}`);
      }
    });

    test('figures are tabular, and Mono has one advance', () => {
      const figs = new Set(Array.from('0123456789', (c) => by.get(c)?.adv));
      expect(figs.size).toBe(1);
      if (s.advance) expect(glyphs.filter((g) => g.adv !== s.advance).map((g) => g.ch)).toEqual([]);
    });

    // Italics are measured upright before they slant, so their slanted boxes do not show the classes.
    if (!s.advance && !s.italic) {
      test('side bearings are consistent within each class of side', () => {
        const lsb = (g: MGlyph): number => (g.box as NonNullable<MGlyph['box']>).x0;
        const straight = Array.from('bhklmnpr').map((c) => lsb(by.get(c) as MGlyph));
        const round = Array.from('cdeoq').map((c) => lsb(by.get(c) as MGlyph));
        const capsStraight = Array.from('BDEFHKLNPR').map((c) => lsb(by.get(c) as MGlyph));
        for (const set of [straight, round, capsStraight]) expect(Math.max(...set) - Math.min(...set)).toBeLessThanOrEqual(s.kind === 'serif' ? 60 : 2);
        expect(Math.min(...round)).toBeLessThan(Math.min(...straight));
      });
    }

    test('the OTF and TTF files have the same glyphs, advances and areas', () => {
      const otf = ot.parse(buildModernOtf(s, glyphs).slice().buffer);
      const ttfBytes = buildModernTtf(s, glyphs);
      const ttf = ot.parse(ttfBytes.slice().buffer);
      const win = cmapLookup(ttfBytes, 3, 1);
      const named = (f: ot.Font): Map<string, ot.Glyph> => {
        const m = new Map<string, ot.Glyph>();
        for (let i = 0; i < f.glyphs.length; i++) m.set(f.glyphs.get(i).name as string, f.glyphs.get(i));
        return m;
      };
      const otfNamed = named(otf);
      const ttfNamed = named(ttf);
      for (const g of glyphs) {
        const a = g.cp >= 0 ? otf.charToGlyph(g.ch) : (otfNamed.get(modernGlyphName(g)) as ot.Glyph);
        expect(a.index).toBeGreaterThan(0);
        expect(a.advanceWidth).toBe(g.adv);
        if (g.cp > 0xffff) continue;
        const b = g.cp >= 0 ? ttf.glyphs.get(win.glyph(g.cp)) : (ttfNamed.get(modernGlyphName(g)) as ot.Glyph);
        expect(b.advanceWidth).toBe(g.adv);
        const area = g.contours.reduce((sum, c) => sum + contourArea(c), 0);
        const ca = pathArea(a.path);
        const ta = -pathArea(b.path);
        if (Math.abs(ca - area) > 1) throw new Error(`${g.ch}: OTF area ${ca} against ${area}`);
        // Quadratics stay within a quarter unit of the cubics and points round to whole units: allow half a unit along the outline.
        const perimeter = g.contours.reduce((sum, c) => {
          const pts = sampleContour(c, 8);
          return sum + pts.reduce((l, p, i) => l + Math.hypot(p.x - (pts[(i + 1) % pts.length] as P).x, p.y - (pts[(i + 1) % pts.length] as P).y), 0);
        }, 0);
        if (Math.abs(ta - area) > perimeter * 0.5) throw new Error(`${g.ch}: TTF area ${ta} against ${area}`);
      }
    });

    test('the build is reproducible', () => {
      expect(buildModernTtf(s, assembleModern(s))).toEqual(buildModernTtf(s, glyphs));
    });
  });
}

describe('italic names and flags', () => {
  for (const s of STYLES) {
    test(`${s.file}: family, style, fsSelection, macStyle, italic angle and caret agree in OTF and TTF`, () => {
      const glyphs = assembleModern(s);
      for (const bytes of [buildModernOtf(s, glyphs), buildModernTtf(s, glyphs)]) {
        const f = ot.parse(bytes.slice().buffer);
        const t = f.tables as Record<string, Record<string, number>>;
        const names = (f.names as Record<string, Record<string, { en: string }>>).windows as Record<string, { en: string }>;
        const style = s.italic ? (s.bold ? 'Bold Italic' : 'Italic') : s.bold ? 'Bold' : 'Regular';
        expect(names.fontFamily?.en).toBe(s.family);
        expect(names.fontSubfamily?.en).toBe(style);
        expect(names.fullName?.en).toBe(style === 'Regular' ? s.family : `${s.family} ${style}`);
        const fs = t.os2?.fsSelection as number;
        expect([!!(fs & 1), !!(fs & 0x20), !!(fs & 0x40)]).toEqual([!!s.italic, s.bold, !s.italic && !s.bold]);
        expect(t.os2?.usWeightClass).toBe(s.bold ? 700 : 400);
        expect(((t.head?.macStyle as number) & 3) >>> 0).toBe((s.bold ? 1 : 0) | (s.italic ? 2 : 0));
        expect(t.post?.italicAngle).toBeCloseTo(-(s.slant ?? 0), 2);
        expect(t.hhea?.caretSlopeRun).toBe(s.italic ? Math.round(1000 * Math.tan(((s.slant ?? 0) * Math.PI) / 180)) : 0);
      }
    });
  }
});

describe('italic joins', () => {
  // The exit strokes of Desk Serif Italic and the hooks of the italic f are separate parts that must overlap
  // their stems: a gap leaves the turn as a loose piece of ink under a hairline of white.
  const outers = (g: MGlyph): number => g.contours.filter((c) => contourArea(c) > 0).length;
  for (const s of STYLES.filter((x) => x.italic && x.kind !== 'sans')) {
    test(`${s.file}: exit strokes and hooks are one piece with their stems`, () => {
      const glyphs = assembleModern(s);
      const pieces: Record<string, number> = s.kind === 'serif' ? { a: 1, d: 1, f: 1, h: 1, i: 2, l: 1, m: 1, n: 1, t: 1, u: 1 } : { f: 1 };
      for (const [ch, n] of Object.entries(pieces)) expect([ch, outers(glyphs.find((g) => g.ch === ch) as MGlyph)]).toEqual([ch, n]);
    });
  }
});
