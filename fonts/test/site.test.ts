// Tests for the specimen site: each outline face is declared once for each weight and style it has, every page links
// only to files the site publishes, the fonts it serves are the built files, and the manifest covers every path.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, test } from 'bun:test';
import { buildAll } from '../build';
import { MANIFEST } from '../src/emoji/manifest';
import { manifest, served, site } from '../src/site/build';

describe('the specimen site', () => {
  const files = buildAll();
  const pages = site(files);
  const byPath = new Map(pages.map((f) => [f.path, f]));
  const text = (path: string): string => new TextDecoder().decode(byPath.get(path)?.data);

  test('fonts.css declares Regular, Bold, Italic and Bold Italic of each outline family apart', () => {
    const css = text('fonts.css');
    for (const family of ['Desk Sans', 'Desk Mono', 'Desk Serif']) {
      const rules = [...css.matchAll(new RegExp(`@font-face\\{font-family:'${family}';[^}]*\\}`, 'g'))].map((m) => m[0]);
      const keys = rules.map((r) => `${/font-weight:(\d+)/.exec(r)?.[1]} ${/font-style:(\w+)/.exec(r)?.[1] ?? 'normal'}`);
      expect(keys.sort()).toEqual(['400 italic', '400 normal', '700 italic', '700 normal']);
    }
  });

  test('every built file is served unchanged or carried in downloads.json, and the zip lists them all', () => {
    const list = JSON.parse(text('downloads.json')) as { entries: [string, { path?: string; base64?: string }][] };
    const entry = new Map(list.entries);
    expect(list.entries).toHaveLength(files.length + 1);
    for (const f of files) {
      const src = entry.get(`desk-fonts/${f.name}`);
      const data = served(f.name) ? byPath.get(`fonts/${f.name}`)?.data : new Uint8Array(Buffer.from(src?.base64 ?? '', 'base64'));
      expect([f.name, served(f.name) ? src?.path : 'base64', data]).toEqual([f.name, served(f.name) ? `fonts/${f.name}` : 'base64', f.data]);
    }
    expect(byPath.has('fonts/OFL.txt')).toBe(true);
  });

  test('the site publishes only file types an artifact serves', () => {
    for (const f of pages) expect([f.path, served(f.path)]).toEqual([f.path, true]);
  });

  test('every page links only to files the site publishes or to the web', () => {
    for (const f of pages.filter((p) => /\.(html|css)$/.test(p.path))) {
      const refs = [...text(f.path).matchAll(/(?:href|src)="([^"#]*)[^"]*"|url\(([^)]+)\)/g)].map((m) => m[1] ?? m[2]);
      for (const ref of refs) {
        if (!ref || /^(https?:|data:)/.test(ref) || ref.startsWith('${')) continue;
        expect([f.path, ref, byPath.has(ref)]).toEqual([f.path, ref, true]);
      }
    }
  });

  test('the publishing manifest names every path but the main page, with its media type', () => {
    const m = manifest(pages) as { file_path: string; files: Record<string, { from: string; contentType: string }> };
    expect(m.file_path).toBe('index.html');
    expect(Object.keys(m.files).sort()).toEqual(pages.map((f) => f.path).filter((p) => p !== 'index.html').sort());
    expect(m.files['fonts/desk-ui.otf']?.contentType).toBe('font/otf');
    expect(m.files['fonts/emoji/desk-emoji.ttf']?.contentType).toBe('font/ttf');
    expect(m.files['fonts/emoji/pixel-16.png']?.contentType).toBe('image/png');
  });

  test('the main page has no document skeleton and the other pages have their own', () => {
    expect(text('index.html').startsWith('<title>')).toBe(true);
    for (const p of ['pixel.html', 'outline.html', 'emoji.html', 'downloads.html']) expect(text(p).startsWith('<!doctype html>')).toBe(true);
  });

  test('the emoji page has every emoji in the manifest', () => {
    const data = JSON.parse(text('emoji-data.js').replace(/^\/\/.*\nconst DATA = /, '').replace(/;\s*$/, ''));
    expect(data.emoji.map((e: { cp: number }) => e.cp)).toEqual(MANIFEST.map((e) => e.cp));
  });
}, 120_000);
