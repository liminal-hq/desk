// The Desk Type Specimens site: five pages with a shared header (Overview, Pixel faces, Outline faces, Emoji and
// Downloads), the built font files served beside them, a zip of every file, and a manifest of every published path
// for publishing the site as a multi-file artifact.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { roadmap } from '../emoji/requests';
import { FACES } from '../faces';
import { STYLES } from '../modern/kit';
import { BUILDING, EDITOR, SPECIMEN_CSS, esc, fontData, fontFacesCss, outlineBody, pixelBody, zip } from '../specimen';
import { emojiPage } from './emoji';

/** The folder the font files are published in, relative to the pages. */
export const FONTS_DIR = 'fonts';
export const REPO = 'https://github.com/liminal-hq/desk';

export interface SiteFile {
  /** The published path, relative to the site's root. */
  path: string;
  data: Uint8Array;
  contentType: string;
}

const PAGES = [
  { path: 'index.html', href: './', label: 'Overview', title: 'Desk Type Specimens' },
  { path: 'pixel.html', href: 'pixel.html', label: 'Pixel faces', title: 'Desk Pixel Faces' },
  { path: 'outline.html', href: 'outline.html', label: 'Outline faces', title: 'Desk Outline Faces' },
  { path: 'emoji.html', href: 'emoji.html', label: 'Emoji', title: 'Desk Emoji' },
  { path: 'downloads.html', href: 'downloads.html', label: 'Downloads', title: 'Desk Font Downloads' },
] as const;
type PagePath = (typeof PAGES)[number]['path'];

/** The header navigation, with the current page marked. */
export function nav(current: PagePath): string {
  return `<nav class="site-nav" aria-label="Pages"><span class="site-name">Desk Type Specimens</span>${PAGES.map(
    (p) => `<a href="${p.href}"${p.path === current ? ' aria-current="page"' : ''}>${p.label}</a>`,
  ).join('')}</nav>`;
}

const TYPES: Record<string, string> = {
  html: 'text/html; charset=utf-8',
  css: 'text/css; charset=utf-8',
  js: 'text/javascript; charset=utf-8',
  json: 'application/json',
  otf: 'font/otf',
  ttf: 'font/ttf',
  png: 'image/png',
  txt: 'text/plain; charset=utf-8',
  zip: 'application/zip',
  dfn: 'application/octet-stream',
  dem: 'application/octet-stream',
};
const typeOf = (path: string): string => TYPES[path.slice(path.lastIndexOf('.') + 1)] ?? 'application/octet-stream';

const GOOGLE_FONTS = `<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&family=JetBrains+Mono:wght@400;500&display=swap">`;

/**
 * A Desk Fonts page. The overview is the artifact's main page, which the viewer wraps in its own document skeleton,
 * so it has no doctype or head of its own; the others are whole documents.
 */
