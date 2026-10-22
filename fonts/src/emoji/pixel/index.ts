// All the pixel emoji art, gathered from the files it is drawn in: one file per category, each with its hand-drawn
// core sizes, its 16 × 16 grids, hand fixes to derived 12 × 12 grids and monochrome overrides.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MonoRule } from '../mono';
import * as animals from './animals';
import * as computer from './computer';
import * as faces from './faces';
import * as food from './food';
import * as media from './media';
import * as hearts from './hearts';
import * as nature from './nature';
import * as objects from './objects';
import * as office from './office';
import * as people from './people';
import * as symbols from './symbols';
import * as travel from './travel';
import type { Art, CoreArt, MonoHand } from './types';

export type { Art, CoreArt, MonoHand } from './types';

interface ArtFile {
  HAND: Record<string, CoreArt>;
  ART: Art;
  FIX12: Art;
  MONO_RULES: Record<string, MonoRule>;
  MONO_HAND: MonoHand;
}

const FILES: [string, ArtFile][] = [
  ['computer.ts', computer],
  ['media.ts', media],
  ['office.ts', office],
  ['faces.ts', faces],
  ['people.ts', people],
  ['hearts.ts', hearts],
  ['animals.ts', animals],
  ['food.ts', food],
  ['nature.ts', nature],
  ['objects.ts', objects],
  ['travel.ts', travel],
  ['symbols.ts', symbols],
];

/** 16 × 16 grids by emoji. An emoji drawn twice is an error. */
export const ART16: Record<string, string> = {};
/** Hand-drawn 12 × 12 grids (core and essential tiers). */
export const ART12: Record<string, string> = {};
/** Hand-drawn 8 × 8 grids (core tier). */
export const ART8: Record<string, string> = {};
/** Hand fixes for derived 12 × 12 grids. */
export const FIX12: Record<string, string> = {};
export const MONO_RULES: Record<string, MonoRule> = {};
export const MONO_HAND: MonoHand = {};
/** The file each emoji is drawn in. */
export const FILE_OF: Record<string, string> = {};

for (const [file, mod] of FILES) {
  const add = (ch: string, grid: string): void => {
    if (ART16[ch]) throw new Error(`${ch} is drawn twice (in ${FILE_OF[ch]} and ${file})`);
    ART16[ch] = grid;
    FILE_OF[ch] = file;
  };
  for (const [ch, art] of Object.entries(mod.HAND)) {
    add(ch, art[16]);
    ART12[ch] = art[12];
    if (art[8]) ART8[ch] = art[8];
  }
  for (const [ch, grid] of Object.entries(mod.ART)) add(ch, grid);
  Object.assign(FIX12, mod.FIX12);
  Object.assign(MONO_RULES, mod.MONO_RULES);
  Object.assign(MONO_HAND, mod.MONO_HAND);
}
