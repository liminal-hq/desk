// Desk Emoji: the vector emoji as a COLR/CPAL colour font, each layer of a drawing one glyph, with a monochrome base
// glyph under each code point; and the monochrome glyphs added to the OpenType files of Desk Sans, Mono and Serif.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as ot from 'opentype.js';
import { type Contour, outline, type Shape, toQuadratic, union } from '../../modern/geom';
import type { MGlyph } from '../../modern/assemble';
import { LINE } from '../../modern/assemble';
import { buildModernOtf } from '../../modern/files';
import type { Layout } from '../../modern/layout';
import type { Style } from '../../modern/kit';
import { VERSION, type Names } from '../../names';
import { glyphName } from '../../otf';
import type { ColourGlyph } from '../colr';
import { type CGlyph, type ColourFontSpec, IGNORABLE } from '../colourfont';
import { BY_CHAR } from '../manifest';
import { replaceTables, unicodeCmap } from '../sfnt';
import { difference, grow, type Layer, LIGHT_COLOURS, COLOUR_ORDER, VCOLOURS, BOX } from './kit';
import { VECTOR } from './art';

export const VECTOR_FONT_NAMES: Names = {
  family: 'Desk Emoji',
  style: 'Regular',
  full: 'Desk Emoji',
  postScript: 'DeskEmoji-Regular',
  unique: `Liminal HQ: Desk Emoji: ${VERSION}`,
  version: `Version ${VERSION}`,
  file: 'desk-emoji',
};

/** One vector emoji: its code point, its layers back to front, and its monochrome shape. */
export interface VectorEmoji {
  ch: string;
  cp: number;
  layers: Layer[];
  mono: Shape;
}

/**
 * The monochrome form of a vector drawing, built back to front like the pixel rule (`mono.ts`): a layer
 * in a dark or saturated colour is ink; a layer in a light colour (`LIGHT_COLOURS`) is paper with an
 * ink edge `edge` units wide, so it clears what is under it and keeps its own outline. A face is its
 * outline, eyes and mouth; a red heart stays solid. A layer's `mono` role overrides the default.
 */
export function monoOf(layers: Layer[], edge: number): Shape {
  let m: Shape = { outer: [], holes: [] };
  for (const l of layers) {
    const role = l.mono ?? (LIGHT_COLOURS.has(l.colour) ? 'fill' : 'ink');
    if (role === 'ink') m = union(m, l.shape);
    else if (role === 'hole') m = difference(m, l.shape);
    else if (role === 'fill') m = union(difference(m, l.shape), difference(l.shape, grow(l.shape, -edge)));
  }
  return m;
}

/** The edge of a light shape in the colour font's monochrome base glyphs. */
export const MONO_EDGE = 64;

let cache: VectorEmoji[] | undefined;

/** Every vector emoji, in code-point order. */
export function vectorSet(): VectorEmoji[] {
  if (cache) return cache;
  cache = Object.entries(VECTOR)
    .map(([ch, draw]) => {
      const e = BY_CHAR.get(ch);
      if (!e) throw new Error(`vector emoji ${ch} is not in the manifest`);
      const layers = draw();
      return { ch, cp: e.cp, layers, mono: monoOf(layers, MONO_EDGE) };
    })
    .sort((a, b) => a.cp - b.cp);
  return cache;
}

/** A path for CFF from fitted contours (outer counter-clockwise). */
function pathOf(contours: Contour[], dx = 0): ot.Path {
  const path = new ot.Path();
  for (const c of contours) {
    path.moveTo(c.start.x + dx, c.start.y);
    for (const g of c.segs) {
      if (g.t === 'L') path.lineTo(g.p.x + dx, g.p.y);
      else (path as unknown as { curveTo(a: number, b: number, c: number, d: number, e: number, f: number): void }).curveTo(g.c1.x + dx, g.c1.y, g.c2.x + dx, g.c2.y, g.p.x + dx, g.p.y);
    }
    path.close();
  }
  return path;
}

const contourGlyph = (name: string, cps: number[], contours: Contour[]): CGlyph => ({
  name,
  cps,
  adv: BOX,
  tt: contours.map((c) => toQuadratic(c).reverse()),
  path: pathOf(contours),
});
const glyphOf = (name: string, cps: number[], shape: Shape): CGlyph => contourGlyph(name, cps, outline(shape));

