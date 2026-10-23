// Writes and reads the Desk's own bitmap font format, .DFN (docs/FORMATS.md), from the same placed glyphs as the OpenType build.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { FaceDef, Glyph } from './face';
import type { Grid } from './grid';

/** One glyph as a `.DFN` file holds it. */
export interface DfnGlyph {
  cp: number;
  width: number;
  rows: Grid;
}

export interface Dfn {
  height: number;
  baseline: number;
  glyphs: DfnGlyph[];
}

/** Bytes per row for a glyph `w` pixels wide. */
export const rowBytes = (w: number): number => Math.ceil(w / 8);

/**
 * Encodes a face. All numbers are little-endian. Header: `DFN1`, height (1 byte), baseline (1 byte:
 * rows above the baseline), glyph count (2 bytes). Then each glyph in code-point order: code point
 * (4 bytes), width (1 byte: the advance, which includes the gap after the glyph), and `height` rows of
 * `ceil(width / 8)` bytes each, the leftmost pixel in the highest bit of the first byte.
 */
export function encodeDfn(face: FaceDef, glyphs: Glyph[]): Uint8Array {
  const size = 8 + glyphs.reduce((n, g) => n + 5 + face.height * rowBytes(g.adv), 0);
  const out = new Uint8Array(size);
  const dv = new DataView(out.buffer);
  out.set([0x44, 0x46, 0x4e, 0x31], 0);
  dv.setUint8(4, face.height);
  dv.setUint8(5, face.ascent);
  dv.setUint16(6, glyphs.length, true);
  let at = 8;
  for (const g of glyphs) {
    if (g.adv > 255) throw new Error(`${face.id}: ${g.ch} is too wide for .DFN`);
    dv.setUint32(at, g.cp, true);
    dv.setUint8(at + 4, g.adv);
    at += 5;
    const bytes = rowBytes(g.adv);
    for (const row of g.rows) {
      row.forEach((v, x) => {
        if (v) out[at + (x >> 3)] = (out[at + (x >> 3)] as number) | (0x80 >> (x & 7));
      });
      at += bytes;
    }
  }
  return out;
}

/** Decodes a `.DFN` file. Throws on a bad header or a short file. */
export function decodeDfn(data: Uint8Array): Dfn {
  const dv = new DataView(data.buffer, data.byteOffset, data.byteLength);
  if (String.fromCharCode(...data.subarray(0, 4)) !== 'DFN1') throw new Error('not a DFN1 file');
  const height = dv.getUint8(4);
  const baseline = dv.getUint8(5);
  const count = dv.getUint16(6, true);
  const glyphs: DfnGlyph[] = [];
  let at = 8;
  for (let i = 0; i < count; i++) {
    const cp = dv.getUint32(at, true);
    const width = dv.getUint8(at + 4);
    at += 5;
    const bytes = rowBytes(width);
    const rows: Grid = [];
    for (let y = 0; y < height; y++) {
      rows.push(Array.from({ length: width }, (_, x) => ((data[at + (x >> 3)] as number) >> (7 - (x & 7))) & 1));
      at += bytes;
    }
    glyphs.push({ cp, width, rows });
  }
  if (at !== data.length) throw new Error(`DFN1 file has ${data.length - at} trailing bytes`);
  return { height, baseline, glyphs };
}
