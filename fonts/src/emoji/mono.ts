// The monochrome form of a pixel emoji, derived from its colours: the glyph that goes into the Desk faces, the
// fallback glyph of the colour font and the mono bitmap of a .DEM file.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Pixels } from './grid';
import { LIGHT, SHADE } from './palette';

/** How one emoji departs from the rule. */
export interface MonoRule {
  /** Slots that are always ink in this emoji (inside the outline too). */
  keep?: number[];
  /** Slots that are paper in this emoji wherever they are not on the silhouette's edge. */
  drop?: number[];
}

const N4 = [
  [1, 0],
  [-1, 0],
  [0, 1],
  [0, -1],
] as const;
const N8 = [...N4, [1, 1], [1, -1], [-1, 1], [-1, -1]] as const;

/**
 * The monochrome rule. A coloured pixel is paper (not inked) when it is a fill and does not lie on the
 * silhouette's edge; every other coloured pixel is ink. A pixel is a fill when
 *
 * 1. its slot is light (`LIGHT`: white, light grey, amber, cyan, with a luminance of 0.45 or more), or
 * 2. its slot is the shade of a light slot (`SHADE`: brown for amber, light grey for white, mid grey for
 *    light grey) and one of its eight neighbours is that light slot, so the shading of a fill goes with it.
 *
 * A pixel is on the edge when one of its four neighbours is transparent or off the grid. So a light shape
 * drawn inside an outline (the amber of a face, the white of a page, the cyan of water) becomes paper and
 * the outline and the detail over it survive; a light shape with no outline keeps its own edge; dark and
 * saturated shapes (a red heart, a green leaf, a blue disc) stay solid. `rule` moves slots between the
 * classes for one emoji; a light slot it keeps as ink takes its shade with it.
 */
export function deriveMono(p: Pixels, rule: MonoRule = {}): number[][] {
  const at = (x: number, y: number): number => p[y]?.[x] ?? -1;
  const shadeOf = new Map([...SHADE].map(([light, shade]) => [shade, light]));
  const fill = (v: number, x: number, y: number): boolean => {
    if (rule.keep?.includes(v)) return false;
    if (rule.drop?.includes(v) || LIGHT.has(v)) return true;
    const partner = shadeOf.get(v);
    // A shade goes with its light slot, unless the rule keeps that slot as ink.
    return partner !== undefined && !rule.keep?.includes(partner) && N8.some(([dx, dy]) => at(x + dx, y + dy) === partner);
  };
  return p.map((row, y) =>
    row.map((v, x) => {
      if (v < 0) return 0;
      if (!fill(v, x, y)) return 1;
      return N4.some(([dx, dy]) => at(x + dx, y + dy) < 0) ? 1 : 0;
    }),
  );
}
