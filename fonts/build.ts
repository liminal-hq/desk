// Builds the Desk bitmap faces from their pixel grids into OpenType (CFF), TrueType and .DFN files in fonts/generated/,
// in the Square and Round styles.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: bun build.ts                      write every face to generated/
//        bun build.ts --check              rebuild in memory and fail if generated/ is stale
//        bun build.ts --specimen <file>    also write the specimen page (glyph charts, samples, editor)

import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { encodeDfn } from './src/dfn';
import { dosGlyphs } from './src/dos';
import { assemble } from './src/face';
import { FACES } from './src/faces';
import { namesOf } from './src/names';
import { buildOtf } from './src/otf';
import { buildTtf } from './src/ttf';

const OUT = join(import.meta.dir, 'generated');

/** Every generated file: its name in `generated/` and its bytes. */
export function buildAll(): { name: string; data: Uint8Array }[] {
  return FACES.flatMap((face) => {
    const glyphs = assemble(face);
    return [
      ...(['square', 'round'] as const).flatMap((style) => {
        const file = namesOf(face, style).file;
        return [
          { name: `${file}.otf`, data: buildOtf(face, glyphs, style) },
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
    for (const f of files) writeFileSync(join(OUT, f.name), f.data);
    console.log(`fonts: wrote ${files.map((f) => f.name).join(', ')}`);
  }
  const at = args.indexOf('--specimen');
  if (at >= 0) {
    const target = args[at + 1];
    if (!target) throw new Error('--specimen needs a file name');
    const { specimen } = await import('./src/specimen');
    writeFileSync(target, specimen(files));
    console.log(`fonts: wrote the specimen to ${target}`);
  }
}
