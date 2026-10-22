// The requested emoji: Scott's Gboard "Smileys and emotions" list (gboard-list.txt, 2026-10-22) parsed into sequences,
// each with its kind and its status against the manifest, and the roadmap table that docs/EMOJI-ROADMAP.md carries.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: bun src/emoji/requests.ts    rewrite the generated part of docs/EMOJI-ROADMAP.md

import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { MANIFEST } from './manifest';
import { pixelSet } from './pixels';
import { REQUESTED_NAMES } from './requested-names';
import { vectorSet } from './vector/font';

export const ZWJ = 0x200d;
export const VS16 = 0xfe0f;
/** The five skin-tone modifiers, light to dark. */
export const TONES = [0x1f3fb, 0x1f3fc, 0x1f3fd, 0x1f3fe, 0x1f3ff] as const;
const isTone = (cp: number): boolean => cp >= 0x1f3fb && cp <= 0x1f3ff;

/**
 * What a sequence is: one code point; a code point with VS16 (emoji presentation of a text-default character);
 * a ZWJ sequence; or a code point with a skin-tone modifier, standing for the family of its five tones.
 */
export type Kind = 'single' | 'vs16' | 'zwj' | 'tone';

export interface Requested {
  /** The code points as listed. */
  seq: number[];
  text: string;
  kind: Kind;
  /** The first code point: the picture a VS16 or tone sequence is drawn from. */
  base: number;
  name: string;
  subgroup: string;
}

/**
 * Splits text into emoji sequences: a ZWJ joins the next code point to the sequence, and VS16 and the skin-tone
 * modifiers attach to the code point before them. Line breaks are ignored. Duplicates are kept, in order.
 */
export function splitSequences(text: string): number[][] {
  const out: number[][] = [];
  let cur: number[] = [];
  let joining = false;
  for (const ch of text) {
    const cp = ch.codePointAt(0) as number;
    if (cp === 0x0a || cp === 0x0d) continue;
    if (cur.length && (cp === VS16 || cp === ZWJ || isTone(cp) || joining)) {
      cur.push(cp);
      joining = cp === ZWJ;
      continue;
    }
    if (cur.length) out.push(cur);
    cur = [cp];
    joining = false;
  }
  if (cur.length) out.push(cur);
  return out;
}

export const keyOf = (seq: readonly number[]): string => seq.map((c) => c.toString(16).toUpperCase().padStart(4, '0')).join(' ');
const NAMES = new Map(REQUESTED_NAMES.map(([k, name, subgroup]) => [k, { name, subgroup }]));

export function kindOf(seq: readonly number[]): Kind {
  if (seq.includes(ZWJ)) return 'zwj';
  if (seq.some(isTone)) return 'tone';
  if (seq.length > 1) return 'vs16';
  return 'single';
}

/** The list, parsed and deduplicated, in its first-seen order. */
export function requested(text = readFileSync(join(import.meta.dir, 'gboard-list.txt'), 'utf8')): Requested[] {
  const seen = new Set<string>();
  const out: Requested[] = [];
  for (const seq of splitSequences(text)) {
    const key = keyOf(seq);
    if (seen.has(key)) continue;
    seen.add(key);
    const named = NAMES.get(key);
    if (!named) throw new Error(`gboard-list.txt: no name for ${key}`);
    out.push({ seq, text: String.fromCodePoint(...seq), kind: kindOf(seq), base: seq[0] as number, ...named });
  }
  return out;
}

export type Status = 'done' | 'planned';

export interface Row extends Requested {
  status: Status;
  /** The pixel sizes drawn for the base picture, and whether it has a vector drawing. */
  sizes: number[];
  vector: boolean;
  note: string;
}

/** Each requested sequence with its status against the manifest and the art. */
export function roadmap(list: Requested[] = requested()): Row[] {
  const drawn = new Map(pixelSet().map((e) => [e.cp, e]));
  const vec = new Set(vectorSet().map((v) => v.cp));
  const inManifest = new Set(MANIFEST.map((e) => e.cp));
  return list.map((r) => {
    const e = inManifest.has(r.base) ? drawn.get(r.base) : undefined;
    const sizes = e ? ([16, 12, 8] as const).filter((s) => e.sizes[s]) : [];
    const vector = vec.has(r.base);
    const base = e ? 'base drawn' : 'base not drawn';
    const [status, note]: [Status, string] =
      r.kind === 'single'
        ? e
          ? ['done', '']
          : ['planned', 'new drawing']
        : r.kind === 'vs16'
          ? e
            ? ['done', 'the base picture; VS16 is an empty glyph in every emoji font']
            : ['planned', 'new drawing of the base']
          : r.kind === 'zwj'
            ? ['planned', `own drawing and a GSUB ligature; ${base}`]
            : ['planned', `five tones by palette substitution; ${base}`];
    return { ...r, status, sizes, vector, note };
  });
}

