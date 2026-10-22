// The glyphs and layout of the outline faces' OpenType features: for now the kerning of Desk Sans and Desk Serif,
// which the GPOS and `kern` tables are written from.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { glyphName } from '../otf';
import type { MGlyph, Raw } from './assemble';
import { kerningPairs } from './kerning';
import type { Style } from './kit';
import type { Layout } from './layout';

/** An unencoded glyph before placing: the character it stands for (for its slant) and its drawing. */
export interface Extra {
  ch: string;
  raw: Raw;
}

/** Every unencoded glyph of a face, by name. */
export function extraGlyphs(_s: Style, _raws: Map<string, Raw>): [string, Extra][] {
  return [];
}

const nameOf = (ch: string): string => glyphName(ch.codePointAt(0) as number);

/** The layout of a face: its GSUB features and its kerning, by glyph name. */
export function layoutOf(s: Style, glyphs: MGlyph[]): Layout {
  const kerning = kerningPairs(s, glyphs).map(([a, b, v]) => [nameOf(a), nameOf(b), v] as [string, string, number]);
  return { features: [], kerning };
}
