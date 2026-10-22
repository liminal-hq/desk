// The Desk Emoji page's script: contact sheets drawn from the build's grids and paths, the style, mono, background
// and zoom switches, search, the detail panel, the list of the set, the tester and the emoji zip. DATA comes from
// emoji-data.js, written by the site build.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

const $ = (s) => document.querySelector(s);
const E = DATA.emoji;
const CATS = DATA.cats;
const hex = (cp) => 'U+' + cp.toString(16).toUpperCase().padStart(4, '0');
const state = { style: '16', ink: 'colour', ground: 'dark', zoom: 3, q: '' };
const hand12 = E.filter((e) => e.o12 === 'drawn').length, fixed12 = E.filter((e) => e.o12 === 'fixed').length;
const nv = E.filter((e) => e.v).length, n8 = E.filter((e) => e.p8).length;
$('#stats').innerHTML = [[E.length, 'pixel emoji, every one drawn at 16 × 16'], [hand12, 'hand-drawn at 12 × 12, ' + fixed12 + ' more hand-fixed'], [n8, 'hand-drawn at 8 × 8'], [nv, 'vector emoji'], [16, 'palette colours'], ...(DATA.planned ? [[DATA.planned, 'more planned: new pictures, skin tones and sequences (docs/EMOJI-ROADMAP.md)']] : [])]
  .map(([n, t]) => `<div><b>${n}</b><span>${t}</span></div>`).join('');

function drawPixel(canvas, e, size, mono, k) {
  const src = e['p' + size], m = e['m' + size];
  canvas.width = size; canvas.height = size;
  canvas.style.width = canvas.style.height = size * k + 'px';
  const ctx = canvas.getContext('2d');
  for (let i = 0; i < size * size; i++) {
    if (mono) { if (m[i] === '1') { ctx.fillStyle = state.ground === 'light' || canvas.dataset.light ? '#050507' : '#e0e0e0'; ctx.fillRect(i % size, (i / size) | 0, 1, 1); } }
    else if (src[i] !== '.') { ctx.fillStyle = DATA.letters[src[i]]; ctx.fillRect(i % size, (i / size) | 0, 1, 1); }
  }
}
function vectorSvg(e, px, mono, light) {
  const ns = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(ns, 'svg');
  svg.setAttribute('viewBox', '0 0 1000 1000'); svg.setAttribute('width', px); svg.setAttribute('height', px);
  svg.setAttribute('role', 'img'); svg.setAttribute('aria-label', e.name);
  const add = (d, fill) => { const p = document.createElementNS(ns, 'path'); p.setAttribute('d', d); p.setAttribute('fill', fill); svg.appendChild(p); };
  if (mono) add(e.vm, light ? '#050507' : '#e0e0e0'); else for (const [c, d] of e.v) add(d, DATA.vcolours[c]);
  return svg;
}
const matches = (e) => {
  if (!state.q) return true;
  const q = state.q.toLowerCase();
  return e.name.includes(q) || e.ch === state.q || hex(e.cp).toLowerCase().includes(q.replace(/^u\+?/, '')) || CATS[e.cat].toLowerCase().includes(q) || e.tier === q;
};
function renderSheets() {
  const root = $('#cats'); root.textContent = '';
  const vector = state.style === 'vector', size = vector ? 16 : Number(state.style);
  const light = state.ground === 'light', mono = state.ink === 'mono';
  let shown = 0;
  for (const [cat, title] of Object.entries(CATS)) {
    const items = E.filter((e) => e.cat === cat && matches(e) && (vector ? e.v : e['p' + size]));
    if (!items.length) continue;
    shown += items.length;
    const box = document.createElement('div'); box.className = 'cat';
    const h = document.createElement('h3'); h.textContent = `${title} · ${items.length}`; box.appendChild(h);
    const tiles = document.createElement('div'); tiles.className = 'tiles';
    for (const e of items) {
      const b = document.createElement('button'); b.type = 'button'; b.className = 'tile' + (light ? ' light' : '');
      b.title = `${e.name} ${hex(e.cp)}`; b.setAttribute('aria-label', `${e.name}, ${hex(e.cp)}`);
      if (vector) b.appendChild(vectorSvg(e, 16 * state.zoom * (state.zoom === 1 ? 1.5 : 1), mono, light));
      else { const c = document.createElement('canvas'); drawPixel(c, e, size, mono, state.zoom); b.appendChild(c); }
      b.addEventListener('click', () => openDetail(e));
      tiles.appendChild(b);
    }
    box.appendChild(tiles); root.appendChild(box);
  }
  const total = vector ? nv : E.filter((e) => e['p' + size]).length;
  $('#count').textContent = `${shown} of ${total}`;
  $('#legend').textContent = vector
    ? 'Vector: flat geometric layers drawn in code, the colours of Desk Emoji. Mono is the base glyph of the colour font and the glyph in Desk Sans, Mono and Serif.'
    : size === 16 ? 'Pixel 16 × 16: every emoji, drawn by hand. This is Desk Emoji Pixel and the glyph in Desk Fixed (two cells wide).'
    : size === 12 ? `Pixel 12 × 12: ${hand12} drawn by hand, the rest derived from the 16 × 16 by removing rows and columns (${fixed12} of those fixed by hand). Used in Desk UI and Desk Title.`
    : 'Pixel 8 × 8: the 40 core emoji, drawn by hand for Desk Small.';
  renderRows();
}
function renderRows() {
  const rows = E.filter(matches).map((e) => `<tr><td class="e">${e.ch}</td><td class="cp">${hex(e.cp)}</td><td>${e.name}</td><td>${CATS[e.cat]}</td><td><span class="pill ${e.tier}">${e.tier}</span></td><td>${e.o12}</td><td>${e.p8 ? 'drawn' : '—'}</td><td>${e.v ? 'yes' : '—'}</td></tr>`);
  $('#rows').innerHTML = rows.join('') || '<tr><td colspan="8">No emoji match that search.</td></tr>';
}
function openDetail(e) {
  const d = $('#detail'); d.hidden = false; d.textContent = '';
  d.innerHTML = `<header><div><h2 id="d-name">${e.name}</h2><span class="note">${hex(e.cp)} · ${CATS[e.cat]} · ${e.tier} · 12 × 12 ${e.o12}</span></div><button class="close" type="button" id="dclose">Close</button></header>`;
  for (const mono of [false, true]) {
    const row = document.createElement('div'); row.className = 'row';
    for (const size of [16, 12, 8]) {
      if (!e['p' + size]) continue;
      const cell = document.createElement('div'); cell.className = 'cell';
      const bx = document.createElement('div'); bx.className = 'box' + (mono ? ' light' : '');
      const c = document.createElement('canvas'); if (mono) c.dataset.light = '1';
      drawPixel(c, e, size, mono, size === 8 ? 6 : size === 12 ? 4 : 4);
      bx.appendChild(c); cell.appendChild(bx); cell.append(`${size} px ${mono ? 'mono' : 'colour'}`); row.appendChild(cell);
    }
    if (e.v) {
      const cell = document.createElement('div'); cell.className = 'cell';
      const bx = document.createElement('div'); bx.className = 'box' + (mono ? ' light' : '');
      bx.appendChild(vectorSvg(e, 64, mono, mono)); cell.appendChild(bx); cell.append(`vector ${mono ? 'mono' : 'colour'}`); row.appendChild(cell);
    }
    d.appendChild(row);
  }
  $('#dclose').addEventListener('click', () => { d.hidden = true; });
  $('#dclose').focus();
}
document.addEventListener('keydown', (ev) => { if (ev.key === 'Escape') $('#detail').hidden = true; });

