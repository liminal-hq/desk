// Writes an assembled face as an OpenType (CFF) font: traced pixel outlines, one pixel to 100 units.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as ot from 'opentype.js';
import type { FaceDef, Glyph } from './face';
import { blank, overlay, parse } from './grid';
import { COPYRIGHT, FONT_EPOCH, LICENCE, LICENCE_URL, namesOf, TIMESTAMP, type Variant } from './names';
import { outlineOf, PIXEL, segments, type Style } from './round';

export { PIXEL };

/** The vertical metrics of a face, in font units. */
export function metrics(face: FaceDef): { upm: number; ascender: number; descender: number } {
  return {
    upm: face.size * PIXEL,
    ascender: face.ascent * PIXEL,
    descender: -(face.height - face.ascent) * PIXEL,
  };
}

/** The OpenType glyph name for a code point. */
export const glyphName = (cp: number): string =>
  cp === 0x20 ? 'space' : cp > 0xffff ? `u${cp.toString(16).toUpperCase()}` : `uni${cp.toString(16).toUpperCase().padStart(4, '0')}`;

/** The outline of a placed glyph in font units, counter-clockwise outside as CFF expects. */
function pathOf(face: FaceDef, rows: number[][], style: Style): ot.Path {
  const path = new ot.Path();
  for (const contour of outlineOf(rows, face.ascent, style)) {
    segments(contour).forEach((s, i) => {
      const [p, k, q] = s;
      if (i === 0) path.moveTo((p as { x: number }).x, (p as { y: number }).y);
      if (s.length === 2) path.lineTo((k as { x: number }).x, (k as { y: number }).y);
      else path.quadraticCurveTo((k as { x: number }).x, (k as { y: number }).y, (q as { x: number }).x, (q as { y: number }).y);
    });
    path.close();
  }
  return path;
}

/** `.notdef`: a hollow box the height of the capitals. */
export function notdef(face: FaceDef): { rows: number[][]; adv: number } {
  const capRows = face.ascent - face.capTop;
  const box = ['####', ...Array.from({ length: capRows - 2 }, () => '#..#'), '####'].join('/');
  const adv = face.cell ?? 4 + face.gap;
  return { rows: overlay(blank(adv, face.height), parse(box), 0, face.capTop), adv };
}

/** Builds one face in one style as an OpenType (CFF) file. */
export function buildOtf(face: FaceDef, glyphs: Glyph[], style: Style = 'square', variant: Variant = 'unicode'): Uint8Array {
  const { upm, ascender, descender } = metrics(face);
  const names = namesOf(face, style, variant);
  const nd = notdef(face);
  const otGlyphs = [new ot.Glyph({ name: '.notdef', advanceWidth: nd.adv * PIXEL, path: pathOf(face, nd.rows, style) })];
  for (const g of glyphs) {
    otGlyphs.push(
      new ot.Glyph({
        name: glyphName(g.cp),
        unicode: g.cp,
        // The DOS variant's glyphs are reached at their real Unicode code points as well as at U+F000 + byte.
        unicodes: variant === 'dos' && g.from && g.from !== '\u0000' ? [g.cp, g.from.codePointAt(0) as number] : [g.cp],
        advanceWidth: g.adv * PIXEL,
        path: pathOf(face, g.rows, style),
      }),
    );
  }
  const bold = face.style === 'Bold';
  const fixed = face.cell !== undefined;
  const cp437 = face.id === 'fixed';
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
    description: face.purpose,
    license: LICENCE,
    licenseURL: LICENCE_URL,
    unitsPerEm: upm,
    ascender,
    descender,
    weightClass: bold ? 700 : 400,
    widthClass: 5,
    // Bit 7, USE_TYPO_METRICS, with BOLD (bit 5) or REGULAR (bit 6).
    fsSelection: (1 << 7) | (bold ? 1 << 5 : 1 << 6),
    panose: [2, 11, bold ? 8 : 5, fixed ? 9 : 3, 0, 0, 0, 0, 0, 0],
    createdTimestamp: TIMESTAMP,
    tables: {
      os2: {
        usWinAscent: ascender,
        usWinDescent: -descender,
        sTypoLineGap: 0,
        // Latin 1, or for the DOS variant the symbol character set.
        ulCodePageRange1: variant === 'dos' ? 0x80000000 : 1,
        // Bit 63 of the code page range: code page 437, claimed only by a face that covers all of it.
        ulCodePageRange2: cp437 ? 0x80000000 : 0,
      },
      post: { isFixedPitch: fixed ? 1 : 0, underlinePosition: -PIXEL, underlineThickness: PIXEL },
    },
    glyphs: otGlyphs,
  });
  const table = font.names as Record<string, Record<string, { en: string }>>;
  for (const platform of ['windows', 'unicode', 'macintosh']) {
    const map = table[platform];
    if (map) map['uniqueID'] = { en: names.unique };
  }
  return finish(new Uint8Array(font.toArrayBuffer()), face.size);
}

/** The sum of a run of big-endian 32-bit words, as an sfnt table checksum. */
function checksum(data: Uint8Array, from: number, length: number): number {
  let sum = 0;
  for (let i = 0; i < length; i += 4) {
    let word = 0;
    for (let k = 0; k < 4; k++) word = (word << 8) | (i + k < length ? (data[from + i + k] as number) : 0);
    sum = (sum + (word >>> 0)) >>> 0;
  }
  return sum;
}

/**
 * Fixes up what opentype.js does not let us set: it stamps the `head` modified date from the clock,
 * which would make every build differ, and it writes 3 as the smallest readable size. Both dates
 * are pinned, the smallest size becomes the face's pixel size, and the checksums are recomputed.
 */
function finish(file: Uint8Array, ppem: number): Uint8Array {
  const out = Uint8Array.from(file);
  const dv = new DataView(out.buffer);
  const tables = dv.getUint16(4);
  let head = -1;
  for (let i = 0; i < tables; i++) {
    const rec = 12 + 16 * i;
    if (String.fromCharCode(...out.subarray(rec, rec + 4)) !== 'head') continue;
    head = dv.getUint32(rec + 8);
    dv.setUint32(head + 8, 0);
    for (const field of [20, 28]) {
      dv.setUint32(head + field, 0);
      dv.setUint32(head + field + 4, TIMESTAMP + FONT_EPOCH);
    }
    dv.setUint16(head + 46, ppem);
    dv.setUint32(rec + 4, checksum(out, head, dv.getUint32(rec + 12)));
  }
  if (head < 0) throw new Error('font has no head table');
  dv.setUint32(head + 8, (0xb1b0afba - checksum(out, 0, out.length)) >>> 0);
  return out;
}
