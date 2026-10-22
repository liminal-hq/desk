// Writes a face as a TrueType font for Windows 3.1-era systems: glyf outlines, a format 4 Unicode cmap with a
// Mac Roman format 6 beside it, Windows and Macintosh names, OS/2 version 1 and post format 2.0.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { FaceDef, Glyph } from './face';
import { SYMBOL_BASE } from './dos';
import { COPYRIGHT, FONT_EPOCH, LICENCE, LICENCE_URL, type Names, namesOf, REVISION, TIMESTAMP, type Variant } from './names';
import { glyphName, metrics, notdef } from './otf';
import { outlineOf, PIXEL, type OPt, type Style } from './round';

/** A growable big-endian byte buffer. */
class Buf {
  private bytes: number[] = [];
  u8(v: number): this {
    this.bytes.push(v & 0xff);
    return this;
  }
  i8(v: number): this {
    return this.u8(v < 0 ? v + 256 : v);
  }
  u16(v: number): this {
    return this.u8(v >> 8).u8(v);
  }
  i16(v: number): this {
    return this.u16(v < 0 ? v + 0x10000 : v);
  }
  u32(v: number): this {
    return this.u16((v >>> 16) & 0xffff).u16(v & 0xffff);
  }
  raw(d: Uint8Array | number[]): this {
    for (const b of d) this.bytes.push(b);
    return this;
  }
  /** Pads with zeros to a multiple of `n` bytes. */
  pad(n: number): this {
    while (this.bytes.length % n) this.bytes.push(0);
    return this;
  }
  get length(): number {
    return this.bytes.length;
  }
  done(): Uint8Array {
    return Uint8Array.from(this.bytes);
  }
}

/** Mac OS Roman, bytes 0x80 to 0xFF, as Unicode. */
export const MAC_ROMAN_HIGH = Array.from(
  'ÄÅÇÉÑÖÜáàâäãåçéèêëíìîïñóòôöõúùûü†°¢£§•¶ß®©™´¨≠ÆØ∞±≤≥¥µ∂∑∏π∫ªºΩæø¿¡¬√ƒ≈∆«»… ÀÃÕŒœ–—“”‘’÷◊ÿŸ⁄€‹›ﬁﬂ‡·‚„‰ÂÊÁËÈÍÎÏÌÓÔÒÚÛÙıˆ˜¯˘˙˚¸˝˛ˇ',
);

/** The sum of a run of big-endian 32-bit words, as an sfnt table checksum. */
export function checksum(data: Uint8Array, from = 0, length = data.length): number {
  let sum = 0;
  for (let i = 0; i < length; i += 4) {
    let word = 0;
    for (let k = 0; k < 4; k++) word = (word << 8) | (i + k < length ? (data[from + i + k] as number) : 0);
    sum = (sum + (word >>> 0)) >>> 0;
  }
  return sum;
}

/**
 * Assembles an sfnt: the table directory in tag order, each table 4-byte aligned with its checksum,
 * and `head.checkSumAdjustment` set over the whole file.
 */
export function sfnt(tables: Record<string, Uint8Array>, version = 0x00010000): Uint8Array {
  const tags = Object.keys(tables).sort();
  const n = tags.length;
  let pow = 1;
  while (pow * 2 <= n) pow *= 2;
  const dir = new Buf().u32(version).u16(n).u16(pow * 16).u16(Math.log2(pow)).u16(n * 16 - pow * 16);
  let offset = 12 + 16 * n;
  const body = new Buf();
  for (const tag of tags) {
    const data = tables[tag] as Uint8Array;
    dir.raw(Array.from(tag, (c) => c.charCodeAt(0))).u32(checksum(data)).u32(offset).u32(data.length);
    body.raw(data).pad(4);
    offset += Math.ceil(data.length / 4) * 4;
  }
  const out = new Buf().raw(dir.done()).raw(body.done()).done();
  const headAt = new DataView(out.buffer).getUint32(12 + 16 * tags.indexOf('head') + 8);
  new DataView(out.buffer).setUint32(headAt + 8, (0xb1b0afba - checksum(out)) >>> 0);
  return out;
}

/** One glyph as the TrueType tables need it: advance in font units, contours clockwise outside. */
export interface TGlyph {
  name: string;
  cps: number[];
  adv: number;
  contours: OPt[][];
}

