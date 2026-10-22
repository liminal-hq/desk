// Writes the specimen page: every face rendered with its built OpenType file, a chart of every glyph drawn from the
// same grids the build uses, a pixel editor, and the built files as a zip.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { assemble, type FaceDef, type Glyph } from './face';
import { FACES } from './faces';
import { format } from './grid';
import { STYLES } from './modern/kit';
import { metrics } from './otf';

/** CRC-32 (IEEE), for the zip. */
function crc32(data: Uint8Array): number {
  let c = ~0;
  for (const byte of data) {
    c ^= byte;
    for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
  }
  return ~c >>> 0;
}

/** A stored (uncompressed) zip with fixed dates, so the same files always give the same bytes. */
export function zip(files: { name: string; data: Uint8Array }[]): Uint8Array {
  const parts: Uint8Array[] = [];
  const central: Uint8Array[] = [];
  let offset = 0;
  // 2026-01-01 00:00 in DOS date and time.
  const date = ((2026 - 1980) << 9) | (1 << 5) | 1;
  for (const f of files) {
    const name = new TextEncoder().encode(f.name);
    const crc = crc32(f.data);
    const local = new Uint8Array(30 + name.length);
    const lv = new DataView(local.buffer);
    lv.setUint32(0, 0x04034b50, true);
    lv.setUint16(4, 10, true);
    lv.setUint16(12, date, true);
    lv.setUint32(14, crc, true);
    lv.setUint32(18, f.data.length, true);
    lv.setUint32(22, f.data.length, true);
    lv.setUint16(26, name.length, true);
    local.set(name, 30);
    const cen = new Uint8Array(46 + name.length);
    const cv = new DataView(cen.buffer);
    cv.setUint32(0, 0x02014b50, true);
    cv.setUint16(4, 20, true);
    cv.setUint16(6, 10, true);
    cv.setUint16(14, date, true);
    cv.setUint32(16, crc, true);
    cv.setUint32(20, f.data.length, true);
    cv.setUint32(24, f.data.length, true);
    cv.setUint16(28, name.length, true);
    cv.setUint32(42, offset, true);
    cen.set(name, 46);
    parts.push(local, f.data);
    central.push(cen);
    offset += local.length + f.data.length;
  }
  const size = central.reduce((n, c) => n + c.length, 0);
  const end = new Uint8Array(22);
  const ev = new DataView(end.buffer);
  ev.setUint32(0, 0x06054b50, true);
  ev.setUint16(8, files.length, true);
  ev.setUint16(10, files.length, true);
  ev.setUint32(12, size, true);
  ev.setUint32(16, offset, true);
  const all = [...parts, ...central, end];
  const out = new Uint8Array(all.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of all) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

const esc = (s: string): string =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const b64 = (d: Uint8Array): string => Buffer.from(d).toString('base64');

const FRENCH = 'Ébauche, à côté, déjà vu, crème brûlée. L’œuvre naît à Québec — « Ça va ? » Où est le Noël ?';
const ALL_LATIN = 'ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖ×ØÙÚÛÜÝÞß àáâãäåæçèéêëìíîïðñòóôõö÷øùúûüýþÿ ŒœŸ';
const SYMBOLS = '¡¢£¤¥¦§¨©ª«¬®¯°±²³´µ¶·¸¹º»¼½¾¿ — – ‘’ “” ‚„ … • ‹› € ← ↑ → ↓ ↔ ↕ ► ◄ ▲ ▼ ● ○ ■ □ ✓ ✗ 📁 💾';
const COMPARE: Record<string, string> = {
  ui: 'Ébauche, à côté · 128 KB · Il1 0O',
  title: 'Files · C:\\FIZZ — Œuvre',
  fixed: 'C:\\>DIR ╔═╗ 0O Il1',
  small: 'Ln 5, Col 32 · 3:06 p.m.',
};
const FRAME = ['┌────┬────╖ ╔════╗ ╭──╮', '│░░░░│▒▒▒▒║ ║▓▓▓▓║ │██│', '├────┼────╢ ╠════╣ ╰──╯', '└────┴────╜ ╚════╝ ▀▄▌▐'];

/** The specimen lines for one face, as HTML. */
function samples(face: FaceDef): string {
  const line = (text: string, cls = ''): string => `<div class="ln ${cls}">${esc(text)}</div>`;
  const mock: Record<string, string> = {
    ui: `<div class="ln menu"><span>File</span><span class="hl">Edit</span><span>View</span><span>Go</span><span>Help</span></div>
      <div class="ln">${esc('Name ▲          Size   Modified')}</div>
      <div class="ln sel">${esc('📁 FIZZ        912 KB   22 Oct')}</div>
      <div class="ln">${esc('You can restore it from Deleted until you empty it.')}</div>`,
    title: `<div class="ln bar">${esc('Files · C:\\FIZZ')}</div><div class="ln">${esc('Audio Player · Cocoa Caper overture')}</div><div class="ln">${esc('Move TODAY.TXT to Deleted?')}</div>`,
    fixed: `${line('C:\\GAMES\\KEEN4>DIR /W')}${line('pub extern "C" fn app_start() -> i32 { 0 }')}${line('Il1| O0Ø 8B 5S 2Z ;: ,. `\'"')}`,
    small: `<div class="ln bar">${esc('Ln 5, Col 32 · 128 characters · C: 408.9 MB free')}</div>${line('Wrap on · Redraw 14 ms · 3 dirty rects · 3:06 p.m.')}${line('Using disk for memory')}`,
  };
  return [
    line('ABCDEFGHIJKLMNOPQRSTUVWXYZ'),
    line('abcdefghijklmnopqrstuvwxyz'),
    line('0123456789 !"#$%&\'()*+,-./:;<=>?@[\\]^_`{|}~'),
    line(FRENCH),
    line(ALL_LATIN),
    line(SYMBOLS),
    mock[face.id] ?? '',
    `<div class="frame">${FRAME.map((l) => line(l)).join('')}</div>`,
  ].join('\n');
}

