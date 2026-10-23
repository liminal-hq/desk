// The pixel emoji assembled: every manifest entry with its colour grids at 16, 12 and 8 pixels, how each was made,
// and its monochrome forms.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { derive12 } from './derive';
import { parseArt, parseMono, type Pixels, type Size } from './grid';
import { MANIFEST, type Emoji } from './manifest';
import { deriveMono } from './mono';
import { ART12, ART16, ART8, FIX12, MONO_HAND, MONO_RULES } from './pixel';

/** How a grid came to be: drawn by hand, derived from the 16 × 16, or derived and then fixed by hand. */
export type Origin = 'drawn' | 'derived' | 'fixed';

/** One size of one emoji. */
export interface Drawing {
  size: Size;
  colour: Pixels;
  mono: number[][];
  origin: Origin;
  /** Whether the mono grid is hand-drawn rather than derived by the rule. */
  monoDrawn: boolean;
}

export interface PixelEmoji extends Emoji {
  sizes: Partial<Record<Size, Drawing>>;
}

const key = (ch: string, size: Size): string => `${ch} @${size}`;

/** The sizes an emoji must have for its tier. */
export const sizesFor = (e: Emoji): Size[] => (e.tier === 'core' ? [16, 12, 8] : [16, 12]);

function drawing(e: Emoji, size: Size, colour: Pixels, origin: Origin): Drawing {
  const hand = MONO_HAND[e.ch]?.[size];
  const mono = hand ? parseMono(hand, size, `${key(e.ch, size)} mono`) : deriveMono(colour, MONO_RULES[e.ch]);
  return { size, colour, mono, origin, monoDrawn: !!hand };
}

/**
 * Assembles every emoji. With `partial`, emoji whose art is not drawn yet are left without sizes
 * rather than failing, for proofing while the set is being drawn.
 */
export function pixelSet(partial = false): PixelEmoji[] {
  if (!partial) return (complete ??= assembleSet(false));
  return assembleSet(true);
}

let complete: PixelEmoji[] | undefined;

function assembleSet(partial: boolean): PixelEmoji[] {
  return MANIFEST.map((e) => {
    const sizes: Partial<Record<Size, Drawing>> = {};
    const src16 = ART16[e.ch];
    if (!src16) {
      if (partial) return { ...e, sizes };
      throw new Error(`${e.ch} ${e.name}: no 16 × 16 drawing`);
    }
    const px16 = parseArt(src16, 16, key(e.ch, 16));
    sizes[16] = drawing(e, 16, px16, 'drawn');
    const hand12 = ART12[e.ch];
    const fix12 = FIX12[e.ch];
    if (hand12) sizes[12] = drawing(e, 12, parseArt(hand12, 12, key(e.ch, 12)), 'drawn');
    else if (fix12) sizes[12] = drawing(e, 12, parseArt(fix12, 12, key(e.ch, 12)), 'fixed');
    else sizes[12] = drawing(e, 12, derive12(px16), 'derived');
    const hand8 = ART8[e.ch];
    if (hand8) sizes[8] = drawing(e, 8, parseArt(hand8, 8, key(e.ch, 8)), 'drawn');
    return { ...e, sizes };
  });
}
