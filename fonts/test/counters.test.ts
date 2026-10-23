// Guards the counters of the outline faces: each glyph keeps the enclosed counters it should have, none of them
// closes up into a pinhole, and open apertures stay open. Desk Sans Bold is held to it; the other faces are measured.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import { assembleModern, type MGlyph } from '../src/modern/assemble';
import { STYLES } from '../src/modern/kit';
import { counters } from './counters';

/** How many enclosed counters each guarded glyph has in Desk Sans. */
const HOLES: Record<string, number> = { a: 1, e: 1, g: 1, s: 0, '8': 2, '3': 0, '5': 0, '&': 2, B: 2, R: 1, ß: 0, '@': 1, o: 1 };
/** Glyphs whose open counter must keep a wide way out. */
const OPEN = Array.from('aceg35s');

/** The narrowest an enclosed counter may be, and the narrowest aperture, in units of the 1000-unit em. */
const MIN_COUNTER = 95;
const MIN_APERTURE = 110;

function measure(file: string): Map<string, ReturnType<typeof counters>> {
  const s = STYLES.find((x) => x.file === file);
  if (!s) throw new Error(`no style ${file}`);
  const glyphs = assembleModern(s);
  const by = new Map(glyphs.map((g) => [g.ch, g]));
  return new Map([...new Set([...Object.keys(HOLES), ...OPEN])].map((ch) => [ch, counters(by.get(ch) as MGlyph)]));
}

describe('Desk Sans Bold counters', () => {
  const m = measure('desk-sans-bold');
  test('every guarded glyph has the counters it should, none a pinhole', () => {
    for (const [ch, want] of Object.entries(HOLES)) {
      const c = m.get(ch) as ReturnType<typeof counters>;
      expect([ch, c.holes.length]).toEqual([ch, want]);
      for (const h of c.holes) if (h < MIN_COUNTER) throw new Error(`${ch}: a counter only ${Math.round(h)} units wide`);
    }
  });
  test('apertures stay open', () => {
    for (const ch of OPEN) {
      const a = (m.get(ch) as ReturnType<typeof counters>).aperture;
      if (a < MIN_APERTURE) throw new Error(`${ch}: aperture only ${Math.round(a)} units wide`);
    }
  });
});

describe('Desk Sans counters', () => {
  const m = measure('desk-sans');
  test('every guarded glyph has the counters it should, none a pinhole', () => {
    for (const [ch, want] of Object.entries(HOLES)) {
      const c = m.get(ch) as ReturnType<typeof counters>;
      expect([ch, c.holes.length]).toEqual([ch, want]);
      for (const h of c.holes) if (h < MIN_COUNTER) throw new Error(`${ch}: a counter only ${Math.round(h)} units wide`);
    }
  });
});
