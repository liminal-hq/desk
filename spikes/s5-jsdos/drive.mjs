// A dependency-free headless driver: serves dist/s5/site, starts Chromium with the DevTools protocol on a pipe-free local port,
// runs each scenario in the s5 page and checks the results. Needs Node 22 or later (a global WebSocket and fetch) and a Chromium.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// usage: node spikes/s5-jsdos/drive.mjs [--keep-going] [scenario...]       CHROMIUM=/path/to/chromium overrides the browser
//
// Screenshots and a results.json go to dist/s5/out/. The exit status is 0 only if every scenario passed its checks.
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync, readFileSync, writeFileSync, mkdirSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, extname, normalize, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(fileURLToPath(new URL('../..', import.meta.url)));
const SITE = join(ROOT, 'dist/s5/site');
const OUT = join(ROOT, 'dist/s5/out');
const EXPECTED = join(ROOT, 'spikes/d0-rust-dos/expected');
mkdirSync(OUT, { recursive: true });

const MIME = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.jsdos': 'application/zip', '.txt': 'text/plain', '.json': 'application/json' };

function serve() {
  const server = createServer((req, res) => {
    const path = normalize(decodeURIComponent(new URL(req.url, 'http://x').pathname)).replace(/^(\.\.[/\\])+/, '');
    const file = join(SITE, path === '/' ? 'index.html' : path);
    if (!file.startsWith(SITE) || !existsSync(file)) { res.writeHead(404).end('not found'); return; }
    res.writeHead(200, { 'content-type': MIME[extname(file)] || 'application/octet-stream' }).end(readFileSync(file));
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

async function launch() {
  const profile = mkdtempSync(join(tmpdir(), 's5-chromium-'));
  const bin = process.env.CHROMIUM || '/usr/bin/chromium';
  const child = spawn(bin, [
    '--headless=new', '--no-sandbox', '--disable-gpu', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--mute-audio',
    '--autoplay-policy=no-user-gesture-required', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run', 'about:blank',
  ], { stdio: ['ignore', 'ignore', 'pipe'] });
  const url = await new Promise((ok, fail) => {
    let text = '';
    child.stderr.on('data', (d) => { text += d; const m = text.match(/DevTools listening on (ws:\/\/\S+)/); if (m) ok(m[1]); });
    child.on('exit', (c) => fail(new Error(`chromium exited early (${c}): ${text.slice(-300)}`)));
  });
  return { child, url, profile, version: child.spawnargs[0] };
}

class Cdp {
  constructor(ws) { this.ws = ws; this.id = 0; this.waiting = new Map(); this.logs = [];
    ws.onmessage = (m) => { const d = JSON.parse(m.data);
      if (d.id && this.waiting.has(d.id)) { const [ok, fail] = this.waiting.get(d.id); this.waiting.delete(d.id); d.error ? fail(new Error(d.error.message)) : ok(d.result); }
      else if (d.method === 'Runtime.consoleAPICalled') this.logs.push(d.params.args.map((a) => a.value ?? a.description).join(' '));
      else if (d.method === 'Runtime.exceptionThrown') this.logs.push('EXCEPTION ' + (d.params.exceptionDetails.exception?.description ?? d.params.exceptionDetails.text)); }; }
  static async open(url) { const ws = new WebSocket(url); await new Promise((ok, fail) => { ws.onopen = ok; ws.onerror = fail; }); return new Cdp(ws); }
  send(method, params = {}, sessionId) { const id = ++this.id; this.ws.send(JSON.stringify({ id, method, params, sessionId })); return new Promise((ok, fail) => this.waiting.set(id, [ok, fail])); }
}

async function newPage(browserUrl, pageUrl) {
  const port = new URL(browserUrl).port;
  const target = await (await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(pageUrl)}`, { method: 'PUT' })).json();
  const cdp = await Cdp.open(target.webSocketDebuggerUrl);
  await cdp.send('Runtime.enable');
  const ev = async (expr) => {
    const r = await cdp.send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result.value;
  };
  return { cdp, ev, id: target.id, port };
}

const norm = (s) => (s ?? '').replace(/\r/g, '').replace(/\s+$/, '');
const results = [];
function record(name, ok, detail) { results.push({ name, ok, ...detail }); console.log(`${ok ? 'PASS' : 'FAIL'} ${name}: ${detail.summary ?? ''}`); }
function savePng(name, dataUrl) { writeFileSync(join(OUT, name), Buffer.from(dataUrl.split(',')[1], 'base64')); }

// Runs a bundle to its DONE.TXT and returns what it printed, its exit status and the timings.
async function runToDone(page, bundle, backend, url, raw = false) {
  const t0 = Date.now();
  await page.cdp.send('Page.navigate', { url: `${url}/index.html` });
  await new Promise((r) => setTimeout(r, 300));
  await page.ev('new Promise(r => { if (window.s5) r(); else addEventListener("load", r); })');
  const started = await page.ev(`s5.${raw ? 'startRaw' : 'start'}(${JSON.stringify(bundle)}, ${JSON.stringify(backend)})`);
  const done = await page.ev('s5.waitFile("DONE.TXT", 60000)');
  const wall = Date.now() - t0;
  const exit = await page.ev('s5.read("EXITCODE.TXT")');
  const stdout = await page.ev('s5.output()');
  const t = await page.ev('s5.t');
  return { done, wall, exit: exit === null ? null : Number(norm(exit)), stdout, t, ready: started.ready };
}

const SCENARIOS = {
  async hello(page, url, opt) {
    const r = await runToDone(page, opt.bundle ?? 'hello', opt.backend, url);
    const want = norm(readFileSync(join(EXPECTED, 'hello.txt'), 'utf8'));
    const got = norm(r.stdout);
    const ok = r.done !== null && got === want && r.exit === 0;
    if (!ok) writeFileSync(join(OUT, `${opt.bundle ?? 'hello'}.stdout.txt`), r.stdout);
    return { ok, summary: `exit ${r.exit}, stdout ${got === want ? 'identical to expected/hello.txt' : 'DIFFERS'} (${got.split('\n').length} lines), boot ${r.ready} ms, done at ${r.wall} ms`, r };
  },
  async exit7(page, url, opt) {
    const r = await runToDone(page, 'hello-exit7', opt.backend, url);
    const ok = r.done !== null && r.exit === 7 && /exiting with status 7/.test(r.stdout);
    return { ok, summary: `exit status read back from the emulated disk: ${r.exit}`, r };
  },
  async desk(page, url, opt) {
    const r = await runToDone(page, 'desk', opt.backend, url);
    const want = 'DESK-SKELETON ok: squares=10 sum=385 big=385001155 sqrt=19.621417';
    const file = await page.ev('s5.read("OUT.TXT")');
    const ok = r.done !== null && norm(r.stdout) === want && norm(file) === want && r.exit === 0;
    return { ok, summary: `exit ${r.exit}, stdout and OUT.TXT ${norm(r.stdout) === want && norm(file) === want ? 'match' : 'DIFFER'}, done at ${r.wall} ms`, r };
  },
  async hw(page, url, opt) {
    const r = await runToDone(page, 'hw', opt.backend, url);
    const want = norm(readFileSync(join(EXPECTED, 'hw.txt'), 'utf8'));
    const got = norm(r.stdout);
    const ok = r.done !== null && got === want && r.exit === 0;
    if (!ok) writeFileSync(join(OUT, 'hw.stdout.txt'), r.stdout);
    return { ok, summary: `exit ${r.exit}, stdout ${got === want ? 'identical to expected/hw.txt (IRQ 0 hook, mode 13h checksum)' : 'DIFFERS'}, done at ${r.wall} ms`, r };
  },
  async vga13(page, url, opt) {
    const t0 = Date.now();
    await page.cdp.send('Page.navigate', { url: `${url}/index.html` });
    await new Promise((r) => setTimeout(r, 300));
    await page.ev('new Promise(r => { if (window.s5) r(); else addEventListener("load", r); })');
    const started = await page.ev(`s5.start("vga13", ${JSON.stringify(opt.backend)})`);
    const report = await page.ev('s5.waitFile("VGA13.TXT", 60000)');
    await new Promise((r) => setTimeout(r, 700)); // let the mode change reach the canvas
    const shot = await page.ev('s5.shot()');
    savePng(`vga13${opt.suffix ?? ''}.png`, shot.png);
    const cmp = await page.ev('s5.compare("vga13", 4)');
    const colours = await page.ev('s5.colours()');
    await page.ev('s5.key(32)');
    const done = await page.ev('s5.waitFile("DONE.TXT", 30000)');
    const exit = Number(norm(await page.ev('s5.read("EXITCODE.TXT")')));
    const stdout = await page.ev('s5.output()');
    const ok = report !== null && /match=true/.test(report) && cmp.mismatches === 0 && done !== null && exit === 0;
    return { ok, summary: `screenshot ${shot.w}x${shot.h}, ${colours} colours, ${cmp.mismatches} of ${cmp.pixels} pixels differ from the reference (worst channel diff ${cmp.worst}); frame buffer checksum ${/match=true/.test(report ?? '') ? 'matches' : 'DIFFERS'}; exit ${exit}; boot ${started.ready} ms, total ${Date.now() - t0} ms`, report, cmp, stdout };
  },
  // The VBE probe. With opt.expect === 'refused' the machine setting is expected to withhold the linear frame buffer, and the
  // scenario passes when the probe says so and ends by itself.
  async vbe(page, url, opt) {
    const t0 = Date.now();
    await page.cdp.send('Page.navigate', { url: `${url}/index.html` });
    await new Promise((r) => setTimeout(r, 300));
    await page.ev('new Promise(r => { if (window.s5) r(); else addEventListener("load", r); })');
    const started = await page.ev(`s5.start(${JSON.stringify(opt.bundle ?? 'vbe')}, ${JSON.stringify(opt.backend)})`);
    const gfx = await page.ev('s5.waitFile("VBEGFX.TXT", 90000)');
    const info = await page.ev('s5.read("VBE.TXT")');
    let shot = null, cmp = null, colours = null;
    if (gfx !== null && /match=/.test(gfx)) {
      await new Promise((r) => setTimeout(r, 700));
      shot = await page.ev('s5.shot()');
      savePng(`${opt.bundle ?? 'vbe'}${opt.suffix ?? ''}.png`, shot.png);
      cmp = await page.ev('s5.compare("vbe", 4)');
      colours = await page.ev('s5.colours()');
      await page.ev('s5.key(32)');
    }
    const done = await page.ev('s5.waitFile("DONE.TXT", 30000)');
    const exitText = await page.ev('s5.read("EXITCODE.TXT")');
    const exit = exitText === null ? null : Number(norm(exitText));
    const stdout = await page.ev('s5.output()');
    const modes = (info ?? '').split('\n').filter((l) => l.startsWith('mode ')).length;
    let ok, what;
    if (opt.expect === 'refused') {
      ok = done !== null && exit === 3 && /no 640x480x8 packed-pixel mode with a linear frame buffer offered/.test(gfx ?? '');
      what = `${modes} modes listed, none with a linear frame buffer; probe says: ${norm(gfx).split('\n').pop()}; exit ${exit}`;
    } else if (opt.expect === 'observe') {
      // The js-dos DOSBox honours vesa_nolfb but not vesa_oldvbe; DOSBox-X honours both. Either way the probe must end cleanly.
      ok = done !== null && (exit === 0 || exit === 3);
      what = `${modes} modes listed, ${norm(gfx).split('\n').pop().slice(0, 150)}; exit ${exit}; VBE.TXT: ${norm(info).split('\n')[2]}`;
    } else {
      ok = gfx !== null && /match=true/.test(gfx) && cmp !== null && cmp.mismatches === 0 && done !== null && exit === 0;
      what = shot ? `screenshot ${shot.w}x${shot.h}, ${colours} colours, ${cmp.mismatches} of ${cmp.pixels} pixels differ (worst ${cmp.worst}); probe: ${norm(gfx).replace(/\n/g, ' | ')}; exit ${exit}` : `no picture: ${norm(gfx)}; exit ${exit}`;
    }
    return { ok, summary: `${what}; total ${Date.now() - t0} ms`, info, gfx, cmp, stdout };
  },
  // An s1-dxe3 module scenario: the transcript, without the lines that hold run-to-run numbers, must equal the native one.
  async s1(page, url, opt) {
    const r = await runToDone(page, `s1-${opt.name}`, opt.backend, url);
    const want = norm(readFileSync(join(ROOT, 'spikes/s1-dxe3/expected', `${opt.name}.txt`), 'utf8'));
    const got = norm(r.stdout.split('\n').filter((l) => !l.startsWith('mem: ')).join('\n'));
    const ok = r.done !== null && got === want && r.exit === 0;
    if (!ok) writeFileSync(join(OUT, `s1-${opt.name}.stdout.txt`), r.stdout);
    return { ok, summary: `exit ${r.exit}, transcript ${got === want ? 'identical to the native one' : 'DIFFERS'} (${want.split('\n').length} lines), done at ${r.wall} ms`, r };
  },
  // The sound probe: three tones in a row, measured from the audio the emulator pushes to the page.
  async tone(page, url, opt) {
    const r = await runToDone(page, opt.bundle ?? 'tone', opt.backend, url, true);
    const audio = await page.ev('s5.analyseAudio()');
    const file = await page.ev('s5.read("TONE.TXT")');
    const hz = audio.segments.map((s) => s.hz);
    const near = (v, want) => Math.abs(v - want) <= want * 0.01;
    const ok = r.done !== null && /opl: detected true/.test(file ?? '') && /DSP version Some\(2\)/.test(file ?? '') && /IRQ 7 pending true/.test(file ?? '')
      && audio.segments.length === 3 && near(hz[0], 440) && near(hz[1], 400) && near(hz[2], 440);
    return { ok, summary: `${audio.segments.length} sounds in ${audio.seconds} s of audio at ${audio.rate} Hz: ${audio.segments.map((s) => `${s.hz} Hz (${s.seconds} s, peak ${s.peak})`).join(', ')}; ${norm(file).split('\n').filter((l) => /^(opl|sb)/.test(l)).join(' | ')}`, r, audio, file };
  },
  // Prints what the emulator reports about itself (DOS version, settings).
  async info(page, url, opt) {
    const r = await runToDone(page, 'info', opt.backend, url);
    const conf = await page.ev('s5.config()');
    writeFileSync(join(OUT, `info${opt.suffix ?? ''}.txt`), norm(r.stdout) + '\n\n--- dosbox.conf as js-dos holds it ---\n' + conf);
    return { ok: r.done !== null, summary: norm(r.stdout).split('\n').filter(Boolean).join(' | '), r };
  },
};

const SETS = [
  ['hello', 'hello', {}], ['hello-solo', 'hello', { bundle: 'hello-solo' }], ['hello-486', 'hello', { bundle: 'hello-486' }],
  ['exit7', 'exit7', {}], ['desk', 'desk', {}], ['hw', 'hw', {}], ['vga13', 'vga13', {}],
  ['vbe', 'vbe', {}], ['vbe-banked', 'vbe', { bundle: 'vbe-banked' }],
  ['vbe-nolfb', 'vbe', { bundle: 'vbe-nolfb', expect: 'refused' }], ['vbe-oldvbe', 'vbe', { bundle: 'vbe-oldvbe', expect: 'observe' }], ['info', 'info', {}],
  ['tone', 'tone', {}], ['tone-compat', 'tone', { bundle: 'tone-compat' }], ['tone-default', 'tone', { bundle: 'tone-default' }],
  ['s1-life', 's1', { name: 'life' }], ['s1-panic', 's1', { name: 'panic' }], ['s1-fault-div-catch', 's1', { name: 'fault-div-catch' }],
];

const args = process.argv.slice(2);
const keepGoing = args.includes('--keep-going');
const wanted = args.filter((a) => !a.startsWith('--'));
const backend = process.env.S5_BACKEND || 'dosbox';
const server = await serve();
const url = `http://127.0.0.1:${server.address().port}`;
const browser = await launch();
let failed = false;
try {
  for (const [name, fn, opt] of SETS) {
    if (wanted.length && !wanted.includes(name)) continue;
    const page = await newPage(browser.url, 'about:blank');
    try {
      const r = await SCENARIOS[fn](page, url, { backend, suffix: backend === 'dosbox' ? '' : `-${backend}`, ...opt });
      record(name, r.ok, { summary: r.summary, detail: r });
      if (!r.ok) { failed = true; console.log((page.cdp.logs.slice(-8)).join('\n')); }
    } catch (e) {
      failed = true;
      record(name, false, { summary: `error: ${e.message}` });
      console.log(page.cdp.logs.slice(-8).join('\n'));
    }
    await fetch(`http://127.0.0.1:${page.port}/json/close/${page.id}`).catch(() => {});
    if (failed && !keepGoing) break;
  }
} finally {
  writeFileSync(join(OUT, `results-${backend}.json`), JSON.stringify(results, null, 1));
  browser.child.kill();
  await new Promise((r) => setTimeout(r, 500));
  server.close();
  rmSync(browser.profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
}
process.exit(failed ? 1 : 0);
