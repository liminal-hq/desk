# The report's explainers and figures as HTML strings: each is driven by real data (transcripts, commits, files in the tree).
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# A step-through explainer is plain HTML that works without script: every step's full text is in an ordered list, and the stage
# shows its final state. site.js adds the controls and hides elements whose data-from is later than the current step (data-to:
# earlier, data-at: marks the step in which something happens). Nothing here invents data: where a figure is a documented flow
# rather than a measurement, its caption says so.
import datetime
import os
import re

from mdlite import esc, inline

EDT = datetime.timezone(datetime.timedelta(hours=-4))


def hhmm(iso):
    return datetime.datetime.fromisoformat(iso.replace("Z", "+00:00")).astimezone(EDT).strftime("%H:%M")


def day_label(iso):
    d = datetime.datetime.fromisoformat(iso.replace("Z", "+00:00")).astimezone(EDT)
    return "%d %s %d" % (d.day, d.strftime("%B"), d.year)


def stepper(xid, label, stage, steps, caption, ctx, cls=""):
    items = "".join(
        '<li class="x-step" data-i="%d"><h4 class="x-title">%s</h4><div class="x-text">%s</div></li>' % (i, inline(s[0], ctx), inline(s[1], ctx))
        for i, s in enumerate(steps))
    return (
        '<figure class="x %s" data-x="%s" data-n="%d" role="group" aria-label="%s">'
        '<div class="x-grid"><div class="x-stage">%s</div>'
        '<div class="x-side"><div class="x-ctl" hidden></div><p class="x-live" aria-live="polite"></p>'
        '<ol class="x-steps">%s</ol></div></div><figcaption>%s</figcaption></figure>'
    ) % (cls, xid, len(steps), esc(label), stage, items, caption)


def assign(lines, rules):
    """The step each line belongs to: the step of the last rule (regex, step) that has matched so far."""
    out, cur = [], None
    for line in lines:
        for rx, step in rules:
            if re.search(rx, line):
                cur = step
        out.append(cur)
    return out


def console(lines, steps, label, highlight=True):
    spans = "".join('<span class="ln" data-from="%s"%s>%s</span>' % (s, (' data-at="%s"' % s) if highlight else "", esc(l)) for l, s in zip(lines, steps) if s is not None)
    return '<pre class="term x-console" tabindex="0" aria-label="%s"><code>%s</code></pre>' % (esc(label), spans)


# ---- the boot sequence ---------------------------------------------------------------------------------------------------------

def expected_matches(expected, actual_lines):
    """True when the captured transcript satisfies tests/native/expected.txt (its {N:a..b} and {HEX} patterns, '...' for a gap)."""
    def rx(line):
        out = ""
        for part in re.split(r"(\{N:\d+\.\.\d+\}|\{HEX\})", line):
            m = re.match(r"\{N:(\d+)\.\.(\d+)\}", part)
            out += r"(\d+)" if m else (r"(?:0x)?[0-9a-fA-F]+" if part == "{HEX}" else re.escape(part))
        return re.compile("^" + out + "$")
    pos = 0
    for line in expected.splitlines():
        if line == "...":
            continue
        r = rx(line)
        while pos < len(actual_lines) and not r.match(actual_lines[pos]):
            pos += 1
        if pos >= len(actual_lines):
            return False
        pos += 1
    return True


def parse_map(lines):
    out = []
    for l in lines:
        m = re.match(r"^\s+map (0x[0-9a-f]+)-(0x[0-9a-f]+) (\w+)$", l)
        if m:
            out.append((int(m.group(1), 16), int(m.group(2), 16), m.group(3)))
    return out


