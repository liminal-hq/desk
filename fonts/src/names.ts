// The names and fixed values every font file carries, shared by the OpenType (CFF) and TrueType writers.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { FaceDef } from './face';
import type { Style } from './round';

export const VERSION = '0.100';
/** The version as a 16.16 fixed number for `head.fontRevision`. */
export const REVISION = Math.round(0.1 * 65536);
/** 2026-01-01T00:00:00Z in Unix seconds: the `head` dates, so that builds are reproducible. */
export const TIMESTAMP = Date.UTC(2026, 0, 1) / 1000;
/** Seconds from 1904-01-01, where font dates count from, to the Unix epoch. */
export const FONT_EPOCH = 2082844800;

export const COPYRIGHT = 'Copyright (c) 2026 Liminal HQ, Scott Morris';
export const LICENCE =
  'This Font Software is licensed under the SIL Open Font License, Version 1.1. ' +
  'This license is available with a FAQ at: https://openfontlicense.org';
export const LICENCE_URL = 'https://openfontlicense.org';

/**
 * Which characters a file addresses: `unicode` (the usual cmap), or `dos`, the 256 characters of code
 * page 437 at their byte values in a symbol cmap (see `dos.ts`).
 */
export type Variant = 'unicode' | 'dos';

export interface Names {
  family: string;
  /** Subfamily (name ID 2): `Regular` or `Bold`. */
  style: string;
  /** Full name (ID 4): the family alone for a regular face, as Windows expects. */
  full: string;
  postScript: string;
  unique: string;
  version: string;
  /** File name without extension. */
  file: string;
}

/** The names of one face in one style. Family names stay within the 31 characters Windows 3.1 allows. */
export function namesOf(face: FaceDef, style: Style, variant: Variant = 'unicode'): Names {
  const base = variant === 'dos' ? `${face.family} DOS` : face.family;
  const family = style === 'round' ? `${base} Round` : base;
  if (family.length > 31) throw new Error(`family name too long: ${family}`);
  const full = face.style === 'Regular' ? family : `${family} ${face.style}`;
  return {
    family,
    style: face.style,
    full,
    postScript: `${family.replace(/ /g, '')}-${face.style}`,
    unique: `Liminal HQ: ${full}: ${VERSION}`,
    version: `Version ${VERSION}`,
    file: `${face.file}${variant === 'dos' ? '-dos' : ''}${style === 'round' ? '-round' : ''}`,
  };
}
