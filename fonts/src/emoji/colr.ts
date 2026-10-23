// COLR version 0 and CPAL version 0 tables: colour glyphs as stacks of plain glyphs, each painted in one palette
// colour, back to front.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { Bytes } from './sfnt';

/** The palette index that means "the text colour" in COLR. */
export const FOREGROUND = 0xffff;

/** One colour glyph: the base glyph id and its layers, back to front. */
export interface ColourGlyph {
  base: number;
  layers: { glyph: number; palette: number }[];
}

/** A COLR version 0 table. Base glyph records are sorted by glyph id, as the format requires. */
export function colrTable(glyphs: ColourGlyph[]): Uint8Array {
  const sorted = [...glyphs].sort((a, b) => a.base - b.base);
  const layers = sorted.flatMap((g) => g.layers);
  const baseOffset = 14;
  const layerOffset = baseOffset + 6 * sorted.length;
  const b = new Bytes().u16(0).u16(sorted.length).u32(baseOffset).u32(layerOffset).u16(layers.length);
  let first = 0;
  for (const g of sorted) {
    b.u16(g.base).u16(first).u16(g.layers.length);
    first += g.layers.length;
  }
  for (const l of layers) b.u16(l.glyph).u16(l.palette);
  return b.done();
}

/** A CPAL version 0 table with one palette of `#rrggbb` colours, fully opaque. */
export function cpalTable(colours: string[]): Uint8Array {
  const b = new Bytes().u16(0).u16(colours.length).u16(1).u16(colours.length).u32(14).u16(0);
  for (const hex of colours) {
    const [r, g, bl] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as [number, number, number];
    b.u8(bl).u8(g).u8(r).u8(0xff);
  }
  return b.done();
}
