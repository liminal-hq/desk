// The shapes the pixel emoji art is written in.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/**
 * Grids keyed by emoji. Each grid is a template string of rows of palette letters (`palette.ts`), `.`
 * for transparent, one row per line; indentation and blank lines around it are ignored.
 */
export type Art = Record<string, string>;

/** The hand-drawn sizes of a core or essential emoji: 8 × 8 only for the core tier. */
export interface CoreArt {
  16: string;
  12: string;
  8?: string;
}

/** Hand-drawn monochrome grids (`#` and `.`) that replace the derived ones, by size. */
export type MonoHand = Record<string, { 16?: string; 12?: string; 8?: string }>;
