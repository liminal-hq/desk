// Writes the .DEM files: the pixel emoji of one size as layered 1-bit bitmaps in the 16-colour Desk palette, for the
// Desk on DOS. The byte layout is in docs/FORMATS.md.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Pixels, Size } from './grid';
import type { PixelEmoji } from './pixels';
import { layersOf } from './pixelfont';

/** One row of a 1-bit bitmap: `ceil(size / 8)` bytes, the leftmost pixel in the highest bit of the first byte. */
function packRow(row: number[]): number[] {
  const out = new Array<number>(Math.ceil(row.length / 8)).fill(0);
  row.forEach((v, x) => {
    if (v) out[x >> 3] = (out[x >> 3] as number) | (0x80 >> (x & 7));
  });
  return out;
}

/**
 * The layers as a .DEM stores them: one per colour, back to front in the colour font's order, each
 * holding only the pixels of its own colour, so the layers never overlap.
 */
export function demLayers(p: Pixels): { slot: number; grid: number[][] }[] {
  return layersOf(p).map(({ slot }) => ({ slot, grid: p.map((r) => r.map((v) => (v === slot ? 1 : 0))) }));
}

/**
 * Encodes every emoji that has `size` as a .DEM file (numbers little-endian):
 *
 * - header, 8 bytes: `DEM1`, the size in pixels (1 byte), 0 (1 byte, reserved), the emoji count (2 bytes);
 * - directory, 8 bytes an emoji, in code-point order: the code point (4 bytes) and the offset of its
 *   record from the start of the file (4 bytes);
 * - records: the layer count (1 byte); each layer as its palette slot (1 byte, 0-15) and `size` rows of
 *   `ceil(size / 8)` bytes; then the monochrome glyph, `size` rows of the same form.
 */
export function encodeDem(set: PixelEmoji[], size: Size): Uint8Array {
  const items = set.filter((e) => e.sizes[size]).sort((a, b) => a.cp - b.cp);
  const records = items.map((e) => {
    const d = e.sizes[size];
    if (!d) throw new Error('unreachable');
    const layers = demLayers(d.colour);
    const bytes: number[] = [layers.length];
    for (const l of layers) {
      bytes.push(l.slot);
      for (const r of l.grid) bytes.push(...packRow(r));
    }
    for (const r of d.mono) bytes.push(...packRow(r));
    return bytes;
  });
  const out: number[] = [0x44, 0x45, 0x4d, 0x31, size, 0, items.length & 0xff, items.length >> 8];
  const u32 = (v: number): number[] => [v & 0xff, (v >>> 8) & 0xff, (v >>> 16) & 0xff, v >>> 24];
  let offset = 8 + 8 * items.length;
  items.forEach((e, i) => {
    out.push(...u32(e.cp), ...u32(offset));
    offset += (records[i] as number[]).length;
  });
  for (const r of records) out.push(...r);
  return Uint8Array.from(out);
}
