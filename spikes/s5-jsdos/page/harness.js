// The s5 page's harness: starts a js-dos bundle and offers the calls a headless driver (or a person at the console) uses to run a
// DOS program, read files back from the emulated disk, press keys, take screenshots and compare them with a reference.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
(function () {
  'use strict';

  var BUNDLES = ['desk', 'hello', 'hello-exit7', 'hello-solo', 'hw', 'vga13', 'vbe', 'vbe-banked', 'hello-486', 'vbe-nolfb', 'vbe-oldvbe', 's1-life', 's1-panic', 's1-fault-div-catch', 'tone', 'tone-compat', 'tone-default'];
  var log = document.getElementById('log');
  var s5 = { stdout: '', events: [], t: {}, ci: null, player: null, exited: false, audio: [] };
  window.s5 = s5;

  function now() { return Math.round(performance.now()); }
  function note(text) { log.textContent += '\n' + text; }

  // Starts a bundle and resolves when the emulator hands over its command interface.
  s5.start = function (bundle, backend) {
    s5.t = { page: now() };
    s5.stdout = '';
    s5.audio = [];
    s5.exited = false;
    return new Promise(function (resolve, reject) {
      var timer = setTimeout(function () { reject(new Error('no ci-ready in 60 s')); }, 60000);
      s5.player = window.Dos(document.getElementById('dos'), {
        url: 'bundles/' + bundle + '.jsdos',
        pathPrefix: 'vendor/js-dos/emulators/',
        backend: backend || 'dosbox',
        autoStart: true,
        kiosk: true,
        noCloud: true,
        onEvent: function (event, arg) {
          s5.events.push(event);
          if (event === 'ci-ready') {
            clearTimeout(timer);
            s5.t.ready = now();
            s5.ci = arg;
            var ev = arg.events();
            ev.onStdout(function (text) { s5.stdout += text; });
            ev.onFrameSize(function (w, h) { if (!s5.t.frame) s5.t.frame = now(); s5.size = [w, h]; });
            ev.onExit(function () { s5.exited = true; s5.t.exit = now(); });
            resolve({ ready: s5.t.ready - s5.t.page });
          }
        },
      });
      s5.t.called = now();
    });
  };

  // The same, through the emulators layer under the player (no player UI, no canvas): the audio then comes to the page as samples
  // (onSoundPush) instead of going straight to an audio worklet, which is what makes sound measurable without a speaker.
  s5.startRaw = async function (bundle, backend, options) {
    s5.t = { page: now() };
    s5.stdout = '';
    s5.audio = [];
    s5.exited = false;
    var emu = window.emulators;
    emu.pathPrefix = 'vendor/js-dos/emulators/';
    var bytes = new Uint8Array(await (await fetch('bundles/' + bundle + '.jsdos')).arrayBuffer());
    s5.t.fetched = now();
    var make = backend === 'dosboxX' ? emu.dosboxXWorker : emu.dosboxWorker;
    var ci = await make.call(emu, bytes, options || { audioWorklet: false });
    s5.t.ready = now();
    s5.ci = ci;
    var ev = ci.events();
    ev.onStdout(function (text) { s5.stdout += text; });
    ev.onSoundPush(function (samples) { s5.audio.push(samples.slice()); });
    ev.onFrameSize(function (w, h) { if (!s5.t.frame) s5.t.frame = now(); s5.size = [w, h]; });
    ev.onExit(function () { s5.exited = true; s5.t.exit = now(); });
    return { ready: s5.t.ready - s5.t.page };
  };

  function sleep(ms) { return new Promise(function (r) { setTimeout(r, ms); }); }

  // Whether a file of the given name (in the root of the emulated C: drive) exists and has contents. fsReadFile on a file that does
  // not exist never settles and refuses a second request for the same name, so existence is checked in the file tree first.
  async function exists(name) {
    var tree = await s5.ci.fsTree();
    return (tree.nodes || []).some(function (n) { return n.name === name && n.size > 0; });
  }

  // Reads a file from the emulated disk as text, or null when it does not exist.
  s5.read = async function (name) {
    if (!(await exists(name))) return null;
    return new TextDecoder().decode(await s5.ci.fsReadFile(name));
  };

  // Waits for a file to appear on the emulated disk and returns its text; null on timeout. A program may still be writing the file
  // when it first shows, so the text is read once the size has stopped changing.
  s5.waitFile = async function (name, ms) {
    var until = now() + (ms || 30000);
    while (now() < until) {
      if (await exists(name)) {
        await sleep(150);
        s5.t[name] = now();
        return s5.read(name);
      }
      await sleep(100);
    }
    return null;
  };

  // What the program printed: the console text between the markers S5RUN.BAT echoes, without the DOSBox banner.
  s5.output = function () {
    var a = s5.stdout.indexOf('S5-BEGIN\r\n'), b = s5.stdout.indexOf('S5-END');
    return a < 0 ? s5.stdout : s5.stdout.slice(a + 10, b < 0 ? undefined : b);
  };

  // A key press and release; the code is js-dos's (GLFW) key code: 32 space, 257 enter.
  s5.key = function (code) {
    s5.ci.sendKeyEvent(code, true);
    return sleep(80).then(function () { s5.ci.sendKeyEvent(code, false); });
  };

  s5.tree = function () { return s5.ci.fsTree(); };

  s5.config = async function () { return (await s5.ci.config()).dosboxConf; };

  // The screen as a PNG data URL and its size.
  s5.shot = async function () {
    var img = await s5.ci.screenshot();
    var canvas = document.createElement('canvas');
    canvas.width = img.width;
    canvas.height = img.height;
    canvas.getContext('2d').putImageData(img, 0, 0);
    s5.lastImage = img;
    return { w: img.width, h: img.height, png: canvas.toDataURL('image/png') };
  };

  // The probes' palette: 6-bit components, scaled to 8 bits the way a VGA DAC readout does.
  function palette(i) {
    var r = i >> 2, g = (i * 7) & 63, b = 63 - (i >> 2);
    return [(r << 2) | (r >> 4), (g << 2) | (g >> 4), (b << 2) | (b >> 4)];
  }
  var PATTERNS = {
    // rt::vga::pattern, 320 x 200
    vga13: { w: 320, h: 200, at: function (x, y) { return y < 100 ? (Math.floor(x / 20) * 16 + Math.floor(y / 12)) & 255 : (x ^ y) & 255; } },
    // the vbe probe's pattern, 640 x 480
    vbe: { w: 640, h: 480, at: function (x, y) { return y < 240 ? (Math.floor(x / 40) * 16 + Math.floor(y / 15)) & 255 : (x ^ y) & 255; } },
  };

  // Compares the last screenshot with the named pattern drawn in the probes' palette; the screenshot may be any whole or
  // fractional multiple of the mode's size. Returns how many pixels differ by more than `tolerance` in any channel.
  s5.compare = function (name, tolerance) {
    var p = PATTERNS[name], img = s5.lastImage, tol = tolerance == null ? 4 : tolerance;
    var bad = 0, worst = 0, first = null;
    for (var Y = 0; Y < img.height; Y++) {
      for (var X = 0; X < img.width; X++) {
        var x = Math.min(p.w - 1, Math.floor(X * p.w / img.width)), y = Math.min(p.h - 1, Math.floor(Y * p.h / img.height));
        var want = palette(p.at(x, y)), o = (Y * img.width + X) * 4, d = 0;
        for (var c = 0; c < 3; c++) d = Math.max(d, Math.abs(img.data[o + c] - want[c]));
        if (d > worst) worst = d;
        if (d > tol) { bad++; if (!first) first = [X, Y, [img.data[o], img.data[o + 1], img.data[o + 2]], want]; }
      }
    }
    return { w: img.width, h: img.height, pixels: img.width * img.height, mismatches: bad, worst: worst, first: first };
  };

  // The number of distinct colours in the last screenshot (a blank or all-black screen has one).
  s5.colours = function () {
    var img = s5.lastImage, seen = {};
    for (var o = 0; o < img.data.length; o += 4) seen[(img.data[o] << 16) | (img.data[o + 1] << 8) | img.data[o + 2]] = 1;
    return Object.keys(seen).length;
  };

  // Finds the stretches of sound in what the emulator pushed to the audio output and measures each one's pitch from its rising zero
  // crossings (after removing the mean). Positions are in seconds of emulated audio, so they do not depend on how fast the host ran.
  s5.analyseAudio = function () {
    var rate = s5.ci.soundFrequency(), total = 0, i, j;
    s5.audio.forEach(function (c) { total += c.length; });
    var all = new Float32Array(total), at = 0;
    s5.audio.forEach(function (c) { all.set(c, at); at += c.length; });
    var win = Math.round(rate / 100), loud = [];
    for (i = 0; i + win <= total; i += win) {
      var sum = 0, mean = 0;
      for (j = 0; j < win; j++) mean += all[i + j];
      mean /= win;
      for (j = 0; j < win; j++) sum += (all[i + j] - mean) * (all[i + j] - mean);
      loud.push(Math.sqrt(sum / win) > 0.01);
    }
    var segments = [], start = -1, quiet = 0;
    loud.forEach(function (l, k) {
      if (l) { if (start < 0) start = k; quiet = 0; }
      else if (start >= 0 && ++quiet > 5) { segments.push([start, k - quiet + 1]); start = -1; quiet = 0; }
    });
    if (start >= 0) segments.push([start, loud.length - quiet]);
    var out = segments.filter(function (s) { return s[1] - s[0] >= 30; }).map(function (s) {
      var from = (s[0] + 10) * win, to = (s[1] - 10) * win, mean = 0, peak = 0, rms = 0;
      for (i = from; i < to; i++) mean += all[i];
      mean /= (to - from);
      var first = -1, last = -1, n = 0;
      for (i = from + 1; i < to; i++) {
        var a = all[i - 1] - mean, b = all[i] - mean;
        peak = Math.max(peak, Math.abs(b));
        rms += b * b;
        if (a < 0 && b >= 0) { if (first < 0) first = i; last = i; n++; }
      }
      return { start: +(s[0] / 100).toFixed(2), seconds: +((s[1] - s[0]) / 100).toFixed(2), hz: n > 1 ? +(((n - 1) * rate) / (last - first)).toFixed(1) : 0,
               peak: +peak.toFixed(3), rms: +Math.sqrt(rms / (to - from)).toFixed(3) };
    });
    return { rate: rate, seconds: +(total / rate).toFixed(2), segments: out };
  };

  s5.stop = async function () {
    try { await s5.ci.exit(); } catch (e) { /* already gone */ }
    try { if (s5.player && s5.player.stop) await s5.player.stop(); } catch (e) { /* already gone */ }
  };

  // By hand: index.html?bundle=hello starts a bundle and shows what it printed.
  var params = new URLSearchParams(location.search);
  document.getElementById('list').innerHTML = BUNDLES.map(function (b) { return '<a href="?bundle=' + b + '">' + b + '</a>'; }).join(', ');
  if (params.has('bundle')) {
    s5.start(params.get('bundle'), params.get('backend') || 'dosbox').then(async function (r) {
      note('ready after ' + r.ready + ' ms');
      var done = await s5.waitFile('DONE.TXT', 60000);
      note(done === null ? 'no DONE.TXT in 60 s' : 'finished; exit status ' + (await s5.read('EXITCODE.TXT')));
      note(s5.output());
    }, function (e) { note('failed: ' + e.message); });
  }
}());
