// Desk Emoji Pixel: the 16 × 16 pixel emoji as a COLR/CPAL colour font, each colour of a drawing one layer of traced
// pixel outlines, with the monochrome glyph as the base glyph for programs that do not read COLR.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as ot from 'opentype.js';
import { VERSION, type Names } from '../names';
import { glyphName } from '../otf';
import { type OPt, PIXEL, segments, squareOutline } from '../round';
import type { ColourGlyph } from './colr';
import { type CGlyph, type ColourFontSpec, IGNORABLE } from './colourfont';
import { areaBySlot, type Pixels } from './grid';
import { SLOTS } from './palette';
import type { PixelEmoji } from './pixels';

/** The grid is 16 pixels on a 16-pixel em: units per em 1600, one pixel 100 units. */
export const PIXEL_UPM = 16 * PIXEL;
/** The grid's top row starts 14 pixels above the baseline, so its bottom row ends 2 below it. */
export const GRID_ASCENT = 14;
/** The line: 15 pixels above the baseline and 4 below, 1.2 em with no line gap. */
export const PIXEL_LINE = { ascender: 15 * PIXEL, descender: -4 * PIXEL };

export const PIXEL_FONT_NAMES: Names = {
  family: 'Desk Emoji Pixel',
  style: 'Regular',
  full: 'Desk Emoji Pixel',
  postScript: 'DeskEmojiPixel-Regular',
  unique: `Liminal HQ: Desk Emoji Pixel: ${VERSION}`,
  version: `Version ${VERSION}`,
  file: 'desk-emoji-pixel',
};

/** A CFF path from counter-clockwise contours of on- and off-curve points. */
export function pathOf(contours: OPt[][]): ot.Path {
  const path = new ot.Path();
  for (const contour of contours) {
    segments(contour).forEach((s, i) => {
      const [p, k, q] = s as [OPt, OPt, OPt | undefined];
      if (i === 0) path.moveTo(p.x, p.y);
      if (!q) path.lineTo(k.x, k.y);
      else (path as unknown as { quadraticCurveTo(a: number, b: number, c: number, d: number): void }).quadraticCurveTo(k.x, k.y, q.x, q.y);
    });
    path.close();
  }
  return path;
}

/** A glyph in both forms from a 0/1 grid placed with its top row `GRID_ASCENT` pixels above the baseline. */
function glyphOf(name: string, cps: number[], adv: number, grid: number[][]): CGlyph {
  const ccw = squareOutline(grid, GRID_ASCENT);
  return { name, cps, adv, tt: ccw.map((c) => [...c].reverse()), path: pathOf(ccw) };
}

/**
 * The layers of a drawing, back to front: black first, then the other colours from the largest area to
 * the smallest (ties by slot). Each layer covers its own colour and every colour in front of it, so no
 * layer has an edge that another layer's edge must meet exactly, and anti-aliasing never shows a seam;
 * the colour on top at each pixel is still that pixel's own colour.
 */
export function layersOf(p: Pixels): { slot: number; grid: number[][] }[] {
  const area = areaBySlot(p);
  const order = [...area.keys()].sort((a, b) => (a === 0 ? -1 : b === 0 ? 1 : (area.get(b) as number) - (area.get(a) as number) || a - b));
  const rank = new Map(order.map((s, i) => [s, i]));
  return order.map((slot, i) => ({
    slot,
    grid: p.map((r) => r.map((v) => (v >= 0 && (rank.get(v) as number) >= i ? 1 : 0))),
  }));
}

/** Everything the colour font is written from. */
export function pixelFontSpec(set: PixelEmoji[]): ColourFontSpec {
  const adv = 16 * PIXEL;
  const drawn = set.filter((e) => e.sizes[16]);
  const bases: CGlyph[] = drawn.map((e) => glyphOf(glyphName(e.cp), [e.cp], adv, e.sizes[16]?.mono as number[][]));
  const blank: CGlyph = { name: 'uni200D', cps: IGNORABLE, adv: 0, tt: [], path: new ot.Path() };
  const layers: CGlyph[] = [];
  const colours: ColourGlyph[] = [];
  const firstLayer = bases.length + 2;
  drawn.forEach((e, i) => {
    const ls = layersOf(e.sizes[16]?.colour as Pixels);
    colours.push({
      base: i + 1,
      layers: ls.map((l) => {
        layers.push(glyphOf(`${glyphName(e.cp)}.c${l.slot}`, [], adv, l.grid));
        return { glyph: firstLayer + layers.length - 1, palette: l.slot };
      }),
    });
  });
  const box = Array.from({ length: 16 }, (_, y) =>
    Array.from({ length: 16 }, (_, x) => (y >= 1 && y <= 14 && x >= 2 && x <= 13 && (y === 1 || y === 14 || x === 2 || x === 13) ? 1 : 0)),
  );
  return {
    names: PIXEL_FONT_NAMES,
    description: 'The Desk pixel emoji in colour: 16 x 16 drawings in the 16-colour Desk palette',
    upm: PIXEL_UPM,
    ascender: PIXEL_LINE.ascender,
    descender: PIXEL_LINE.descender,
    ppem: 16,
    integerPpem: true,
    glyphs: [...bases, blank, ...layers],
    notdef: glyphOf('.notdef', [], adv, box),
    colours,
    palette: SLOTS.map((s) => s.hex),
  };
}