/** The roadmap's priority groups, in the recommended order of work, by the subgroups in emoji-test.txt. */
export const PRIORITY: [string, (r: Requested) => boolean][] = [
  ['Faces and emotions', (r) => r.kind !== 'zwj' && r.kind !== 'tone' && (r.subgroup.startsWith('face-') || r.subgroup === 'cat-face' || r.subgroup === 'monkey-face')],
  ['Hearts and symbols', (r) => r.kind !== 'zwj' && r.kind !== 'tone' && (r.subgroup === 'heart' || r.subgroup === 'emotion')],
  ['Body parts and hands, with their tones', (r) => r.kind !== 'zwj' && (r.subgroup.startsWith('hand') || r.subgroup === 'body-parts' || r.subgroup === 'person-symbol')],
  ['Creatures, sky and objects', (r) => r.kind !== 'zwj' && r.kind !== 'tone'],
  ['ZWJ sequences', (r) => r.kind === 'zwj'],
];

export const groupOf = (r: Requested): string => (PRIORITY.find(([, f]) => f(r)) as [string, unknown])[0];

const START = '<!-- roadmap:start (generated by `bun src/emoji/requests.ts` in fonts/; do not edit by hand) -->';
const END = '<!-- roadmap:end -->';

/** The generated part of the roadmap: totals and every requested sequence by priority group. */
export function roadmapMarkdown(rows: Row[] = roadmap()): string {
  const count = (f: (r: Row) => boolean): number => rows.filter(f).length;
  const kinds: Kind[] = ['single', 'vs16', 'zwj', 'tone'];
  const lines: string[] = [START, ''];
  lines.push(`${rows.length} distinct sequences (${count((r) => r.kind === 'single')} single code points, ${count((r) => r.kind === 'vs16')} VS16 forms, ${count((r) => r.kind === 'zwj')} ZWJ sequences, ${count((r) => r.kind === 'tone')} tone families of ${TONES.length} tones each), from ${new Set(rows.map((r) => r.base)).size} distinct first code points.`, '');
  lines.push('| Kind | Done | Planned |', '|---|---|---|');
  for (const k of kinds) lines.push(`| ${k} | ${count((r) => r.kind === k && r.status === 'done')} | ${count((r) => r.kind === k && r.status === 'planned')} |`);
  lines.push(`| All | ${count((r) => r.status === 'done')} | ${count((r) => r.status === 'planned')} |`, '');
  const newBases = new Set(rows.filter((r) => r.kind !== 'zwj' && !r.sizes.length).map((r) => r.base));
  const toneFamilies = rows.filter((r) => r.kind === 'tone');
  lines.push(
    `Still to draw: ${newBases.size} new base pictures, ${count((r) => r.kind === 'zwj')} ZWJ pictures, and ${toneFamilies.length * TONES.length} tone variants (${toneFamilies.length} families × ${TONES.length}, generated by palette substitution once their bases are drawn; ${toneFamilies.filter((r) => r.sizes.length).length} of those bases are drawn already).`,
    '',
  );
  for (const [group] of PRIORITY) {
    const mine = rows.filter((r) => groupOf(r) === group);
    if (!mine.length) continue;
    lines.push(`### ${group}`, '', `${mine.filter((r) => r.status === 'done').length} done, ${mine.filter((r) => r.status === 'planned').length} planned.`, '');
    lines.push('| Emoji | Code points | Short name | Kind | Status | Pixel sizes | Vector | Tones (light to dark) | Note |', '|---|---|---|---|---|---|---|---|---|');
    for (const r of mine) {
      const tones = r.kind === 'tone' ? TONES.map(() => 'planned').join(', ') : '—';
      lines.push(
        `| ${r.text} | \`${keyOf(r.seq)}\` | ${r.name} | ${r.kind} | ${r.status} | ${r.sizes.length ? r.sizes.join(', ') : '—'} | ${r.vector ? 'yes' : '—'} | ${tones} | ${r.note || '—'} |`,
      );
    }
    lines.push('');
  }
  lines.push(END);
  return lines.join('\n');
}

export const ROADMAP_DOC = join(import.meta.dir, '..', '..', '..', 'docs', 'EMOJI-ROADMAP.md');

/** The roadmap document with its generated part replaced by `markdown`. */
export function withRoadmap(doc: string, markdown = roadmapMarkdown()): string {
  const a = doc.indexOf(START);
  const b = doc.indexOf(END);
  if (a < 0 || b < a) throw new Error('docs/EMOJI-ROADMAP.md: no generated section markers');
  return doc.slice(0, a) + markdown + doc.slice(b + END.length);
}

if (import.meta.main) {
  writeFileSync(ROADMAP_DOC, withRoadmap(readFileSync(ROADMAP_DOC, 'utf8')));
  console.log(`roadmap: wrote ${ROADMAP_DOC}`);
}
