// The Desk Fixed DOS variant: the 256 characters of code page 437 addressed by their DOS byte values.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { CP437 } from './charset';
import type { FaceDef, Glyph } from './face';
import { blank } from './grid';

/** Where the symbol cmap puts byte 0: Windows reaches a symbol font's byte `b` at `0xF000 + b`. */
export const SYMBOL_BASE = 0xf000;

/**
 * The 256 glyphs of a fixed face in code page 437 order, each at code point `0xF000 + byte`. Bytes
 * 0x01 to 0x1F and 0x7F draw the pictures the VGA ROM shows for them; byte 0x00 is blank, as on the
 * DOS screen.
 */
export function dosGlyphs(face: FaceDef, glyphs: Glyph[]): Glyph[] {
  if (face.cell === undefined) throw new Error('the DOS variant needs a fixed face');
  const byChar = new Map(glyphs.map((g) => [g.ch, g]));
  return CP437.map((ch, byte) => {
    const cp = SYMBOL_BASE + byte;
    if (byte === 0) {
      return { ch: String.fromCodePoint(cp), cp, rows: blank(face.cell as number, face.height), adv: face.cell as number, kind: 'space' };
    }
    const g = byChar.get(ch);
    if (!g) throw new Error(`${face.id} has no glyph for code page 437 byte 0x${byte.toString(16)}`);
    return { ...g, ch: String.fromCodePoint(cp), cp, from: ch };
  });
}
