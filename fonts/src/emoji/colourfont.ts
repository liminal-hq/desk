// Writes a colour emoji font: base glyphs (the monochrome fallback, reached through the cmap), layer glyphs, and
// COLR version 0 and CPAL tables, as TrueType (glyf) and OpenType (CFF) files with the same shapes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as ot from 'opentype.js';
import { COPYRIGHT, LICENCE, LICENCE_URL, type Names } from '../names';
import { finish } from '../otf';
import type { OPt } from '../round';
import { type TGlyph, writeTtf } from '../ttf';
import { type ColourGlyph, colrTable, cpalTable } from './colr';
import { replaceTables, unicodeCmap } from './sfnt';

/** One glyph of a colour font, in both outline forms. */
export interface CGlyph {
  name: string;
  cps: number[];
  adv: number;
  /** TrueType contours: on- and off-curve points, outer contours clockwise. */
  tt: OPt[][];
  /** The same shape for CFF, outer contours counter-clockwise. */
  path: ot.Path;
}

export interface ColourFontSpec {
  names: Names;
  description: string;
  upm: number;
  ascender: number;
  descender: number;
  /** Smallest readable size in pixels, for `head.lowestRecPPEM`. */
  ppem: number;
  /** Integer ppem (head flags bit 3), for pixel outlines. */
  integerPpem: boolean;
  /** Every glyph after `.notdef`, base glyphs first. Glyph id `i + 1` is `glyphs[i]`. */
  glyphs: CGlyph[];
  notdef: CGlyph;
  colours: ColourGlyph[];
  palette: string[];
}

/** Code points every emoji font maps to an empty glyph so they never show as a missing glyph. */
export const IGNORABLE = [0x200d, 0xfe0e, 0xfe0f];

function cmapOf(spec: ColourFontSpec): Uint8Array {
  const map = new Map<number, number>();
  spec.glyphs.forEach((g, i) => g.cps.forEach((cp) => map.set(cp, i + 1)));
  return unicodeCmap(map);
}

/** The TrueType file. */
export function colourTtf(spec: ColourFontSpec): Uint8Array {
  const toT = (g: CGlyph): TGlyph => ({ name: g.name, cps: g.cps, adv: g.adv, contours: g.tt });
  return writeTtf({
    names: spec.names,
    upm: spec.upm,
    ascender: spec.ascender,
    descender: spec.descender,
    lineGap: 0,
    ppem: spec.ppem,
    integerPpem: spec.integerPpem,
    bold: false,
    fixed: false,
    symbol: false,
    cp437: false,
    lines: { strikeSize: spec.upm / 16, strikePos: Math.round(spec.upm * 0.3), underPos: -spec.upm / 16, underSize: spec.upm / 16 },
    glyphs: [toT(spec.notdef), ...spec.glyphs.map(toT)],
    extra: { cmap: cmapOf(spec), COLR: colrTable(spec.colours), CPAL: cpalTable(spec.palette) },
  });
}

/** The OpenType (CFF) file: opentype.js writes the outlines, then the cmap, COLR and CPAL are set. */
export function colourOtf(spec: ColourFontSpec): Uint8Array {
  const { names } = spec;
  const list = [new ot.Glyph({ name: '.notdef', advanceWidth: spec.notdef.adv, path: spec.notdef.path })];
  for (const g of spec.glyphs) {
    list.push(new ot.Glyph({ name: g.name, unicodes: g.cps, unicode: g.cps[0], advanceWidth: g.adv, path: g.path }));
  }
  const font = new ot.Font({
    familyName: names.family,
    styleName: names.style,
    fullName: names.full,
    postScriptName: names.postScript,
    version: names.version,
    copyright: COPYRIGHT,
    designer: 'Scott Morris',
    manufacturer: 'Liminal HQ',
    manufacturerURL: 'https://github.com/liminal-hq/desk',
    description: spec.description,
    license: LICENCE,
    licenseURL: LICENCE_URL,
    unitsPerEm: spec.upm,
    ascender: spec.ascender,
    descender: spec.descender,
    weightClass: 400,
    widthClass: 5,
    fsSelection: (1 << 7) | (1 << 6),
    tables: {
      os2: { usWinAscent: spec.ascender, usWinDescent: -spec.descender, sTypoLineGap: 0, ulCodePageRange1: 1 },
      post: { isFixedPitch: 0, underlinePosition: -spec.upm / 16, underlineThickness: spec.upm / 16 },
    },
    glyphs: list,
  });
  const table = font.names as Record<string, Record<string, { en: string }>>;
  for (const platform of ['windows', 'unicode', 'macintosh']) {
    const map = table[platform];
    if (map) map['uniqueID'] = { en: names.unique };
  }
  const otf = finish(new Uint8Array(font.toArrayBuffer()), spec.ppem);
  return replaceTables(otf, { cmap: cmapOf(spec), COLR: colrTable(spec.colours), CPAL: cpalTable(spec.palette) });
}
