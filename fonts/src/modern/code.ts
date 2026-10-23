// Desk Mono's code ligatures: operators such as -> => != === drawn as one continuous glyph that keeps the cells of
// the characters it stands for. As in other monospaced code faces, every character but the last becomes an empty
// spacer cell and the last becomes the drawing, which reaches back over the spacers, so every glyph keeps the 600-unit
// advance and the caret still steps one character at a time.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { glyphName } from '../otf';
import { type Raw, shapeBox } from './assemble';
import { difference, type P, rect, type Shape, stroke, transform, union } from './geom';
import { Kit, type Style } from './kit';
import type { ChainRule, SubLookup } from './layout';

/** The code ligatures, longest first so that === is tried before ==. */
export const CODE_LIGATURES = ['<=>', '===', '!==', '...', '->', '=>', '<-', '<=', '>=', '!=', '==', '::', '..', '//', '/*', '*/', '&&', '||', '|>', '<|', '>>', '<<', '<>', '++', '--', '~=', '=~'];

/** Characters that make a longer operator: a ligature next to one of these is left as plain characters. */
const OPERATORS = Array.from('-=<>!:./*&|+~');

/** The name of the empty cell that stands in for every character of a ligature but the last. */
export const SPACER = 'lig.spacer';

const AGL: Record<string, string> = {
  '-': 'hyphen',
  '=': 'equal',
  '<': 'less',
  '>': 'greater',
  '!': 'exclam',
  ':': 'colon',
  '.': 'period',
  '/': 'slash',
  '*': 'asterisk',
  '&': 'ampersand',
  '|': 'bar',
  '+': 'plus',
  '~': 'asciitilde',
};

/** The glyph name of a code ligature, such as `hyphen_greater.liga`. */
export const codeLigatureName = (seq: string): string => `${Array.from(seq, (c) => AGL[c]).join('_')}.liga`;

const nameOf = (ch: string): string => glyphName(ch.codePointAt(0) as number);

/** The drawing of one ligature across `n` cells from x = 0, in the style's units. */
function drawLigature(k: Kit, seq: string, raws: Map<string, Raw>): void {
  const cell = k.s.advance as number;
  const n = seq.length;
  const T = n * cell;
  const t = k.V * 0.9;
  const c = Math.round(k.xh * 0.5 + 10) + 40;
  const m = Math.round(cell * 0.14);
  const a = Math.round(440 * 0.42);
  /** A character's own drawing, centred in a cell and nudged by `dx`. */
  const glyph = (ch: string, at: number, dx = 0, dy = 0): void => {
    const raw = raws.get(ch) as Raw;
    const box = shapeBox(raw.shape);
    if (!box) return;
    const x = at * cell + (cell - (box.x1 - box.x0)) / 2 - box.x0 + dx;
    k.add(transform(raw.shape, (p) => ({ x: p.x + x, y: p.y + dy })));
  };
  const chevron = (tipX: number, dir: 1 | -1, half = a, y = c): void => {
    const back = tipX - dir * half;
    k.add(...stroke([{ x: back, y: y + half }, { x: tipX, y }, { x: back, y: y - half }], t, 'butt'));
  };
  const bars = (x0: number, x1: number, ys: number[], h = t * 0.95): void => {
    for (const y of ys) k.add(rect(x0, y - h / 2, x1, y + h / 2));
  };
  const eq = [c + 70 + t * 0.44, c - 70 - t * 0.44];
  const W = Math.min(T - 2 * m, 720);
  const x0 = (T - W) / 2;
  const x1 = (T + W) / 2;
  const tip = t * 0.35;
  switch (seq) {
    case '->':
      bars(m, T - m - tip, [c]);
      chevron(T - m, 1);
      break;
    case '<-':
      bars(m + tip, T - m, [c]);
      chevron(m, -1);
      break;
    case '=>':
      bars(m, T - m - 70, eq);
      chevron(T - m, 1, a + 40);
      break;
    case '<=>':
      bars(m + 70, T - m - 70, eq);
      chevron(T - m, 1, a + 40);
      chevron(m, -1, a + 40);
      break;
    case '<=':
    case '>=': {
      // ≤ and ≥: the angle raised, a bar beneath.
      const dir = seq === '<=' ? -1 : 1;
      const tx = dir === 1 ? x1 - tip : x0 + tip;
      k.add(...stroke([{ x: tx - dir * (W - tip), y: c + 60 + a }, { x: tx, y: c + 60 }, { x: tx - dir * (W - tip), y: c + 60 - a }], t, 'butt'));
      bars(x0, x1, [c - a - 20]);
      break;
    }
    case '==':
      bars(m, T - m, eq);
      break;
    case '===':
    case '!==': {
      const step = 120;
      bars(m, T - m, [c + step, c, c - step], t * 0.8);
      if (seq === '!==') k.add(...stroke([{ x: T / 2 - 150, y: c - 260 }, { x: T / 2 + 150, y: c + 260 }], t * 0.9));
      break;
    }
    case '!=':
      bars(x0, x1, eq);
      k.add(...stroke([{ x: T / 2 - 120, y: c - 230 }, { x: T / 2 + 120, y: c + 230 }], t * 0.9));
      break;
    case '::':
      glyph(':', 0, 130);
      glyph(':', 1, -130);
      break;
    case '..':
      glyph('.', 0, 150);
      glyph('.', 1, -150);
      break;
    case '...':
      glyph('.', 0, 300);
      glyph('.', 1);
      glyph('.', 2, -300);
      break;
    case '//':
      glyph('/', 0, 130);
      glyph('/', 1, -130);
      break;
    case '/*':
    case '*/': {
      // The star is centred on the operators' height and drawn in close to the slash.
      const star = raws.get('*') as Raw;
      const sb = shapeBox(star.shape) as NonNullable<ReturnType<typeof shapeBox>>;
      const dy = c - (sb.y0 + sb.y1) / 2;
      if (seq === '/*') {
        glyph('/', 0, 110);
        glyph('*', 1, -110, dy);
      } else {
        glyph('*', 0, 110, dy);
        glyph('/', 1, -110);
      }
      break;
    }
    case '&&':
      glyph('&', 0, 70);
      glyph('&', 1, -70);
      break;
    case '||':
      glyph('|', 0, 150);
      glyph('|', 1, -150);
      break;
    case '|>':
    case '<|': {
      // A pipe joined to the angle: the angle's arms run from the ends of a short bar to its tip.
      const dir = seq === '|>' ? 1 : -1;
      const h = a + 60;
      const bx = dir === 1 ? x0 + 60 : x1 - 60 - t;
      k.add(rect(bx, c - h - t * 0.3, bx + t, c + h + t * 0.3));
      const back = dir === 1 ? bx + t / 2 : bx + t / 2;
      const tx = dir === 1 ? x1 - tip : x0 + tip;
      k.add(...stroke([{ x: back, y: c + h }, { x: tx, y: c }, { x: back, y: c - h }], t, 'butt'));
      break;
    }
    case '>>':
      chevron(T / 2 + 40 + 2 * a * 0.62, 1);
      chevron(T / 2 + 40, 1);
      break;
    case '<<':
      chevron(T / 2 - 40 - 2 * a * 0.62, -1);
      chevron(T / 2 - 40, -1);
      break;
    case '<>': {
      // A diamond: the two angles meet top and bottom.
      const A = (x1 - x0) / 2 - 80;
      const B = a + 20;
      const L = Math.hypot(A, B);
      const d = t / 2;
      const diamond = (ax: number, by: number): P[] => [{ x: T / 2 - ax, y: c }, { x: T / 2, y: c - by }, { x: T / 2 + ax, y: c }, { x: T / 2, y: c + by }];
      k.add(difference(union(diamond(A + (d * L) / B, B + (d * L) / A)), union(diamond(A - (d * L) / B, B - (d * L) / A))));
      break;
    }
    case '++':
      bars(cell / 2 - 230, T - cell / 2 + 230, [c]);
      for (const x of [cell / 2, T - cell / 2]) k.add(rect(x - t / 2, c - 230, x + t / 2, c + 230));
      break;
    case '--':
      bars(m, T - m, [c - 10]);
      break;
    case '~=':
    case '=~': {
      const wave = (y: number): void => {
        const pts: P[] = Array.from({ length: 41 }, (_, i) => ({ x: x0 + (i / 40) * W, y: y + 60 * Math.sin((i / 40) * 2 * Math.PI + Math.PI * 0.8) }));
        k.add(...stroke(pts, t * 0.95));
      };
      if (seq === '~=') {
        wave(c + 100);
        bars(x0, x1, [c - 120]);
      } else {
        bars(x0, x1, [c + 120]);
        wave(c - 100);
      }
      break;
    }
    default:
      throw new Error(`no drawing for the code ligature ${seq}`);
  }
}