function group(id, key, after) {
  $(id).addEventListener('click', (ev) => {
    const b = ev.target.closest('button'); if (!b) return;
    for (const x of $(id).querySelectorAll('button')) x.setAttribute('aria-pressed', String(x === b));
    state[key] = key === 'zoom' ? Number(b.dataset.v) : b.dataset.v; after();
  });
}
group('#style', 'style', renderSheets); group('#ink', 'ink', renderSheets); group('#ground', 'ground', renderSheets); group('#zoom', 'zoom', renderSheets);
$('#q').addEventListener('input', (ev) => { state.q = ev.target.value.trim(); renderSheets(); });

const FACES = [
  { name: 'Desk Fixed (pixel emoji, mono, 16 px)', family: '"DE Fixed"', sizes: [16, 32, 48], note: 'Desk Fixed carries every pixel emoji at 16 × 16, two cells wide, in its OpenType file.' },
  { name: 'Desk UI (pixel emoji, mono, 12 px)', family: '"DE UI"', sizes: [12, 24, 36], note: 'Desk UI carries the 12 × 12 emoji in its OpenType file.' },
  { name: 'Desk Title (pixel emoji, mono, 12 px)', family: '"DE Title"', sizes: [12, 24, 36], note: 'Desk Title carries the 12 × 12 emoji in its OpenType file.' },
  { name: 'Desk Small (core emoji, mono, 8 px)', family: '"DE Small"', sizes: [8, 16, 24], note: 'Desk Small carries only the 40 core emoji at 8 × 8; the others fall back to another font.' },
  { name: 'Desk Emoji Pixel (colour) with Desk UI', family: '"DE Pixel", "DE UI"', sizes: [24, 48], note: 'Colour pixel emoji through COLR. Desk Emoji Pixel is exact at multiples of 16 px and Desk UI at multiples of 12 px, so 48 px is exact for both.' },
  { name: 'Desk Sans (vector emoji, mono)', family: '"DE Sans"', sizes: [16, 24, 36], note: 'Desk Sans carries the 153 vector emoji in its OpenType file; the others fall back to another font.' },
  { name: 'Desk Mono (vector emoji, mono, two cells)', family: '"DE Mono"', sizes: [16, 24, 36], note: 'In Desk Mono each emoji is two cells wide, so columns stay aligned.' },
  { name: 'Desk Emoji (colour) with Desk Sans', family: '"DE Vector", "DE Pixel", "DE Sans"', sizes: [16, 24, 36, 48], note: 'Vector colour emoji, with Desk Emoji Pixel filling in the emoji that have no vector drawing.' },
];
const face = $('#face'), size = $('#size');
FACES.forEach((f, i) => face.add(new Option(f.name, i)));
function sizes() { const f = FACES[face.value]; size.textContent = ''; f.sizes.forEach((s) => size.add(new Option(s + ' px', s))); size.value = f.sizes[f.sizes.length > 2 ? 1 : 0]; }
function renderTest() {
  const f = FACES[face.value]; const p = $('#preview');
  p.style.fontFamily = f.family + ', "IBM Plex Sans", sans-serif'; p.style.fontSize = size.value + 'px'; p.style.lineHeight = '1.25';
  p.textContent = $('#text').value; $('#facenote').textContent = f.note;
}
face.addEventListener('change', () => { sizes(); renderTest(); }); size.addEventListener('change', renderTest); $('#text').addEventListener('input', renderTest);
$('#tground').addEventListener('click', (ev) => {
  const b = ev.target.closest('button'); if (!b) return;
  for (const x of $('#tground').querySelectorAll('button')) x.setAttribute('aria-pressed', String(x === b));
  $('#preview').className = 'preview ' + b.dataset.v;
});
face.value = 4; sizes(); size.value = 48; renderTest();