function faceData(face: FaceDef, glyphs: Glyph[]): unknown {
  return {
    id: face.id,
    family: face.family,
    height: face.height,
    ascent: face.ascent,
    capTop: face.capTop,
    bold: !!face.bold,
    glyphs: glyphs.map((g) => ({
      cp: g.cp,
      ch: g.ch,
      adv: g.adv,
      rows: format(g.rows),
      kind: g.kind,
      from: g.from,
      final: g.final,
      src: g.source === undefined ? undefined : typeof g.source === 'string' ? { rows: g.source, top: 0 } : g.source,
    })),
  };
}

function faceSection(face: FaceDef, glyphs: Glyph[]): string {
  const m = metrics(face);
  const fam = `'${face.family}'`;
  const advance =
    face.cell !== undefined
      ? `${face.cell} px for every glyph`
      : `drawn width + ${face.gap} px; space ${face.space} px; box drawing ${face.boxWidth} px`;
  const drawn = glyphs.filter((g) => g.kind === 'drawn').length;
  const composed = glyphs.filter((g) => g.kind === 'composed').length;
  const generated = glyphs.filter((g) => g.kind === 'generated').length;
  const rows: [string, string][] = [
    ['Cell', `${face.cell ?? 'proportional'}${face.cell ? ' × ' : ', '}${face.height} rows`],
    ['Em', `${face.size} px = ${m.upm} units (1 px = 100)`],
    ['Ascent / descent', `${face.ascent} / ${face.height - face.ascent} rows (${m.ascender} / ${m.descender})`],
    ['Cap / x-height', `${face.ascent - face.capTop} / ${face.ascent - face.xTop} rows`],
    ['Advance', advance],
    ['Glyphs', `${glyphs.length}: ${drawn} drawn, ${composed} composed, ${generated} generated`],
    ['Files', `${face.file}.otf · ${face.dfn}`],
  ];
  return `
<section class="face" id="${face.id}" aria-labelledby="h-${face.id}">
  <header class="face-head">
    <h2 id="h-${face.id}" style="font-family:${fam};font-size:${face.size * 3}px;line-height:${face.height * 3}px">${esc(face.family)}</h2>
    <p class="purpose">${esc(face.purpose.replace(' x ', ' × '))}${face.bold ? '. Drawn as one-pixel skeletons and emboldened by the build, with hand-drawn bold forms where that fails.' : '.'}</p>
  </header>
  <dl class="metrics">${rows.map(([k, v]) => `<div><dt>${k}</dt><dd>${esc(v)}</dd></div>`).join('')}</dl>
  <div class="spec-tools">
    <span class="label">Rendered with the built ${esc(face.file)}.otf at</span>
    <div class="scale" role="group" aria-label="Scale" data-scale-for="spec-${face.id}">
      <button type="button" data-n="1" aria-pressed="false">1×</button>
      <button type="button" data-n="2" aria-pressed="true">2×</button>
      <button type="button" data-n="3" aria-pressed="false">3×</button>
    </div>
  </div>
  <div class="screen-wrap"><div class="screen face-${face.id}" id="spec-${face.id}" style="--n:2;--fs:${face.size}px;--lh:${face.height}px;font-family:${fam}">
${samples(face)}
  </div></div>
  <div class="compare">${(['square', 'round'] as const)
    .map(
      (style) => `<figure><figcaption class="label">${style === 'square' ? 'Square' : 'Round'} · ${esc(face.file)}${style === 'round' ? '-round' : ''}.ttf / .otf</figcaption>${[1, 2, 3]
        .map(
          (n) =>
            `<div class="screen-wrap"><div class="screen mini" style="--n:${n};--fs:${face.size}px;--lh:${face.height}px;font-family:'${face.family}${style === 'round' ? ' Round' : ''}'"><div class="ln">${esc(COMPARE[face.id] ?? '')}</div></div></div>`,
        )
        .join('')}</figure>`,
    )
    .join('')}</div>
  <div class="chart-head">
    <h3>Every glyph, as the build draws it</h3>
    <label class="label" for="filter-${face.id}">Show</label>
    <select id="filter-${face.id}">
      <option value="all">All ${glyphs.length}</option>
      <option value="ascii">ASCII</option>
      <option value="latin">Latin-1 and French</option>
      <option value="box">Box drawing and blocks</option>
      <option value="other">Symbols and code page 437</option>
    </select>
  </div>
  <p class="hint">Each tile is the glyph's full cell at 3 px per pixel, with the baseline marked; the caption is the code point and the advance in pixels. Select a tile to enlarge or edit it.</p>
  <div class="chart" id="chart-${face.id}"></div>
</section>`;
}

