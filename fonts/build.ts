// Builds the Desk fonts into fonts/generated/: the pixel faces from their grids as OpenType (CFF), TrueType and .DFN
// files in the Square and Round styles, and the outline faces (Desk Sans, Mono and Serif) from their skeletons.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: bun build.ts                      write every face to generated/
//        bun build.ts --check              rebuild in memory and fail if generated/ is stale
//        bun build.ts --specimen <file>    also write the specimen page (glyph charts, samples, editor)

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { encodeDfn } from './src/dfn';
import { dosGlyphs } from './src/dos';
import { buildEmoji } from './src/emoji/build';
import { pixelOtfWithEmoji } from './src/emoji/faces';
import { modernOtfWithEmoji } from './src/emoji/vector/font';
import { assemble } from './src/face';
import { FACES } from './src/faces';
import { namesOf } from './src/names';
import { buildOtf } from './src/otf';
import { assembleModern } from './src/modern/assemble';
import { layoutOf } from './src/modern/features';
import { buildModernTtf } from './src/modern/files';
import { STYLES } from './src/modern/kit';
import { buildTtf } from './src/ttf';

const OUT = join(import.meta.dir, 'generated');

/** Every generated file: its name in `generated/` and its bytes. */
export function buildAll(): { name: string; data: Uint8Array }[] {
  return [...buildPixel(), ...buildModern(), ...buildEmoji()];
}

/** The outline faces: Desk Sans, Desk Mono and Desk Serif, each Regular and Bold, as OTF and TTF. */
export function buildModern(): { name: string; data: Uint8Array }[] {
  return STYLES.flatMap((s) => {
    const glyphs = assembleModern(s);
    const layout = layoutOf(s, glyphs);
    return [
      // The OpenType files also carry the monochrome vector emoji (src/emoji/vector/font.ts); the TrueType files do not.
      { name: `${s.file}.otf`, data: modernOtfWithEmoji(s, glyphs, undefined, layout) },
      { name: `${s.file}.ttf`, data: buildModernTtf(s, glyphs, layout) },
    ];
  });
}

/** The pixel faces, in Square and Round, with the .DFN files and Desk Fixed DOS. */
export function buildPixel(): { name: string; data: Uint8Array }[] {
  return FACES.flatMap((face) => {
    const glyphs = assemble(face);
    return [
      ...(['square', 'round'] as const).flatMap((style) => {
        const file = namesOf(face, style).file;
        return [
          // The OpenType files also carry the monochrome emoji (src/emoji/faces.ts); the TrueType files do not.
          { name: `${file}.otf`, data: pixelOtfWithEmoji(face, glyphs, style) },
          { name: `${file}.ttf`, data: buildTtf(face, glyphs, style) },
        ];
      }),
      // The DOS variant of the fixed face: code page 437 at its byte values, in both styles.
      ...(face.id === 'fixed'
        ? (['square', 'round'] as const).flatMap((style) => {
            const dos = dosGlyphs(face, glyphs);
            const file = namesOf(face, style, 'dos').file;
            return [
              { name: `${file}.otf`, data: buildOtf(face, dos, style, 'dos') },
              { name: `${file}.ttf`, data: buildTtf(face, dos, style, 'dos') },
            ];
          })
        : []),
      // Round is outline-only: the DOS bitmaps stay square.
      { name: face.dfn, data: encodeDfn(face, glyphs) },
    ];
  });
}

const same = (a: Uint8Array, b: Uint8Array): boolean => a.length === b.length && a.every((v, i) => v === b[i]);

if (import.meta.main) {
  const args = process.argv.slice(2);
  const files = buildAll();
  if (args.includes('--check')) {
    const stale = files.filter((f) => {
      const path = join(OUT, f.name);
      return !existsSync(path) || !same(new Uint8Array(readFileSync(path)), f.data);
    });
    if (stale.length > 0) {
      console.error(`stale or missing in fonts/generated: ${stale.map((f) => f.name).join(', ')}`);
      console.error('run `bun build.ts` in fonts/ and commit the result');
      process.exit(1);
    }
    console.log(`fonts: ${files.length} generated files are up to date`);
  } else {
    for (const f of files) {
      mkdirSync(dirname(join(OUT, f.name)), { recursive: true });
      writeFileSync(join(OUT, f.name), f.data);
    }
    console.log(`fonts: wrote ${files.map((f) => f.name).join(', ')}`);
  }
  const at = args.indexOf('--specimen');
  if (at >= 0) {
    const target = args[at + 1];
    if (!target) throw new Error('--specimen needs a file name');
    const { specimen } = await import('./src/specimen');
    writeFileSync(target, specimen(files.filter((f) => !f.name.startsWith('emoji/'))));
    console.log(`fonts: wrote the specimen to ${target}`);
  }
}
