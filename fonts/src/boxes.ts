// Box drawing and block elements, generated to fill the whole cell so that lines and shades join between neighbours.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { blank, type Grid } from './grid';

/** The arms of a box-drawing character: up, down, left, right; 0 none, 1 single, 2 double. */
type Arms = [number, number, number, number];

const ARMS: Record<string, Arms> = {
  '─': [0, 0, 1, 1],
  '│': [1, 1, 0, 0],
  '┌': [0, 1, 0, 1],
  '┐': [0, 1, 1, 0],
  '└': [1, 0, 0, 1],
  '┘': [1, 0, 1, 0],
  '├': [1, 1, 0, 1],
  '┤': [1, 1, 1, 0],
  '┬': [0, 1, 1, 1],
  '┴': [1, 0, 1, 1],
  '┼': [1, 1, 1, 1],
  '═': [0, 0, 2, 2],
  '║': [2, 2, 0, 0],
  '╒': [0, 1, 0, 2],
  '╓': [0, 2, 0, 1],
  '╔': [0, 2, 0, 2],
  '╕': [0, 1, 2, 0],
  '╖': [0, 2, 1, 0],
  '╗': [0, 2, 2, 0],
  '╘': [1, 0, 0, 2],
  '╙': [2, 0, 0, 1],
  '╚': [2, 0, 0, 2],
  '╛': [1, 0, 2, 0],
  '╜': [2, 0, 1, 0],
  '╝': [2, 0, 2, 0],
  '╞': [1, 1, 0, 2],
  '╟': [2, 2, 0, 1],
  '╠': [2, 2, 0, 2],
  '╡': [1, 1, 2, 0],
  '╢': [2, 2, 1, 0],
  '╣': [2, 2, 2, 0],
  '╤': [0, 1, 2, 2],
  '╥': [0, 2, 1, 1],
  '╦': [0, 2, 2, 2],
  '╧': [1, 0, 2, 2],
  '╨': [2, 0, 1, 1],
  '╩': [2, 0, 2, 2],
  '╪': [1, 1, 2, 2],
  '╫': [2, 2, 1, 1],
  '╬': [2, 2, 2, 2],
};

/** Rounded corners: the square corner they are drawn from. */
const ROUNDED: Record<string, string> = { '╭': '┌', '╮': '┐', '╯': '┘', '╰': '└' };

/** Where the lines run in a `w` by `h` cell: the single line's column and row. */
export interface BoxCell {
  w: number;
  h: number;
  /** Column of a single vertical line; a double line uses the columns either side. */
  cx: number;
  /** Row of a single horizontal line; a double line uses the rows either side. */
  cy: number;
}

/** The centre lines of a cell: the column and row just before the middle. */
export const boxCell = (w: number, h: number): BoxCell => ({
  w,
  h,
  cx: Math.floor((w - 1) / 2),
  cy: Math.floor((h - 1) / 2),
});

/** Every character this module generates. */
export const GENERATED = [...Object.keys(ARMS), ...Object.keys(ROUNDED), ...'░▒▓█▀▄▌▐'];

/**
 * Draws a box-drawing character. A single arm meets a double line at the near line when the double
 * line runs straight through (a tee) and at the far line when it turns (a corner); double lines
 * meet with an inner and an outer corner. Arms reach the cell edge, so neighbours join.
 */
export function boxGlyph(ch: string, cell: BoxCell): Grid {
  const round = ROUNDED[ch];
  if (round) {
    const g = boxGlyph(round, cell);
    (g[cell.cy] as number[])[cell.cx] = 0;
    return g;
  }
  const arms = ARMS[ch];
  if (!arms) throw new Error(`not a box-drawing character: ${ch}`);
  const [U, D, L, R] = arms;
  const { w, h, cx, cy } = cell;
  const g = blank(w, h);
  const hline = (y: number, x0: number, x1: number): void => {
    for (let x = x0; x <= x1; x++) (g[y] as number[])[x] = 1;
  };
  const vline = (x: number, y0: number, y1: number): void => {
    for (let y = y0; y <= y1; y++) (g[y] as number[])[x] = 1;
  };
  const vDouble = U === 2 || D === 2;
  const hDouble = L === 2 || R === 2;
  const vThrough = U === 2 && D === 2;
  const hThrough = L === 2 && R === 2;

  if (L === 1) hline(cy, 0, !vDouble || R ? cx : vThrough ? cx - 1 : cx + 1);
  if (R === 1) hline(cy, !vDouble || L ? cx : vThrough ? cx + 1 : cx - 1, w - 1);
  if (L === 2) {
    hline(cy - 1, 0, !vDouble ? cx : U === 2 ? cx - 1 : cx + 1);
    hline(cy + 1, 0, !vDouble ? cx : D === 2 ? cx - 1 : cx + 1);
  }
  if (R === 2) {
    hline(cy - 1, !vDouble ? cx : U === 2 ? cx + 1 : cx - 1, w - 1);
    hline(cy + 1, !vDouble ? cx : D === 2 ? cx + 1 : cx - 1, w - 1);
  }
  if (U === 1) vline(cx, 0, !hDouble || D ? cy : hThrough ? cy - 1 : cy + 1);
  if (D === 1) vline(cx, !hDouble || U ? cy : hThrough ? cy + 1 : cy - 1, h - 1);
  if (U === 2) {
    vline(cx - 1, 0, !hDouble ? cy : L === 2 ? cy - 1 : cy + 1);
    vline(cx + 1, 0, !hDouble ? cy : R === 2 ? cy - 1 : cy + 1);
  }
  if (D === 2) {
    vline(cx - 1, !hDouble ? cy : L === 2 ? cy + 1 : cy - 1, h - 1);
    vline(cx + 1, !hDouble ? cy : R === 2 ? cy + 1 : cy - 1, h - 1);
  }
  return g;
}

/**
 * Draws a block or shade. Halves split the cell at its middle; the shades are 25, 50 and 75 per
 * cent ink in patterns that repeat every two columns and four rows, so they tile across cells
 * whose width is even and whose height is a multiple of four.
 */
export function blockGlyph(ch: string, cell: BoxCell): Grid {
  const { w, h } = cell;
  const light = (x: number, y: number): boolean => (y % 4 === 0 && x % 2 === 0) || (y % 4 === 2 && x % 2 === 1);
  const fill: Record<string, (x: number, y: number) => boolean> = {
    '█': () => true,
    '▀': (_, y) => y < Math.floor(h / 2),
    '▄': (_, y) => y >= Math.floor(h / 2),
    '▌': (x) => x < Math.floor(w / 2),
    '▐': (x) => x >= Math.floor(w / 2),
    '░': light,
    '▒': (x, y) => (x + y) % 2 === 0,
    '▓': (x, y) => !light(x, y),
  };
  const f = fill[ch];
  if (!f) throw new Error(`not a block character: ${ch}`);
  const g = blank(w, h);
  g.forEach((r, y) => r.forEach((_, x) => (r[x] = f(x, y) ? 1 : 0)));
  return g;
}

/** Draws any generated character. */
export const generated = (ch: string, cell: BoxCell): Grid =>
  ARMS[ch] || ROUNDED[ch] ? boxGlyph(ch, cell) : blockGlyph(ch, cell);