/** Text in Desk Fixed DOS: each byte becomes the private-use code point its symbol cmap uses. */
const dosText = (bytes: number[]): string => bytes.map((b) => `&#x${(0xf000 + b).toString(16)};`).join('');

/** The Desk Fixed DOS section: a box-drawing frame typed as DOS bytes and the code page 437 chart. */
function dosSection(): string {
  const row = (l: number, fill: number, mid: number, r: number): number[] => [l, ...Array(6).fill(fill), mid, ...Array(6).fill(fill), r];
  const text = (t: string): number[] => Array.from(t, (c) => c.charCodeAt(0));
  const frame = [
    row(0xc9, 0xcd, 0xcb, 0xbb),
    [0xba, ...text(' DESK '), 0xba, ...text(' 0.1  '), 0xba],
    row(0xcc, 0xcd, 0xce, 0xb9),
    [0xba, 0xb0, 0xb0, 0xb1, 0xb1, 0xb2, 0xb2, 0xba, 0xdb, 0xdf, 0xdc, 0xdd, 0xde, 0xfe, 0xba],
    row(0xc8, 0xcd, 0xca, 0xbc),
    [0x01, 0x20, 0x02, 0x20, 0x03, 0x20, 0x04, 0x20, 0x05, 0x20, 0x06, 0x20, 0x0e, 0x20, 0x0f, 0x20, 0x10, 0x11, 0x18, 0x19, 0x1a, 0x1b, 0x7f],
  ];
  const cells = Array.from({ length: 16 }, (_, hi) =>
    `<tr><th scope="row">${hi.toString(16).toUpperCase()}x</th>${Array.from({ length: 16 }, (_, lo) => `<td><span class="dos">${dosText([hi * 16 + lo])}</span></td>`).join('')}</tr>`,
  ).join('');
  return `
<section class="face" id="dos" aria-labelledby="h-dos">
  <header class="face-head">
    <h2 id="h-dos" style="font-family:'Desk Fixed DOS';font-size:48px;line-height:48px">${dosText(text('Desk Fixed DOS'))}</h2>
    <p class="purpose">Desk Fixed with its 256 glyphs at their code page 437 byte values, for Windows 3.1-era programs and anything that wants the OEM layout. In the TrueType file it is a symbol font (byte <em>b</em> at U+F000 + <em>b</em>) with a Unicode-platform table beside it; the OpenType file maps the real code points and the private-use copies. Not tried on Windows 3.1 itself.</p>
  </header>
  <div class="screen-wrap"><div class="screen" style="--n:2;--fs:16px;--lh:16px;font-family:'Desk Fixed DOS'">${frame.map((l) => `<div class="ln">${dosText(l)}</div>`).join('')}</div></div>
  <p class="hint">The frame above is typed as DOS bytes (U+F000 + byte). The same glyphs at their real Unicode code points:</p>
  <div class="screen-wrap"><div class="screen" style="--n:2;--fs:16px;--lh:16px;font-family:'Desk Fixed DOS'"><div class="ln">${esc('☺☻♥♦♣♠ ╔═╦═╗ ░▒▓█ ▀▄▌▐ αßΓπΣσµτ ≡±≥≤⌠⌡÷≈°√ⁿ²■')}</div></div></div>
  <h3>Code page 437, by byte</h3>
  <div class="table-wrap"><table class="cp437"><tr><th></th>${Array.from({ length: 16 }, (_, i) => `<th scope="col">x${i.toString(16).toUpperCase()}</th>`).join('')}</tr>${cells}</table></div>
</section>`;
}

const PARAGRAPH =
  'Desk keeps your files, programs and notes in one calm place. Open a window, drag a file to the desktop, and run any DOS program with the whole machine to itself; the Desk comes back when it exits. Les accents du français, comme à Québec ou à Montréal, s’affichent sans détour : élève, garçon, Noël, cœur.';
const CODE = `fn app_start(desk: &mut Desk) -> i32 {
    let win = desk.open("Notes", 640, 480)?;   // 0O Il1 {}[]
    for (i, line) in lines.iter().enumerate() {
        win.text(8, 16 * i as i32, line);      // == != <= >=
    }
    0
}`;

