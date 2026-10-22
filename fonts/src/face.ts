// A face's definition and its assembly: drawn grids, composed accents, derived bold and generated box drawing,
// placed into whole cells ready for the OpenType and .DFN writers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { boxCell, generated, GENERATED } from './boxes';
import { blank, embolden, format, inkSpan, overlay, parse, sized, width, type Grid } from './grid';

/** The accent marks a face draws once; composed letters place them over a base letter. */
export type MarkName = 'grave' | 'acute' | 'circ' | 'tilde' | 'diaer' | 'ring' | 'cedilla' | 'macron' | 'caron';

/**
 * A drawn glyph: rows of `#` and `.` joined with `/`, starting at the cap-height row. The object form
 * moves the first row `top` rows from there (negative is up), for a glyph that reaches above the cap
 * height such as a bracket or an accented capital drawn by hand.
 */
export type Source = string | { rows: string; top: number };

export interface FaceDef {
  /** Short identifier: `fixed`, `ui`, `title` or `small`. */
  id: string;
  /** OpenType family name. */
  family: string;
  /** OpenType style: the title face is the only bold one. */
  style: 'Regular' | 'Bold';
  /** File name of the OpenType file, without extension. */
  file: string;
  /** File name of the `.DFN` file, 8.3. */
  dfn: string;
  /** What the face is for, one line. */
  purpose: string;
  /** Nominal size in pixels: the em is this many pixel rows, so units per em is `size × 100`. */
  size: number;
  /** Rows in a cell, top to bottom: the line height when lines are set solid. */
  height: number;
  /** Rows above the baseline. */
  ascent: number;
  /** Row where capitals (and drawn glyphs) start. */
  capTop: number;
  /** Row where the x-height starts. */
  xTop: number;
  /** The fixed advance in pixels, or `undefined` for a proportional face. */
  cell?: number;
  /** Blank columns after each glyph in a proportional face. */
  gap: number;
  /** Advance of the space in pixels. */
  space: number;
  /** Width of a box-drawing or block cell, in pixels. */
  boxWidth: number;
  /** Draw capitals one row shorter under an accent, for faces with no room above the capitals. */
  squashCaps?: boolean;
  /** Derive the face from its drawings by emboldening them (`embolden` in `grid.ts`). */
  bold?: boolean;
  /** Marks for lowercase letters, at the face's final weight; `upper` replaces any of them for capitals. */
  marks: Record<MarkName, string> & { upper?: Partial<Record<MarkName, string>> };
  /** The drawn glyphs, keyed by character. */
  glyphs: Record<string, Source>;
  /** Hand-drawn replacements for emboldened glyphs, in the same form as `glyphs` but at their final weight. */
  boldFixes?: Record<string, Source>;
}

/** How a glyph came to be. */
export type Kind = 'drawn' | 'composed' | 'generated' | 'space';

/** A glyph placed in its cell: `rows` is `height` rows of `adv` pixels, the baseline after row `ascent - 1`. */
export interface Glyph {
  ch: string;
  cp: number;
  rows: Grid;
  adv: number;
  kind: Kind;
  /** For composed glyphs, the base letter and the mark. */
  from?: string;
  /** For drawn glyphs, the source exactly as written, for the editor. */
  source?: Source;
  /** In a bold face, whether `source` is already at the final weight (a bold fix) rather than a skeleton. */
  final?: boolean;
}

