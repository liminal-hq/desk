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
      [['State', n.state + (n.pr ? ' · PR #' + n.pr : '')], ['What', n.what], ['Why it matters', n.why], ['Needs', n.needs.join(', ') || 'nothing'],
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
})();
