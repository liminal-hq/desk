// The Desk Emoji page of the site: its data (every emoji's grids, mono bits and vector paths, the palettes and the
// emoji files), its font faces, and the page itself from emoji.html and emoji.js.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { EMOJI_DIR } from '../emoji/build';
import { formatArt } from '../emoji/grid';
import { CATEGORIES } from '../emoji/manifest';
import { SLOTS } from '../emoji/palette';
import { pixelSet } from '../emoji/pixels';
import { vectorSet } from '../emoji/vector/font';
import { TOP, VCOLOURS, type Shape } from '../emoji/vector/kit';
import { outline } from '../modern/geom';

const r = Math.round;

/** A shape as SVG path data in the 1000-unit box, y down, the top of the box `TOP` units above the baseline. */
export function pathOf(shape: Shape): string {
  const pt = (p: { x: number; y: number }): string => `${r(p.x)} ${r(TOP - p.y)}`;
  return outline(shape)
    .map((c) => `M${pt(c.start)}` + c.segs.map((g) => (g.t === 'L' ? `L${pt(g.p)}` : `C${pt(g.c1)} ${pt(g.c2)} ${pt(g.p)}`)).join('') + 'Z')
    .join('');
}

const bits = (g: number[][]): string => g.map((row) => row.map((v) => (v ? '1' : '0')).join('')).join('');
const flat = (g: number[][]): string => formatArt(g).replace(/\n/g, '');

/** The page's data: every pixel emoji with its vector drawing where it has one, the palettes and the emoji files. */
export function emojiData(files: { name: string; data: Uint8Array }[]) {
  const vec = new Map(vectorSet().map((v) => [v.ch, v]));
  const emoji = pixelSet().map((e) => {
    const v = vec.get(e.ch);
    const { 16: s16, 12: s12, 8: s8 } = e.sizes;
    if (!s16 || !s12) throw new Error(`${e.ch} ${e.name}: no 16 × 16 or 12 × 12 drawing`);
    return {
      cp: e.cp,
      ch: e.ch,
      name: e.name,
      cat: e.category,
      tier: e.tier,
      o12: s12.origin,
      p16: flat(s16.colour),
      m16: bits(s16.mono),
      p12: flat(s12.colour),
      m12: bits(s12.mono),
      ...(s8 ? { p8: flat(s8.colour), m8: bits(s8.mono) } : {}),
      ...(v ? { v: v.layers.map((l) => [l.colour, pathOf(l.shape)]), vm: pathOf(v.mono) } : {}),
    };
  });
  const dir = `${EMOJI_DIR}/`;
  return {
    files: files
      .filter((f) => f.name.startsWith(dir))
      .map((f): [string, number] => [f.name.slice(dir.length), f.data.length])
      .sort((a, b) => (a[0] < b[0] ? -1 : 1)),
    // The .dem files go in the page itself: artifacts do not serve them as files.
    dems: Object.fromEntries(files.filter((f) => f.name.startsWith(dir) && f.name.endsWith('.dem')).map((f) => [f.name.slice(dir.length), Buffer.from(f.data).toString('base64')])),
    letters: Object.fromEntries(SLOTS.map((s) => [s.letter, s.hex])),
    vcolours: VCOLOURS,
    cats: CATEGORIES,
    emoji,
  };
}

/** The families the page sets its text in, and the published font file each loads. */
export const EMOJI_PAGE_FONTS: Record<string, string> = {
  'DE Pixel': `${EMOJI_DIR}/desk-emoji-pixel.ttf`,
  'DE Vector': `${EMOJI_DIR}/desk-emoji.ttf`,
  'DE Fixed': 'desk-fixed.otf',
  'DE UI': 'desk-ui.otf',
  'DE Title': 'desk-title.otf',
  'DE Small': 'desk-small.otf',
  'DE Sans': 'desk-sans.otf',
  'DE Mono': 'desk-mono.otf',
};

/**
 * The emoji page, emoji-data.js and emoji.js; `nav` is the site's header navigation, `fonts` the fonts folder and
 * `planned` the number of requested emoji the roadmap still lists as planned.
 */
export function emojiPage(files: { name: string; data: Uint8Array }[], nav: string, fonts: string, planned: number): Record<string, string> {
  const faces = Object.entries(EMOJI_PAGE_FONTS)
    .map(([family, file]) => `@font-face{font-family:"${family}";src:url(${fonts}/${file});font-display:block}`)
    .join('\n');
  const html = readFileSync(join(import.meta.dir, 'emoji.html'), 'utf8').replace('/*FONTS*/', faces).replace('<!--NAV-->', nav);
  const data = JSON.stringify({ ...emojiData(files), planned }).replace(/</g, '\\u003c');
  return {
    'emoji.html': html,
    'emoji-data.js': `// The data the Desk Emoji page draws from, written by the site build (fonts/src/site/emoji.ts).\nconst DATA = ${data};\n`,
    'emoji.js': readFileSync(join(import.meta.dir, 'emoji.js'), 'utf8'),
  };
}