/** The Modern section: the outline faces at real sizes, beside the pixel faces, a proof sheet each and a tester. */
function modernSection(): string {
  const fams = [
    { name: 'Desk Sans', css: "'Desk Sans'", note: 'UI and body text, figures tabular. Regular for text, Bold for titles.' },
    { name: 'Desk Mono', css: "'Desk Mono'", note: 'Code and terminals: one advance, a dotted zero, a tailed l, slab i, serifed I and 1, box drawing to the cell.' },
    { name: 'Desk Serif', css: "'Desk Serif'", note: 'Long reading (Notes, documents): moderate contrast, slab serifs with brackets, lining tabular figures.' },
  ];
  const sizes = [11, 12, 14, 16, 18, 24, 32, 48];
  const proof = (css: string, mono: boolean): string => `
    <div class="mod-proof" style="font-family:${css}">
      <div class="mp-big">Aa Gg Rr 357 &amp;@</div>
      <div>ABCDEFGHIJKLMNOPQRSTUVWXYZ</div>
      <div>abcdefghijklmnopqrstuvwxyz</div>
      <div>0123456789 ¼ ½ ¾ ² ³ % ‰ € £ ¥ ¢</div>
      <div>!?.,:;…·•–—‘’“”‚„«»‹›()[]{}/\\|@#&amp;*+=&lt;&gt;^~_</div>
      <div>ÀÂÄÆÇÈÉÊËÎÏÔŒÙÛÜŸ àâäæçèéêëîïôœùûüÿ ÁÍÑÓÚÝÞßðøþ Šš Žž</div>
      <div>←↑→↓↔↕ ►◄▲▼ ●○■□ ✓✗ ™ © ® ° ± × ÷</div>
      <p class="mp-para" style="font-weight:400">${esc(PARAGRAPH)}</p>
      <p class="mp-para" style="font-weight:700">${esc(PARAGRAPH.slice(0, 120))}…</p>
      <div class="mp-ui"><span>File</span><span>Edit</span><span>View</span><span>Help</span><span class="mp-dim">C:\\DESK\\NOTES · 12:45 · ✓ Saved</span></div>
      <table class="mp-table"><tr><td>AUTO.SAV</td><td>2 048</td><td>22 Oct</td></tr><tr><td>FIZZ.EXE</td><td>912 384</td><td>21 Oct</td></tr><tr><td>README.TXT</td><td>1 117</td><td>20 Oct</td></tr></table>
      ${mono ? `<pre class="mp-code">${esc(CODE)}\n${esc('┌──────┬──────┐\n│ ░▒▓█ │ ▀▄▌▐ │\n╞══════╪══════╡\n└──────┴──────┘')}</pre>` : ''}
    </div>`;
  return `
<section class="face" id="modern" aria-labelledby="h-modern">
  <header class="face-head">
    <h2 id="h-modern" style="font-family:'Desk Sans';font-weight:700;font-size:40px;line-height:48px">Modern: Desk Sans, Mono and Serif</h2>
    <p class="purpose">Outline faces drawn from stroke skeletons in the build (no tracing of any other font), each Regular and Bold, as OTF and TrueType. They are for larger sizes and modern systems; the pixel faces above stay the Desk's own text at its native sizes. They carry no hinting, so below about 14 px they look soft and uneven. An italic is a later option.</p>
  </header>
  <h3>Sizes, beside the pixel faces</h3>
  <div class="screen-wrap"><div class="mod-sizes">
    ${sizes
      .map(
        (z) => `<div class="ms-row"><span class="ms-z">${z} px</span>
        <span style="font:${z}px/1.25 'Desk Sans'">Files · déjà vu</span>
        <span style="font:700 ${z}px/1.25 'Desk Sans'">Files</span>
        <span style="font:${z}px/1.25 'Desk Mono'">C:\\DESK 0O1l</span>
        <span style="font:${z}px/1.25 'Desk Serif'">Lettre à Noël</span></div>`,
      )
      .join('')}
    <div class="ms-row"><span class="ms-z">pixel</span>
      <span style="font:12px/14px 'Desk UI'">Files · déjà vu</span>
      <span style="font:12px/14px 'Desk Title'">Files</span>
      <span style="font:16px/16px 'Desk Fixed'">C:\\DESK 0O1l</span>
      <span style="font:24px/28px 'Desk UI'">Files · déjà vu (2×)</span></div>
  </div></div>
  <h3>Type tester</h3>
  <div class="tester">
    <label class="label" for="tt-text">Text</label>
    <input id="tt-text" type="text" value="Ébauche, à côté, déjà vu — 0O Il1 {}" aria-label="Text to render">
    <label class="label" for="tt-face">Face</label>
    <select id="tt-face">
      ${['Desk Sans', 'Desk Mono', 'Desk Serif'].flatMap((f) => [`<option value="${f}|400">${f} Regular</option>`, `<option value="${f}|700">${f} Bold</option>`]).join('')}
      <option value="Desk UI|400">Desk UI (pixel)</option><option value="Desk Title|700">Desk Title (pixel)</option><option value="Desk Fixed|400">Desk Fixed (pixel)</option><option value="Desk Small|400">Desk Small (pixel)</option>
      <option value="Desk UI Round|400">Desk UI Round</option><option value="Desk Fixed DOS|400">Desk Fixed DOS</option>
    </select>
    <label class="label" for="tt-size">Size</label>
    <select id="tt-size">${[8, 11, 12, 14, 16, 18, 24, 32, 48, 72].map((z) => `<option value="${z}"${z === 24 ? ' selected' : ''}>${z} px</option>`).join('')}</select>
  </div>
  <div class="screen-wrap"><div id="tt-out" class="tt-out" style="font-family:'Desk Sans';font-size:24px">Ébauche, à côté, déjà vu — 0O Il1 {}</div></div>
  ${fams.map((f) => `<h3>${f.name}</h3><p class="hint">${esc(f.note)}</p>${proof(f.css, f.name === 'Desk Mono')}`).join('')}
</section>`;
}

