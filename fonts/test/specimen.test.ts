// Tests for the specimen page: each outline face is declared once for each weight and style it has, so the browser
// picks the italic only when asked for it.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { expect, test } from 'bun:test';
import { buildAll } from '../build';
import { specimen } from '../src/specimen';

test('the specimen declares Regular, Bold, Italic and Bold Italic of each outline family apart', () => {
  const page = specimen(buildAll());
  for (const family of ['Desk Sans', 'Desk Mono', 'Desk Serif']) {
    const rules = [...page.matchAll(new RegExp(`@font-face\\{font-family:'${family}';[^}]*\\}`, 'g'))].map((m) => m[0]);
    const keys = rules.map((r) => `${/font-weight:(\d+)/.exec(r)?.[1]} ${/font-style:(\w+)/.exec(r)?.[1] ?? 'normal'}`);
    expect(keys.sort()).toEqual(['400 italic', '400 normal', '700 italic', '700 normal']);
  }
}, 60_000);