/** Everything a TrueType file is written from, whatever face it comes from. */
export interface TtfSpec {
  names: Names;
  upm: number;
  ascender: number;
  descender: number;
  lineGap: number;
  /** Smallest readable size in pixels, for `head.lowestRecPPEM`. */
  ppem: number;
  /** Integer ppem (head flags bit 3): set for pixel faces, whose outlines need whole pixels. */
  integerPpem: boolean;
  bold: boolean;
  /** Italic angle in degrees, positive leaning right; 0 for an upright face. */
  italicAngle?: number;
  fixed: boolean;
  /** The symbol cmap of Desk Fixed DOS. */
  symbol: boolean;
  /** Claim code page 437 in OS/2. */
  cp437: boolean;
  /** Strikeout size and position, underline position and thickness, in font units. */
  lines: { strikeSize: number; strikePos: number; underPos: number; underSize: number };
  glyphs: TGlyph[];
  /** Further tables, such as `gasp` and `kern`. */
  extra?: Record<string, Uint8Array>;
}

/** A simple glyf entry (no instructions) and its bounds. */
function glyfEntry(contours: OPt[][]): { data: Uint8Array; box: [number, number, number, number]; points: number } {
  const pts = contours.flat();
  if (pts.length === 0) return { data: new Uint8Array(0), box: [0, 0, 0, 0], points: 0 };
  const xs = pts.map((p) => p.x);
  const ys = pts.map((p) => p.y);
  const box: [number, number, number, number] = [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
  const b = new Buf().i16(contours.length).i16(box[0]).i16(box[1]).i16(box[2]).i16(box[3]);
  let end = -1;
  for (const c of contours) b.u16((end += c.length));
  b.u16(0); // no instructions
  const flags: number[] = [];
  const xb = new Buf();
  const yb = new Buf();
  let px = 0;
  let py = 0;
  for (const p of pts) {
    let f = p.on ? 0x01 : 0;
    const dx = p.x - px;
    const dy = p.y - py;
    if (dx === 0) f |= 0x10;
    else if (Math.abs(dx) < 256) {
      f |= 0x02 | (dx > 0 ? 0x10 : 0);
      xb.u8(Math.abs(dx));
    } else xb.i16(dx);
    if (dy === 0) f |= 0x20;
    else if (Math.abs(dy) < 256) {
      f |= 0x04 | (dy > 0 ? 0x20 : 0);
      yb.u8(Math.abs(dy));
    } else yb.i16(dy);
    flags.push(f);
    px = p.x;
    py = p.y;
  }
  b.raw(flags).raw(xb.done()).raw(yb.done()).pad(2);
  return { data: b.done(), box, points: pts.length };
}

/** A format 4 subtable mapping BMP code points to glyph ids. */
function cmap4(map: Map<number, number>): Uint8Array {
  const cps = [...map.keys()].filter((c) => c <= 0xfffe).sort((a, b) => a - b);
  const segs: { start: number; end: number; delta: number }[] = [];
  for (const cp of cps) {
    const gid = map.get(cp) as number;
    const last = segs[segs.length - 1];
    if (last && cp === last.end + 1 && gid - cp === last.delta) last.end = cp;
    else segs.push({ start: cp, end: cp, delta: gid - cp });
  }
  segs.push({ start: 0xffff, end: 0xffff, delta: 1 });
  const n = segs.length;
  let pow = 1;
  while (pow * 2 <= n) pow *= 2;
  const b = new Buf().u16(4).u16(16 + 8 * n).u16(0).u16(n * 2).u16(pow * 2).u16(Math.log2(pow)).u16(2 * n - 2 * pow);
  for (const s of segs) b.u16(s.end);
  b.u16(0);
  for (const s of segs) b.u16(s.start);
  for (const s of segs) b.u16((s.delta + 0x10000) % 0x10000);
  for (let i = 0; i < n; i++) b.u16(0);
  return b.done();
}

/** A format 6 subtable for Mac Roman bytes 0 to 255. */
function cmap6(map: Map<number, number>): Uint8Array {
  const b = new Buf().u16(6).u16(10 + 512).u16(0).u16(0).u16(256);
  for (let byte = 0; byte < 256; byte++) {
    const cp = byte < 0x80 ? byte : (MAC_ROMAN_HIGH[byte - 0x80] as string).codePointAt(0);
    b.u16(map.get(cp as number) ?? 0);
  }
  return b.done();
}

/**
 * The cmap of the DOS variant: a symbol subtable (3, 0) with byte `b` at `0xF000 + b`, which is how
 * Windows 3.1 reaches a SYMBOL_CHARSET font; Macintosh (1, 0) with the bytes themselves; and a Unicode
 * platform subtable (0, 3) with the real code points of the code page 437 characters, for modern
 * systems. There is no Windows Unicode (3, 1) subtable: Windows treats a font that has one as a text
 * font and would not use the symbol one; era Windows does not read platform 0.
 */
function symbolCmapTable(map: Map<number, number>): Uint8Array {
  const pua = new Map([...map].filter(([cp]) => cp >= SYMBOL_BASE && cp <= SYMBOL_BASE + 0xff));
  const uni = new Map([...map].filter(([cp]) => !pua.has(cp)));
  const sym = cmap4(pua);
  const real = cmap4(uni);
  const f6 = new Buf().u16(6).u16(10 + 512).u16(0).u16(0).u16(256);
  for (let byte = 0; byte < 256; byte++) f6.u16(map.get(SYMBOL_BASE + byte) ?? 0);
  const header = 4 + 8 * 3;
  return new Buf()
    .u16(0)
    .u16(3)
    .u16(0).u16(3).u32(header + sym.length)
    .u16(1).u16(0).u32(header + sym.length + real.length)
    .u16(3).u16(0).u32(header)
    .raw(sym)
    .raw(real)
    .raw(f6.done())
    .done();
}

function cmapTable(map: Map<number, number>): Uint8Array {
  const f4 = cmap4(map);
  const f6 = cmap6(map);
  // Unicode (0, 3) and Windows Unicode BMP (3, 1) share the format 4 subtable; Macintosh Roman (1, 0) uses format 6.
  const header = 4 + 8 * 3;
  return new Buf()
    .u16(0)
    .u16(3)
    .u16(0).u16(3).u32(header)
    .u16(1).u16(0).u32(header + f4.length)
    .u16(3).u16(1).u32(header)
    .raw(f4)
    .raw(f6)
    .done();
}

function nameTable(records: [number, string][]): Uint8Array {
  const entries: { platform: number; encoding: number; language: number; id: number; data: number[] }[] = [];
  for (const [id, text] of records) {
    if (/[^\x20-\x7e]/.test(text)) throw new Error(`name ${id} is not plain ASCII: ${text}`);
    entries.push({ platform: 1, encoding: 0, language: 0, id, data: Array.from(text, (c) => c.charCodeAt(0)) });
    entries.push({
      platform: 3,
      encoding: 1,
      language: 0x409,
      id,
      data: Array.from(text).flatMap((c) => [c.charCodeAt(0) >> 8, c.charCodeAt(0) & 0xff]),
    });
  }
  entries.sort((a, b) => a.platform - b.platform || a.encoding - b.encoding || a.language - b.language || a.id - b.id);
  const b = new Buf().u16(0).u16(entries.length).u16(6 + 12 * entries.length);
  const strings = new Buf();
  for (const e of entries) {
    b.u16(e.platform).u16(e.encoding).u16(e.language).u16(e.id).u16(e.data.length).u16(strings.length);
    strings.raw(e.data);
  }
  return b.raw(strings.done()).done();
}

/** OpenType Unicode range bits for the blocks these faces use. */
const RANGE_BITS: [number, number, number][] = [
  [0x0000, 0x007f, 0],
  [0x0080, 0x00ff, 1],
  [0x0100, 0x017f, 2],
  [0x02b0, 0x02ff, 5],
  [0x0370, 0x03ff, 7],
  [0x2000, 0x206f, 31],
  [0x2070, 0x209f, 32],
  [0x20a0, 0x20cf, 33],
  [0x2100, 0x214f, 34],
  [0x2190, 0x21ff, 37],
  [0x2200, 0x22ff, 38],
  [0x2300, 0x23ff, 39],
  [0x2500, 0x257f, 43],
  [0x2580, 0x259f, 44],
  [0x25a0, 0x25ff, 45],
  [0x2600, 0x26ff, 46],
  [0x2700, 0x27bf, 47],
  [0xe000, 0xf8ff, 60],
];

/**
 * Builds one face in one style as a TrueType file. For the DOS variant, `glyphs` are the 256 from
 * `dosGlyphs()` and the cmap is the symbol one.
 */
export function buildTtf(face: FaceDef, glyphs: Glyph[], style: Style = 'square', variant: Variant = 'unicode'): Uint8Array {
  const { upm, ascender, descender } = metrics(face);
  const dos = variant === 'dos';
  // The traditional first four glyphs, then every BMP glyph in code-point order. TrueType wants outer
  // contours clockwise, the reverse of the CFF outlines.
  const cw = (cs: OPt[][]): OPt[][] => cs.map((c) => [...c].reverse());
  const nd = notdef(face);
  const list: TGlyph[] = [{ name: '.notdef', cps: [], adv: nd.adv * PIXEL, contours: cw(outlineOf(nd.rows, face.ascent, style)) }];
  if (!dos) {
    list.push({ name: '.null', cps: [0], adv: 0, contours: [] });
    list.push({ name: 'nonmarkingreturn', cps: [0x0d], adv: face.space * PIXEL, contours: [] });
  }
  for (const g of glyphs) {
    if (g.cp > 0xffff) continue;
    const cps = dos && g.from && g.from !== '\u0000' ? [g.cp, g.from.codePointAt(0) as number] : [g.cp];
    list.push({ name: glyphName(g.cp), cps, adv: g.adv * PIXEL, contours: cw(outlineOf(g.rows, face.ascent, style)) });
  }
  return writeTtf({
    names: namesOf(face, style, variant),
    upm,
    ascender,
    descender,
    lineGap: 0,
    ppem: face.size,
    integerPpem: true,
    bold: face.style === 'Bold',
    fixed: face.cell !== undefined,
    symbol: dos,
    cp437: face.id === 'fixed',
    lines: { strikeSize: PIXEL, strikePos: Math.round(((face.ascent - face.xTop) * PIXEL) / 2) + 50, underPos: -PIXEL, underSize: PIXEL },
    glyphs: list,
  });
}

/** Writes a TrueType file from a spec. */
export function writeTtf(spec: TtfSpec): Uint8Array {
  const { names, upm, ascender, descender, lineGap, bold, fixed } = spec;
  const angle = spec.italicAngle ?? 0;
  const italic = angle !== 0;
  const dos = spec.symbol;
  const list = spec.glyphs;
  const glyf = new Buf();
  const loca = new Buf();
  const hmtx = new Buf();
  let [xMin, yMin, xMax, yMax] = [Infinity, Infinity, -Infinity, -Infinity];
  let minLsb = Infinity;
  let minRsb = Infinity;
  let maxExtent = -Infinity;
  let maxPoints = 0;
  let maxContours = 0;
  const cmap = new Map<number, number>();
  list.forEach((g, gid) => {
    const e = glyfEntry(g.contours);
    loca.u32(glyf.length);
    glyf.raw(e.data);
    const adv = g.adv;
    const lsb = e.points ? e.box[0] : 0;
    hmtx.u16(adv).i16(lsb);
    if (e.points) {
      xMin = Math.min(xMin, e.box[0]);
      yMin = Math.min(yMin, e.box[1]);
      xMax = Math.max(xMax, e.box[2]);
      yMax = Math.max(yMax, e.box[3]);
      minLsb = Math.min(minLsb, lsb);
      minRsb = Math.min(minRsb, adv - e.box[2]);
      maxExtent = Math.max(maxExtent, e.box[2]);
      maxPoints = Math.max(maxPoints, e.points);
      maxContours = Math.max(maxContours, g.contours.length);
    }
    for (const cp of g.cps) cmap.set(cp, gid);
  });
  loca.u32(glyf.length);
  const advances = list.map((g) => g.adv);
  const inked = advances.filter((a) => a > 0);
  // The symbol variant's character range is its 0xF000 block, which Windows uses to find the bytes.
  const cps = [...cmap.keys()].filter((c) => (dos ? c >= SYMBOL_BASE : c >= 0x20));
  let r1 = 0;
  let r2 = 0;
  for (const cp of cps) {
    const bit = RANGE_BITS.find(([a, b]) => cp >= a && cp <= b)?.[2];
    if (bit === undefined) continue;
    if (bit < 32) r1 |= 1 << bit;
    else r2 |= 1 << (bit - 32);
  }
  const head = new Buf()
    .u32(0x00010000)
    .u32(REVISION)
    .u32(0) // checkSumAdjustment, set by sfnt()
    .u32(0x5f0f3cf5)
    .u16(spec.integerPpem ? 0b1011 : 0b0011) // baseline at y=0, left side bearing at x=0, integer ppem
    .u16(upm)
    .u32(0).u32(TIMESTAMP + FONT_EPOCH)
    .u32(0).u32(TIMESTAMP + FONT_EPOCH)
    .i16(xMin).i16(yMin).i16(xMax).i16(yMax)
    .u16((bold ? 1 : 0) | (italic ? 2 : 0))
    .u16(spec.ppem)
    .i16(2)
    .i16(1) // long loca
    .i16(0)
    .done();
  const hhea = new Buf()
    .u32(0x00010000)
    .i16(ascender).i16(descender).i16(lineGap)
    .u16(Math.max(...advances))
    .i16(minLsb).i16(minRsb).i16(maxExtent)
    // The caret leans with an italic: rise over run.
    .i16(italic ? 1000 : 1).i16(italic ? Math.round(1000 * Math.tan((angle * Math.PI) / 180)) : 0).i16(0)
    .i16(0).i16(0).i16(0).i16(0)
    .i16(0)
    .u16(list.length)
    .done();
  const maxp = new Buf()
    .u32(0x00010000)
    .u16(list.length)
    .u16(maxPoints).u16(maxContours)
    .u16(0).u16(0) // no composite glyphs
    .u16(2) // zones
    .u16(0).u16(0).u16(0).u16(0).u16(0).u16(0).u16(0).u16(0)
    .done();
  const sub = Math.round(upm * 0.65);
  const os2 = new Buf()
    .u16(1)
    .i16(Math.round(inked.reduce((a, b) => a + b, 0) / inked.length))
    .u16(bold ? 700 : 400)
    .u16(5)
    .u16(0) // fsType 0: installable embedding
    .i16(sub).i16(sub).i16(0).i16(Math.round(upm * 0.14))
    .i16(sub).i16(sub).i16(0).i16(Math.round(upm * 0.48))
    .i16(spec.lines.strikeSize)
    .i16(spec.lines.strikePos)
    .i16(0)
    .raw([2, 11, bold ? 8 : 5, fixed ? 9 : 3, 0, 0, 0, 0, 0, 0])
    .u32(r1).u32(r2).u32(0).u32(0)
    .raw(Array.from('LMHQ', (c) => c.charCodeAt(0)))
    // fsSelection: italic (bit 0), bold (bit 5), or regular (bit 6) when neither.
    .u16((italic ? 0x01 : 0) | (bold ? 0x20 : 0) | (!italic && !bold ? 0x40 : 0))
    .u16(Math.min(...cps))
    .u16(Math.min(0xffff, Math.max(...cps)))
    .i16(ascender).i16(descender).i16(lineGap)
    .u16(ascender).u16(-descender)
    // Code pages: Latin 1 (1252) and Macintosh Roman, or the symbol character set for the DOS variant;
    // the fixed face also claims code page 437.
    .u32(dos ? 0x80000000 : (1 << 0) | (1 << 29))
    .u32(spec.cp437 ? 0x80000000 : 0)
    .done();
  const post = new Buf()
    .u32(0x00020000)
    .u32(Math.round(-angle * 65536) >>> 0)
    .i16(spec.lines.underPos).i16(spec.lines.underSize)
    .u32(fixed ? 1 : 0)
    .u32(0).u32(0).u32(0).u32(0)
    .u16(list.length);
  // Standard Macintosh names for the first four glyphs; every other name is stored in the table.
  const standard: Record<string, number> = { '.notdef': 0, '.null': 1, nonmarkingreturn: 2, space: 3 };
  const extra: string[] = [];
  for (const g of list) {
    const std = standard[g.name];
    if (std !== undefined) post.u16(std);
    else post.u16(258 + extra.push(g.name) - 1);
  }
  for (const s of extra) post.u8(s.length).raw(Array.from(s, (c) => c.charCodeAt(0)));
  const name = nameTable([
    [0, COPYRIGHT],
    [1, names.family],
    [2, names.style],
    [3, names.unique],
    [4, names.full],
    [5, names.version],
    [6, names.postScript],
    [13, LICENCE],
    [14, LICENCE_URL],
  ]);
  return sfnt({
    head,
    hhea,
    maxp,
    'OS/2': os2,
    hmtx: hmtx.done(),
    cmap: dos ? symbolCmapTable(cmap) : cmapTable(cmap),
    loca: loca.done(),
    glyf: glyf.done(),
    name,
    post: post.done(),
    ...spec.extra,
  });
}