def kib(n):
    return "{:,} KiB".format(n // 1024) if n % 1024 == 0 else "{:,} B".format(n)


def boot(D, ctx, serial, expected):
    if not expected_matches(expected, serial.splitlines()):
        raise ValueError("the captured serial transcript does not satisfy tests/native/expected.txt")
    serial = serial.strip("\n")
    lines = serial.splitlines()
    entries = parse_map(lines)
    if len(entries) != 6:
        raise ValueError("expected six E820 entries in the transcript, found %d" % len(entries))
    kernel_bytes = D["measured"].get("nativeKernelBytes")
    stage2 = D["measured"].get("nativeStage2Bytes")
    usable_kib = sum((e - s) // 1024 for s, e, k in entries if k == "usable" and s < 0x1000000)
    m = re.search(r"usable RAM (\d+) KiB", serial)
    low_usable = [(s, e) for s, e, k in entries if k == "usable"]
    # The kernel counts whole 1 KiB; the sum of the printed entries must equal what it reports.
    total_usable = sum((e - s) // 1024 for s, e in low_usable)
    if not m or int(m.group(1)) != total_usable:
        raise ValueError("usable RAM in the transcript (%s) is not the sum of its usable entries (%d KiB)" % (m and m.group(1), total_usable))
    img = re.search(r"kernel image (0x[0-9a-f]+)-(0x[0-9a-f]+)", serial)
    ks, ke = int(img.group(1), 16), int(img.group(2), 16)
    heap = re.search(r"heap: (\d+) KiB at (0x[0-9a-f]+)", serial)
    hs, hlen = int(heap.group(2), 16), int(heap.group(1)) * 1024

    rules = [(r"^stage2: loader running", 3), (r"^stage2: a20", 4), (r"^stage2: memory map from E820", 5), (r"^stage2: entering", 7),
             (r"^Desk native kernel", 8), (r"^gdt:", 9), (r"^memory: usable", 13), (r"^frames: alloc", 14), (r"^timer:", 15), (r"^fpu:", 16)]
    steps_of = assign(lines, rules)
    n = 0
    for i, l in enumerate(lines):
        if re.match(r"^\s+map 0x", l):
            steps_of[i] = 10 if n < 3 else (11 if n == 3 else 12)
            n += 1

    # First mebibyte, zoomed: where the loader puts things.
    def X1(a):
        return 20 + a / 1048576 * 600
    z = ['<svg viewBox="0 0 640 160" role="img" class="diagram" aria-label="The first mebibyte of memory: the boot information block at 0x500, stage 1 at 0x7C00, stage 2 at 0x7E00, the kernel staged at 0x10000, then the BIOS map entries as they are printed.">',
         '<text x="20" y="14" font-size="12" class="d-title">First 1 MiB (to scale)</text>',
         '<rect x="20" y="62" width="600" height="26" class="m-bar"/>']
    for s, e, k in entries:
        if s < 0x100000:
            z.append('<rect x="%g" y="62" width="%g" height="26" class="m-%s" data-from="10"><title>%s %s-%s</title></rect>' % (
                X1(s), max(1.2, X1(e) - X1(s)), k, k, hex(s), hex(e)))
    z.append('<rect x="%g" y="62" width="%g" height="26" class="m-hole" data-from="10"><title>no map entry: 0xa0000-0xf0000</title></rect>' % (X1(0xA0000), X1(0xF0000) - X1(0xA0000)))
    z.append('<text x="%g" y="105" text-anchor="middle" font-size="11" class="d-muted" data-from="10">no entry here (0xa0000 to 0xf0000)</text>' % ((X1(0xA0000) + X1(0xF0000)) / 2))
    z.append('<text x="%g" y="105" text-anchor="middle" font-size="11" class="d-muted" data-from="10">usable, to 0x9fc00</text>' % (X1(0x9FC00) / 2 + 10))
    marks = [(0x500, "info block 0x500", 6, 4, 28, "start"), (0x7C00, "stage 1 0x7C00", 1, 4, 40, "start"), (0x7E00, "stage 2 0x7E00", 2, 4, 52, "start")]
    for a, label, step, _, y, anchor in marks:
        z.append('<g data-from="%d"><line x1="%g" x2="%g" y1="%d" y2="62" class="d-line" stroke-width="1"/><circle cx="%g" cy="62" r="2.5" class="m-mark"/><text x="%g" y="%d" font-size="11" class="d-text2" text-anchor="%s">%s</text></g>' % (
            step, X1(a), X1(a), y + 2, X1(a), X1(a) + 5, y, anchor, esc(label)))
    stg = X1(0x10000)
    z.append('<g data-from="2" data-to="7"><rect x="%g" y="62" width="%g" height="26" class="m-stage"/><line x1="%g" x2="%g" y1="88" y2="124" class="d-line"/><text x="%g" y="136" font-size="11" class="d-text2">kernel staged at 0x10000 (%s)</text></g>' % (
        stg, max(2, kernel_bytes / 1048576 * 600 if kernel_bytes else 20), stg, stg, stg + 4, ("{:,} B".format(kernel_bytes) if kernel_bytes else "size recorded in snapshots/measured.json")))
    z.append('<text x="20" y="152" font-size="11" class="d-muted">0</text><text x="620" y="152" text-anchor="end" font-size="11" class="d-muted">1 MiB (0x100000)</text>')
    z.append("</svg>")

    # Low 16 MiB, to scale, with the kernel and heap placed from the transcript.
    def X(a):
        return 20 + a / 0x1000000 * 600
    big = ['<svg viewBox="0 0 640 190" role="img" class="diagram" aria-label="The first 16 MiB of the address space, filled in from the six BIOS map entries the kernel prints, then the kernel image and the heap placed above 1 MiB.">',
           '<text x="20" y="14" font-size="12" class="d-title">First 16 MiB (to scale), from the printed map</text>',
           '<rect x="20" y="40" width="600" height="34" class="m-bar"/>']
    for idx, (s, e, k) in enumerate(entries[:5]):
        step = 10 if idx < 3 else (11 if idx == 3 else 12)
        big.append('<rect x="%g" y="40" width="%g" height="34" class="m-%s" data-from="%d"><title>%s %s-%s</title></rect>' % (
            X(s), max(1.2, X(min(e, 0x1000000)) - X(s)), k, step, k, hex(s), hex(e)))
    big.append('<rect x="%g" y="40" width="%g" height="34" class="m-kernel" data-from="13"><title>kernel image %s-%s</title></rect>' % (X(ks), max(2, X(ke) - X(ks)), hex(ks), hex(ke)))
    big.append('<rect x="%g" y="84" width="%g" height="16" class="m-heap" data-from="14"><title>heap %s, %s</title></rect>' % (X(hs), X(hs + hlen) - X(hs), hex(hs), kib(hlen)))
    big.append('<text x="%g" y="116" font-size="11" class="d-text2" data-from="13">kernel image %s-%s</text>' % (X(ks) - 4, hex(ks), hex(ke)))
    big.append('<text x="%g" y="132" font-size="11" class="d-text2" data-from="14">heap: %s at %s</text>' % (X(hs) - 4, kib(hlen), hex(hs)))
    big.append('<text x="20" y="170" font-size="11" class="d-muted">0</text><text x="%g" y="170" text-anchor="middle" font-size="11" class="d-muted">1 MiB</text><text x="620" y="170" text-anchor="end" font-size="11" class="d-muted">16 MiB (0x1000000)</text>' % X(0x100000))
    last = entries[5]
    big.append('<g data-from="12"><rect x="520" y="146" width="100" height="14" class="m-reserved"/><text x="515" y="158" text-anchor="end" font-size="11" class="d-text2">entry 6 sits at the top of 4 GiB: %s-0x100000000, reserved (%s)</text></g>' % (hex(last[0]), kib(0x100000000 - last[0])))
    big.append("</svg>")

    key = ('<ul class="m-key" aria-label="Key"><li><i class="sw m-usable"></i>usable RAM</li><li><i class="sw m-reserved"></i>reserved</li>'
           '<li><i class="sw m-kernel"></i>kernel image</li><li><i class="sw m-heap"></i>heap</li><li><i class="sw m-stage"></i>kernel staged</li></ul>')
    badge = ('<p class="x-badge"><span data-to="6" class="b-real">Real mode, 16-bit: the BIOS still answers</span>'
             '<span data-from="7" class="b-prot">Protected mode, 32-bit: flat 4 GiB segments, interrupts off until the kernel installs its own</span></p>')
    stage = badge + "".join(z) + "".join(big) + key + console(lines, steps_of, "The serial transcript of a real QEMU run, shown up to the current step")
    steps = [
        ("Power on", "The BIOS runs its power-on self-test and looks for something to boot. In the run this page replays that was SeaBIOS inside QEMU (`-cpu 486 -m 16`); there is no serial output yet, so nothing is printed."),
        ("The BIOS loads sector 0", "It reads the first 512 bytes of the floppy to `0000:7C00` and jumps there in 16-bit real mode with the boot drive in `DL`. The BIOS accepts a sector only if its last two bytes are `55 AA`. This is `boot/stage1.asm`."),
        ("Stage 1 reads the rest", "With INT 13h it reads stage 2 (17 sectors) to `0000:7E00` and the kernel (%s) to physical `0x10000`, a track at a time and never across a 64 KiB boundary, because the floppy's DMA cannot cross one." % (("%s bytes" % "{:,}".format(kernel_bytes)) if kernel_bytes else "size not recorded")),
        ("Stage 2 starts", "Stage 2 starts the serial port and prints its first line: `%s`. From here a headless run records everything." % lines[0]),
        ("A20", "The 8086 wrapped addresses at 1 MiB and the PC keeps doing so until the A20 gate is opened. Stage 2 tests the gate first; this run printed `%s`, so there was nothing to do. The fast gate and keyboard-controller methods exist for other machines and have their own test variants." % lines[1]),
        ("The BIOS memory map", "Only the BIOS knows what RAM exists, and only in real mode. Stage 2 walks INT 15h `AX=E820` and prints only the count: `%s`. The six entries themselves are printed later, by the kernel, and the diagram fills in from those lines." % lines[2]),
        ("The boot information block", "Stage 2 writes a block at `0x500`: the magic `DESK`, the boot drive, the map source, the entry count and the entries. The kernel is handed its address."),
        ("Protected mode", "Stage 2 loads a three-entry GDT (null, a flat 4 GiB code segment at `0x08`, a flat 4 GiB data segment at `0x10`), sets `CR0.PE` and far-jumps into 32-bit code. It prints `%s` just before." % lines[3]),
        ("Copy and jump", "In protected mode the kernel is copied from `0x10000` to 1 MiB and stage 2 jumps to its first byte. The kernel's `_start` sets a stack, clears `.bss` and calls `kmain`, which prints its banner: `%s`." % lines[4]),
        ("Tables and timer", "`kmain` brings up its own GDT and IDT (32 exception gates and the timer gate), remaps the PIC to `0x20` and `0x28` with only IRQ 0 unmasked, and sets the PIT to 100 Hz. The transcript says so line by line."),
        ("The map, entries 1 to 3", "The kernel prints each entry it was given. The first three are low memory: usable up to `0x9fc00`, a reserved sliver up to `0xa0000` and reserved ROM space from `0xf0000` to 1 MiB. Nothing covers `0xa0000` to `0xf0000`."),
        ("The map, entry 4", "One big usable block from 1 MiB to `0xfe0000`: %s." % kib(entries[3][1] - entries[3][0])),
        ("The map, entries 5 and 6", "A reserved block from `0xfe0000` to 16 MiB, and a reserved block at the very top of the 4 GiB address space, from `0xfffc0000`."),
        ("Counting RAM", "The kernel adds the usable entries: %s + %s = %s, which is the `%s` line. Its image occupies `%s` to `%s`, and the frame allocator keeps the rest in 4 KiB frames." % (
            kib((entries[0][1] - entries[0][0]) // 1024 * 1024), kib((entries[3][1] - entries[3][0]) // 1024 * 1024), "{:,} KiB".format(total_usable),
            [l for l in lines if l.startswith("memory: usable")][0], hex(ks), hex(ke))),
        ("Frames, heap, `alloc`", "It allocates and frees two frames, takes %s of them for the heap at `%s`, and runs `Vec`, `String` and `format!` on it. Free heap afterwards is the same %s." % (kib(hlen), hex(hs), kib(hlen))),
        ("A tick test", "With interrupts on it waits for 10 timer ticks with `hlt`: `%s`." % [l for l in lines if l.startswith("timer:")][0]),
        ("The x87 and done", "It enables the FPU, checks the control word (`0x037f`), computes `sqrt(2)` and the harmonic sum H(10), then prints `boot complete` and exits QEMU through the `isa-debug-exit` port."),
    ]
    cap = ('A step-through of <code>docs/NATIVE-BOOT.md</code>. The transcript is a real run of the floppy image built from <code>main</code> (QEMU 11.1.2, <code>-cpu 486</code>; stored as <code>tools/progress-report/media/native-serial.txt</code>) and satisfies <code>tests/native/expected.txt</code>, which the generator checks. '
           'The diagram is drawn from the six <code>map</code> lines in it. The first three steps and the info block come from the document, not from the transcript. Not run on real hardware.')
    return stepper("boot", "Boot sequence stepper", stage, steps, cap, ctx)


# ---- a DXE3 module's life ------------------------------------------------------------------------------------------------------

def modules(D, ctx):
    t = D["transcripts"]
    panic = t["s1Panic"].splitlines()
    rules = [(r"^\s+window> app 1 opens", 0), (r"counter: start app=2", 1), (r"hello\[1\]: event 8", 2), (r"crash: start app=3", 3),
             (r"holding 5 blocks", 4), (r"module CRASH crashed: panic: panic with heap", 5), (r"unloaded CRASH .*reclaimed 6 blocks", 6),
             (r"hello\[1\]: event 9", 7), (r"crash: start app=4", 8), (r"window> app 5 opens", 9), (r"counter: stop app=2", 10)]
    steps_of = assign(panic, rules)
    for r, s in rules:
        if not any(re.search(r, l) for l in panic):
            raise ValueError("the panic transcript no longer has a line matching %r" % r)
    reclaimed = re.search(r"reclaimed (\d+) blocks, (\d+) bytes", t["s1Panic"]).groups()

    def lastline(name, rx):
        return [l for l in t[name].splitlines() if re.search(rx, l)]
    leak = lastline("s1Leak", r"^leak:")
    leakc = lastline("s1LeakCrash", r"^leak:")
    ctrl = lastline("s1LeakControl", r"^leak:")
    # (name, [(from, to, label, class)])
    slots = [
        ("HELLO.DXE", "app 1", [(0, 9, "running", "run"), (10, None, "stopped and unloaded", "gone")]),
        ("COUNTER.DXE", "app 2", [(0, 0, "not loaded", "none"), (1, 9, "running", "run"), (10, None, "stopped and unloaded", "gone")]),
        ("CRASH.DXE", "app 3", [(0, 2, "not loaded", "none"), (3, 3, "running", "run"), (4, 4, "holding 5 blocks of heap", "warn"), (5, 5, "crashed: panic caught", "bad"),
                                  (6, None, "unloaded, %s blocks and %s bytes reclaimed" % reclaimed, "gone")]),
        ("CRASH.DXE again", "app 4", [(0, 7, "not loaded", "none"), (8, None, "panicked, caught and unloaded, 0 blocks to reclaim", "gone")]),
        ("HELLO2.DXE", "app 5", [(0, 8, "not loaded", "none"), (9, None, "ran, stopped and unloaded", "gone")]),
    ]
    rows = []
    for name, app, states in slots:
        cells = "".join('<span class="mod-state mod-%s" data-from="%d"%s>%s</span>' % (
            cls, a, (' data-to="%d"' % b) if b is not None else "", esc(label)) for a, b, label, cls in states)
        rows.append('<li class="mod"><b>%s</b><small>%s</small><span class="mod-states">%s</span></li>' % (esc(name), app, cells))
    host = ('<div class="host" aria-label="The core, which stays running"><div class="host-h">Desk core (the host)</div><ul class="mods">%s</ul>'
            '<p class="host-note" data-at="5">Caught: the core jumps back with <code>longjmp</code> and carries on. The other modules keep their state.</p></div>' % "".join(rows))
    leak_rows = [("Stop and unload, 200 rounds: virtual memory", leak[1:2]), ("Same, heap break", leak[2:3]),
                 ("With a panic each round: virtual memory", leakc[1:2]), ("With a panic each round: heap break", leakc[2:3]),
                 ("Control, a 4 KB leak a round: virtual memory", ctrl[1:2]), ("Control: heap break", ctrl[2:3])]
    leakbox = ('<div class="leakbox" data-from="11"><div class="host-h">The 200-round leak test</div><ul>%s</ul></div>' % "".join(
        "<li><span>%s</span><code>%s</code></li>" % (esc(label), esc(l[0] if l else "(missing)")) for label, l in leak_rows))
    stage = host + leakbox + console(panic, steps_of, "The host's transcript (panic scenario), shown up to the current step")
    steps = [
        ("Load HELLO", "The core `dlopen`s `HELLO.DXE`, finds `_app_start`, `_app_event` and `_app_stop` with `dlsym` and starts it. The transcript shows the window it asks for and its first log line."),
        ("Load COUNTER", "A second module, with its own copy of `core` and `alloc` and its own state: `counter: start app=2`."),
        ("Events are delivered", "The core hands events to HELLO: `hello[1]: event 8 kind=1`. The call goes into the module and back."),
        ("Load CRASH", "The third module starts and handles an ordinary event. The host notes it holds no blocks yet."),
        ("CRASH takes memory and panics", "It allocates five blocks and then panics on purpose: `crash: holding 5 blocks, then panicking`."),
        ("The core catches it", "The Rust panic handler records the panic and the host recovers with `setjmp` and `longjmp`, which the transcript shows only as its result: `module CRASH crashed: panic: panic with heap (line 60)` and `state crashed`. The mechanism is in `docs/S1-DXE3.md`."),
        ("Unload", "The host unloads the dead module and reclaims what it held: %s blocks, %s bytes (`dlclose` returned 0)." % reclaimed),
        ("Everyone else carries on", "HELLO is untouched and gets its next event, `hello[1]: event 9`."),
        ("Crash again, this time with no memory held", "A fresh CRASH module panics with a different message and is unloaded with nothing to reclaim."),
        ("A new module takes the freed slot", "`HELLO2` loads, shows its own window and state (`kept=1`), and is stopped and unloaded."),
        ("An orderly shutdown", "COUNTER and HELLO are stopped and unloaded in turn, and the host reports `panic ok, the core kept running`."),
        ("Does any of it leak?", "Separate runs repeat load, run, stop and unload 200 times. Memory after round 200 is not lower and the heap break is not higher, with or without a panic each round. The control run, which leaks 4 KB a round on purpose, is reported as a leak, so the check can see one."),
    ]
    cap = ('Replayed from <code>spikes/s1-dxe3/expected/panic.txt</code>, <code>leak.txt</code>, <code>leak-crash.txt</code> and <code>leak-control.txt</code>, the transcripts that <code>spikes/s1-dxe3/run.sh</code> compares with each DOSBox run byte for byte. '
           'The module slots are the transcript\'s own events laid out by step; <code>longjmp</code> is described by the spike document and does not appear in the transcript. Headless DOSBox Staging only: no real hardware.')
    return stepper("modules", "Module life-cycle animation", stage, steps, cap, ctx)


# ---- link and pack -------------------------------------------------------------------------------------------------------------

def pipeline(D, ctx):
    s, loc = D["docSizes"], D["loc"]
    need = ["DESK.EXE", "HELLO.EXE", "CWSDPMI.EXE", "solo/HELLO.EXE", "CWSDSTUB.EXE"]
    for k in need:
        if k not in s:
            raise ValueError("docs/D0-RUST-DOS.md no longer states a size for %s" % k)
    b = "{:,}".format

    def box(x, y, w, h, l1, l2, cls, step=None, at=None):
        attrs = (' data-from="%d"' % step if step is not None else "") + (' data-at="%d"' % at if at is not None else "")
        return ('<g class="pl-box pl-%s"%s><rect x="%d" y="%d" width="%d" height="%d" rx="6"/><text x="%g" y="%g" text-anchor="middle" class="a-t1">%s</text>'
                '<text x="%g" y="%g" text-anchor="middle" class="a-t2">%s</text></g>') % (cls, attrs, x, y, w, h, x + w / 2, y + h / 2 - 2, esc(l1), x + w / 2, y + h / 2 + 14, esc(l2))

    def arrow(x1, y1, x2, y2, step=None):
        a = ' data-from="%d"' % step if step is not None else ""
        return '<line x1="%d" y1="%d" x2="%d" y2="%d" class="d-line" stroke-width="1.5" marker-end="url(#pl-arrow)"%s/>' % (x1, y1, x2, y2, a)

    g = ['<svg viewBox="0 0 980 330" role="img" class="diagram" aria-label="Two ways to build a DOS program. Top: the skeleton route, rustc COFF objects, coff2go32.py, DJGPP gcc, ld and libc, giving DESK.EXE at %s bytes. Bottom: the pure Rust route, rustc ELF objects, rust-lld, elf2go32.py with the 2 KiB stub, giving HELLO.EXE at %s bytes, run under CWSDPMI.EXE.">' % (b(s["DESK.EXE"]), b(s["HELLO.EXE"])),
         '<defs><marker id="pl-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" class="d-arrow"/></marker></defs>',
         '<text x="10" y="22" font-size="13" class="d-title">Skeleton route (PR #3): still in use</text>']
    y1, y2 = 34, 190
    g += [box(10, y1, 150, 64, "Rust core", "%s lines" % b(loc["skeletonCoreRs"]), "old"), arrow(160, y1 + 32, 190, y1 + 32),
          box(190, y1, 150, 64, "rustc objects", "COFF", "old"), arrow(340, y1 + 32, 370, y1 + 32),
          box(370, y1, 170, 64, "coff2go32.py", "%s lines of Python" % b(loc["coff2go32Py"]), "old"), arrow(540, y1 + 32, 570, y1 + 32),
          box(570, y1, 200, 64, "DJGPP gcc, ld, libc", "+ C layer, %s lines" % b(loc["skeletonC"]), "old"), arrow(770, y1 + 32, 800, y1 + 32),
          box(800, y1, 170, 64, "DESK.EXE", "%s bytes" % b(s["DESK.EXE"]), "old")]
    g.append('<text x="10" y="%d" font-size="13" class="d-title">Pure Rust route (PR #7): the spike</text>' % (y2 - 12))
    g += [box(10, y2, 150, 64, "Rust program + rt", "rt: %s lines" % b(loc["d0RtRs"]), "new", 0, 0), arrow(160, y2 + 32, 190, y2 + 32, 1),
          box(190, y2, 150, 64, "rustc objects", "ELF, no_std", "new", 1, 1), arrow(340, y2 + 32, 370, y2 + 32, 2),
          box(370, y2, 170, 64, "rust-lld", "link.ld, static ELF", "new", 2, 2), arrow(540, y2 + 32, 570, y2 + 32, 3),
          box(570, y2, 200, 64, "elf2go32.py + stub", "%s lines; the 2 KiB stub" % b(loc["elf2go32Py"]), "new", 3, 3), arrow(770, y2 + 32, 800, y2 + 32, 4),
          box(800, y2, 170, 64, "HELLO.EXE", "%s bytes" % b(s["HELLO.EXE"]), "new", 4, 4),
          arrow(885, y2 + 64, 885, y2 + 92, 5),
          box(690, y2 + 92, 280, 44, "CWSDPMI.EXE beside it", "%s bytes, the DPMI host" % b(s["CWSDPMI.EXE"]), "ext", 5, 5)]
    g.append('<text x="10" y="%d" font-size="12" class="d-text2" data-from="5">Self-contained variant: the same image on CWSDSTUB.EXE (%s bytes) is %s bytes with the host inside.</text>' % (y2 + 118, b(s["CWSDSTUB.EXE"]), b(s["solo/HELLO.EXE"])))
    g.append("</svg>")
    ratio = s["DESK.EXE"] / s["HELLO.EXE"]
    steps = [
        ("Write a Rust program", "A `no_std` program declares `rt::entry!(main)`. `rt` is the runtime: its own `_start`, a DPMI client, INT 21h I/O, a heap and a panic handler, %s lines in all." % b(loc["d0RtRs"])),
        ("rustc makes ELF objects", "With `-Zbuild-std` and the custom target `dos-elf.json`, rustc writes ELF objects. No C compiler runs."),
        ("rust-lld links them", "The linker that ships with Rust resolves every symbol and relocation using `link.ld` and writes a static 32-bit ELF executable."),
        ("elf2go32.py packs the result", "The packer reads the ELF's program headers and writes a DJGPP COFF image: the first load segment becomes `.text`, the next `.data` and `.bss`. It is %s lines of Python and only rearranges bytes." % b(loc["elf2go32Py"])),
        ("A stub in front", "The packer puts DJGPP's 2 KiB stub (written out by `stubify -g`) in front. The result, `HELLO.EXE`, is %s bytes. The skeleton's `DESK.EXE` is %s bytes (%.1f times as large) and does less." % (b(s["HELLO.EXE"]), b(s["DESK.EXE"]), ratio)),
        ("The stub finds the DPMI host", "At run time the stub loads `CWSDPMI.EXE` (%s bytes) from beside the program unless a host is already resident, switches to protected mode and jumps to `_start`. Packing the same image on `CWSDSTUB.EXE` instead gives one %s-byte file with the host built in; the arithmetic is %s of image plus the %s-byte stub and host." % (
            b(s["CWSDPMI.EXE"]), b(s["solo/HELLO.EXE"]), b(s["solo/HELLO.EXE"] - s["CWSDSTUB.EXE"]), b(s["CWSDSTUB.EXE"]))),
    ]
    stage = "".join(g) + ('<div class="x-replay"><div class="host-h">First lines the packed program printed in DOSBox</div>' + console(
        D["transcripts"]["d0Hello"].splitlines()[:10], [5] * 10, "The first lines of the program's transcript", highlight=False) + "</div>")
    cap = ('Sizes are the ones recorded in <code>docs/D0-RUST-DOS.md</code>, read back by the generator; line counts are counted in the tree of <code>main</code>. '
           'Intermediate sizes (the ELF, the objects) were not recorded, so none are shown. Run only in headless DOSBox Staging with CWSDPMI; not on real hardware.')
    return stepper("pipeline", "Link-and-pack pipeline", stage, steps, cap, ctx, "x-wide")


# ---- merge day -------------------------------------------------------------------------------------------------------------------

def mergeday(D, ctx):
    prs = sorted(D["prs"], key=lambda p: p["mergedAt"])
    sync_by_hash = {s["hash"]: s for s in D["sync"]}
    items = []
    for p in prs:
        sy = next((sync_by_hash[c["hash"]] for c in reversed(p["commits"]) if c["hash"] in sync_by_hash), None)
        items.append((p, sy))
    times = [datetime.datetime.fromisoformat(x["date"]) for p, sy in items for x in ([sy] if sy else []) + [{"date": p["mergedAt"]}]]
    t0, t1 = min(times), max(times)
    span = (t1 - t0).total_seconds() or 1

    def X(iso):
        return 150 + (datetime.datetime.fromisoformat(iso) - t0).total_seconds() / span * 780
    lane = {"main": 40}
    rows = [("main", "main")]
    for p, sy in items:
        if sy:
            rows.append((sy["branch"], "PR #%d" % p["number"]))
    H = 40 + 38 * len(rows)
    g = ['<svg viewBox="0 0 980 %d" role="img" class="diagram" aria-label="Seven pull requests merged into main between %s and %s Eastern time, each preceded by a merge of main into its branch.">' % (H, hhmm(t0.isoformat()), hhmm(t1.isoformat()))]
    for i, (name, tag) in enumerate(rows):
        y = 40 + 38 * i
        lane[name] = y
        g.append('<text x="0" y="%d" font-size="12" class="d-text2">%s</text><line x1="146" x2="940" y1="%d" y2="%d" class="m-lane"/>' % (y + 4, esc(name), y, y))
    steps = []
    for p, sy in items:
        if sy:
            i = len(steps)
            xs, ys = X(sy["date"]), lane[sy["branch"]]
            g.append('<g data-from="%d"><rect x="%g" y="%g" width="10" height="10" class="m-sync"><title>%s: merge main into %s</title></rect><text x="%g" y="%g" font-size="11" class="d-text2">%s</text></g>' % (
                i, xs - 5, ys - 5, sy["hash"], esc(sy["branch"]), xs + 8, ys - 8, hhmm(sy["date"])))
            conflicts = "; ".join("%s" % c["text"] for c in sy["conflicts"]) or "no conflicts recorded"
            clean = (" Merged cleanly: %s." % ", ".join("`%s`" % c for c in sy["clean"])) if sy["clean"] else ""
            steps.append(("Merge main into `%s`" % sy["branch"], "At %s, `%s`. Conflicts resolved: %s.%s" % (hhmm(sy["date"]), sy["hash"], conflicts, clean)))
        i = len(steps)
        xm, ym = X(p["mergedAt"]), lane["main"]
        if sy:
            g.append('<line x1="%g" y1="%g" x2="%g" y2="%g" class="d-line" stroke-width="1.2" data-from="%d"/>' % (X(sy["date"]), lane[sy["branch"]], xm, ym, i))
        g.append('<g data-from="%d"><circle cx="%g" cy="%d" r="7" class="m-merge"><title>%s PR #%d</title></circle><text x="%g" y="%d" text-anchor="middle" font-size="11" class="d-text2">#%d · %s</text></g>' % (
            i, xm, ym, p["mergeCommit"], p["number"], xm, ym - 14, p["number"], hhmm(p["mergedAt"])))
        steps.append(("PR #%d merged" % p["number"], "At %s, `%s`: %s. The pull request had %d commits and CI on the merge commit was %s." % (
            hhmm(p["mergedAt"]), p["mergeCommit"], p["title"], p["commitCount"], p["ci"])))
    g.append("</svg>")
    cap = ("Every point is a real commit: merges of <code>main</code> into a branch (squares) come from <code>git log --merges</code> and the merge commits on main (circles) from <code>git log --first-parent</code>; "
           "times are the authors' (Eastern Daylight Time). The conflict notes are the merge commits' own messages. The order is the order they were merged, not the order of their numbers.")
    return stepper("mergeday", "Merge day replay", "".join(g), steps, cap, ctx, "x-wide")


def conflicts_table(D, ctx):
    rows = []
    for s in D["sync"]:
        for c in s["conflicts"]:
            rows.append("<tr><td><code>%s</code></td><td>%s</td><td>%s</td><td>%s</td></tr>" % (s["hash"], hhmm(s["date"]), esc(s["branch"]), inline(c["text"], ctx)))
    return ('<div class="tbl"><table><thead><tr><th scope="col">Merge</th><th scope="col">Time</th><th scope="col">Branch</th><th scope="col">How the conflict was resolved</th></tr></thead><tbody>%s</tbody></table></div>' % "".join(rows))


# ---- figures -------------------------------------------------------------------------------------------------------------------

def faces_figure(D):
    pal = D["faces"]["palette"]
    drawings = D["faces"]["drawings"]
    out = []
    for d in drawings:
        rects = []
        for y, row in enumerate(d["grid"]):
            for x, ch in enumerate(row):
                if ch != "." and ch in pal:
                    rects.append('<rect x="%d" y="%d" width="1" height="1" fill="%s"/>' % (x, y, pal[ch]))
        out.append('<figure class="face"><div class="tile"><svg viewBox="0 0 16 16" role="img" aria-label="The 16 by 16 drawing of the distorted face at commit %s" shape-rendering="crispEdges">%s</svg></div>'
                   '<figcaption><b>%s</b> · <code>%s</code><br>%s</figcaption></figure>' % (d["commit"], "".join(rects), esc(day_label(d["date"]) + ", " + hhmm(d["date"])), d["commit"], esc(d["subject"])))
    return '<div class="faces">%s</div>' % "".join(out)


def tree_layout(roadmap):
    nodes = roadmap["nodes"]
    nb = {n["id"]: n for n in nodes}
    depth = {}

    def dep(i):
        if i not in depth:
            n = nb[i]
            depth[i] = max(dep(d) for d in n["deps"]) + 1 if n["deps"] else 0
        return depth[i]
    for n in nodes:
        dep(n["id"])
    return nb, depth


STATE_LABEL = {
    "done": "Catalogued: merged, CI green, checked beyond an emulator or not hardware-dependent",
    "done-emu": "Catalogued in an emulator; real hardware uncatalogued",
    "review": "In a pull request, awaiting review",
    "progress": "Partly done",
    "planned": "Wish list: planned",
    "research": "Wish list: an idea or research",
    "decision": "Open decision or blocked",
}
STATE_SHORT = {"done": "catalogued", "done-emu": "catalogued in an emulator", "review": "in review", "progress": "partly done",
               "planned": "planned", "research": "idea", "decision": "open decision"}


def state_icon(state, size=14):
    c = 'class="si si-%s"' % state
    if state == "done":
        body = '<circle cx="7" cy="7" r="6.5" class="si-fill"/><path d="M3.8 7.2 L6 9.4 L10.2 4.8" class="si-tick"/>'
    elif state == "done-emu":
        body = '<circle cx="7" cy="7" r="5.8" class="si-ring"/><path d="M4.2 7.2 L6.1 9.1 L9.8 5" class="si-ring"/>'
    elif state == "review":
        body = '<rect x="1.5" y="1.5" width="11" height="11" rx="2" class="si-fill"/><path d="M7 4 V7 L9 8.5" class="si-tick"/>'
    elif state == "progress":
        body = '<circle cx="7" cy="7" r="5.8" class="si-ring"/><path d="M7 1.2 A5.8 5.8 0 0 1 7 12.8 Z" class="si-fill"/>'
    elif state == "planned":
        body = '<circle cx="7" cy="7" r="5.6" class="si-ring" stroke-dasharray="2.2 1.8"/>'
    elif state == "research":
        body = '<circle cx="7" cy="7" r="5.8" class="si-ring" stroke-dasharray="0.1 2.4" stroke-linecap="round"/><text x="7" y="10.2" text-anchor="middle" font-size="8.5" font-weight="700" class="si-text">?</text>'
    else:
        body = '<path d="M7 0.8 L13.2 7 L7 13.2 L0.8 7 Z" class="si-fill"/><rect x="6.2" y="3.6" width="1.6" height="4.4" class="si-tick-fill"/><rect x="6.2" y="9" width="1.6" height="1.6" class="si-tick-fill"/>'
    return '<svg %s width="%d" height="%d" viewBox="0 0 14 14" aria-hidden="true" focusable="false">%s</svg>' % (c, size, size, body)


def legend(states):
    return '<ul class="legend" aria-label="Legend">%s</ul>' % "".join("<li>%s%s</li>" % (state_icon(s), esc(STATE_LABEL[s])) for s in states)


def tree(D):
    r = D["roadmap"]
    nodes = r["nodes"]
    nb, depth = tree_layout(r)
    bands = ["foundations", "hosted", "convergence", "native", "games"]
    binfo = {b["id"]: b for b in r["branches"]}
    children = {}
    for n in nodes:
        for d in n["deps"]:
            children.setdefault(d, []).append(n["id"])

    def band(n):
        return "convergence" if n["branch"] == "milestone" else n["branch"]

    def count(b):
        ns = [n for n in nodes if n["branch"] == b]
        return sum(1 for n in ns if n["state"].startswith("done")), len(ns)
    NW, NH, CP, RP, X0, HEAD, BGAP = 134, 44, 148, 52, 62, 30, 14
    pos, bandy, y = {}, {}, 6
    maxcol = max(depth.values())
    for b in bands:
        ns = [n for n in nodes if band(n) == b]
        cols = {}
        for n in ns:
            cols.setdefault(depth[n["id"]], []).append(n)
        maxrows = max(len(c) for c in cols.values())
        bandy[b] = (y, HEAD + maxrows * RP + 2)
        for c in sorted(cols):
            def py(n):
                ps = [pos[d]["y"] for d in n["deps"] if d in pos]
                return sum(ps) / len(ps) if ps else 0
            for i, n in enumerate(sorted(cols[c], key=py)):
                pos[n["id"]] = {"x": X0 + (c - 1) * CP, "y": y + HEAD + i * RP}
        y += bandy[b][1] + BGAP
    W, Hh = X0 + (maxcol - 1) * CP + NW + 8, y
    g = ['<svg viewBox="0 0 %d %d" width="%d" height="%d" role="img" class="diagram tree" aria-label="Roadmap tree with %d steps in five branches. %s The list below the tree has every step.">' % (
        W, Hh, W, Hh, len(nodes) - 1, " ".join("%s: %d of %d done." % (binfo[b]["label"], count(b)[0], count(b)[1]) for b in bands))]
    g.append('<defs><pattern id="hatch" patternUnits="userSpaceOnUse" width="6" height="6" patternTransform="rotate(45)"><rect width="6" height="6" class="t-hatch-bg"/><line x1="0" y1="0" x2="0" y2="6" class="t-hatch" stroke-width="2.5"/></pattern></defs>')
    for b in bands:
        by, bh = bandy[b]
        d, a = count(b)
        g.append('<rect x="52" y="%d" width="%d" height="%d" rx="6" class="t-band"/><text x="62" y="%d" font-size="13" class="t-bandl">%s<tspan class="d-muted" font-weight="400">   %d of %d steps done</tspan></text>' % (by, W - 52, bh, by + 19, esc(binfo[b]["label"]), d, a))
    ry, rh = 6, Hh - 6 - BGAP
    g.append('<rect x="0" y="%d" width="40" height="%d" rx="6" class="t-root"/><text x="20" y="%g" text-anchor="middle" font-size="22" transform="rotate(-90 20 %g)" class="t-rootl">Desk</text>' % (ry, rh, ry + rh / 2, ry + rh / 2))
    crit = {n["id"] for n in nodes if n.get("critical")}
    for n in nodes:
        if n["id"] == "desk":
            continue
        for d in n["deps"]:
            a, b = pos.get(d), pos.get(n["id"])
            if not b or (not a and d != "desk"):
                continue
            x1 = 40 if d == "desk" else a["x"] + NW
            y1 = b["y"] + NH / 2 if d == "desk" else a["y"] + NH / 2
            x2, y2 = b["x"], b["y"] + NH / 2
            c = n["id"] in crit and (d in crit or d == "desk")
            mx = (x1 + x2) / 2
            g.append('<path d="M%g,%g C%g,%g %g,%g %g,%g" fill="none" class="%s" stroke-width="%s"/>' % (x1, y1, mx, y1, mx, y2, x2 - 2, y2, "t-edge-crit" if c else "t-edge", 2.6 if c else 1.3))
    for n in nodes:
        if n["id"] == "desk":
            continue
        p, s = pos[n["id"]], n["state"]
        words, lines = n["label"].split(" "), [""]
        for w in words:
            if lines[-1] and len((lines[-1] + " " + w).strip()) > 15:
                lines.append(w)
            else:
                lines[-1] = (lines[-1] + " " + w).strip()
        lines = [n["label"]] if n["id"] == "first-window" else lines[:2]
        g.append('<g class="node" transform="translate(%d,%d)" tabindex="0" role="button" data-id="%s" aria-label="%s">' % (
            p["x"], p["y"], esc(n["id"]), esc("%s: %s%s%s" % (n["label"], STATE_SHORT[s], (", PR %d" % n["pr"]) if n.get("pr") else "", ", on the critical path" if n.get("critical") else ""))))
        if n.get("critical"):
            g.append('<rect x="-3" y="-3" width="%d" height="%d" rx="8" fill="none" class="t-crit" stroke-width="2"/>' % (NW + 6, NH + 6))
        g.append('<rect class="focus t-node t-n-%s" x="0" y="0" width="%d" height="%d" rx="6" stroke-width="1.6"/>' % (s, NW, NH))
        g.append('<rect x="1" y="1" width="26" height="%d" rx="5" class="t-tint t-tint-%s"/>' % (NH - 2, s))
        g.append('<g transform="translate(7,%g)">%s</g>' % (8 if n.get("pr") else (NH - 14) / 2, state_icon(s)))
        for i, l in enumerate(lines):
            g.append('<text x="33" y="%g" font-size="12" class="t-label">%s</text>' % (NH / 2 + (i - (len(lines) - 1) / 2) * 14 + 4, esc(l)))
        if n.get("pr"):
            g.append('<text x="14" y="%d" text-anchor="middle" font-size="9" class="t-pr">#%d</text>' % (NH - 4, n["pr"]))
        g.append("</g>")
    g.append("</svg>")
    # the list form: also the narrow layout and the text alternative
    lis = []
    for b in bands:
        d, a = count(b)
        items = []
        for n in sorted([n for n in nodes if band(n) == b], key=lambda n: depth[n["id"]]):
            kids = [nb[k]["label"] for k in children.get(n["id"], [])]
            needs = [nb[k]["label"] for k in n["deps"]]
            pr = (" · #%d" % n["pr"]) if n.get("pr") else ""
            items.append('<li class="%s"><details id="node-%s"><summary class="vnode%s">%s<span class="lbl">%s</span><span class="st">%s</span></summary><div class="vmore"><div>%s</div><div><b>Why it matters:</b> %s</div><div><b>Needs:</b> %s</div><div><b>Unlocks:</b> %s</div><div><b>Evidence:</b> %s</div></div></details></li>' % (
                "crit" if n.get("critical") else "", esc(n["id"]), " crit" if n.get("critical") else "", state_icon(n["state"], 16), esc(n["label"] + pr), esc(STATE_SHORT[n["state"]]),
                esc(n["what"]), esc(n["why"]), esc(", ".join(needs) or "nothing"), esc(", ".join(kids) or "nothing yet"), esc(n["evidence"])))
        lis.append('<div class="vband"><h3><span>%s</span><span class="muted">%d of %d done</span></h3><ol>%s</ol></div>' % (esc(binfo[b]["label"]), d, a, "".join(items)))
    used = []
    for s in ["done", "done-emu", "progress", "planned", "research", "decision"]:
        if any(n["state"] == s for n in nodes):
            used.append(s)
    detail = '<div class="detail" id="treeDetail" aria-live="polite"><div class="muted">Choose a step in the tree to read what it is, why it matters, what it needs and what it unlocks.</div></div>'
    nodedata = {n["id"]: {"label": n["label"], "state": STATE_SHORT[n["state"]], "pr": n.get("pr"), "critical": bool(n.get("critical")), "what": n["what"], "why": n["why"],
                          "needs": [nb[k]["label"] for k in n["deps"]], "unlocks": [nb[k]["label"] for k in children.get(n["id"], [])], "evidence": n["evidence"],
                          "branch": binfo.get(band(n), {"label": "Root"})["label"]} for n in nodes}
    import json
    return ('<figure class="treefig"><div class="tree-wide"><div class="chart scroll" tabindex="0" aria-label="Roadmap tree, scrolls sideways">%s</div>%s</div>'
            '%s<details class="alt" open><summary>Every step as a list</summary>%s</details>'
            '<script type="application/json" id="treeData">%s</script>'
            '<figcaption>Every box is a row of <code>tools/progress-report/roadmap.json</code>. A step only counts as done if its pull request is merged and CI passed on the merge commit, which the generator checks. The amber line is the critical path to the first milestone.</figcaption></figure>') % (
        "".join(g), detail, legend(used + ([]) ), "".join(lis), json.dumps(nodedata, ensure_ascii=False).replace("</", "<\\/"))


def specimens():
    rows = [
        ("Desk Title", "desk-title", "The quick brown fox jumps over the lazy dog", 'font-size:24px'),
        ("Desk UI", "desk-ui", "Files · Programs · Settings — Pack my box with five dozen liquor jugs", 'font-size:22px'),
        ("Desk Small", "desk-small", "SMALL CAPS FOR LABELS AND STATUS LINES 0123456789", 'font-size:16px'),
        ("Desk Fixed", "desk-fixed", "C:\\DESK> dir /w   FIZZ.EXE  README.TXT  SLOT1.SAV", 'font-size:16px'),
        ("Desk Sans", "desk-sans", "Waltz, nymph, for quick jigs vex Bud. Office affluent fiasco: fi fl ff ffi", 'font-size:20px'),
        ("Desk Serif", "desk-serif", "A calm graphical system. Office affluent fiasco: fi fl ff ffi", 'font-size:20px'),
        ("Desk Mono", "desk-mono", "fn main() -> i32 { x != y && a <= b => c === d }", 'font-size:16px;font-feature-settings:"liga" 1'),
    ]
    out = []
    for name, cls, text, style in rows:
        out.append('<div class="spec-row"><div class="tag">%s</div><div class="sample spec-%s" style="%s">%s</div></div>' % (name, cls, style, esc(text)))
    return ('<figure class="spec"><div class="ground spec-dark"><div class="g-tag">on the Desk\'s dark ground</div>%s</div>'
            '<div class="ground spec-light"><div class="g-tag">on its light ground</div>%s</div>'
            '<figcaption>Live text set in the fonts built in PR #1, loaded from the published site\'s own <code>fonts/</code> files (the OpenType files from <code>fonts/generated/</code>). The pixel faces are meant for whole sizes; the ligatures in Desk Sans and Desk Mono come from the font\'s own tables.</figcaption></figure>') % ("".join(out), "".join(out))


def contact():
    return ('<figure class="contact"><div class="tabs" role="group" aria-label="Contact sheet size">'
            '<button type="button" data-sheet="16" aria-pressed="true">16 px</button><button type="button" data-sheet="12" aria-pressed="false">12 px</button>'
            '<button type="button" data-sheet="8" aria-pressed="false">8 px (40 emoji)</button></div>'
            '<div class="sheet" tabindex="0" aria-label="Contact sheet of the pixel emoji, scrolls"><img id="sheetImg" src="img/pixel-16.png" alt="Contact sheet of all 480 pixel emoji at 16 by 16 pixels, drawn in the 16-colour Desk palette"></div>'
            '<figcaption>The generated contact sheets from <code>fonts/generated/emoji/</code>, drawn from the same letter grids that build the fonts. At 8 px only 40 emoji are drawn.</figcaption></figure>')


def design_shots():
    shots = [("shots/01-v4-800.jpg", "The Afterglow Desk prototype at 800 by 600: the Files window over the desktop, a launcher column at the left and the taskbar at the bottom."),
             ("shots/02-v4-16.jpg", "The same prototype in its 16-colour mode."),
             ("shots/v4-menu.jpg", "The prototype's Desk menu."),
             ("shots/files-resize.jpg", "A Files window being resized.")]
    return '<div class="shots">%s</div>' % "".join(
        '<figure><img src="%s" alt="%s" loading="lazy" width="924" height="540"><figcaption>%s</figcaption></figure>' % (s, esc(a), esc(a)) for s, a in shots)


def qemu_figure():
    return ('<figure class="qemu"><div class="screen"><img src="img/qemu-boot.png" alt="The QEMU screen after the boot: the loader and kernel lines on a 720 by 400 text screen" width="720" height="400"></div>'
            '<figcaption>The VGA text screen at the end of a headless QEMU run of the floppy image (<code>-cpu 486 -m 16</code>), captured with the monitor\'s <code>screendump</code>; the same lines went to the serial port as the transcript above.</figcaption></figure>')


def nexts(D):
    cards = []
    for b in D["roadmap"]["branches"]:
        cards.append('<section class="next-card"><h3>%s</h3><p>%s</p><p class="blk"><b>In the way:</b> %s</p></section>' % (esc(b["label"]), esc(b["next"]), esc(b["blocker"])))
    return '<div class="nexts">%s</div>' % "".join(cards)
