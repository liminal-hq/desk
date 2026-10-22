// Writes an outline face as OpenType (CFF, cubic) and TrueType (glyf, quadratic) files with the same shapes.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as ot from 'opentype.js';
import { COPYRIGHT, LICENCE, LICENCE_URL, type Names, TIMESTAMP, VERSION } from '../names';
import { finish, glyphName } from '../otf';
import { checksum, type TGlyph, writeTtf } from '../ttf';
import { LINE, type MGlyph } from './assemble';
import { type Contour, outline, rect, toQuadratic, union, difference } from './geom';
import type { Style } from './kit';

export const UPM = 1000;
/** The smallest size the outline faces are meant for, though they are drawn for 14 px and up. */
const PPEM = 8;

export function modernNames(s: Style): Names {
  // The four-style model: one family, with Regular, Bold, Italic and Bold Italic as its styles.
  const style = s.italic ? (s.bold ? 'Bold Italic' : 'Italic') : s.bold ? 'Bold' : 'Regular';
  const full = style === 'Regular' ? s.family : `${s.family} ${style}`;
  return {
    family: s.family,
    style,
    full,
    postScript: `${s.family.replace(/ /g, '')}-${style.replace(/ /g, '')}`,
    unique: `Liminal HQ: ${full}: ${VERSION}`,
    version: `Version ${VERSION}`,
    file: s.file,
  };
}

/** `.notdef`: a hollow box. */
export function notdefContours(s: Style): { adv: number; contours: Contour[] } {
  const t = Math.round(s.H);
  const box = difference(union(rect(60, 0, 440, s.cap)), union(rect(60 + t, t, 440 - t, s.cap - t)));
  return { adv: 500, contours: outline(box) };
}

function pathOf(contours: Contour[]): ot.Path {
  const path = new ot.Path();
  for (const c of contours) {
    path.moveTo(c.start.x, c.start.y);
    for (const g of c.segs) {
      if (g.t === 'L') path.lineTo(g.p.x, g.p.y);
      else (path as unknown as { curveTo(a: number, b: number, c: number, d: number, e: number, f: number): void }).curveTo(g.c1.x, g.c1.y, g.c2.x, g.c2.y, g.p.x, g.p.y);
    }
    path.close();
  }
  return path;
}

const os2 = (s: Style, glyphs: MGlyph[]): Record<string, number> => ({
  usWinAscent: LINE.ascender,
  usWinDescent: -LINE.descender,
  sTypoLineGap: 0,
  ulCodePageRange1: 1 | (1 << 29),
  ulCodePageRange2: s.kind === 'mono' ? 0x80000000 : 0,
  sxHeight: s.xh,
  sCapHeight: s.cap,
  ...(glyphs.length ? {} : {}),
});

/** The OpenType (CFF) file of an outline face. */
export function buildModernOtf(s: Style, glyphs: MGlyph[]): Uint8Array {
  const names = modernNames(s);
  const nd = notdefContours(s);
  const list = [new ot.Glyph({ name: '.notdef', advanceWidth: nd.adv, path: pathOf(nd.contours) })];
  for (const g of glyphs) list.push(new ot.Glyph({ name: glyphName(g.cp), unicode: g.cp, advanceWidth: g.adv, path: pathOf(g.contours) }));
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
    license: LICENCE,
    licenseURL: LICENCE_URL,
    unitsPerEm: UPM,
    ascender: LINE.ascender,
    descender: LINE.descender,
    weightClass: s.bold ? 700 : 400,
    widthClass: 5,
    fsSelection: (1 << 7) | (s.italic ? 1 : 0) | (s.bold ? 1 << 5 : 0) | (!s.italic && !s.bold ? 1 << 6 : 0),
    italicAngle: -(s.slant ?? 0),
    panose: [2, s.kind === 'serif' ? 2 : 11, s.bold ? 8 : 5, s.kind === 'mono' ? 9 : 3, 0, 0, 0, 0, 0, 0],
    createdTimestamp: TIMESTAMP,
    tables: {
      os2: os2(s, glyphs),
      post: { isFixedPitch: s.kind === 'mono' ? 1 : 0, underlinePosition: -100, underlineThickness: Math.round(s.H * 0.8) },
    },
    glyphs: list,
  });
  const table = font.names as Record<string, Record<string, { en: string }>>;
  for (const platform of ['windows', 'unicode', 'macintosh']) {
    const map = table[platform];
    if (map) map['uniqueID'] = { en: names.unique };
  }
  return caretSlope(finish(new Uint8Array(font.toArrayBuffer()), PPEM), s);
}

/**
 * Sets the caret slope in `hhea` for an italic (opentype.js writes an upright caret) and fixes the
 * checksums that cover it.
 */
function caretSlope(file: Uint8Array, s: Style): Uint8Array {
  if (!s.italic) return file;
  const out = Uint8Array.from(file);
  const dv = new DataView(out.buffer);
  const tables = dv.getUint16(4);
  let head = -1;
  for (let i = 0; i < tables; i++) {
    const rec = 12 + 16 * i;
    const tag = String.fromCharCode(...out.subarray(rec, rec + 4));
    if (tag === 'head') head = dv.getUint32(rec + 8);
    if (tag !== 'hhea') continue;
    const at = dv.getUint32(rec + 8);
    dv.setInt16(at + 18, 1000);
    dv.setInt16(at + 20, Math.round(1000 * Math.tan(((s.slant ?? 0) * Math.PI) / 180)));
    dv.setUint32(rec + 4, checksum(out, at, dv.getUint32(rec + 12)));
  }
  dv.setUint32(head + 8, 0);
  dv.setUint32(head + 8, (0xb1b0afba - checksum(out)) >>> 0);
  return out;
}

/** A `gasp` table asking for smoothing at every size: the outline faces carry no hinting. */
function gasp(): Uint8Array {
  // Version 1, one range up to 0xFFFF ppem: grey-scale (0x2) and symmetric smoothing (0x8).
  return Uint8Array.from([0, 1, 0, 1, 0xff, 0xff, 0, 0x0a]);
}

/** The TrueType file of an outline face: the same contours converted to quadratics, wound clockwise. */
export function buildModernTtf(s: Style, glyphs: MGlyph[]): Uint8Array {
  const tt = (cs: Contour[]): TGlyph['contours'] => cs.map((c) => toQuadratic(c).reverse());
  const nd = notdefContours(s);
  const list: TGlyph[] = [
    { name: '.notdef', cps: [], adv: nd.adv, contours: tt(nd.contours) },
    { name: '.null', cps: [0], adv: 0, contours: [] },
    { name: 'nonmarkingreturn', cps: [0x0d], adv: glyphs.find((g) => g.cp === 0x20)?.adv ?? 250, contours: [] },
  ];
  for (const g of glyphs) if (g.cp <= 0xffff) list.push({ name: glyphName(g.cp), cps: [g.cp], adv: g.adv, contours: tt(g.contours) });
  return writeTtf({
    names: modernNames(s),
    upm: UPM,
    ascender: LINE.ascender,
    descender: LINE.descender,
    lineGap: 0,
    ppem: PPEM,
    integerPpem: false,
    bold: s.bold,
    italicAngle: s.slant ?? 0,
    fixed: s.kind === 'mono',
    symbol: false,
    cp437: s.kind === 'mono',
    lines: { strikeSize: Math.round(s.H * 0.8), strikePos: Math.round(s.xh / 2 + 20), underPos: -100, underSize: Math.round(s.H * 0.8) },
    glyphs: list,
    extra: { gasp: gasp() },
  });
}