/** The whole specimen page, ready to publish. */
export function specimen(files: { name: string; data: Uint8Array }[]): string {
  const byName = new Map(files.map((f) => [f.name, f.data]));
  const faces = FACES.map((face) => ({ face, glyphs: assemble(face) }));
  const ofl = new Uint8Array(readFileSync(join(import.meta.dir, '..', 'generated', 'OFL.txt')));
  const bundle = zip([...files.map((f) => ({ name: `desk-fonts/${f.name}`, data: f.data })), { name: 'desk-fonts/OFL.txt', data: ofl }]);
  const fontFaces = faces
    .flatMap(({ face }) =>
      (['', '-round'] as const).map((suffix) => {
        const data = byName.get(`${face.file}${suffix}.otf`) as Uint8Array;
        const family = suffix ? `${face.family} Round` : face.family;
        return `@font-face{font-family:'${family}';src:url(data:font/otf;base64,${b64(data)}) format('opentype');font-weight:${face.style === 'Bold' ? 700 : 400};font-display:block}`;
      }),
    )
    .join('\n') +
    `\n@font-face{font-family:'Desk Fixed DOS';src:url(data:font/otf;base64,${b64(byName.get('desk-fixed-dos.otf') as Uint8Array)}) format('opentype');font-display:block}` +
    STYLES.map(
      (st) =>
        `\n@font-face{font-family:'${st.family}';src:url(data:font/otf;base64,${b64(byName.get(`${st.file}.otf`) as Uint8Array)}) format('opentype');font-weight:${st.bold ? 700 : 400};font-display:block}`,
    ).join('');
  const data = JSON.stringify({ faces: faces.map(({ face, glyphs }) => faceData(face, glyphs)) }).replace(/</g, '\\u003c');
  const client = readFileSync(join(import.meta.dir, 'specimen-client.js'), 'utf8');
  const total = faces.reduce((n, f) => n + f.glyphs.length, 0);
  return `<title>Desk Fonts</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&family=JetBrains+Mono:wght@400;500&display=swap">
<style>
${fontFaces}
/* Layout: one long column per face, each a screen panel of real rendering over a chart of the raw grids. */
:root{
  --bg:#f4f4f8; --surface:#ffffff; --line:#d9dce5; --fg:#12121a; --muted:#5a6273; --accent:#a5560b;
  --tile:#eceef3; --ink:#12121a; --baseline:#e0a35a;
  --screen-bg:#ffffff; --screen-fg:#12121a; --screen-dim:#5a6273; --screen-hl:#ffe2bd; --screen-sel:#fbe7cf;
  --font-body:'Inter',-apple-system,'Segoe UI',sans-serif; --font-mono:'JetBrains Mono','Cascadia Code',monospace;
  --font-display:'Desk Title','Inter',sans-serif;
}
@media (prefers-color-scheme: dark){:root:not([data-theme="light"]){
  --bg:#050507; --surface:#12121a; --line:#262a37; --fg:#e0e0e0; --muted:#8d97a8; --accent:#ffaa40;
  --tile:#12121a; --ink:#e0e0e0; --baseline:#7d5422;
  --screen-bg:#0c0c10; --screen-fg:#e0e0e0; --screen-dim:#8d97a8; --screen-hl:#372612; --screen-sel:#1b1e28; color-scheme:dark}}
:root[data-theme="dark"]{
  --bg:#050507; --surface:#12121a; --line:#262a37; --fg:#e0e0e0; --muted:#8d97a8; --accent:#ffaa40;
  --tile:#12121a; --ink:#e0e0e0; --baseline:#7d5422;
  --screen-bg:#0c0c10; --screen-fg:#e0e0e0; --screen-dim:#8d97a8; --screen-hl:#372612; --screen-sel:#1b1e28; color-scheme:dark}
*{box-sizing:border-box}
body{background:var(--bg);color:var(--fg);font:15px/1.6 var(--font-body);padding-left:max(16px,env(safe-area-inset-left,0px));padding-right:max(16px,env(safe-area-inset-right,0px));padding-block:32px 64px}
main{max-width:1080px;margin:0 auto;display:flex;flex-direction:column;gap:56px}
h1,h2,h3{margin:0;font-weight:normal;text-wrap:balance}
h1{font-family:var(--font-display);font-size:36px;line-height:42px;color:var(--fg)}
h3{font-family:var(--font-display);font-size:24px;line-height:28px}
p{margin:0;max-width:68ch}
a{color:var(--accent)}
code,pre,.mono{font-family:var(--font-mono);font-size:13px}
.label{font-size:12px;letter-spacing:.04em;text-transform:uppercase;color:var(--muted)}
.intro{display:flex;flex-direction:column;gap:16px}
.intro .lede{font-size:17px;color:var(--fg)}
.facts{display:flex;flex-wrap:wrap;gap:8px 20px;color:var(--muted);font-size:13px}
.status{border-left:3px solid var(--accent);padding:4px 0 4px 14px;color:var(--fg);font-size:14px}
nav.toc{display:flex;flex-wrap:wrap;gap:8px}
nav.toc a{font-family:'Desk UI',sans-serif;font-size:22px;line-height:28px;color:var(--fg);text-decoration:none;border:1px solid var(--line);padding:4px 12px;background:var(--surface)}
nav.toc a:hover,nav.toc a:focus-visible{border-color:var(--accent);outline:none}
.actions{display:flex;flex-wrap:wrap;gap:10px;align-items:center}
button{font:500 14px var(--font-body);color:var(--fg);background:var(--surface);border:1px solid var(--line);padding:6px 14px;cursor:pointer}
button:hover{border-color:var(--accent)}
button:focus-visible,select:focus-visible,.tile:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
button.primary{background:var(--accent);color:var(--bg);border-color:var(--accent)}
button:disabled{opacity:.45;cursor:default}
select{font:14px var(--font-body);color:var(--fg);background:var(--surface);border:1px solid var(--line);padding:5px 8px}
.face{display:flex;flex-direction:column;gap:18px;border-top:1px solid var(--line);padding-top:28px}
.face-head{display:flex;flex-direction:column;gap:6px}
.face-head h2{color:var(--fg)}
.purpose{color:var(--muted)}
.metrics{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:10px 24px;margin:0}
.metrics div{min-width:0}
.metrics dt{font-size:12px;letter-spacing:.04em;text-transform:uppercase;color:var(--muted)}
.metrics dd{margin:0;font-size:14px;font-variant-numeric:tabular-nums}
.spec-tools,.chart-head{display:flex;flex-wrap:wrap;align-items:center;gap:10px}
.chart-head h3{flex:1 1 100%}
.scale{display:inline-flex}
.scale button{padding:3px 12px;font-size:13px}
.scale button+button{border-left:none}
.scale button[aria-pressed="true"]{background:var(--accent);color:var(--bg);border-color:var(--accent)}
.screen-wrap{overflow-x:auto;border:1px solid var(--line);background:var(--screen-bg)}
.screen{padding:16px;color:var(--screen-fg);font-size:calc(var(--fs) * var(--n));line-height:calc(var(--lh) * var(--n));-webkit-font-smoothing:none;font-smooth:never;text-rendering:optimizeSpeed;width:max-content;min-width:100%}
.screen .ln{white-space:pre;min-height:calc(var(--lh) * var(--n))}
.screen .frame{margin-top:calc(var(--lh) * var(--n) / 2)}
.screen .menu{display:flex;gap:0}
.screen .menu span{padding:0 calc(8px * var(--n))}
.screen .menu .hl,.screen .bar{background:var(--screen-hl)}
.screen .sel{background:var(--screen-sel)}
.screen .bar{padding:0 calc(4px * var(--n))}
.hint{color:var(--muted);font-size:13px}
.compare{display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:16px}
.compare figure{margin:0;display:flex;flex-direction:column;gap:6px;min-width:0}
.screen.mini{padding:8px 12px}
.chart{display:flex;flex-wrap:wrap;gap:6px}
.tile{display:flex;flex-direction:column;align-items:center;gap:3px;padding:6px;background:var(--surface);border:1px solid var(--line);min-width:52px}
.tile canvas{image-rendering:pixelated}
.tile-cap{font:10px/1 var(--font-mono);color:var(--muted);font-variant-numeric:tabular-nums}
.tile.edited{border-color:var(--accent)}
dialog{border:1px solid var(--line);background:var(--surface);color:var(--fg);padding:20px;max-width:min(760px,calc(100vw - 32px));width:100%}
dialog::backdrop{background:rgba(5,5,7,.6)}
.ed-top{display:flex;gap:16px;align-items:flex-start;justify-content:space-between}
#ed-char{font-family:var(--font-body);font-size:40px;line-height:1}
.ed-body{display:flex;flex-wrap:wrap;gap:20px;margin-top:14px}
.ed-grid-wrap{overflow:auto;max-width:100%}
#ed-grid{display:grid;grid-template-columns:repeat(var(--cols),20px);gap:1px;background:var(--line);border:1px solid var(--line);width:max-content}
.px{width:20px;height:20px;background:var(--tile);border:0;padding:0;display:block}
.px.on{background:var(--ink)}
.px.base{box-shadow:inset 0 -3px 0 var(--baseline)}
.px.cap{box-shadow:inset 0 3px 0 var(--line)}
.ed-side{display:flex;flex-direction:column;gap:12px;min-width:0;flex:1 1 220px}
.previews{display:flex;gap:12px;align-items:flex-end;background:var(--screen-bg);padding:10px;border:1px solid var(--line)}
.previews canvas{image-rendering:pixelated}
#ed-source,#edits-text{white-space:pre-wrap;word-break:break-all;background:var(--bg);border:1px solid var(--line);padding:10px;margin:0;font-size:12px;max-height:240px;overflow:auto}
.notes{display:flex;flex-direction:column;gap:12px}
.notes ul{margin:0;padding-left:20px;display:flex;flex-direction:column;gap:6px;max-width:72ch}
pre.cmd{background:var(--surface);border:1px solid var(--line);padding:12px 14px;overflow-x:auto;margin:0}
table.dfn{border-collapse:collapse;font-size:14px}
table.dfn td,table.dfn th{border-bottom:1px solid var(--line);padding:6px 12px 6px 0;text-align:left;vertical-align:top}
.table-wrap{overflow-x:auto}
.mod-sizes{padding:12px 16px;color:var(--screen-fg);display:flex;flex-direction:column;gap:6px;width:max-content;min-width:100%}
.ms-row{display:flex;align-items:baseline;gap:28px;white-space:nowrap}
.ms-z{font:11px var(--font-mono);color:var(--muted);width:44px}
.tester{display:flex;flex-wrap:wrap;gap:8px 12px;align-items:center}
.tester input{flex:1 1 260px;min-width:0;font:15px var(--font-body);padding:6px 8px;border:1px solid var(--line);background:var(--surface);color:var(--fg)}
.tt-out{padding:16px;color:var(--screen-fg);white-space:pre-wrap;word-break:break-word;line-height:1.3}
.mod-proof{background:var(--screen-bg);color:var(--screen-fg);border:1px solid var(--line);padding:16px;display:flex;flex-direction:column;gap:6px;font-size:18px;line-height:1.35;overflow-x:auto}
.mp-big{font-size:56px;line-height:1.1}
.mp-para{max-width:64ch;margin:4px 0}
.mp-ui{display:flex;gap:16px;flex-wrap:wrap;font-size:14px;border-top:1px solid var(--line);padding-top:6px}
.mp-dim{color:var(--muted)}
.mp-table{border-collapse:collapse;font-size:15px}
.mp-table td{padding:2px 16px 2px 0}
.mp-table td+td{text-align:right}
.mp-code{font:inherit;font-size:15px;line-height:1.2;margin:6px 0 0;white-space:pre}
table.cp437{border-collapse:collapse;background:var(--screen-bg);color:var(--screen-fg)}
table.cp437 th{font:11px var(--font-mono);color:var(--muted);padding:2px 6px;font-weight:normal}
table.cp437 td{border:1px solid var(--line);padding:0;width:24px;text-align:center}
table.cp437 .dos{font-family:'Desk Fixed DOS';font-size:16px;line-height:16px;display:block;-webkit-font-smoothing:none}
@media (prefers-reduced-motion: reduce){*{transition:none!important}}
</style>
<main>
  <section class="intro" aria-labelledby="top">
    <span class="label">Liminal HQ · Desk · bitmap faces</span>
    <h1 id="top">Desk Fonts</h1>
    <p class="lede">Four pixel faces for Desk, the graphical shell for MS-DOS. Each glyph is a grid of pixels written once in <code>fonts/src/faces/*.ts</code>. The build traces those grids into OpenType outlines at 100 units a pixel and writes the same pixels as <code>.DFN</code> files for the DOS side.</p>
    <div class="facts"><span>${total} glyphs in 4 faces</span><span>ASCII · code page 1252 · Latin-1 · French Œ œ Ÿ · code page 437 box drawing and blocks</span><span>Square and Round · OTF and TrueType</span><span>SIL Open Font License 1.1</span></div>
    <p class="status">Everything on this page is generated from the build's own data: the samples use the built OTF files, and the charts draw the grids the build traced. FreeType renders the TrueType and OTF files pixel-exact at the native sizes. Not yet checked: the faces have not been seen in the Desk itself, in DOSBox or on a real VGA screen, no DOS program reads the <code>.DFN</code> files yet, and the TrueType files have not been installed on Windows 3.1 or OS/2.</p>
    <nav class="toc" aria-label="Faces">${faces.map(({ face }) => `<a href="#${face.id}" style="font-family:'${face.family}';font-size:${face.size * 2}px;line-height:${face.height * 2}px">${esc(face.family)}</a>`).join('')}<a href="#dos" style="font-family:'Desk Fixed';font-size:32px;line-height:32px">Desk Fixed DOS</a><a href="#modern" style="font-family:'Desk Sans';font-size:24px;line-height:32px">Modern</a></nav>
    <div class="actions">
      <button type="button" class="primary" id="zip-save" data-needs-downloads hidden>Save desk-fonts.zip</button>
      <span class="hint" id="no-downloads">The OTF and .DFN files are in <code>fonts/generated/</code> in the repository; this view cannot save files.</span>
      <span class="hint" data-needs-downloads hidden>Ten OTF and ten TrueType files (Square and Round, with Desk Fixed DOS), four .DFN files and the licence, ${Math.round(bundle.length / 1024)} KB.</span>
    </div>
  </section>
${faces.map(({ face, glyphs }) => faceSection(face, glyphs)).join('\n')}
${dosSection()}
${modernSection()}
  <section class="notes" aria-labelledby="h-edits">
    <h3 id="h-edits">Your edits</h3>
    <p>Edits stay in this browser only. Paste the lines into the named face file, replacing the glyph's existing line, then rebuild.</p>
    <div class="actions"><span class="label" id="edits-count">0 edited glyphs</span>
      <button type="button" id="edits-copy">Copy lines</button>
      <button type="button" id="edits-save" data-needs-downloads hidden>Save as .txt</button>
      <button type="button" id="edits-clear">Discard all</button></div>
    <pre id="edits-text" class="mono"></pre>
  </section>
  <section class="notes" aria-labelledby="h-build">
    <h3 id="h-build">Building</h3>
    <pre class="cmd">cd fonts
bun install                 # opentype.js 2.0.0, pinned
bun build.ts                # write generated/*.otf and generated/*.dfn
bun build.ts --check        # fail if generated/ is stale (tools/check.sh runs this)
./validate.sh               # optional: fc-scan, otfinfo, then fontTools, OTS and FreeType in Docker
bun test                    # coverage, cell rules, traced area = ink, round trips</pre>
    <ul>
      <li>One pixel is 100 font units and the em is the face's pixel size, so the faces are exact at whole multiples of 12, 12, 16 and 8 px: 9, 9, 12 and 6 pt at 96 dpi. Desk UI is 11 px type on a 12 px em. Set <code>line-height</code> to the cell height times the scale, or box drawing will not join.</li>
      <li>Outlines are the traced pixel edges: neighbours merged, holes wound the other way, straight runs reduced to corners. A test checks that every glyph's outline area equals its ink.</li>
      <li>Each face is built in two styles, Square and Round, as both OpenType (CFF) and TrueType files. Round softens every outer corner with a small curve that only removes ink, so it keeps every metric and is a drop-in; the .DFN files stay square. The TrueType files are built for Windows 3.1-era systems: format 4 and Mac Roman cmaps, OS/2 version 1, post format 2.0, no hinting. FreeType renders every glyph pixel-exact at the native size and twice it. Not tried on real Windows 3.1.</li>
      <li>Accented letters are composed from a base and a mark; box drawing and blocks are generated to the whole cell; Desk Title is emboldened from skeletons.</li>
    </ul>
    <div class="table-wrap"><table class="dfn">
      <tr><th>.DFN field</th><th>Size</th><th>Value</th></tr>
      <tr><td>Magic</td><td>4</td><td><code>DFN1</code></td></tr>
      <tr><td>Height</td><td>1</td><td>rows in every glyph (the cell height)</td></tr>
      <tr><td>Baseline</td><td>1</td><td>rows above the baseline</td></tr>
      <tr><td>Glyph count</td><td>2</td><td>little-endian; glyphs follow in code-point order</td></tr>
      <tr><td>Per glyph</td><td>4 + 1 + rows</td><td>code point (little-endian), width (the advance, gap included), then <em>height</em> rows of ⌈width ÷ 8⌉ bytes, leftmost pixel in the highest bit</td></tr>
    </table></div>
  </section>
</main>
<dialog id="editor" aria-labelledby="ed-title">
  <div class="ed-top"><div><span id="ed-char"></span><h3 id="ed-title"></h3><p class="hint" id="ed-meta"></p></div><button type="button" id="ed-close">Close</button></div>
  <p id="ed-where"></p>
  <div class="ed-body">
    <div class="ed-grid-wrap"><div id="ed-grid"></div></div>
    <div class="ed-side">
      <span class="label">Result at 1×, 2× and 3×</span>
      <div class="previews"><canvas id="ed-prev-1"></canvas><canvas id="ed-prev-2"></canvas><canvas id="ed-prev-3"></canvas></div>
      <div class="actions"><button type="button" id="ed-narrower">− column</button><button type="button" id="ed-wider">+ column</button><button type="button" id="ed-reset">Reset</button><button type="button" id="ed-copy" class="primary">Copy source line</button></div>
      <pre id="ed-source" class="mono"></pre>
    </div>
  </div>
</dialog>
<script type="application/json" id="font-data">${data}</script>
<script type="text/plain" id="zip-data">${b64(bundle)}</script>
<script>
${client}
</script>
`;
}