/** Accented letters: the letter, its base and its mark. */
export const ACCENTS: [string, string, MarkName][] = [
  ...(
    [
      ['A', 'ÀÁÂÃÄÅ', ['grave', 'acute', 'circ', 'tilde', 'diaer', 'ring']],
      ['E', 'ÈÉÊË', ['grave', 'acute', 'circ', 'diaer']],
      ['I', 'ÌÍÎÏ', ['grave', 'acute', 'circ', 'diaer']],
      ['O', 'ÒÓÔÕÖ', ['grave', 'acute', 'circ', 'tilde', 'diaer']],
      ['U', 'ÙÚÛÜ', ['grave', 'acute', 'circ', 'diaer']],
      ['a', 'àáâãäå', ['grave', 'acute', 'circ', 'tilde', 'diaer', 'ring']],
      ['e', 'èéêë', ['grave', 'acute', 'circ', 'diaer']],
      ['ı', 'ìíîï', ['grave', 'acute', 'circ', 'diaer']],
      ['o', 'òóôõö', ['grave', 'acute', 'circ', 'tilde', 'diaer']],
      ['u', 'ùúûü', ['grave', 'acute', 'circ', 'diaer']],
    ] as [string, string, MarkName[]][]
  ).flatMap(([base, letters, marks]) =>
    Array.from(letters).map((ch, i): [string, string, MarkName] => [ch, base, marks[i] as MarkName]),
  ),
  ['Ç', 'C', 'cedilla'],
  ['ç', 'c', 'cedilla'],
  ['Ñ', 'N', 'tilde'],
  ['ñ', 'n', 'tilde'],
  ['Ý', 'Y', 'acute'],
  ['ý', 'y', 'acute'],
  ['ÿ', 'y', 'diaer'],
  ['Ÿ', 'Y', 'diaer'],
  ['Š', 'S', 'caron'],
  ['š', 's', 'caron'],
  ['Ž', 'Z', 'caron'],
  ['ž', 'z', 'caron'],
];

/** Spacing marks that stand alone as characters, drawn from the lowercase marks. */
const SPACING_MARKS: [string, MarkName][] = [
  ['`', 'grave'],
  ['´', 'acute'],
  ['^', 'circ'],
  ['~', 'tilde'],
  ['¨', 'diaer'],
  ['°', 'ring'],
  ['¸', 'cedilla'],
  ['¯', 'macron'],
  ['ˆ', 'circ'],
  ['˜', 'tilde'],
];

/** Characters that copy another glyph. */
const ALIASES: [string, string][] = [
  ['­', '-'],
];

/** A glyph's drawing in a full-height canvas, before it is placed in its cell. */
interface Canvas {
  rows: Grid;
  kind: Kind;
  from?: string;
  source?: Source;
  final?: boolean;
}

/** Parses a drawn glyph into a full-height canvas as wide as its widest row. */
export function canvasOf(face: FaceDef, src: Source): Grid {
  const { rows, top } = typeof src === 'string' ? { rows: src, top: 0 } : src;
  const g = parse(rows);
  const y = face.capTop + top;
  if (y < 0 || y + g.length > face.height) {
    throw new Error(`${face.id}: '${rows}' runs outside the ${face.height}-row cell`);
  }
  return overlay(blank(width(g), face.height), g, 0, y);
}

/** Removes one repeated row from the capital zone and shifts what is above it down by one. */
function squash(face: FaceDef, g: Grid, ch: string): Grid {
  const mid = (face.capTop + face.ascent - 1) / 2;
  let best = -1;
  for (let r = face.capTop + 1; r < face.ascent; r++) {
    const same = (g[r] as number[]).join() === (g[r - 1] as number[]).join();
    if (same && (best < 0 || Math.abs(r - mid) <= Math.abs(best - mid))) best = r;
  }
  if (best < 0) throw new Error(`${face.id}: cannot shorten ${ch} under an accent`);
  const out = g.map((r) => r.slice());
  out.splice(best, 1);
  out.unshift(new Array<number>(width(g)).fill(0));
  return out;
}

/** Places mark `m` over (or, for a cedilla, under) the base glyph `g`, widening the canvas if it must. */
function compose(face: FaceDef, g: Grid, name: MarkName, upper: boolean, ch: string): Grid {
  const m = parse((upper && face.marks.upper?.[name]) || face.marks[name]);
  const mw = width(m);
  const span = inkSpan(g);
  if (!span) throw new Error(`${face.id}: blank base for ${ch}`);
  const [lo, hi] = span;
  // Centre on the ink; an acute leans right when the centre falls between two columns.
  let x = lo + Math.floor((hi - lo + 1 - mw + (name === 'acute' ? 1 : 0)) / 2);
  let base = g;
  let y: number;
  if (name === 'cedilla') y = face.ascent;
  else if (upper && face.squashCaps) {
    base = squash(face, g, ch);
    y = face.capTop + 1 - m.length;
  } else y = (upper ? face.capTop : face.xTop) - 1 - m.length;
  if (y < 0 || y + m.length > face.height) throw new Error(`${face.id}: no room for the mark on ${ch}`);
  // Widen when the mark overhangs the base.
  const left = Math.max(0, -x);
  const w = Math.max(width(base) + left, x + left + mw);
  const out = overlay(blank(w, face.height), base, left, 0);
  x += left;
  return overlay(out, m, x, y);
}

