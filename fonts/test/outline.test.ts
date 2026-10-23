// Tests for the pixel-edge tracer: merged shapes, holes, corner-touching pixels and straight runs.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import { parse } from '../src/grid';
import { area, trace } from '../src/outline';

const total = (rows: string): number => trace(parse(rows), 10).reduce((a, c) => a + area(c), 0);

describe('trace', () => {
  test('a solid block is one counter-clockwise rectangle of four points', () => {
    const cs = trace(parse('###/###'), 2);
    expect(cs).toHaveLength(1);
    expect(cs[0]).toHaveLength(4);
    expect(area(cs[0] as never)).toBe(6);
  });

  test('neighbouring pixels merge and straight runs lose their middle points', () => {
    const cs = trace(parse('#####'), 1);
    expect(cs).toEqual([
      [
        { x: 0, y: 0 },
        { x: 5, y: 0 },
        { x: 5, y: 1 },
        { x: 0, y: 1 },
      ],
    ]);
  });

  test('a ring is an outer contour and a hole wound the other way', () => {
    const cs = trace(parse('###/#.#/###'), 3);
    expect(cs).toHaveLength(2);
    const areas = cs.map(area).sort((a, b) => a - b);
    expect(areas).toEqual([-1, 9]);
  });

  test('pixels that touch only at a corner stay separate contours', () => {
    const cs = trace(parse('#./.#'), 2);
    expect(cs).toHaveLength(2);
    for (const c of cs) {
      expect(c).toHaveLength(4);
      expect(area(c)).toBe(1);
    }
  });

  test('a hole touching the outside at a corner keeps the area exact', () => {
    expect(total('###./#.#./###.')).toBe(8);
    expect(total('##../#.#./.###')).toBe(7);
    expect(total('.#./#.#/.#.')).toBe(4);
  });

  test('the area always equals the ink, for every pattern of a 3 × 3 grid', () => {
    for (let bits = 0; bits < 512; bits++) {
      const rows = [0, 1, 2]
        .map((y) => [0, 1, 2].map((x) => ((bits >> (y * 3 + x)) & 1 ? '#' : '.')).join(''))
        .join('/');
      let ink = 0;
      for (let b = bits; b; b >>= 1) ink += b & 1;
      expect(total(rows)).toBe(ink);
    }
  });
});
