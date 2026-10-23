// Assembles an outline face: draws every glyph with the kit, unions and fits the outlines, spaces them, composes the
// accented letters and generates the box drawing of Desk Mono.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { boxCell, GENERATED, generated } from '../boxes';
import { ACCENTS } from '../face';
import { type Contour, outline, type P, type Shape, transform, union, rect } from './geom';
import { Kit, type Style } from './kit';
import { MONO_ITALIC, SERIF_ITALIC } from './italics';
import { LETTERS, type Draw, type Side } from './letters';
import { MARKS, SPACING_MARKS } from './marks';
import { extraGlyphs } from './features';
import { SYMBOLS } from './symbols';

/** One glyph of an outline face, in font units. */
export interface MGlyph {
  ch: string;
  /** The code point, or -1 for a glyph reached only through a layout feature. */
  cp: number;
  /** The glyph name of an unencoded glyph (a ligature or an alternate). */
  name?: string;
  adv: number;
  contours: Contour[];
  /** Ink box after spacing, or null for a blank glyph. */
  box: { x0: number; y0: number; x1: number; y1: number } | null;
}

/** The highest an accent may reach: inside the ascender with a little room. */
const MARK_TOP = 935;

/** Line metrics shared by every outline face: the em is 1000 units and a line is 1200. */
export const LINE = { ascender: 950, descender: -250 };

export const sideBearing = (s: Style, side: Side): number => {
  if (typeof side === 'number') return side;
  const k = { s: 1, r: 0.74, d: 0.24, o: 0.5 }[side];
  return Math.round(s.sb * k);
};

export function shapeBox(sh: Shape): { x0: number; y0: number; x1: number; y1: number } | null {
  const pts = sh.outer.flat();
  if (!pts.length) return null;
  return {
    x0: Math.min(...pts.map((p) => p.x)),
    y0: Math.min(...pts.map((p) => p.y)),
    x1: Math.max(...pts.map((p) => p.x)),
    y1: Math.max(...pts.map((p) => p.y)),
  };
}

/** A drawn glyph before spacing: its shape and its side kinds. */
export interface Raw {
  shape: Shape;
  sides: [Side, Side];
  /** The drawn body width, for marks that centre on it. */
  w: number;
  /** A fixed advance with the ink centred (tabular figures). */
  fixed?: number;
  /** Already placed: the shape is in final position and this is its advance (ligatures). */
  preset?: number;
}

export function draw(s: Style, fn: Draw): Raw {
  const k = new Kit(s);
  const d = fn(k);
  return { shape: k.shape(), sides: d.sides, w: d.w, fixed: d.fixed };
}

/** Places a raw glyph: side bearings from its side kinds, or centred in the Mono cell. */
export function place(s: Style, ch: string, raw: Raw, name?: string): MGlyph {
  const box = shapeBox(raw.shape);
  const cp = name ? -1 : (ch.codePointAt(0) as number);
  if (raw.preset !== undefined) {
    const sh = s.italic && !UPRIGHT_IN_ITALIC.has(ch) ? slant(raw.shape, s) : raw.shape;
    return { ch, cp, name, adv: raw.preset, contours: box ? outline(sh) : [], box: box ? shapeBox(sh) : null };
  }
  if (!box) {
    const adv = s.advance ?? Math.round(sideBearing(s, raw.sides[0]) + raw.w + sideBearing(s, raw.sides[1]));
    return { ch, cp, name, adv, contours: [], box: null };
  }
  let dx: number;
  let adv: number;
  if (s.advance || raw.fixed) {
    adv = s.advance ?? (raw.fixed as number);
    dx = Math.round((adv - (box.x1 - box.x0)) / 2 - box.x0);
  } else {
    dx = Math.round(sideBearing(s, raw.sides[0]) - box.x0);
    adv = Math.round(box.x1 + dx + sideBearing(s, raw.sides[1]));
  }
  const moved = transform(raw.shape, (p: P) => ({ x: p.x + dx, y: p.y }));
  // An italic slants after spacing, about the middle of the x-height, so the ink stays centred on its advance.
  const final = s.italic && !UPRIGHT_IN_ITALIC.has(ch) ? slant(moved, s) : moved;
  return { ch, cp, name, adv, contours: outline(final), box: shapeBox(final) };
}

/** Symbols that stay upright in an italic: arrows and geometric shapes. */
const UPRIGHT_IN_ITALIC = new Set(Array.from('←↑→↓↔↕►◄▲▼●○■□'));

/** Slants a shape by the style's angle about the middle of its x-height. */
export function slant(sh: Shape, s: Style): Shape {
  const t = Math.tan(((s.slant ?? 0) * Math.PI) / 180);
  const y0 = s.xh / 2;
  return transform(sh, (p) => ({ x: p.x + (p.y - y0) * t, y: p.y }));
}

