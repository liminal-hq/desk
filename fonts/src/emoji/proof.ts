// Writes a proof sheet of the pixel emoji drawn so far, for reviewing art while it is being drawn.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: bun src/emoji/proof.ts <out.png> [16|12|8] [category ...]

import { writeFileSync } from 'node:fs';
import type { Size } from './grid';
import type { Category } from './manifest';
import { pixelSet } from './pixels';
import { contactSheet } from './sheet';

const [out, size = '16', ...cats] = process.argv.slice(2);
if (!out) throw new Error('usage: bun src/emoji/proof.ts <out.png> [16|12|8] [category ...]');
const set = pixelSet(true);
writeFileSync(out, contactSheet(set, Number(size) as Size, 6, cats.length ? (cats as Category[]) : undefined));
const drawn = set.filter((e) => e.sizes[16]).length;
console.log(`proof: ${drawn} of ${set.length} emoji drawn; wrote ${out}`);
