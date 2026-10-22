// Tests for the Round style: valid contours, winding, ink only ever removed and never added outside the pixels.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import { assemble } from '../src/face';
import { FACES } from '../src/faces';
import { ink, parse, type Grid } from '../src/grid';
import { contourArea, PIXEL, roundOutline, segments, type OPt } from '../src/round';

/** Points along every segment, in pixel units with y up. */
function samples(contours: OPt[][], steps = 12): { x: number; y: number }[] {
  const out: { x: number; y: number }[] = [];
  for (const c of contours)
    for (const s of segments(c))
      for (let i = 0; i <= steps; i++) {
        const t = i / steps;
        const u = 1 - t;
        const [p, k, q] = s as [OPt, OPt, OPt?];
        const pt = q
          ? { x: u * u * p.x + 2 * u * t * k.x + t * t * q.x, y: u * u * p.y + 2 * u * t * k.y + t * t * q.y }
          : { x: u * p.x + t * k.x, y: u * p.y + t * k.y };
        out.push({ x: pt.x / PIXEL, y: pt.y / PIXEL });
      }
  return out;
}

/** Whether a point (pixel units, y up from the baseline) lies in or on an inked pixel. */
const inInk = (rows: Grid, ascent: number, p: { x: number; y: number }): boolean => {
  const e = 1e-9;
  for (const cx of [Math.floor(p.x - e), Math.floor(p.x + e)])
    for (const cy of [Math.floor(ascent - p.y - e), Math.floor(ascent - p.y + e)]) if (rows[cy]?.[cx]) return true;
  return false;
};

/** Non-zero winding of a point against the contours, with curves flattened finely. */
function winding(contours: OPt[][], x: number, y: number): number {
  let w = 0;
  for (const c of contours) {
    const pts = samples([c], 24);
    for (let i = 0; i + 1 < pts.length; i++) {
      const a = pts[i] as { x: number; y: number };
      const b = pts[i + 1] as { x: number; y: number };
      if (a.y <= y && b.y > y && (b.x - a.x) * (y - a.y) - (x - a.x) * (b.y - a.y) > 0) w++;
      else if (a.y > y && b.y <= y && (b.x - a.x) * (y - a.y) - (x - a.x) * (b.y - a.y) < 0) w--;
    }
  }
  return w;
}

describe('roundOutline', () => {
  test('a lone pixel becomes a dot: four corner curves joined by short flats', () => {
    const [c] = roundOutline(parse('#'), 1);
    expect(c).toHaveLength(12);
    expect((c as OPt[]).filter((p) => !p.on).map((p) => [p.x, p.y])).toEqual([
      [0, 0],
      [100, 0],
      [100, 100],
      [0, 100],
    ]);
    // Each corner loses r² / 6 of a pixel: the triangle minus the two thirds of it the curve keeps.
    expect(contourArea(c as OPt[])).toBeCloseTo(PIXEL * PIXEL * (1 - (4 * 0.45 * 0.45) / 6), 6);
  });

  test('concave corners stay sharp and a one-pixel end gets a round cap', () => {
    const [c] = roundOutline(parse('#./##'), 2);
    const sharp = (c as OPt[]).filter((p) => p.on && p.x === 100 && p.y === 100);
    expect(sharp).toHaveLength(1);
    // The top end of the stem: two curves with a sliver of flat edge between them.
    expect((c as OPt[]).filter((p) => p.on && p.y === 200).map((p) => p.x).sort()).toEqual([45, 55]);
  });

  test('holes keep their corners and their winding', () => {
    const cs = roundOutline(parse('###/#.#/###'), 3);
    const areas = cs.map(contourArea).sort((a, b) => a - b);
    expect(areas[0]).toBe(-PIXEL * PIXEL);
    expect(areas[1]).toBeLessThan(9 * PIXEL * PIXEL);
  });
});

for (const face of FACES) {
  describe(`${face.family} Round`, () => {
    const glyphs = assemble(face);
    test('every contour alternates correctly, winds like the square one, and stays inside the pixels', () => {
      for (const g of glyphs) {
        const cs = roundOutline(g.rows, face.ascent);
        for (const c of cs) {
          for (let i = 0; i < c.length; i++) {
            const a = c[i] as OPt;
            const b = c[(i + 1) % c.length] as OPt;
            if (!a.on && !b.on) throw new Error(`${face.id} ${g.ch}: two control points in a row`);
            if (a.on && b.on && a.x === b.x && a.y === b.y) throw new Error(`${face.id} ${g.ch}: repeated point`);
          }
        }
        for (const p of samples(cs)) {
          if (!inInk(g.rows, face.ascent, p)) throw new Error(`${face.id} ${g.ch}: outline at ${p.x},${p.y} leaves the pixels`);
        }
      }
    });

    test('area is at most the square ink and at least 80 per cent of it', () => {
      for (const g of glyphs) {
        const square = ink(g.rows) * PIXEL * PIXEL;
        const round = roundOutline(g.rows, face.ascent).reduce((s, c) => s + contourArea(c), 0);
        if (round > square + 1e-6 || round < 0.8 * square)
          throw new Error(`${face.id} ${g.ch}: round area ${round} against square ${square}`);
      }
    });

    test('no point outside the inked pixels is filled (supersampled)', () => {
      for (const g of glyphs.filter((x) => 'Aagé@%&0ß½▒╬'.includes(x.ch))) {
        const cs = roundOutline(g.rows, face.ascent);
        for (let y = 0; y < face.height; y++)
          for (let x = 0; x < g.adv; x++)
            for (const [dx, dy] of [
              [0.1, 0.1],
              [0.9, 0.1],
              [0.5, 0.5],
              [0.1, 0.9],
              [0.9, 0.9],
            ] as const) {
              const filled = winding(cs, x + dx, face.ascent - y - dy) !== 0;
              if (filled && !g.rows[y]?.[x]) throw new Error(`${face.id} ${g.ch}: filled outside ink at ${x},${y}`);
              if (dx === 0.5 && !filled && g.rows[y]?.[x]) throw new Error(`${face.id} ${g.ch}: pixel centre ${x},${y} lost`);
            }
      }
    });
  });
}