/** Assembles every glyph of a face, sorted by code point. */
export function assemble(face: FaceDef): Glyph[] {
  const canvases = new Map<string, Canvas>();
  for (const [ch, src] of Object.entries(face.glyphs)) {
    if (Array.from(ch).length !== 1) throw new Error(`${face.id}: key '${ch}' is not one character`);
    canvases.set(ch, { rows: canvasOf(face, src), kind: 'drawn', source: src });
  }
  const get = (ch: string): Grid => {
    const c = canvases.get(ch);
    if (!c) throw new Error(`${face.id}: missing ${ch}`);
    return c.rows;
  };
  // A bold face is emboldened before anything is composed, so its marks are drawn bold already.
  if (face.bold) {
    for (const c of canvases.values()) c.rows = embolden(c.rows);
    for (const [ch, src] of Object.entries(face.boldFixes ?? {})) {
      const c = canvases.get(ch);
      if (!c) throw new Error(`${face.id}: bold fix for undrawn ${ch}`);
      c.rows = canvasOf(face, src);
      c.source = src;
      c.final = true;
    }
  }
  // The dotless i, if not drawn, is the i without anything above the x-height.
  if (!canvases.has('ı')) {
    const rows = get('i').map((r, y) => (y < face.xTop ? r.map(() => 0) : r.slice()));
    canvases.set('ı', { rows, kind: 'composed', from: 'i' });
  }
  for (const [ch, base, mark] of ACCENTS) {
    if (canvases.has(ch)) continue;
    const upper = base !== 'ı' && base === base.toUpperCase();
    canvases.set(ch, {
      rows: compose(face, get(base), mark, upper, ch),
      kind: 'composed',
      from: `${base} + ${mark}`,
    });
  }
  for (const [ch, mark] of SPACING_MARKS) {
    if (canvases.has(ch)) continue;
    const m = parse(face.marks[mark]);
    const y = mark === 'cedilla' ? face.ascent : face.xTop - 1 - m.length;
    canvases.set(ch, { rows: overlay(blank(width(m), face.height), m, 0, y), kind: 'composed', from: mark });
  }
  for (const [ch, of] of ALIASES) {
    if (!canvases.has(ch)) canvases.set(ch, { rows: get(of), kind: 'composed', from: of });
  }
  const glyphs: Glyph[] = [];
  const place = (ch: string, c: Canvas): void => {
    const w = width(c.rows);
    let adv: number;
    if (face.cell !== undefined) {
      if (w > face.cell) throw new Error(`${face.id}: ${ch} is ${w} wide, wider than the ${face.cell}-pixel cell`);
      adv = face.cell;
    } else adv = c.kind === 'generated' || c.kind === 'space' ? w : w + face.gap;
    glyphs.push({
      ch,
      cp: ch.codePointAt(0) as number,
      rows: sized(c.rows, adv, face.height),
      adv,
      kind: c.kind,
      from: c.from,
      source: c.source,
      final: c.final,
    });
  };
  for (const [ch, c] of canvases) place(ch, c);
  const cell = boxCell(face.cell ?? face.boxWidth, face.height);
  for (const ch of GENERATED) {
    if (!canvases.has(ch)) place(ch, { rows: generated(ch, cell), kind: 'generated' });
  }
  for (const ch of [' ', ' ']) place(ch, { rows: blank(face.space, face.height), kind: 'space' });
  glyphs.sort((a, b) => a.cp - b.cp);
  for (let i = 1; i < glyphs.length; i++) {
    if (glyphs[i]?.cp === glyphs[i - 1]?.cp) throw new Error(`${face.id}: ${glyphs[i]?.ch} defined twice`);
  }
  return glyphs;
}

/** A glyph's rows as source text, for messages and the specimen. */
export const rowsText = (g: Glyph): string => format(g.rows);