const kb = (n) => n > 1024 * 1024 ? (n / 1048576).toFixed(1) + ' MB' : Math.round(n / 1024) + ' KB';
$('#filelist').innerHTML = DATA.files.map(([n, s]) => `<div><code>emoji/${n}</code><span class="note">${kb(s)}</span></div>`).join('');

renderSheets();

// A zip of the emoji files, built here: stored (not compressed), which the format allows and every unzip reads.
const CRC = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
const crc32 = (b) => { let c = 0xffffffff; for (const v of b) c = CRC[(c ^ v) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
function zip(entries) {
  const enc = new TextEncoder(), parts = [], dir = []; let off = 0;
  for (const [name, data] of entries) {
    const n = enc.encode(name), c = crc32(data), h = new DataView(new ArrayBuffer(30));
    h.setUint32(0, 0x04034b50, true); h.setUint16(4, 20, true); h.setUint16(8, 0, true); h.setUint16(12, 0x5621, true); h.setUint16(10, 0, true);
    h.setUint32(14, c, true); h.setUint32(18, data.length, true); h.setUint32(22, data.length, true); h.setUint16(26, n.length, true);
    const d = new DataView(new ArrayBuffer(46));
    d.setUint32(0, 0x02014b50, true); d.setUint16(4, 20, true); d.setUint16(6, 20, true); d.setUint16(14, 0x5621, true);
    d.setUint32(16, c, true); d.setUint32(20, data.length, true); d.setUint32(24, data.length, true); d.setUint16(28, n.length, true); d.setUint32(42, off, true);
    parts.push(new Uint8Array(h.buffer), n, data); dir.push(new Uint8Array(d.buffer), n);
    off += 30 + n.length + data.length;
  }
  const size = dir.reduce((a, b) => a + b.length, 0), e = new DataView(new ArrayBuffer(22));
  e.setUint32(0, 0x06054b50, true); e.setUint16(8, entries.length, true); e.setUint16(10, entries.length, true); e.setUint32(12, size, true); e.setUint32(16, off, true);
  return new Blob([...parts, ...dir, new Uint8Array(e.buffer)]);
}
(async () => {
  const downloads = window.claude ? await window.claude.use('downloads') : null;
  if (!downloads) return;
  const btn = $('#dl'); btn.hidden = false;
  btn.addEventListener('click', async () => {
    btn.disabled = true;
    try {
      const entries = [];
      for (const [n] of DATA.files) {
        const res = await fetch('fonts/emoji/' + n);
        if (!res.ok) throw { code: 'missing', message: n + ' could not be loaded' };
        entries.push(['desk-emoji/emoji/' + n, new Uint8Array(await res.arrayBuffer())]);
      }
      const ofl = await fetch('fonts/OFL.txt');
      if (ofl.ok) entries.push(['desk-emoji/OFL.txt', new Uint8Array(await ofl.arrayBuffer())]);
      await downloads.save({ filename: 'desk-emoji.zip', data: zip(entries) });
      $('#dlnote').textContent = 'Saved desk-emoji.zip.';
    } catch (err) {
      $('#dlnote').textContent = err && err.code === 'declined' ? 'Download cancelled.' : 'The download is not available here: ' + ((err && err.message) || 'unknown error') + '. The files are in fonts/generated/emoji/ in the repository.';
    } finally { btn.disabled = false; }
  });
})();