/**
 * The code ligature glyphs of Desk Mono, by name, and the empty spacer: each drawn over its cells and placed so the
 * drawing ends in the last cell (the earlier cells are at negative x), with the one-cell advance.
 */
export function codeLigatureGlyphs(s: Style, raws: Map<string, Raw>): [string, Raw][] {
  const cell = s.advance as number;
  const out: [string, Raw][] = [[SPACER, { shape: { outer: [], holes: [] }, sides: [0, 0], w: 0, preset: cell }]];
  for (const seq of CODE_LIGATURES) {
    const k = new Kit(s);
    drawLigature(k, seq, raws);
    const sh: Shape = union(k.shape());
    const shift = -(seq.length - 1) * cell;
    out.push([codeLigatureName(seq), { shape: transform(sh, (p) => ({ x: p.x + shift, y: p.y })), sides: [0, 0], w: 0, preset: cell }]);
  }
  return out;
}

/**
 * The `calt` lookup: for each ligature, longest first, a rule that leaves it alone next to another operator character
 * (so `<!--`, `-->` and `====` stay plain), then a rule that turns its characters into spacers and its last
 * character into the drawing.
 */
export function codeLigatureLookup(): SubLookup {
  const ops = OPERATORS.map(nameOf);
  const spacers: SubLookup = { type: 'single', map: OPERATORS.map((c) => [nameOf(c), SPACER]) };
  const rules: ChainRule[] = [];
  for (const seq of CODE_LIGATURES) {
    const input = Array.from(seq, (c) => [nameOf(c)]);
    const lastCh = seq[seq.length - 1] as string;
    const final: SubLookup = { type: 'single', map: [[nameOf(lastCh), codeLigatureName(seq)]] };
    rules.push({ back: [ops], input, ahead: [], subs: [] });
    rules.push({ back: [], input, ahead: [ops], subs: [] });
    rules.push({
      back: [],
      input,
      ahead: [],
      subs: [...Array.from({ length: seq.length - 1 }, (_, i) => ({ at: i, lookup: spacers })), { at: seq.length - 1, lookup: final }],
    });
  }
  return { type: 'chain', rules };
}
