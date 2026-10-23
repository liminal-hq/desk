// Shared by the Desk Fonts pages of the site: offers files to save through the viewer's downloads capability. A button
// with `data-save="<path>"` fetches that published file and saves it; a button with `data-zip` builds desk-fonts.zip
// from downloads.json (published paths, and base64 for the files artifacts do not serve) and saves it. Elements with
// `data-needs-downloads` show only where saving works, and `data-no-downloads` only where it does not.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

(() => {
  let downloads = null;
  const show = () => {
    for (const el of document.querySelectorAll('[data-needs-downloads]')) el.hidden = !downloads;
    for (const el of document.querySelectorAll('[data-no-downloads]')) el.hidden = !!downloads;
  };
  show();

  const CRC = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc32 = (b) => {
    let c = 0xffffffff;
    for (const v of b) c = CRC[(c ^ v) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  // A stored zip with the build's fixed date (2026-01-01), the same layout as `zip()` in fonts/src/specimen.ts.
  function zip(entries) {
    const enc = new TextEncoder();
    const date = ((2026 - 1980) << 9) | (1 << 5) | 1;
    const parts = [];
    const dir = [];
    let off = 0;
    for (const [name, data] of entries) {
      const n = enc.encode(name);
      const c = crc32(data);
      const h = new DataView(new ArrayBuffer(30));
      h.setUint32(0, 0x04034b50, true);
      h.setUint16(4, 10, true);
      h.setUint16(12, date, true);
      h.setUint32(14, c, true);
      h.setUint32(18, data.length, true);
      h.setUint32(22, data.length, true);
      h.setUint16(26, n.length, true);
      const d = new DataView(new ArrayBuffer(46));
      d.setUint32(0, 0x02014b50, true);
      d.setUint16(4, 20, true);
      d.setUint16(6, 10, true);
      d.setUint16(14, date, true);
      d.setUint32(16, c, true);
      d.setUint32(20, data.length, true);
      d.setUint32(24, data.length, true);
      d.setUint16(28, n.length, true);
      d.setUint32(42, off, true);
      parts.push(new Uint8Array(h.buffer), n, data);
      dir.push(new Uint8Array(d.buffer), n);
      off += 30 + n.length + data.length;
    }
    const size = dir.reduce((a, b) => a + b.length, 0);
    const e = new DataView(new ArrayBuffer(22));
    e.setUint32(0, 0x06054b50, true);
    e.setUint16(8, entries.length, true);
    e.setUint16(10, entries.length, true);
    e.setUint32(12, size, true);
    e.setUint32(16, off, true);
    return new Blob([...parts, ...dir, new Uint8Array(e.buffer)]);
  }
  const get = async (path) => {
    const res = await fetch(path);
    if (!res.ok) throw { message: `${path} could not be loaded` };
    return new Uint8Array(await res.arrayBuffer());
  };
  async function bundle() {
    const list = JSON.parse(new TextDecoder().decode(await get('downloads.json')));
    const entries = [];
    for (const [name, src] of list.entries)
      entries.push([name, src.path ? await get(src.path) : Uint8Array.from(atob(src.base64), (c) => c.charCodeAt(0))]);
    return { filename: list.zip, data: zip(entries) };
  }

  document.addEventListener('click', async (e) => {
    const btn = e.target.closest('[data-save], [data-zip]');
    if (!btn || !downloads) return;
    const old = btn.textContent;
    btn.disabled = true;
    try {
      if (btn.dataset.zip) {
        btn.textContent = 'Building the zip…';
        await downloads.save(await bundle());
      } else {
        const path = btn.dataset.save;
        await downloads.save({ filename: path.split('/').pop(), data: await get(path) });
      }
      btn.textContent = 'Saved';
    } catch (err) {
      btn.textContent = err && err.code === 'declined' ? 'Not saved' : 'Saving is unavailable here';
    } finally {
      btn.disabled = false;
      setTimeout(() => (btn.textContent = old), 2000);
    }
  });
  if (window.claude && window.claude.use) {
    window.claude.use('downloads').then(
      (d) => {
        downloads = d;
        show();
      },
      () => show(),
    );
  }
})();
