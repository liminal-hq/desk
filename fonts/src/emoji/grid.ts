// Colour pixel grids for the emoji: parsing the letter grids the art is drawn in, writing them back, and small
// helpers on them.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { CLEAR, SLOTS, SLOT_OF } from './palette';

/** Rows of palette slots (0-15), `-1` where the pixel is transparent. Top row first. */
export type Pixels = number[][];

/** The three emoji sizes in pixels. */
export const SIZES = [16, 12, 8] as const;
export type Size = (typeof SIZES)[number];

/**
 * Parses an art grid: `size` lines of `size` letters, one per pixel, `.` for transparent and a palette
 * letter (`palette.ts`) for a colour. Blank lines and indentation around the grid are ignored.
 */
export function parseArt(src: string, size: number, what: string): Pixels {
  const lines = src
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  if (lines.length !== size) throw new Error(`${what}: ${lines.length} rows, want ${size}`);
  return lines.map((line, y) => {
    const row = Array.from(line);
    if (row.length !== size) throw new Error(`${what}: row ${y} is ${row.length} wide, want ${size}: '${line}'`);
    return row.map((c, x) => {
      if (c === CLEAR) return -1;
      const slot = SLOT_OF.get(c);
      if (slot === undefined) throw new Error(`${what}: unknown letter '${c}' at ${x},${y}`);
      return slot;
    });
  });
}

/** Parses a monochrome grid of `#` (ink) and `.` (blank), `size` lines of `size`. */
export function parseMono(src: string, size: number, what: string): number[][] {
  const lines = src
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
  if (lines.length !== size) throw new Error(`${what}: ${lines.length} rows, want ${size}`);
  return lines.map((line, y) => {
    if (line.length !== size) throw new Error(`${what}: row ${y} is ${line.length} wide, want ${size}`);
    return Array.from(line, (c) => {
      if (c === '#') return 1;
      if (c === '.') return 0;
      throw new Error(`${what}: bad pixel '${c}' in a mono grid`);
    });
  });
}

/** Writes pixels back as letter rows, the form the art is drawn in. */
export const formatArt = (p: Pixels): string =>
  p.map((r) => r.map((v) => (v < 0 ? CLEAR : (SLOTS[v] as { letter: string }).letter)).join('')).join('\n');

/** Writes a 0/1 grid as `#` and `.` rows. */
export const formatMono = (g: number[][]): string => g.map((r) => r.map((v) => (v ? '#' : '.')).join('')).join('\n');

/** The slots a grid uses, ascending. */
export const slotsUsed = (p: Pixels): number[] => [...new Set(p.flat().filter((v) => v >= 0))].sort((a, b) => a - b);

/** The number of pixels of each slot. */
export function areaBySlot(p: Pixels): Map<number, number> {
  const m = new Map<number, number>();
  for (const v of p.flat()) if (v >= 0) m.set(v, (m.get(v) ?? 0) + 1);
  return m;
}

/** A 0/1 grid of the pixels that are not transparent. */
export const silhouette = (p: Pixels): number[][] => p.map((r) => r.map((v) => (v >= 0 ? 1 : 0)));
