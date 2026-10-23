// The monochrome pixel emoji in the Desk pixel faces: each face's size of every emoji placed in its cell, added to
// the OpenType (CFF) files at the real code points with a cmap that reaches beyond the BMP. The TrueType files and
// the .DFN files do not carry them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { FaceDef, Glyph } from '../face';
import { blank } from '../grid';
import { buildOtf } from '../otf';
import type { Style } from '../round';
import type { Size } from './grid';
import { type PixelEmoji, pixelSet } from './pixels';
import { unicodeCmap, replaceTables } from './sfnt';

/** Where a face's emoji go: the size drawn, the top row and left column in the cell, and the advance. */
export interface Placement {
  size: Size;
  top: number;
  left: number;
  adv: number;
}

/**
 * The placement in each face. Desk Fixed takes the 16 × 16 over the whole 16-row cell, two cells wide.
 * Desk UI and Desk Title take the 12 × 12 in rows 1 to 12, centred on the capitals (rows 3 to 10 in UI),
 * with the faces' one-pixel gap after it. Desk Small takes the 8 × 8 over its whole 8-row cell, centred
 * in two 6-pixel cells; only the core tier has an 8 × 8.
 */
export const PLACEMENT: Record<string, Placement> = {
  fixed: { size: 16, top: 0, left: 0, adv: 16 },
  ui: { size: 12, top: 1, left: 0, adv: 13 },
  title: { size: 12, top: 1, left: 0, adv: 13 },
  small: { size: 8, top: 0, left: 2, adv: 12 },
};

/**
 * The emoji glyphs for a face, placed in its cell: every emoji drawn at the face's size whose code point
 * the face does not already have. Where a face already draws a character (the folder and floppy disk, or
 * the code page 437 pictures in Desk Fixed), its own glyph, made to sit in text, is kept.
 */
export function emojiGlyphs(face: FaceDef, have: Glyph[], set: PixelEmoji[] = pixelSet()): Glyph[] {
  const at = PLACEMENT[face.id];
  if (!at) throw new Error(`no emoji placement for ${face.id}`);
  const taken = new Set(have.map((g) => g.cp));
  const out: Glyph[] = [];
  for (const e of set) {
    const d = e.sizes[at.size];
    if (!d || taken.has(e.cp)) continue;
    const rows = blank(at.adv, face.height);
    d.mono.forEach((r, y) => r.forEach((v, x) => {
      if (v) (rows[at.top + y] as number[])[at.left + x] = 1;
    }));
    out.push({ ch: e.ch, cp: e.cp, rows, adv: at.adv, kind: 'drawn' });
  }
  return out.sort((a, b) => a.cp - b.cp);
}

/**
 * A face's OpenType file with the emoji: `buildOtf` with the emoji glyphs after the face's own, then
 * the cmap replaced by one with format 4 for the BMP on (0, 3) and (3, 1) and format 12 for every code
 * point on (0, 4) and (3, 10). Everything else in the file is as `buildOtf` wrote it.
 */
export function pixelOtfWithEmoji(face: FaceDef, glyphs: Glyph[], style: Style, set?: PixelEmoji[]): Uint8Array {
  const all = [...glyphs, ...emojiGlyphs(face, glyphs, set)];
  const otf = buildOtf(face, all, style);
  // buildOtf writes .notdef and then the glyphs in the order given.
  const map = new Map(all.map((g, i) => [g.cp, i + 1]));
  return replaceTables(otf, { cmap: unicodeCmap(map) });
}
