// Shared by the Desk Fonts pages of the site: offers published files to save through the viewer's downloads
// capability. A button with `data-save="<path>"` fetches that file and saves it; elements with `data-needs-downloads`
// show only where saving works, and `data-no-downloads` only where it does not.
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
  document.addEventListener('click', async (e) => {
    const btn = e.target.closest('[data-save]');
    if (!btn || !downloads) return;
    const path = btn.dataset.save;
    const old = btn.textContent;
    btn.disabled = true;
    try {
      const res = await fetch(path);
      if (!res.ok) throw { message: `${path} could not be loaded` };
      await downloads.save({ filename: path.split('/').pop(), data: new Uint8Array(await res.arrayBuffer()) });
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
