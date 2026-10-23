// The colours the pixel emoji are drawn in: the 16-colour Desk set (docs/PALETTE.md) and the letter each slot is
// written as in the art grids.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** One slot of the 16-colour set. */
export interface Slot {
  slot: number;
  /** The letter that stands for this slot in an art grid. */
  letter: string;
  hex: string;
  /** What the slot is for in the emoji. */
  use: string;
}

/**
 * The 16-colour Desk set, slot for slot as in `docs/PALETTE.md`, with the letter each is drawn with.
 * Letters are lowercase and mnemonic; `.` is transparent.
 */
export const SLOTS: readonly Slot[] = [
  { slot: 0, letter: 'k', hex: '#050507', use: 'outline, eyes, mouths (blacK)' },
  { slot: 1, letter: 'x', hex: '#0c0c10', use: 'near-black surfaces' },
  { slot: 2, letter: 'z', hex: '#111116', use: 'screens, dark glass' },
  { slot: 3, letter: 'q', hex: '#1b1e28', use: 'dark plastic, night sky' },
  { slot: 4, letter: 'l', hex: '#333847', use: 'dark grey: shade of grey and metal (Lines)' },
  { slot: 5, letter: 'm', hex: '#7c8796', use: 'mid grey: metal, stone (Muted)' },
  { slot: 6, letter: 's', hex: '#b4bfce', use: 'light grey: shade of white, steel (Silver)' },
  { slot: 7, letter: 'w', hex: '#e0e0e0', use: 'white, paper, highlights' },
  { slot: 8, letter: 'o', hex: '#ffaa40', use: 'amber: faces, hands, gold, the yellow of the set (Orange)' },
  { slot: 9, letter: 'n', hex: '#7d5422', use: 'brown: shade of amber, wood (browN)' },
  { slot: 10, letter: 'u', hex: '#372612', use: 'dark brown: shade of brown (Umber)' },
  { slot: 11, letter: 'r', hex: '#f43f5e', use: 'rose red: hearts, alerts (Red)' },
  { slot: 12, letter: 'p', hex: '#a78bfa', use: 'purple' },
  { slot: 13, letter: 'c', hex: '#22d3ee', use: 'cyan: water, ice, glass' },
  { slot: 14, letter: 'g', hex: '#2ec66a', use: 'green: plants, go' },
  { slot: 15, letter: 'b', hex: '#60a5fa', use: 'blue: sky, links' },
];

/** The transparent pixel in an art grid. */
export const CLEAR = '.';

/** Slot number by letter. */
export const SLOT_OF: ReadonlyMap<string, number> = new Map(SLOTS.map((s) => [s.letter, s.slot]));

/** Red, green and blue (0-255) of a slot. */
export function rgb(slot: number): [number, number, number] {
  const hex = SLOTS[slot]?.hex;
  if (!hex) throw new Error(`no palette slot ${slot}`);
  return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as [number, number, number];
}

/** WCAG relative luminance of a slot, 0 (black) to 1 (white). */
export function luminance(slot: number): number {
  const lin = (v: number): number => {
    const c = v / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  const [r, g, b] = rgb(slot);
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

/**
 * Light slots: luminance of at least 0.45. In a monochrome glyph a light pixel inside an outline is paper,
 * not ink (see `mono.ts`). The set is white, light grey, amber and cyan.
 */
export const LIGHT: ReadonlySet<number> = new Set(SLOTS.filter((s) => luminance(s.slot) >= 0.45).map((s) => s.slot));

/**
 * The shade each light slot is shaded with, light slot to shade slot: amber with brown, white with light
 * grey, light grey with mid grey. Shading sits on the lower right of a shape, against its outline.
 */
export const SHADE: ReadonlyMap<number, number> = new Map([
  [8, 9],
  [7, 6],
  [6, 5],
]);
