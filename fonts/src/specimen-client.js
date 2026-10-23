// The specimen page's script: glyph charts drawn from the build's own grids, scale switches, and a pixel editor
// that exports source lines for the face files.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

(() => {
  const DATA = JSON.parse(document.getElementById('font-data').textContent);
  const ZIP = document.getElementById('zip-data').textContent.trim();
  const $ = (sel, root = document) => root.querySelector(sel);
  const css = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const hex = (cp) => 'U+' + cp.toString(16).toUpperCase().padStart(4, '0');
  const parse = (s) => s.split('/').map((r) => Array.from(r, (c) => (c === '#' ? 1 : 0)));
  const store = {
    get(k, d) {
      try {
        const v = localStorage.getItem(k);
        return v ? JSON.parse(v) : d;
      } catch {
        return d;
      }
    },
    set(k, v) {
      try {
        localStorage.setItem(k, JSON.stringify(v));
      } catch {
        /* storage unavailable: edits last for this visit only */
      }
    },
  };

  // Same rule as embolden() in fonts/src/grid.ts.
  function embolden(g) {
    return g.map((r) => {
      const o = new Array(r.length + 1).fill(0);
      r.forEach((v, x) => {
        if (!v) return;
        o[x] = 1;
        if (!r[x + 2] || r[x + 1]) o[x + 1] = 1;
        else if (x > 0 && !r[x - 1] && !r[x - 2]) o[x - 1] = 1;
      });
      return o;
    });
  }

  function draw(canvas, rows, px, opts = {}) {
    const h = rows.length;
    const w = Math.max(1, ...rows.map((r) => r.length));
    canvas.width = w * px;
    canvas.height = h * px;
    const c = canvas.getContext('2d');
    c.fillStyle = css('--tile');
    c.fillRect(0, 0, canvas.width, canvas.height);
    if (opts.ascent !== undefined) {
      c.fillStyle = css('--baseline');
      c.fillRect(0, opts.ascent * px, canvas.width, Math.max(1, px >> 2));
    }
    c.fillStyle = opts.ink || css('--ink');
    rows.forEach((r, y) => r.forEach((v, x) => v && c.fillRect(x * px, y * px, px, px)));
  }

  const GROUPS = {
    all: () => true,
    ascii: (cp) => cp >= 0x20 && cp <= 0x7e,
    latin: (cp) => (cp >= 0xa0 && cp <= 0xff) || cp === 0x131 || (cp >= 0x152 && cp <= 0x178),
    box: (cp) => cp >= 0x2500 && cp <= 0x259f,
    other: (cp) => !(cp <= 0xff || (cp >= 0x2500 && cp <= 0x259f) || cp === 0x131 || (cp >= 0x152 && cp <= 0x178)),
  };

  // ---- glyph charts ----
  for (const face of DATA.faces) {
    const chart = $(`#chart-${face.id}`);
    const filter = $(`#filter-${face.id}`);
    const render = () => {
      chart.textContent = '';
      const keep = GROUPS[filter.value];
      for (const g of face.glyphs) {
        if (!keep(g.cp)) continue;
        const b = document.createElement('button');
        b.type = 'button';
        b.className = 'tile';
        b.title = `${g.ch} ${hex(g.cp)}, ${g.adv} px`;
        b.setAttribute('aria-label', `${hex(g.cp)} ${g.ch}, ${g.adv} pixels wide, ${g.kind}`);
        const cv = document.createElement('canvas');
        draw(cv, parse(g.rows), 3, { ascent: face.ascent });
        const cap = document.createElement('span');
        cap.className = 'tile-cap';
        cap.textContent = `${g.cp.toString(16).toUpperCase().padStart(4, '0')} · ${g.adv}`;
        b.append(cv, cap);
        if (edits[face.id]?.[g.cp]) b.classList.add('edited');
        b.addEventListener('click', () => openEditor(face, g));
        chart.append(b);
      }
    };
    filter.addEventListener('change', render);
    face.render = render;
  }

  // ---- scale switches on the specimens ----
  for (const group of document.querySelectorAll('[data-scale-for]')) {
    const target = $(`#${group.dataset.scaleFor}`);
    group.addEventListener('click', (e) => {
      const btn = e.target.closest('button[data-n]');
      if (!btn) return;
      target.style.setProperty('--n', btn.dataset.n);
      for (const b of group.querySelectorAll('button')) b.setAttribute('aria-pressed', String(b === btn));
    });
  }

  // ---- the editor ----
  let edits = store.get('desk-font-edits', {});
  const dlg = $('#editor');
  let cur = null;

  const sourceCanvas = (face, src) => {
    const g = parse(src.rows);
    const w = Math.max(1, ...g.map((r) => r.length));
    const rows = Array.from({ length: face.height }, () => new Array(w).fill(0));
    g.forEach((r, y) => r.forEach((v, x) => (rows[face.capTop + src.top + y][x] = v)));
    return rows;
  };

  function sourceLine(face, ch, rows) {
    const used = rows.map((r) => r.some((v) => v));
    let a = used.indexOf(true);
    let b = used.lastIndexOf(true);
    if (a < 0) a = b = face.capTop;
    const text = rows.slice(a, b + 1).map((r) => "'" + r.map((v) => (v ? '#' : '.')).join('') + "'");
    const key = /^[A-Za-z_$][\w$]*$/.test(ch) ? ch : "'" + ch.replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
    const call = a === face.capTop ? `R(${text.join(', ')})` : `at(${a}, ${text.join(', ')})`;
    return `  ${key}: ${call},`;
  }

  function openEditor(face, g) {
    const editable = g.kind === 'drawn' && g.src;
    const saved = edits[face.id]?.[g.cp];
    const rows = saved ? saved.rows.map((r) => r.slice()) : editable ? sourceCanvas(face, g.src) : parse(g.rows);
    cur = { face, g, rows, editable };
    $('#ed-char').textContent = g.ch;
    $('#ed-title').textContent = `${hex(g.cp)} in ${face.family}`;
    const where = !editable
      ? g.kind === 'composed'
        ? `Composed from ${g.from}: edit the base letter or the face's marks instead.`
        : g.kind === 'generated'
          ? 'Generated to fill the cell (boxes.ts), so lines join: not drawn by hand.'
          : 'The space: blank, with the face’s space advance.'
      : face.bold && !g.final
        ? 'Drawn one pixel wide in title.ts (glyphs); the build emboldens it. Click pixels to edit the skeleton.'
        : face.bold
          ? 'A hand-drawn bold form in title.ts (boldFixes). Click pixels to edit it.'
          : `Drawn in ${face.id}.ts. Click pixels to edit.`;
    $('#ed-where').textContent = where;
    $('#ed-meta').textContent = `Advance ${g.adv} px · cell ${face.height} rows, baseline under row ${face.ascent - 1} · ${g.kind}`;
    for (const id of ['ed-copy', 'ed-reset', 'ed-wider', 'ed-narrower']) $('#' + id).hidden = !editable;
    paint();
    if (!dlg.open) dlg.showModal();
  }

  function finalRows() {
    const { face, g, rows, editable } = cur;
    if (!editable) return rows;
    return face.bold && !g.final ? embolden(rows) : rows;
  }

  function paint() {
    const { face, rows, editable } = cur;
    const grid = $('#ed-grid');
    grid.textContent = '';
    grid.style.setProperty('--cols', rows[0].length);
    rows.forEach((r, y) =>
      r.forEach((v, x) => {
        const cell = document.createElement(editable ? 'button' : 'span');
        cell.className = 'px' + (v ? ' on' : '') + (y === face.ascent - 1 ? ' base' : '') + (y === face.capTop ? ' cap' : '');
        if (editable) {
          cell.type = 'button';
          cell.setAttribute('aria-label', `row ${y}, column ${x}, ${v ? 'ink' : 'blank'}`);
          cell.addEventListener('click', () => {
            r[x] = r[x] ? 0 : 1;
            changed();
          });
        }
        grid.append(cell);
      }),
    );
    const fin = finalRows();
    for (const n of [1, 2, 3]) draw($(`#ed-prev-${n}`), fin, n, { ink: css('--screen-fg') });
    $('#ed-source').textContent = editable ? sourceLine(face, cur.g.ch, rows) : '';
    $('#ed-source').hidden = !editable;
  }

  function changed() {
    const { face, g, rows } = cur;
    edits[face.id] = edits[face.id] || {};
    edits[face.id][g.cp] = { ch: g.ch, rows, line: sourceLine(face, g.ch, rows), final: !!g.final };
    store.set('desk-font-edits', edits);
    paint();
    listEdits();
  }

  $('#ed-wider').addEventListener('click', () => {
    cur.rows.forEach((r) => r.push(0));
    changed();
  });
  $('#ed-narrower').addEventListener('click', () => {
    if (cur.rows[0].length > 1) cur.rows.forEach((r) => r.pop());
    changed();
  });
  $('#ed-reset').addEventListener('click', () => {
    delete edits[cur.face.id]?.[cur.g.cp];
    store.set('desk-font-edits', edits);
    cur.rows = sourceCanvas(cur.face, cur.g.src);
    paint();
    listEdits();
  });
  $('#ed-close').addEventListener('click', () => dlg.close());
  dlg.addEventListener('close', () => cur && cur.face.render());

  async function copy(text, btn) {
    const old = btn.textContent;
    try {
      await navigator.clipboard.writeText(text);
      btn.textContent = 'Copied';
    } catch {
      const pre = document.createElement('textarea');
      pre.value = text;
      btn.after(pre);
      pre.select();
      btn.textContent = 'Select and copy below';
    }
    setTimeout(() => (btn.textContent = old), 1600);
  }
  $('#ed-copy').addEventListener('click', (e) => copy(sourceLine(cur.face, cur.g.ch, cur.rows), e.currentTarget));

  function editsText() {
    const out = [];
    for (const face of DATA.faces) {
      const mine = edits[face.id];
      if (!mine || !Object.keys(mine).length) continue;
      const fixes = Object.values(mine).filter((e) => e.final);
      const plain = Object.values(mine).filter((e) => !e.final);
      if (plain.length) out.push(`// fonts/src/faces/${face.id}.ts, glyphs`, ...plain.map((e) => e.line), '');
      if (fixes.length) out.push(`// fonts/src/faces/${face.id}.ts, boldFixes`, ...fixes.map((e) => e.line), '');
    }
    return out.join('\n');
  }

  function listEdits() {
    const n = Object.values(edits).reduce((a, m) => a + Object.keys(m).length, 0);
    $('#edits-count').textContent = n === 1 ? '1 edited glyph' : `${n} edited glyphs`;
    $('#edits-text').textContent = n ? editsText() : 'No edits yet. Click any drawn glyph in a chart to edit it.';
    for (const id of ['edits-copy', 'edits-clear']) $('#' + id).disabled = !n;
    $('#edits-save').disabled = !n;
  }
  $('#edits-copy').addEventListener('click', (e) => copy(editsText(), e.currentTarget));
  $('#edits-clear').addEventListener('click', () => {
    edits = {};
    store.set('desk-font-edits', edits);
    listEdits();
    for (const f of DATA.faces) f.render();
  });

  // ---- files: the zip of OTF, DFN and the licence, and the edits as text ----
  const b64 = (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
  let downloads = null;
  const showSave = () => {
    for (const el of document.querySelectorAll('[data-needs-downloads]')) el.hidden = !downloads;
    $('#no-downloads').hidden = !!downloads;
  };
  async function save(filename, data, btn) {
    const old = btn.textContent;
    try {
      await downloads.save({ filename, data });
      btn.textContent = 'Saved';
    } catch (err) {
      btn.textContent = err && err.code === 'declined' ? 'Not saved' : 'Saving is unavailable here';
    }
    setTimeout(() => (btn.textContent = old), 2000);
  }
  $('#zip-save').addEventListener('click', (e) => save('desk-fonts.zip', b64(ZIP), e.currentTarget));
  $('#edits-save').addEventListener('click', (e) => save('desk-font-edits.txt', editsText(), e.currentTarget));
  showSave();
  if (window.claude && window.claude.use) {
    window.claude.use('downloads').then((d) => {
      downloads = d;
      showSave();
    }, () => showSave());
  }

  // ---- the type tester ----
  const tt = { text: $('#tt-text'), face: $('#tt-face'), size: $('#tt-size'), italic: $('#tt-italic'), out: $('#tt-out') };
  if (tt.out) {
    const feats = [...document.querySelectorAll('[data-feature]')];
    const show = () => {
      const [family, weight] = tt.face.value.split('|');
      tt.out.style.fontFamily = `'${family}'`;
      tt.out.style.fontWeight = weight;
      tt.out.style.fontStyle = tt.italic && tt.italic.checked ? 'italic' : 'normal';
      tt.out.style.fontSize = `${tt.size.value}px`;
      // Every switch is stated, so turning off kern, liga or calt overrides the browser's defaults.
      tt.out.style.fontFeatureSettings = feats.map((el) => `"${el.dataset.feature}" ${el.checked ? 1 : 0}`).join(', ');
      tt.out.textContent = tt.text.value || ' ';
    };
    for (const el of [tt.text, tt.face, tt.size, tt.italic, ...feats]) if (el) el.addEventListener('input', show);
    show();
  }

  for (const f of DATA.faces) f.render();
  listEdits();
})();
