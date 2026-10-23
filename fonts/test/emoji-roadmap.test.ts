// Tests for the requested emoji: the Gboard list parses into the expected sequences, every one is named, and the
// status table in docs/EMOJI-ROADMAP.md matches the manifest and the art.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { ROADMAP_DOC, requested, roadmapMarkdown, splitSequences, withRoadmap } from '../src/emoji/requests';

describe('the requested emoji', () => {
  const list = requested();

  test('sequences attach VS16, ZWJ and skin tones to the code point before them', () => {
    expect(splitSequences('😮‍💨❤️👍🏽🙂\n🙂')).toEqual([[0x1f62e, 0x200d, 0x1f4a8], [0x2764, 0xfe0f], [0x1f44d, 0x1f3fd], [0x1f642], [0x1f642]]);
  });

  test('the Gboard list has 244 distinct sequences of the four kinds', () => {
    expect(list).toHaveLength(244);
    const kinds = (k: string): number => list.filter((r) => r.kind === k).length;
    expect([kinds('single'), kinds('vs16'), kinds('zwj'), kinds('tone')]).toEqual([180, 9, 7, 48]);
    expect(list.filter((r) => r.kind === 'zwj').map((r) => r.text).join(' ')).toBe('😶‍🌫️ 🙂‍↔️ 🙂‍↕️ 😮‍💨 😵‍💫 ❤️‍🔥 ❤️‍🩹');
    expect(list.every((r) => r.name.length > 0)).toBe(true);
  });

  test('docs/EMOJI-ROADMAP.md carries the current status of every requested sequence', () => {
    const doc = readFileSync(ROADMAP_DOC, 'utf8');
    expect(withRoadmap(doc, roadmapMarkdown())).toBe(doc);
  });
});
