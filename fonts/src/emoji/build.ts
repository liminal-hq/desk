// The emoji files the build writes into fonts/generated/emoji/: the pixel and vector colour fonts, the .DEM files and
// the proof sheets of the pixel emoji.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { colourOtf, colourTtf } from './colourfont';
import { encodeDem } from './dem';
import { SIZES } from './grid';
import { pixelFontSpec } from './pixelfont';
import { type PixelEmoji, pixelSet } from './pixels';
import { contactSheet } from './sheet';
import { vectorFontSpec } from './vector/font';

/** The directory under `generated/` that holds the emoji files. */
export const EMOJI_DIR = 'emoji';

/** Every emoji file, named relative to `generated/`. */
export function buildEmoji(set: PixelEmoji[] = pixelSet()): { name: string; data: Uint8Array }[] {
  const spec = pixelFontSpec(set);
  const vector = vectorFontSpec();
  return [
    { name: `${EMOJI_DIR}/desk-emoji-pixel.ttf`, data: colourTtf(spec) },
    { name: `${EMOJI_DIR}/desk-emoji-pixel.otf`, data: colourOtf(spec) },
    { name: `${EMOJI_DIR}/desk-emoji.ttf`, data: colourTtf(vector) },
    { name: `${EMOJI_DIR}/desk-emoji.otf`, data: colourOtf(vector) },
    ...SIZES.map((size) => ({ name: `${EMOJI_DIR}/emoji${size}.dem`, data: encodeDem(set, size) })),
    ...SIZES.map((size) => ({ name: `${EMOJI_DIR}/pixel-${size}.png`, data: contactSheet(set, size) })),
  ];
}
