// Behaviour for the progress report: step-through explainers, the contact-sheet tabs and the roadmap tree's detail panel.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Everything works without this file: explainers show their last state with every step's text in a list, the tree has a list form,
// and the contact sheet shows the 16 px sheet. The script only adds controls.
(function () {
  'use strict';
  var reduce = window.matchMedia ? window.matchMedia('(prefers-reduced-motion: reduce)') : { matches: false };

  function el(tag, attrs, text) {
    var e = document.createElement(tag);
    Object.keys(attrs || {}).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text != null) e.textContent = text;
    return e;
  }

  // ---- step-through explainers ----------------------------------------------------------------------------------------------
  // Elements with data-from are visible from that step on, data-to up to that step, and an element with only data-at shows in
  // that step alone. data-at also marks the "current" element.
  function stepper(root) {
    var n = parseInt(root.getAttribute('data-n'), 10);
    var items = Array.prototype.slice.call(root.querySelectorAll('.x-step'));
    var ctl = root.querySelector('.x-ctl');
    var live = root.querySelector('.x-live');
    var list = root.querySelector('.x-steps');
    var cur = 0, timer = null;
    var nodes = Array.prototype.slice.call(root.querySelectorAll('[data-from],[data-to],[data-at]'));

    var bPrev = el('button', { type: 'button', 'aria-label': 'Previous step' }, 'Previous');
    var bPlay = el('button', { type: 'button', 'aria-pressed': 'false' }, 'Play');
    var bNext = el('button', { type: 'button', 'aria-label': 'Next step' }, 'Next');
    var bReset = el('button', { type: 'button' }, 'Restart');
    var pos = el('span', { 'class': 'x-pos' }, '');
    [bReset, bPrev, bPlay, bNext, pos].forEach(function (b) { ctl.appendChild(b); });
    ctl.hidden = false;
    ctl.setAttribute('role', 'group');
    ctl.setAttribute('aria-label', 'Step controls');

    items.forEach(function (li, i) {
      var t = li.querySelector('.x-title');
      var b = el('button', { type: 'button' });
      while (t.firstChild) b.appendChild(t.firstChild);
      t.appendChild(b);
      b.addEventListener('click', function () { stop(); go(i); });
    });

    function visible(e, step) {
      var from = e.getAttribute('data-from'), to = e.getAttribute('data-to'), at = e.getAttribute('data-at');
      if (from === null && to === null) return at === null ? true : parseInt(at, 10) === step;
      if (from !== null && step < parseInt(from, 10)) return false;
      if (to !== null && step > parseInt(to, 10)) return false;
      return true;
    }

    function go(i) {
      cur = Math.max(0, Math.min(n - 1, i));
      nodes.forEach(function (e) {
        e.classList.toggle('is-off', !visible(e, cur));
        var at = e.getAttribute('data-at');
        e.classList.toggle('is-now', at !== null && parseInt(at, 10) === cur);
      });
      items.forEach(function (li, k) {
        if (k === cur) li.setAttribute('aria-current', 'step'); else li.removeAttribute('aria-current');
        li.classList.toggle('is-future', k > cur);
      });
      var title = items[cur].querySelector('.x-title').textContent;
      pos.textContent = 'Step ' + (cur + 1) + ' of ' + n;
      live.textContent = 'Step ' + (cur + 1) + ' of ' + n + ': ' + title;
      // a disabled button drops keyboard focus, so hand it to its neighbour first
      if (cur === n - 1 && document.activeElement === bNext) bPrev.focus();
      if (cur === 0 && document.activeElement === bPrev) bNext.focus();
      bPrev.disabled = cur === 0;
      bNext.disabled = cur === n - 1;
      // keep the current step in view inside its own list, without moving the page
      var li = items[cur];
      if (list && list.scrollHeight > list.clientHeight) {
        list.scrollTop = li.offsetTop - list.offsetTop - 8;
      }
      root.querySelectorAll('.x-console').forEach(function (c) {
        var last = null;
        c.querySelectorAll('.ln:not(.is-off)').forEach(function (l) { last = l; });
        if (last) c.scrollTop = last.offsetTop - c.clientHeight + last.offsetHeight + 4; else c.scrollTop = 0;
      });
    }

    function stop() {
      if (timer) { clearInterval(timer); timer = null; }
      bPlay.textContent = 'Play';
      bPlay.setAttribute('aria-pressed', 'false');
    }
    function play() {
      if (cur >= n - 1) go(0);
      bPlay.textContent = 'Pause';
      bPlay.setAttribute('aria-pressed', 'true');
      timer = setInterval(function () {
        if (cur >= n - 1) { stop(); return; }
        go(cur + 1);
      }, 2200);
    }
    bPlay.addEventListener('click', function () { if (timer) stop(); else play(); });
    bNext.addEventListener('click', function () { stop(); go(cur + 1); });
    bPrev.addEventListener('click', function () { stop(); go(cur - 1); });
    bReset.addEventListener('click', function () { stop(); go(0); });
    root.addEventListener('keydown', function (e) {
      if (e.target && (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA')) return;
      if (e.altKey || e.ctrlKey || e.metaKey) return;
      var k = e.key;
      var inScroll = e.target.closest && e.target.closest('.x-console,.x-steps');
      if (k === 'ArrowRight' && !inScroll) { stop(); go(cur + 1); e.preventDefault(); }
      else if (k === 'ArrowLeft' && !inScroll) { stop(); go(cur - 1); e.preventDefault(); }
      else if (k === 'Home' && !inScroll) { stop(); go(0); e.preventDefault(); }
      else if (k === 'End' && !inScroll) { stop(); go(n - 1); e.preventDefault(); }
    });
    if (reduce.matches) {
      var note = el('span', { 'class': 'x-pos' }, 'Your system asks for less motion: nothing moves until you press Play or Next.');
      ctl.appendChild(note);
    }
    root.setAttribute('data-ready', '1');
    go(0);
  }
  Array.prototype.forEach.call(document.querySelectorAll('figure.x[data-x]'), stepper);

  // ---- contact sheet -----------------------------------------------------------------------------------------------------------
  Array.prototype.forEach.call(document.querySelectorAll('.contact'), function (box) {
    var img = box.querySelector('img');
    var alts = { '16': 'all 480 pixel emoji at 16 by 16 pixels', '12': 'all 480 pixel emoji at 12 by 12 pixels', '8': 'the 40 core pixel emoji at 8 by 8 pixels' };
    Array.prototype.forEach.call(box.querySelectorAll('[data-sheet]'), function (b) {
      b.addEventListener('click', function () {
        var s = b.getAttribute('data-sheet');
        img.src = 'img/pixel-' + s + '.png';
        img.alt = 'Contact sheet of ' + alts[s] + ', drawn in the 16-colour Desk palette';
        Array.prototype.forEach.call(box.querySelectorAll('[data-sheet]'), function (o) { o.setAttribute('aria-pressed', String(o === b)); });
      });
    });
  });

  // ---- the roadmap tree ---------------------------------------------------------------------------------------------------------
  var dataEl = document.getElementById('treeData');
  var detail = document.getElementById('treeDetail');
  if (dataEl && detail) {
    var D = JSON.parse(dataEl.textContent);
    var show = function (id) {
      var n = D[id];
      if (!n) return;
      detail.textContent = '';
      var h = el('h3', {}, n.label);
      detail.appendChild(el('div', { 'class': 'eyebrow' }, n.branch + (n.critical ? ' · on the critical path' : '')));
      detail.appendChild(h);
      var dl = el('dl');
      [['State', n.state + ' · ' + n.tag + (n.pr ? ' · PR ' + n.pr : '')], ['What', n.what], ['Why it matters', n.why], ['Needs', n.needs.join(', ') || 'nothing'],
       ['Unlocks', n.unlocks.join(', ') || 'nothing yet'], ['Evidence', n.evidence]].forEach(function (r) {
        dl.appendChild(el('dt', {}, r[0]));
        dl.appendChild(el('dd', {}, r[1]));
      });
      detail.appendChild(dl);
      Array.prototype.forEach.call(document.querySelectorAll('.node'), function (g) { g.classList.toggle('sel', g.getAttribute('data-id') === id); });
    };
    Array.prototype.forEach.call(document.querySelectorAll('.node'), function (g) {
      var id = g.getAttribute('data-id');
      g.addEventListener('click', function () { show(id); });
      g.addEventListener('focus', function () { show(id); });
      g.addEventListener('keydown', function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); show(id); } });
    });
    show('first-window');
  }

  // ---- the gallery ----------------------------------------------------------------------------------------------------------------
  // Filter chips are links to their own ids, so they work as plain links; with script they also filter. State is kept in the hash as
  // dot-separated tokens (#kind-screenshot.track-native.sort-new), which a shared link can restore. Nothing is hidden for good: the list
  // view and every tile stay in the page.
  var tiles = document.getElementById('tiles');
  if (tiles) {
    var state = { kind: '', track: '', sort: 'chapter', view: 'grid' };
    var chips = Array.prototype.slice.call(document.querySelectorAll('.chip[data-group]'));
    var sortBtns = Array.prototype.slice.call(document.querySelectorAll('[data-sort]'));
    var viewBtns = Array.prototype.slice.call(document.querySelectorAll('[data-view]'));
    var items = Array.prototype.slice.call(tiles.querySelectorAll('.tile'));
    var rows = Array.prototype.slice.call(document.querySelectorAll('#figList tbody tr'));
    var listBox = document.getElementById('figList');
    var count = document.getElementById('galleryCount');
    var fromHash = function () {
      var h = (location.hash || '').replace(/^#/, '').split('.');
      h.forEach(function (t) {
        var m = /^(kind|track)-([a-z0-9-]+)$/.exec(t);
        if (m) state[m[1]] = m[2] === 'all' ? '' : m[2];
        if (t === 'sort-new') state.sort = 'new';
        if (t === 'view-list') state.view = 'list';
      });
    };
    var toHash = function () {
      var t = [];
      if (state.kind) t.push('kind-' + state.kind);
      if (state.track) t.push('track-' + state.track);
      if (state.sort === 'new') t.push('sort-new');
      if (state.view === 'list') t.push('view-list');
      try { history.replaceState(null, '', t.length ? '#' + t.join('.') : location.pathname + location.search); } catch (e) { /* a sandboxed frame may refuse */ }
    };
    var apply = function () {
      var shown = 0;
      items.forEach(function (li) {
        var ok = (!state.kind || li.getAttribute('data-kind') === state.kind) && (!state.track || li.getAttribute('data-track') === state.track);
        li.hidden = !ok;
        if (ok) shown++;
      });
      rows.forEach(function (tr) {
        tr.hidden = !((!state.kind || tr.getAttribute('data-kind') === state.kind) && (!state.track || tr.getAttribute('data-track') === state.track));
      });
      var sorted = items.slice().sort(function (a, b) {
        if (state.sort === 'new') {
          var d = b.getAttribute('data-date').localeCompare(a.getAttribute('data-date'));
          if (d) return d;
          return parseInt(b.getAttribute('data-i'), 10) - parseInt(a.getAttribute('data-i'), 10);
        }
        return parseInt(a.getAttribute('data-i'), 10) - parseInt(b.getAttribute('data-i'), 10);
      });
      sorted.forEach(function (li) { tiles.appendChild(li); });
      chips.forEach(function (c) { c.setAttribute('aria-pressed', String(c.getAttribute('data-value') === state[c.getAttribute('data-group')])); });
      sortBtns.forEach(function (b) { b.setAttribute('aria-pressed', String(b.getAttribute('data-sort') === state.sort)); });
      viewBtns.forEach(function (b) { b.setAttribute('aria-pressed', String(b.getAttribute('data-view') === state.view)); });
      tiles.hidden = state.view === 'list';
      if (listBox) listBox.open = state.view === 'list';
      count.textContent = 'Showing ' + shown + ' of ' + items.length + ' figures' + (state.kind ? ', kind ' + state.kind : '') + (state.track ? ', track ' + state.track : '') + '.';
    };
    chips.forEach(function (c) {
      c.addEventListener('click', function (e) {
        e.preventDefault();
        state[c.getAttribute('data-group')] = c.getAttribute('data-value');
        apply(); toHash();
      });
    });
    sortBtns.forEach(function (b) { b.addEventListener('click', function () { state.sort = b.getAttribute('data-sort'); apply(); toHash(); }); });
    viewBtns.forEach(function (b) { b.addEventListener('click', function () { state.view = b.getAttribute('data-view'); apply(); toHash(); }); });
    fromHash();
    apply();
    window.addEventListener('hashchange', function () { state.kind = ''; state.track = ''; state.sort = 'chapter'; state.view = 'grid'; fromHash(); apply(); });
  }
})();