/** Everything the Desk Emoji colour font is written from. */
export function vectorFontSpec(set: VectorEmoji[] = vectorSet()): ColourFontSpec {
  const monos = monoContours(set, MONO_EDGE);
  const bases = set.map((e) => contourGlyph(glyphName(e.cp), [e.cp], monos.get(e.cp) ?? []));
  const blank: CGlyph = { name: 'uni200D', cps: IGNORABLE, adv: 0, tt: [], path: new ot.Path() };
  const layers: CGlyph[] = [];
  const colours: ColourGlyph[] = [];
  const firstLayer = bases.length + 2;
  set.forEach((e, i) => {
    colours.push({
      base: i + 1,
      layers: e.layers.map((l, k) => {
        layers.push(glyphOf(`${glyphName(e.cp)}.l${k}`, [], l.shape));
        return { glyph: firstLayer + layers.length - 1, palette: COLOUR_ORDER.indexOf(l.colour) };
      }),
    });
  });
  const box = union(...[[100, -100, 900, 750]].map(([x0, y0, x1, y1]) => [
    { x: x0 as number, y: y0 as number }, { x: x1 as number, y: y0 as number }, { x: x1 as number, y: y1 as number }, { x: x0 as number, y: y1 as number },
  ]));
  const inner = grow(box, -60);
  return {
    names: VECTOR_FONT_NAMES,
    description: 'The Desk vector emoji in colour: flat geometric drawings for use beside Desk Sans, Mono and Serif',
    upm: 1000,
    ascender: LINE.ascender,
    descender: LINE.descender,
    ppem: 12,
    integerPpem: false,
    glyphs: [...bases, blank, ...layers],
    notdef: glyphOf('.notdef', [], difference(box, inner)),
    colours,
    palette: COLOUR_ORDER.map((c) => VCOLOURS[c]),
  };
}

/** The edge of a light shape in the outline faces' monochrome emoji: Desk Emoji's in Regular, heavier in Bold. */
export const BOLD_EDGE = 120;

const fitted = new Map<number, Map<number, Contour[]>>();

/** The fitted monochrome outline of each emoji with light edges `edge` units wide, by code point (cached). */
function monoContours(set: VectorEmoji[], edge: number): Map<number, Contour[]> {
  let byCp = fitted.get(edge);
  if (!byCp) {
    byCp = new Map(set.map((e) => [e.cp, outline(edge === MONO_EDGE ? e.mono : monoOf(e.layers, edge))]));
    if (set === cache) fitted.set(edge, byCp);
  }
  return byCp;
}

/** The monochrome emoji for an outline face: the vector set, with heavier light edges in the Bold faces. */
export function modernEmojiGlyphs(s: Style, have: MGlyph[], set: VectorEmoji[] = vectorSet()): MGlyph[] {
  const taken = new Set(have.map((g) => g.cp));
  // Desk Mono keeps its one advance doubled, two cells, with the drawing centred in them.
  const adv = s.advance ? 2 * s.advance : BOX;
  const dx = (adv - BOX) / 2;
  const contours = monoContours(set, s.bold ? BOLD_EDGE : MONO_EDGE);
  const shift = (c: Contour): Contour => ({
    start: { x: c.start.x + dx, y: c.start.y },
    segs: c.segs.map((g) => (g.t === 'L' ? { t: 'L' as const, p: { x: g.p.x + dx, y: g.p.y } } : { t: 'C' as const, c1: { x: g.c1.x + dx, y: g.c1.y }, c2: { x: g.c2.x + dx, y: g.c2.y }, p: { x: g.p.x + dx, y: g.p.y } })),
  });
  return set
    .filter((e) => !taken.has(e.cp))
    .map((e) => ({ ch: e.ch, cp: e.cp, adv, contours: (contours.get(e.cp) ?? []).map(shift), box: null }));
}

/**
 * An outline face's OpenType file with the vector emoji: `buildModernOtf` with the emoji glyphs after the
 * face's own, then the cmap replaced by one with format 4 for the BMP and format 12 for every code point.
 */
export function modernOtfWithEmoji(s: Style, glyphs: MGlyph[], set?: VectorEmoji[], layout?: Layout): Uint8Array {
  const all = [...glyphs, ...modernEmojiGlyphs(s, glyphs, set)];
  const otf = buildModernOtf(s, all, layout);
  // Alternate and ligature glyphs have no code point (cp < 0) and stay out of the cmap.
  const map = new Map(all.flatMap((g, i): [number, number][] => (g.cp >= 0 ? [[g.cp, i + 1]] : [])));
  return replaceTables(otf, { cmap: unicodeCmap(map) });
}