function page(path: PagePath, body: string, after = '', scripts: string[] = ['site.js', 'specimen.js']): string {
  const p = PAGES.find((x) => x.path === path) as (typeof PAGES)[number];
  const head = `<title>${p.title}</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="stylesheet" href="site.css">
${GOOGLE_FONTS}
<link rel="stylesheet" href="fonts.css">
<link rel="stylesheet" href="specimen.css">`;
  const content = `<main>
${nav(path)}
${body}
</main>
${after}
${scripts.map((s) => `<script src="${s}"></script>`).join('\n')}
`;
  if (path === 'index.html') return `${head}\n${content}`;
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
${head}
</head>
<body>
${content}</body>
</html>
`;
}

const size = (n: number): string => (n >= 1024 * 1024 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
/** The file types the viewer's downloads capability accepts, of those the site publishes. */
const SAVEABLE = new Set(['ttf', 'png', 'txt', 'zip']);

/** The overview: what each family is for, which file to use where, what has been checked, and how it is built. */
function overviewBody(files: { name: string; data: Uint8Array }[], zipBytes: number): string {
  const rows = roadmap();
  const done = rows.filter((r) => r.status === 'done').length;
  const fam = (name: string, href: string, what: string, styles: string, files: string): string =>
    `<tr><td><a href="${href}">${name}</a></td><td>${what}</td><td>${styles}</td><td>${files}</td></tr>`;
  const sys = (system: string, pixel: string, outline: string, emoji: string, install: string): string =>
    `<tr><th scope="row">${system}</th><td>${pixel}</td><td>${outline}</td><td>${emoji}</td><td>${install}</td></tr>`;
  const emojiCount = files.filter((f) => f.name.startsWith('emoji/')).length;
  return `
  <section class="intro" aria-labelledby="top">
    <span class="label">Liminal HQ · Desk · type</span>
    <h1 id="top">Desk Fonts</h1>
    <p class="lede">The type of Desk, the graphical shell for MS-DOS: four pixel faces for the Desk's own screens, three outline families for larger sizes and modern systems, and emoji in pixel and vector form. Everything is generated from the build's own data in <code>fonts/</code> of the repository.</p>
    <div class="facts"><span>${FACES.length} pixel faces, Square and Round</span><span>${STYLES.length / 4} outline families in four styles</span><span>OTF, TrueType, .DFN and .DEM</span><span>SIL Open Font License 1.1</span></div>
    <div class="actions">
      <button type="button" class="primary" data-save="desk-fonts.zip" data-needs-downloads hidden>Save desk-fonts.zip</button>
      <span class="hint" data-needs-downloads hidden>Every file on the Downloads page and the licence, ${size(zipBytes)}.</span>
      <span class="hint" data-no-downloads>The files are on the <a href="downloads.html">Downloads</a> page and in <code>fonts/generated/</code> in the repository.</span>
    </div>
  </section>
  <section class="notes" aria-labelledby="h-families">
    <h3 id="h-families">The families</h3>
    <div class="table-wrap"><table class="dfn">
      <tr><th>Family</th><th>For</th><th>Styles</th><th>Files</th></tr>
      ${fam('Desk UI', 'pixel.html#ui', 'Menus, lists, buttons and body text: 11 px type on a 12 px em', 'Square, Round', '<code>desk-ui.otf</code>, <code>.ttf</code>, <code>ui.dfn</code>')}
      ${fam('Desk Title', 'pixel.html#title', 'Window titles and headings, bold', 'Square, Round', '<code>desk-title.otf</code>, <code>.ttf</code>, <code>title.dfn</code>')}
      ${fam('Desk Fixed', 'pixel.html#fixed', 'Terminal, code and paths, 8 × 16 cells; Desk Fixed DOS addresses its glyphs by code page 437 byte', 'Square, Round', '<code>desk-fixed.otf</code>, <code>.ttf</code>, <code>fixed.dfn</code>, <code>desk-fixed-dos.*</code>')}
      ${fam('Desk Small', 'pixel.html#small', 'Status bars and badges, 6 × 8 cells', 'Square, Round', '<code>desk-small.otf</code>, <code>.ttf</code>, <code>small.dfn</code>')}
      ${fam('Desk Sans', 'outline.html', 'UI and body text at larger sizes', 'Regular, Bold, Italic, Bold Italic', '<code>desk-sans*.otf</code>, <code>.ttf</code>')}
      ${fam('Desk Mono', 'outline.html', 'Code and terminals, with code ligatures', 'Regular, Bold, Italic, Bold Italic', '<code>desk-mono*.otf</code>, <code>.ttf</code>')}
      ${fam('Desk Serif', 'outline.html', 'Long reading: Notes and documents', 'Regular, Bold, Italic, Bold Italic', '<code>desk-serif*.otf</code>, <code>.ttf</code>')}
      ${fam('Desk Emoji Pixel', 'emoji.html', 'Colour pixel emoji (COLR) beside any font; the pixel faces carry the same emoji in monochrome', 'Colour', '<code>emoji/desk-emoji-pixel.ttf</code>, <code>.otf</code>, <code>emoji*.dem</code>')}
      ${fam('Desk Emoji', 'emoji.html', 'Colour vector emoji (COLR) beside Desk Sans, Mono and Serif, which carry them in monochrome', 'Colour', '<code>emoji/desk-emoji.ttf</code>, <code>.otf</code>')}
    </table></div>
  </section>
  <section class="notes" aria-labelledby="h-which">
    <h3 id="h-which">Which file to use</h3>
    <div class="table-wrap"><table class="dfn">
      <tr><th>System</th><th>Pixel faces</th><th>Outline faces</th><th>Emoji</th><th>Installing</th></tr>
      ${sys('Windows 3.1 and 3.11', '<code>.ttf</code>; Desk Fixed DOS for code page 437 bytes', '<code>.ttf</code>', 'None: the <code>.ttf</code> files carry no emoji, and most emoji are beyond the Basic Multilingual Plane', 'Control Panel → Fonts → Add…, with TrueType enabled')}
      ${sys('Windows 95, NT 3.5x and 4', '<code>.ttf</code>', '<code>.ttf</code>', 'None', 'Control Panel → Fonts → File → Install New Font…')}
      ${sys('OS/2 Warp', '<code>.ttf</code>', '<code>.ttf</code>', 'None', 'System Setup → Font Palette → Edit font → Add')}
      ${sys('Windows 10 and 11', '<code>.otf</code> (monochrome emoji included)', '<code>.otf</code>', 'Desk Emoji Pixel or Desk Emoji for colour, listed before the text face', 'Right-click → Install')}
      ${sys('Linux', '<code>.otf</code>', '<code>.otf</code>', 'As Windows 10', 'Copy to <code>~/.local/share/fonts/</code>, run <code>fc-cache -f</code>')}
      ${sys('macOS', '<code>.otf</code>', '<code>.otf</code>', 'As Windows 10', 'Open the file → Install Font')}
      ${sys('Browsers', '<code>.otf</code> in <code>@font-face</code>', '<code>.otf</code>', '<code>desk-emoji-pixel.ttf</code> listed first for colour', '<code>@font-face</code>')}
      ${sys('DOS (the Desk)', '<code>.dfn</code>', '—', '<code>emoji16.dem</code>, <code>emoji12.dem</code>, <code>emoji8.dem</code>', 'Read by the Desk')}
    </table></div>
    <p class="hint">Do not install both formats of one family on the same system. The pixel faces are exact at whole multiples of their size (12, 12, 16 and 8 px; 9, 9, 12 and 6 pt at 96 dpi).</p>
  </section>
  <section class="notes" aria-labelledby="h-checked">
    <h3 id="h-checked">Checked and not checked</h3>
    <ul>
      <li><b>Checked:</b> fontTools reads and OTS sanitises every file; FreeType renders every pixel glyph exactly at its native size and twice it, compared with the <code>.DFN</code> bitmaps; every colour emoji renders through COLR exactly against the <code>.DEM</code> files; HarfBuzz shapes the outline faces' kerning, ligatures and features; headless Chromium shows every face and both colour fonts.</li>
      <li><b>Not checked:</b> no DOS program reads the <code>.DFN</code> or <code>.DEM</code> files yet; the TrueType files have not been installed on real Windows 3.1, 95, NT 3.5x or OS/2; Windows, macOS, Safari and Firefox have not rendered the outline faces or the colour fonts; nothing has been seen in the Desk, in DOSBox or on a VGA screen.</li>
      <li><b>Emoji still to come:</b> ${rows.length - done} of the ${rows.length} emoji Scott asked for from Gboard's "Smileys and emotions" are planned (new pictures, skin tones and ZWJ sequences); <code>docs/EMOJI-ROADMAP.md</code> lists each one.</li>
    </ul>
    <p>Source: <a href="${REPO}">liminal-hq/desk</a>; the fonts are in <a href="${REPO}/pull/1">pull request #1</a> and the emoji in <a href="${REPO}/pull/2">pull request #2</a>. The reference is <code>docs/FONTS.md</code> and <code>docs/EMOJI.md</code>.</p>
  </section>
${BUILDING}
`;
}

/** The downloads: every published font file by family and format, with its size, a link and a save button. */
function downloadsBody(files: { name: string; data: Uint8Array }[], zipBytes: number): string {
  const byName = new Map(files.map((f) => [f.name, f.data]));
  const row = (name: string, what: string): string => {
    const data = byName.get(name);
    if (!data) throw new Error(`downloads: no file ${name}`);
    const path = `${FONTS_DIR}/${name}`;
    const ext = name.slice(name.lastIndexOf('.') + 1);
    const save = SAVEABLE.has(ext)
      ? `<button type="button" data-save="${path}" data-needs-downloads hidden>Save</button>`
      : '<span class="hint">in the zip</span>';
    return `<tr><td><a href="${path}"><code>${esc(name)}</code></a></td><td>${what}</td><td class="num">${size(data.length)}</td><td>${save}</td></tr>`;
  };
  const table = (id: string, title: string, rows: string[]): string => `
  <section class="notes" aria-labelledby="h-${id}">
    <h3 id="h-${id}">${title}</h3>
    <div class="table-wrap"><table class="dfn dl"><tr><th>File</th><th>What</th><th>Size</th><th></th></tr>${rows.join('')}</table></div>
  </section>`;
  const pixel = FACES.flatMap((f) => [
    row(`${f.file}.otf`, `${f.family}, OpenType, with the monochrome emoji`),
    row(`${f.file}.ttf`, `${f.family}, TrueType for Windows 3.1-era systems`),
    row(`${f.file}-round.otf`, `${f.family} Round, OpenType`),
    row(`${f.file}-round.ttf`, `${f.family} Round, TrueType`),
    row(f.dfn, `${f.family} for the Desk on DOS (.DFN)`),
  ]);
  const dos = ['desk-fixed-dos.otf', 'desk-fixed-dos.ttf', 'desk-fixed-dos-round.otf', 'desk-fixed-dos-round.ttf'].map((n) =>
    row(n, `Desk Fixed DOS${n.includes('round') ? ' Round' : ''}, ${n.endsWith('.otf') ? 'OpenType' : 'TrueType symbol font'}`),
  );
  const outline = STYLES.flatMap((s) => {
    const style = `${s.family} ${[s.bold ? 'Bold' : '', s.italic ? 'Italic' : ''].filter(Boolean).join(' ') || 'Regular'}`;
    return [row(`${s.file}.otf`, `${style}, OpenType, with the monochrome vector emoji`), row(`${s.file}.ttf`, `${style}, TrueType`)];
  });
  const emoji = files
    .filter((f) => f.name.startsWith('emoji/'))
    .map((f) => {
      const n = f.name.slice('emoji/'.length);
      const what = n.startsWith('desk-emoji-pixel')
        ? 'Desk Emoji Pixel, colour (COLR)'
        : n.startsWith('desk-emoji')
          ? 'Desk Emoji, vector colour (COLR)'
          : n.endsWith('.dem')
            ? `Every ${n.replace(/\D/g, '')} × ${n.replace(/\D/g, '')} emoji for the Desk on DOS (.DEM)`
            : `Contact sheet at ${n.replace(/\D/g, '')} × ${n.replace(/\D/g, '')}`;
      return row(f.name, `${what}, ${n.slice(n.lastIndexOf('.') + 1).toUpperCase()}`);
    });
  return `
  <section class="intro" aria-labelledby="top">
    <span class="label">Liminal HQ · Desk · files</span>
    <h1 id="top">Downloads</h1>
    <p class="lede">Every built file, as the build writes it to <code>fonts/generated/</code>. The fonts are under the SIL Open Font License 1.1.</p>
    <div class="actions">
      <button type="button" class="primary" data-save="desk-fonts.zip" data-needs-downloads hidden>Save desk-fonts.zip</button>
      <span class="hint">desk-fonts.zip holds all ${files.length - 1} files and the licence, ${size(zipBytes)}.</span>
    </div>
    <p class="hint">Each name links to the file itself. Where this view can save files, a Save button offers the TrueType, PNG and licence files one at a time; the viewer saves only those types, so the OTF, .DFN and .DEM files come in the zip. <span data-no-downloads>This view cannot save files: use the links, or <code>fonts/generated/</code> in the repository.</span></p>
  </section>
  ${table('pixel', 'Pixel faces', pixel)}
  ${table('dos', 'Desk Fixed DOS', dos)}
  ${table('outline', 'Outline faces', outline)}
  ${table('emoji', 'Emoji', emoji)}
  ${table('licence', 'Licence', [row('OFL.txt', 'SIL Open Font License 1.1')])}
`;
}

/**
 * Every file of the site. `files` are the build's generated files (names relative to `fonts/generated/`); the
 * licence is read from `fonts/generated/OFL.txt`.
 */
export function site(files: { name: string; data: Uint8Array }[]): SiteFile[] {
  const ofl = new Uint8Array(readFileSync(join(import.meta.dir, '..', '..', 'generated', 'OFL.txt')));
  const all = [...files, { name: 'OFL.txt', data: ofl }];
  const bundle = zip(all.map((f) => ({ name: `desk-fonts/${f.name}`, data: f.data })));
  const text = (s: string): Uint8Array => new TextEncoder().encode(s);
  const pages: Record<string, string> = {
    'index.html': page('index.html', overviewBody(files, bundle.length)),
    'pixel.html': page('pixel.html', pixelBody(), `${EDITOR}\n<script type="application/json" id="font-data">${fontData()}</script>`),
    'outline.html': page('outline.html', outlineBody()),
    'downloads.html': page('downloads.html', downloadsBody(all, bundle.length), '', ['site.js']),
    ...emojiPage(files, nav('emoji.html'), FONTS_DIR, roadmap().filter((r) => r.status === 'planned').length),
    'site.css': readFileSync(join(import.meta.dir, 'site.css'), 'utf8'),
    'site.js': readFileSync(join(import.meta.dir, 'site.js'), 'utf8'),
    'specimen.css': `${SPECIMEN_CSS}\ntable.dl td.num{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}\ntable.dl td{vertical-align:middle}\ntable.dl button{padding:3px 10px;font-size:13px}\n`,
    'fonts.css': `/* The built fonts, served beside the pages. */\n${fontFacesCss(FONTS_DIR)}\n`,
    'specimen.js': readFileSync(join(import.meta.dir, '..', 'specimen-client.js'), 'utf8'),
  };
  return [
    ...Object.entries(pages).map(([path, s]) => ({ path, data: text(s), contentType: typeOf(path) })),
    ...all.map((f) => ({ path: `${FONTS_DIR}/${f.name}`, data: f.data, contentType: typeOf(f.name) })),
    { path: 'desk-fonts.zip', data: bundle, contentType: typeOf('desk-fonts.zip') },
  ];
}

/**
 * The publishing manifest: the main page and every other published path with its media type, in the shape the
 * Artifact tool's `files` parameter takes (sources relative to the site folder, which is the publish's `root`).
 */
export function manifest(files: SiteFile[]): unknown {
  return {
    title: 'Desk Type Specimens',
    file_path: 'index.html',
    capabilities: { downloads: {} },
    files: Object.fromEntries(
      files.filter((f) => f.path !== 'index.html').map((f) => [f.path, { from: f.path, contentType: f.contentType.split(';')[0] }]),
    ),
    sizes: Object.fromEntries(files.map((f) => [f.path, f.data.length])),
  };
}
