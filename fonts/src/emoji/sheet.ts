// Contact sheets of the pixel emoji for review: each emoji in colour on a dark tile and on a light tile and in mono,
// enlarged, with its code point, grouped by category, as an indexed PNG.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { assemble, type Glyph } from '../face';
import { small } from '../faces/small';
import type { Pixels, Size } from './grid';
import { CATEGORIES, type Category } from './manifest';
import { SLOTS, rgb } from './palette';
import type { PixelEmoji } from './pixels';
import { encodePng } from './png';

// Palette indices beyond the 16 slots.
const BG = 16; // the sheet
const LIGHT_TILE = 17; // the 1993 look's page colour
const DARK_TILE = 18; // the Desk's content colour (slot 2)
const TEXT = 19;
const DIM = 20;
const PALETTE: [number, number, number, number][] = [
  ...SLOTS.map((s) => [...rgb(s.slot), 255] as [number, number, number, number]),
  [38, 40, 50, 255],
  [244, 244, 248, 255],
  [17, 17, 22, 255],
  [224, 224, 224, 255],
  [124, 135, 150, 255],
];

/** A paletted canvas. */
class Canvas {
  px: Uint8Array;
  constructor(
    readonly w: number,
    readonly h: number,
  ) {
    this.px = new Uint8Array(w * h).fill(BG);
  }
  rect(x: number, y: number, w: number, h: number, c: number): void {
    for (let j = y; j < y + h; j++) for (let i = x; i < x + w; i++) if (i >= 0 && j >= 0 && i < this.w && j < this.h) this.px[j * this.w + i] = c;
  }
}

const SMALL = new Map<string, Glyph>(assemble(small).map((g) => [g.ch, g]));

/** Writes text in Desk Small at `scale`, returning the width drawn. */
function text(c: Canvas, s: string, x: number, y: number, colour: number, scale = 1): number {
  let at = x;
  for (const ch of s) {
    const g = SMALL.get(ch) ?? SMALL.get('?');
    if (!g) continue;
    g.rows.forEach((row, j) => row.forEach((v, i) => v && c.rect(at + i * scale, y + j * scale, scale, scale, colour)));
    at += g.adv * scale;
  }
  return at - x;
}

function blit(c: Canvas, p: Pixels | number[][], x: number, y: number, scale: number, mono: boolean): void {
  p.forEach((row, j) =>
    row.forEach((v, i) => {
      if (mono ? v === 1 : v >= 0) c.rect(x + i * scale, y + j * scale, scale, scale, mono ? 0 : v);
    }),
  );
}

const SCALE: Record<Size, number> = { 16: 3, 12: 4, 8: 6 };

/**
 * A contact sheet of every emoji that has `size`, by category. Each cell shows the colour drawing on
 * the Desk's dark content colour, the same on a light page, and the monochrome glyph, each enlarged
 * so they are 48 pixels square, over the code point.
 */
export function contactSheet(set: PixelEmoji[], size: Size, perRow = 8, only?: Category[]): Uint8Array {
  const k = SCALE[size];
  const tile = size * k + 8;
  const cellW = tile * 3 + 4 * 2 + 16;
  const cellH = tile + 14;
  const groups = (Object.keys(CATEGORIES) as Category[])
    .filter((cat) => !only || only.includes(cat))
    .map((cat) => ({ cat, items: set.filter((e) => e.category === cat && e.sizes[size]) }))
    .filter((g) => g.items.length > 0);
  const headH = 28;
  const height = 16 + groups.reduce((h, g) => h + headH + Math.ceil(g.items.length / perRow) * cellH + 8, 0);
  const width = 16 + perRow * cellW;
  const c = new Canvas(width, height);
  let y = 12;
  for (const g of groups) {
    text(c, `${CATEGORIES[g.cat]} · ${size}×${size}`, 16, y + 4, TEXT, 2);
    y += headH;
    g.items.forEach((e, n) => {
      const d = e.sizes[size];
      if (!d) return;
      const x = 16 + (n % perRow) * cellW;
      const yy = y + Math.floor(n / perRow) * cellH;
      c.rect(x, yy, tile, tile, DARK_TILE);
      blit(c, d.colour, x + 4, yy + 4, k, false);
      c.rect(x + tile + 4, yy, tile, tile, LIGHT_TILE);
      blit(c, d.colour, x + tile + 8, yy + 4, k, false);
      c.rect(x + 2 * (tile + 4), yy, tile, tile, LIGHT_TILE);
      blit(c, d.mono, x + 2 * (tile + 4) + 4, yy + 4, k, true);
      const mark = d.origin === 'drawn' ? '' : d.origin === 'derived' ? ' d' : ' f';
      text(c, `${e.cp.toString(16).toUpperCase()}${mark}${d.monoDrawn ? ' m' : ''}`, x, yy + tile + 3, DIM);
    });
    y += Math.ceil(g.items.length / perRow) * cellH + 8;
  }
  return encodePng(width, height, PALETTE, c.px);
}