/** Composes an accented letter: the base with a mark centred over it (or the cedilla under it). */
export function compose(s: Style, base: Raw, mark: string, upper: boolean, isI: boolean): Raw {
  const box = shapeBox(base.shape) as NonNullable<ReturnType<typeof shapeBox>>;
  const k = new Kit(s);
  const m = MARKS[mark] as (k: Kit, upper: boolean) => Shape;
  const ms = m(k, upper);
  const mb = shapeBox(ms);
  if (!mb) return base;
  let cx = (box.x0 + box.x1) / 2;
  if (isI) cx = box.x0 + s.V / 2 + (s.kind === 'mono' ? (box.x1 - box.x0 - s.V) / 2 : 0);
  let dy: number;
  if (mark === 'cedilla') dy = 0 - mb.y1 + s.H * 0.6;
  else {
    const top = upper ? s.cap : s.xh;
    const gap = upper ? 40 : 56;
    dy = top + s.os + gap - mb.y0;
  }
  const dx = cx - (mb.x0 + mb.x1) / 2;
  // A mark over a capital is squeezed if it would rise past the line's ascender.
  const room = MARK_TOP - (mb.y0 + dy);
  const f = mark !== 'cedilla' && mb.y1 - mb.y0 > room ? room / (mb.y1 - mb.y0) : 1;
  const placed = transform(ms, (p) => ({ x: p.x + dx, y: mb.y0 + dy + (p.y - mb.y0) * f }));
  return { shape: union(base.shape, placed), sides: base.sides, w: base.w };
}

/** Every glyph of an outline face, sorted by code point. */
export function assembleModern(s: Style): MGlyph[] {
  const raws = new Map<string, Raw>();
  const italicForms = !s.italic ? {} : s.kind === 'serif' ? SERIF_ITALIC : s.kind === 'mono' ? MONO_ITALIC : {};
  for (const [ch, fn] of Object.entries({ ...LETTERS, ...SYMBOLS, ...italicForms })) raws.set(ch, draw(s, fn));
  // Accented letters, Š š Ž ž included, composed from a base and a mark.
  for (const [ch, base, mark] of ACCENTS) {
    const b = raws.get(base);
    if (!b) throw new Error(`${s.file}: no base ${base} for ${ch}`);
    const upper = base !== 'ı' && base === base.toUpperCase();
    raws.set(ch, compose(s, b, mark, upper, base === 'ı'));
  }
  for (const [ch, mark] of SPACING_MARKS) {
    const k = new Kit(s);
    const ms = (MARKS[mark] as (k: Kit, upper: boolean) => Shape)(k, false);
    const mb = shapeBox(ms) as NonNullable<ReturnType<typeof shapeBox>>;
    const dy = mark === 'cedilla' ? -mb.y1 + s.H * 0.6 : s.xh + s.os + 56 - mb.y0;
    raws.set(ch, { shape: transform(ms, (p) => ({ x: p.x, y: p.y + dy })), sides: ['o', 'o'], w: mb.x1 - mb.x0 });
  }
  raws.set('­', raws.get('-') as Raw);
  const glyphs: MGlyph[] = [];
  for (const [ch, raw] of raws) glyphs.push(place(s, ch, raw));
  // Spaces.
  const space = s.advance ?? Math.round(s.xh * 0.5);
  for (const ch of [' ', ' ']) glyphs.push({ ch, cp: ch.codePointAt(0) as number, adv: space, contours: [], box: null });
  // Desk Mono: box drawing and blocks fill the whole cell (the line), so they join.
  if (s.advance) {
    // A coarse grid whose cells are about a stem wide, so box lines weigh what the letters do.
    const cols = Math.max(3, Math.round(s.advance / s.V) | 1);
    const ux = s.advance / cols;
    const rows = Math.round((LINE.ascender - LINE.descender) / ux);
    const uy = (LINE.ascender - LINE.descender) / rows;
    const cell = boxCell(cols, rows);
    for (const ch of GENERATED) {
      const g = generated(ch, cell);
      const polys = g.flatMap((row, y) =>
        row.map((v, x) => (v ? rect(x * ux, LINE.ascender - (y + 1) * uy, (x + 1) * ux, LINE.ascender - y * uy) : null)).filter((p) => p !== null),
      );
      const sh = union(...(polys as P[][]));
      glyphs.push({ ch, cp: ch.codePointAt(0) as number, adv: s.advance, contours: outline(sh), box: shapeBox(sh) });
    }
  }
  glyphs.sort((a, b) => a.cp - b.cp);
  // Glyphs reached only through layout features: ligatures and alternates, after the encoded ones.
  for (const [name, x] of extraGlyphs(s, raws)) glyphs.push(place(s, x.ch, x.raw, name));
  return glyphs;
}
