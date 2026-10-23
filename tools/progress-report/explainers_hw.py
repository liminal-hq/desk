# The explainers of the hardware chapters (the HAL, input, graphics, std, the browser and the two input pipelines), each driven by files
# captured from the branches that hold the real thing (media/captures.json) and refusing to render when the capture disagrees.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# The conventions are those of explainers.py: every step's full text is in an ordered list, the stage shows its last state without
# script, and site.js hides what has not happened yet (data-from, data-to, data-at). Where something is derived rather than recorded
# (the scancode bytes of a key that a transcript only shows decoded), the figure says so, and the derivation is checked against the
# recorded transcript before the page is written.
import os
import re

from explainers import console, esc, stepper
from mdlite import Raw, inline

HERE = os.path.dirname(os.path.abspath(__file__))


def media(name):
    with open(os.path.join(HERE, "media", name), encoding="utf-8") as f:
        return f.read()


def hx(v, width=2):
    return "0x%0*X" % (width, v)


def code_block(lines, marks, label, cls=""):
    """Source lines as a keyboard-reachable block; marks is {line index: (first step, last step)} for the lines a step points at."""
    out = []
    for i, l in enumerate(lines):
        mk = ""
        if i in marks:
            a, b = marks[i]
            mk = '<i class="mk" data-from="%d" data-to="%d" aria-hidden="true">&#9654;</i>' % (a, b)
        out.append('<span class="ln">%s%s</span>' % (mk, esc(l) if l else " "))
    return '<pre class="term x-code %s" tabindex="0" aria-label="%s"><code>%s</code></pre>' % (cls, esc(label), "".join(out))


def captable(name, ctx, caption_for=""):
    """A table captured from a document on another branch, as an HTML table (the cell text goes through the inline reader)."""
    rows = [r for r in media(name).strip().splitlines() if r.startswith("|")]
    if len(rows) < 3:
        raise ValueError("capture %s has no table" % name)

    def cells(r):
        return [c.strip() for c in re.split(r"(?<!\\)\|", r.strip().strip("|"))]

    def fix(c):
        return re.sub(r"``\s*(.*?)\s*``", lambda m: "`%s`" % m.group(1).replace("`", "'"), c)
    head = cells(rows[0])
    body = [cells(r) for r in rows[2:]]
    th = "".join('<th scope="col">%s</th>' % inline(fix(c), ctx) for c in head)
    tr = "".join("<tr>%s</tr>" % "".join("<td>%s</td>" % inline(fix(c), ctx) for c in r) for r in body)
    return ('<div class="tbl"><table><caption class="sr">%s</caption><thead><tr>%s</tr></thead><tbody>%s</tbody></table></div>' % (esc(caption_for or name), th, tr))


def cap_info(D, name):
    for c in D.get("captures", []):
        if c["file"] == name:
            return c
    return {}


def cap_note(D, names):
    """One line saying where captured files came from and whether they still match their branch."""
    bits = []
    for n in names:
        c = cap_info(D, n)
        if c:
            bits.append("<code>%s:%s</code> at <code>%s</code> (%s)" % (esc(c["branch"]), esc(c["path"]), esc(c.get("commit") or "?"),
                                                                       {"matches": "identical to the branch head now", "not checked": "the branch was not available to re-check", "differs": "the branch has moved on", "missing": "capture missing"}[c["status"]]))
    return "Captured from " + "; ".join(bits) + "."


# ---- the layers of the HAL ------------------------------------------------------------------------------------------------------

def hal_layers(D, ctx):
    """The four kinds of code and the one crate with two edges, as a diagram: drawn solid blue where it is built and in an open pull
    request, dashed where only designed."""
    W, H = 900, 470
    o = ['<svg viewBox="0 0 %d %d" role="img" class="diagram" aria-label="Layers of the HAL: the Desk core calls the upper edge of one crate, desk-hal; drivers are generic over its lower edge and contain no unsafe; three platforms implement the lower edge, and each holds the one audited unsafe step that makes a port bus, a memory region or an interrupt handler.">' % (W, H),
         '<defs><marker id="hl-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" class="d-arrow"/></marker></defs>']

    def box(x, y, w, h, kind, t1, t2="", t3=""):
        dash = ' stroke-dasharray="6 4"' if kind == "plan" else ""
        o.append('<rect x="%d" y="%d" width="%d" height="%d" rx="6" class="a-%s" stroke-width="1.6"%s/>' % (x, y, w, h, kind, dash))
        cx = x + w / 2
        ys = [y + h / 2 + 4] if not t2 else ([y + h / 2 - 3, y + h / 2 + 13] if not t3 else [y + h / 2 - 10, y + h / 2 + 5, y + h / 2 + 20])
        o.append('<text x="%g" y="%g" text-anchor="middle" font-size="13" class="a-t1 a-t1-%s">%s</text>' % (cx, ys[0], kind, esc(t1)))
        if t2:
            o.append('<text x="%g" y="%g" text-anchor="middle" font-size="11.5" class="a-t2">%s</text>' % (cx, ys[1], esc(t2)))
        if t3:
            o.append('<text x="%g" y="%g" text-anchor="middle" font-size="11.5" class="a-t2">%s</text>' % (cx, ys[2], esc(t3)))

    def arrow(x1, y1, x2, y2, label="", dx=8, anchor="start"):
        o.append('<line x1="%g" y1="%g" x2="%g" y2="%g" class="d-line" stroke-width="1.3" marker-end="url(#hl-arrow)"/>' % (x1, y1, x2, y2))
        if label:
            o.append('<text x="%g" y="%g" font-size="11" class="d-muted" text-anchor="%s">%s</text>' % ((x1 + x2) / 2 + dx, (y1 + y2) / 2 + 4, anchor, esc(label)))

    o.append('<text x="10" y="22" font-size="12" class="d-muted">Desk core</text><text x="10" y="112" font-size="12" class="d-muted">desk-hal</text>'
             '<text x="10" y="226" font-size="12" class="d-muted">Drivers</text><text x="10" y="358" font-size="12" class="d-muted">Platforms</text>')
    box(130, 8, 760, 44, "plan", "Desk core: compositor, windows, widgets, event loop", "no ports, no DOS, no unsafe; sees only the upper edge")
    arrow(510, 52, 510, 78, "calls the upper edge", 8)
    box(130, 80, 760, 112, "review", "desk-hal: one crate, two edges")
    box(146, 118, 360, 60, "plan", "Upper edge (core side)", "Display · InputSource · Clock · DebugOut", "mostly sketches: only DebugOut and the input types exist")
    box(520, 118, 354, 60, "review", "Lower edge (driver side)", "PortIo · MmioRegion · IrqRegistry · Clock", "SpscRing · TickCounter · Irq · HalError")
    arrow(697, 192, 697, 214, "generic over P: PortIo, static dispatch", 8)
    box(130, 216, 760, 96, "review", "Drivers: device logic written once, no unsafe")
    box(146, 248, 390, 52, "review", "serial · PIT · PIC", "host-tested on the mock bus (PR #8)")
    box(548, 248, 156, 52, "review", "8042 · keyboard", "PR #10")
    box(716, 248, 158, 52, "review", "VGA/VBE · INT 33h", "PRs #11 and #15")
    box(130, 322, 760, 26, "plan", "not started: OPL, Sound Blaster, DMA, IDE and floppy")
    arrow(697, 312, 697, 322)
    o.append('<line x1="130" y1="356" x2="890" y2="356" class="d-line" stroke-dasharray="3 4" stroke-width="1"/>')
    o.append('<text x="130" y="372" font-size="12" class="d-warn">The audited unsafe boundary: once per platform</text>')
    box(130, 382, 240, 70, "review", "Mock platform", "MockPortBus, device models", "host tests (PR #8)")
    box(386, 382, 240, 70, "review", "Native kernel", "kernel/src/hal.rs", "RawPorts::new, IDT stubs (PR #10)")
    box(642, 382, 248, 70, "review", "Hosted DOS runtime", "rt::hal, rt::irq", "DPMI calls (PRs #11 and #15)")
    for cx in (250, 506, 766):
        arrow(cx, 382, cx, 352)
    o.append("</svg>")
    return "\n".join(o)


# ---- the PIT divisor test, step by step ---------------------------------------------------------------------------------------------

def pit_test(D, ctx):
    """A step-through of `programming_100_hz_writes_what_the_kernel_writes` and its neighbour, from the real test code in hal/drivers/pit."""
    src = media("hal-pit.rs.txt")
    consts = {}
    for k in ("INPUT_HZ", "CHANNEL0", "COMMAND", "MAX_DIVISOR", "MIN_DIVISOR", "CMD_CH0_LOHI_RATE", "CMD_CH0_LOHI_SQUARE", "CMD_CH0_LATCH"):
        m = re.search(r"const %s: \w+ = ([0-9_]+|0x[0-9A-Fa-f]+);" % k, src)
        if not m:
            raise ValueError("the PIT source no longer defines %s; update explainers_hw.pit_test" % k)
        consts[k] = int(m.group(1).replace("_", ""), 0)
    m = re.search(r"    fn programming_100_hz_writes_what_the_kernel_writes\(\) \{\n(.*?)\n    \}\n", src, re.S)
    m2 = re.search(r"    fn the_chip_model_sees_mode_2_and_the_reload_value\(\) \{\n(.*?)\n    \}\n", src, re.S)
    if not m or not m2:
        raise ValueError("the PIT tests this explainer replays have been renamed; update explainers_hw.pit_test")
    body = [l[4:] if l.startswith("    ") else l for l in m.group(1).splitlines()]
    script = [(int(a, 16), int(b, 16)) for a, b in re.findall(r"Access::outb\((0x[0-9A-Fa-f]+), (0x[0-9A-Fa-f]+)\)", m.group(1))]
    hz = 100
    divisor = (consts["INPUT_HZ"] + hz // 2) // hz
    reload_ = divisor % consts["MAX_DIVISOR"]
    expect = [(consts["COMMAND"], consts["CMD_CH0_LOHI_RATE"]), (consts["CHANNEL0"], reload_ & 0xFF), (consts["CHANNEL0"], reload_ >> 8)]
    if "Ok(%s)" % "{:,}".format(divisor).replace(",", "_") not in m.group(1) or script != expect:
        raise ValueError("the PIT test's script %r is not what the divisor arithmetic gives %r" % (script, expect))
    cmd = consts["CMD_CH0_LOHI_RATE"]
    chan, acc, mode, bcd = cmd >> 6, (cmd >> 4) & 3, (cmd >> 1) & 7, cmd & 1
    if (chan, acc, mode, bcd) != (0, 3, 2, 0):
        raise ValueError("command byte %s does not decode as channel 0, lo/hi, mode 2, binary" % hex(cmd))
    body2 = m2.group(1)
    hz2 = 1000
    div2 = (consts["INPUT_HZ"] + hz2 // 2) // hz2
    if "(3, 2, false, %s, true)" % "{:_}".format(div2) not in body2 or "[(0x43, 0x36), (0x40, 0), (0x40, 0)]" not in body2:
        raise ValueError("the PIT chip-model test no longer says what the explainer says it does")
    bios = (consts["COMMAND"], consts["CMD_CH0_LOHI_SQUARE"])

    marks = {}
    for i, l in enumerate(body):
        if "scripted" in l:
            marks[i] = (0, 0)
        elif "program_periodic(100)" in l:
            marks[i] = (1, 3)
        elif "script_done" in l:
            marks[i] = (7, 7)
    src_html = code_block(["#[test]", "fn programming_100_hz_writes_what_the_kernel_writes() {"] + body + ["}"], {k + 2: v for k, v in marks.items()},
                          "The test code, as it is in hal/drivers/pit/src/lib.rs")

    names = ["command register, 0x%02X" % consts["COMMAND"], "channel 0, low byte", "channel 0, high byte"]
    meaning = ["channel 0, lo then hi byte, mode 2 (rate generator), binary", "low byte of the reload value", "high byte of the reload value"]
    rows = []
    for k, ((port, val), nm, mn) in enumerate(zip(script, names, meaning)):
        rows.append('<tr><th scope="row">%d</th><td><code>outb(%s, %s)</code></td><td>%s</td><td class="bus-seen"><span data-from="%d" class="ok">seen, in order</span><span data-to="%d" class="wait">waiting</span></td></tr>' % (
            k + 1, hx(port), hx(val), esc(mn), 4 + k, 3 + k))
    bus = ('<div class="tbl"><table class="bus"><caption>The mock bus: the script the test hands it, and what it has seen</caption><thead><tr><th scope="col">#</th><th scope="col">Access</th><th scope="col">What it does</th>'
           '<th scope="col">Bus</th></tr></thead><tbody>%s</tbody></table></div>' % "".join(rows))
    bits = "".join('<span class="bit b%d">%d</span>' % (i, (cmd >> (7 - i)) & 1) for i in range(8))
    cmdbox = ('<div class="px-box" data-from="4"><div class="px-h">Command byte %s</div><div class="bits" role="img" aria-label="Command byte %s as bits: %s">%s</div>'
              '<ul class="px-l"><li><b>bits 7-6</b> = %s: channel 0</li><li><b>bits 5-4</b> = %s: low byte then high byte</li><li><b>bits 3-1</b> = %s: mode 2, rate generator</li><li><b>bit 0</b> = %d: binary count</li></ul></div>' % (
                  hx(cmd), hx(cmd), format(cmd, "08b"), bits, format(chan, "02b"), format(acc, "02b"), format(mode, "03b"), bcd))
    arith = ('<div class="px-box" data-from="1"><div class="px-h">The divisor</div><p class="px-calc"><code>(%s + %d / 2) / %d</code> <span data-from="2">= <b>%s</b> <span class="muted">(%s)</span></span></p>'
             '<p class="px-calc" data-from="3"><code>%s %% %s</code> as a 16-bit reload value = <b>%s</b>: low byte <b>%s</b>, high byte <b>%s</b></p></div>' % (
                 "{:,}".format(consts["INPUT_HZ"]), hz, hz, "{:,}".format(divisor), hx(divisor, 4), "{:,}".format(divisor), "{:,}".format(consts["MAX_DIVISOR"]), hx(reload_, 4), hx(reload_ & 0xFF), hx(reload_ >> 8)))
    chip = ('<div class="px-box" data-from="8"><div class="px-h">Second test: the same driver against a chip model (<code>Pit8254</code>)</div>'
            '<p class="px-calc">1,000 Hz asks for <b>%s</b>; the model reports <code>(access 3, mode 2, bcd false, reload %s, loaded true)</code>. <code>restore_bios_rate()</code> then writes <code>outb(%s, %s)</code>, <code>outb(0x40, 0)</code>, <code>outb(0x40, 0)</code>: mode 3, the BIOS rate, 65,536 written as 0.</p></div>' % (
                "{:,}".format(div2), "{:,}".format(div2), hx(bios[0]), hx(bios[1])))
    stage = '<div class="px-pit">%s%s%s%s%s</div>' % (src_html, arith, cmdbox, bus, chip)
    steps = [
        ("The test hands the bus a script", "`MockPortBus::scripted([...])` is told the three port writes it must see, in this order. A write that differs, comes out of order or is missing fails the test. The three values were written out by hand from the native kernel's own `kernel/src/pit.rs`, which is what makes this a check against the kernel and not against itself."),
        ("Ask for 100 Hz", "`Pit::new(&bus).program_periodic(100)`: the driver is generic over the bus, so the same code that will run on a real port bus runs here against the script."),
        ("Work out the divisor", "`divisor_for(100)` is `(INPUT_HZ + hz / 2) / hz` with the input clock %s Hz, which is %s: the nearest whole divisor, rounded as the kernel rounds. The test asserts `Ok(%s)`." % ("{:,}".format(consts["INPUT_HZ"]), "{:,}".format(divisor), "{:_}".format(divisor))),
        ("Split it into bytes", "The chip is programmed with a 16-bit reload value, one byte at a time. %s is %s, so the low byte is %s and the high byte %s. A divisor of 65,536 would be written as 0; 100 Hz is not that case." % ("{:,}".format(divisor), hx(reload_, 4), hx(reload_ & 0xFF), hx(reload_ >> 8))),
        ("First write: the command", "`outb(0x43, %s)`. Read as bits it selects channel 0, low byte then high byte, mode 2 (a rate generator, so the chip reloads and keeps ticking) and a binary count. The script expected exactly this." % hx(cmd)),
        ("Second write: the low byte", "`outb(0x40, %s)` to channel 0's data port. The mock bus compares it with the script's second line and records a match." % hx(reload_ & 0xFF)),
        ("Third write: the high byte", "`outb(0x40, %s)`. The chip now holds the reload value %s and counts down from it, interrupting about every 10 ms." % (hx(reload_ >> 8), hx(reload_, 4))),
        ("The script is done", "`assert!(bus.script_done())` passes: all three accesses were seen, in order, and nothing else. The whole check ran on the development machine's CPU as an ordinary `cargo test`, with no emulator, no kernel and no hardware."),
        ("A second test, with a chip model", "Another test attaches `Pit8254`, a model of the chip's registers, to the bus and programs 1,000 Hz. The model ends in mode 2 with reload %s, and restoring the BIOS rate puts it back in mode 3 with reload 0 (65,536). The model is the author's reading of the data sheet; it has never been compared with a real 8254." % "{:,}".format(div2)),
    ]
    cap = ("A step-through of two real tests from <code>hal/drivers/pit/src/lib.rs</code> (PR #8). The source, the constants and the three expected writes are read from the captured file, and the page refuses to build if the divisor arithmetic here disagrees with the test's own script. %s" % cap_note(D, ["hal-pit.rs.txt"]))
    return stepper("pit-test", "Programming the PIT for 100 Hz, as a test against a mock port bus", stage, steps, cap, ctx, "x-wide")


# ---- typing in the dark: the injected input, replayed -----------------------------------------------------------------------------

KEYNAMES = {"a": "A", "1": "Num1", "ret": "Enter", "kp_enter": "KpEnter", "up": "Up", "esc": "Escape", "delete": "Delete", "shift": "LeftShift",
            "ctrl": "LeftCtrl", "alt": "LeftAlt"}


def kbd_tables():
    src = media("kbd-lib.rs.txt")
    out = {}
    for fn, ext in (("plain_key", False), ("extended_key", True)):
        m = re.search(r"pub fn %s\(scancode: u8\) -> KeyCode \{\n(.*?)\n\}\n" % fn, src, re.S)
        if not m:
            raise ValueError("the keyboard decoder no longer has %s; update explainers_hw.kbd_tables" % fn)
        for code, name in re.findall(r"0x([0-9A-Fa-f]{2}) => (\w+),", m.group(1)):
            out.setdefault(name, []).append((ext, int(code, 16)))
    return out


def decode_replica(stream, tables):
    """A small model of desk_kbd::Decoder and char_for, enough for the keys in the injected run: prefix state, make and break codes,
    the shift and control and alt modifiers, and the characters. Returns (KeyName, down, mods, char) tuples."""
    inv = {(ext, code): name for name, lst in tables.items() for ext, code in lst}
    ext, mods, out = False, set(), []
    for b in stream:
        if b == 0xE0:
            ext = True
            continue
        down, code = not (b & 0x80), b & 0x7F
        name = inv[(ext, code)]
        ext = False
        flag = {"LeftShift": "shift", "LeftCtrl": "ctrl", "LeftAlt": "alt"}.get(name)
        if flag:
            (mods.add if down else mods.discard)(flag)
        shift = "shift" in mods
        ch = None
        if down:
            if name == "A":
                ch = "A" if shift else "a"
            elif name == "Num1":
                ch = "!" if shift else "1"
            elif name in ("Enter", "KpEnter"):
                ch = "\\n"
            elif name == "Escape":
                ch = "\\u{1b}"
        order = [m for m in ("shift", "ctrl", "alt") if m in mods]
        out.append((name, down, "+".join(order) or "-", ch))
    return out


def event_line(ev):
    name, down, mods, ch = ev
    return "key: %s %s mods=%s%s" % (name, "down" if down else "up", mods, (" char='%s'" % ch) if ch is not None else "")


def mouse_packet(dx, dy, buttons):
    """The four bytes an IntelliMouse sends for a motion of (dx, dy) screen units and a button state: dy positive is down the screen,
    the device counts up as positive, so its Y count is the negation."""
    ydev = -dy
    b0 = 0x08 | buttons | (0x10 if dx < 0 else 0) | (0x20 if ydev < 0 else 0)
    return [b0, dx & 0xFF, ydev & 0xFF, 0]


def packet_decode(p):
    dx = p[1] - (0x100 if p[0] & 0x10 else 0)
    ydev = p[2] - (0x100 if p[0] & 0x20 else 0)
    return dx, -ydev, p[0] & 7


def input_replay(D, ctx):
    transcript = media("input-expected.txt").splitlines()
    inject = [l.split() for l in media("input-inject.txt").splitlines() if l.strip() and not l.startswith("#")]
    cmds = [c for c in inject if c[0] in ("mouse_move", "mouse_button", "sendkey")]
    start = transcript.index("input: ready")
    real = [l for l in transcript[start + 1:] if l.startswith(("key: ", "mouse: "))]
    real_keys = [l for l in real if l.startswith("key: ")]
    real_mouse = [l for l in real if l.startswith("mouse: ")]
    tail = next(l for l in transcript if l.startswith("input: escape released"))
    tables = kbd_tables()

    def bytes_for(name, brk):
        ext, code = tables[name][0]
        return ([0xE0] if ext else []) + [code | (0x80 if brk else 0)]

    steps, rows = [], []  # rows: per command dict
    ki = mi = 0
    head = 0
    mhead = mtail = 0
    mbtn = 0
    for c in cmds:
        row = {"cmd": " ".join(c)}
        if c[0] == "sendkey":
            parts = c[1].split("-")
            names = [KEYNAMES[p] for p in parts]
            stream = [b for n in names for b in bytes_for(n, False)] + [b for n in reversed(names) for b in bytes_for(n, True)]
            evs = decode_replica(stream, tables)
            lines = [event_line(e) for e in evs]
            if lines != real_keys[ki:ki + len(lines)]:
                raise ValueError("the decoded keys for %r (%r) differ from the recorded transcript (%r): the replica no longer matches" % (c, lines, real_keys[ki:ki + len(lines)]))
            ki += len(lines)
            row.update(kind="key", stream=stream, lines=lines, head0=head, head1=head + len(stream))
            head += len(stream)
        else:
            if c[0] == "mouse_move":
                dx, dy = int(c[1]), int(c[2])
            else:
                mbtn = int(c[1])
                dx = dy = 0
            pk = mouse_packet(dx, dy, mbtn)
            ddx, ddy, bb = packet_decode(pk)
            line = "mouse: dx=%d dy=%d buttons=%d wheel=0" % (ddx, ddy, bb)
            if line != real_mouse[mi]:
                raise ValueError("the decoded mouse packet for %r (%s) differs from the recorded transcript (%s)" % (c, line, real_mouse[mi]))
            mi += 1
            row.update(kind="mouse", stream=pk, lines=[line], head0=mhead, head1=mhead + 1)
            mhead += 1
        rows.append(row)
    if ki != len(real_keys) or mi != len(real_mouse):
        raise ValueError("the replay did not use every recorded event (%d of %d keys, %d of %d mouse)" % (ki, len(real_keys), mi, len(real_mouse)))
    if not re.search(r"after %d key events and %d mouse events" % (len(real_keys), len(real_mouse)), tail):
        raise ValueError("the recorded summary line does not match the event counts")
    if "dropped keys 0, dropped mouse events 0" not in "\n".join(transcript):
        raise ValueError("the recorded run no longer ends with nothing dropped")

    # the transcript console: the recorded lines, each shown from the step that decodes it
    console_lines, console_steps = ["input: ready"], [0]
    for i, row in enumerate(rows):
        for l in row["lines"]:
            console_lines.append(l)
            console_steps.append(i + 1)
    console_lines.append(tail)
    console_steps.append(len(rows) + 1)

    stages = []
    for i, row in enumerate(rows):
        n = i + 1
        isk = row["kind"] == "key"
        size = 64 if isk else 32
        h0, h1 = row["head0"], row["head1"]
        cells_ = []
        for s in range(size):
            pushed = any(((h0 + k) & (size - 1)) == s for k in range(h1 - h0))
            seen = s < h1 and not pushed
            cells_.append('<i class="c%s"></i>' % (" new" if pushed else (" old" if seen else "")))
        hexs = " ".join("%02X" % b for b in row["stream"])
        stages.append(
            '<div class="px-ring" data-at="%d"><div class="px-h">%s</div>'
            '<p class="px-calc"><b>%s</b> → %s <span class="muted">(%s)</span></p>'
            '<div class="cells" role="img" aria-label="%s ring after this step: %d slots, head %d, tail %d">%s</div>'
            '<p class="px-ix">head <b>%d</b> · tail <b>%d</b> · slot %s <span class="muted">(index and %d; the indices only ever count up)</span></p></div>' % (
                n, "Key ring: 64 bytes, filled by IRQ 1, drained by the kernel's loop" if isk else "Mouse ring: 32 events, one per packet, filled by IRQ 12",
                esc(row["cmd"]), "bytes <code>%s</code> into the ring%s" % (hexs, "" if isk else " (a four-byte packet the IRQ 12 handler assembles into one event)"),
                "derived from the scancode set 1 table" if isk else "derived from the PS/2 packet format",
                "Key" if isk else "Mouse", size, h1, h1, "".join(cells_), h1, h1, "%d" % ((h1 - 1) & (size - 1)) if h1 else "-", size - 1))
    intro = ('<div class="px-ring" data-at="0"><div class="px-h">Before anything is injected</div><p class="px-calc">The kernel has printed <code>input: ready</code>: both handlers are registered and both rings are empty (head 0, tail 0). The test script then plays %d commands into QEMU\'s monitor.</p></div>' % len(rows))
    outro = ('<div class="px-ring" data-at="%d"><div class="px-h">Escape released</div><p class="px-calc">%s</p><p class="px-calc muted">%d bytes went through the key ring of 64 slots and %d events through the mouse ring of 32, so neither wrapped.</p></div>' % (
        len(rows) + 1, esc(tail), head, mhead))
    cons = console(console_lines, console_steps, "The recorded transcript of the injected-input run, shown up to the current step")
    stage = "".join([intro] + stages + [outro]) + cons
    n_steps = len(rows) + 2
    st = [("Handlers live, rings empty", "The kernel's transcript says `input: ready`. The script `tests/native/input.inject` waits for that line before it sends anything, so no key can arrive before there is a handler to take it.")]
    for i, row in enumerate(rows):
        if row["kind"] == "key":
            hexs = " ".join("%02X" % b for b in row["stream"])
            evs = "; ".join(l[5:] for l in row["lines"])
            st.append(("Inject `%s`" % row["cmd"], "QEMU's PS/2 keyboard sends the scancodes; the controller translates them to set 1 and the IRQ 1 handler pushes each byte into the key ring: `%s`. The kernel's loop then drains the ring through the decoder, which prints (from the recorded transcript): %s." % (hexs, evs)))
        else:
            st.append(("Inject `%s`" % row["cmd"], "QEMU's mouse sends a four-byte packet `%s`. The IRQ 12 handler assembles it and pushes one `MouseEvent` into the mouse ring; the loop drains it and prints the recorded line `%s`." % (" ".join("%02X" % b for b in row["stream"]), row["lines"][0])))
    st.append(("Escape released, run over", "The kernel prints `%s`. The same transcript, with its variable lines as ranges, is what `tests/native/expected-input.txt` demands of every CI run." % tail))
    cap = ("Replay of the recorded run. <b>Recorded:</b> the injected commands and every decoded event line, from <code>tests/native/input.inject</code> and <code>expected-input.txt</code> (PR #10). <b>Derived, not recorded:</b> the scancode and packet bytes, which the transcript does not log; they come from the decoder's own tables and the packet format, and the page refuses to build unless a model of the decoder turns those bytes back into exactly the recorded lines. The ring indices assume the loop drains after each command, as the algorithm allows; the run did not log them. %s" % cap_note(D, ["input-expected.txt", "input-inject.txt", "kbd-lib.rs.txt"]))
    return stepper("input-replay", "The injected keyboard and mouse run, replayed with the ring state", stage, st, cap, ctx, "x-wide")


# ---- mode 13h, register by register -----------------------------------------------------------------------------------------------

def _arr(body, key):
    m = re.search(r"\b%s: \[(.*?)\]" % key, body, re.S)
    if not m:
        raise ValueError("MODE_13H has no %s in the captured regs.rs" % key)
    return [int(x, 16) for x in re.findall(r"0x[0-9A-Fa-f]{2}", m.group(1))]


def mode13_writes():
    """The port writes of desk_vga::regs::set_mode_registers for mode 13h, from the table in the captured source, as
    (step, kind, port, index, value, note). The order is the one the function's own comment lists; the markers below check that the
    captured function still has those steps."""
    src = media("vga-regs.rs.txt")
    m = re.search(r"pub const MODE_13H: ModeRegs = ModeRegs \{(.*?)\n\};", src, re.S)
    if not m:
        raise ValueError("the captured regs.rs has no MODE_13H table")
    body = m.group(1)
    misc = int(re.search(r"misc: (0x[0-9A-Fa-f]{2})", body).group(1), 16)
    seq, crtc, gc, attr = _arr(body, "seq"), _arr(body, "crtc"), _arr(body, "gc"), _arr(body, "attr")
    if (len(seq), len(crtc), len(gc), len(attr)) != (5, 25, 9, 21):
        raise ValueError("MODE_13H register counts changed: %r" % ((len(seq), len(crtc), len(gc), len(attr)),))
    fn = re.search(r"pub fn set_mode_registers.*?\n\}\n", src, re.S)
    for marker in ("ports.outb(ATTR_ADDR, 0x00);", "seq_write(ports, seq::RESET, 0x01);", "ports.outb(MISC_OUTPUT, mode.misc);", "seq_write(ports, seq::RESET, mode.seq[0]);",
                   "& 0x7F", "ports.outb(ATTR_ADDR, 0x20);", "ports.outb(crate::dac::PEL_MASK, 0xFF);"):
        if not fn or marker not in fn.group(0):
            raise ValueError("set_mode_registers no longer contains %r; update explainers_hw.mode13_writes" % marker)
    w = []
    w.append((1, "in", 0x3DA, None, None, "reset the attribute flip-flop"))
    w.append((1, "out", 0x3C0, None, 0x00, "video off (palette address source bit clear)"))
    w.append((2, "out", 0x3C4, 0, None, "sequencer index 0: reset"))
    w.append((2, "out", 0x3C5, None, 0x01, "synchronous reset while the clock changes"))
    w.append((2, "out", 0x3C2, None, misc, "miscellaneous output"))
    for i in range(1, 5):
        w.append((2, "out", 0x3C4, i, None, "sequencer index %d" % i))
        w.append((2, "out", 0x3C5, None, seq[i], "sequencer register %d" % i))
    w.append((2, "out", 0x3C4, 0, None, "sequencer index 0"))
    w.append((2, "out", 0x3C5, None, seq[0], "release the reset"))
    w.append((3, "out", 0x3D4, 0x11, None, "CRTC index 0x11"))
    w.append((3, "out", 0x3D5, None, crtc[0x11] & 0x7F, "bit 7 cleared so registers 0 to 7 accept writes"))
    for i, v in enumerate(crtc):
        w.append((4, "out", 0x3D4, i, None, "CRTC index %d" % i))
        w.append((4, "out", 0x3D5, None, v, "CRTC register %d" % i))
    for i, v in enumerate(gc):
        w.append((5, "out", 0x3CE, i, None, "graphics controller index %d" % i))
        w.append((5, "out", 0x3CF, None, v, "graphics controller register %d" % i))
    w.append((6, "in", 0x3DA, None, None, "reset the attribute flip-flop again"))
    for i, v in enumerate(attr):
        w.append((6, "out", 0x3C0, None, i, "attribute index %d" % i))
        w.append((6, "out", 0x3C0, None, v, "attribute register %d" % i))
    w.append((6, "out", 0x3C0, None, 0x20, "video on"))
    w.append((7, "out", 0x3C6, None, 0xFF, "DAC mask: every entry reachable"))
    return w, dict(misc=misc, seq=seq, crtc=crtc, gc=gc, attr=attr)


def vga13(D, ctx):
    w, regs = mode13_writes()
    mode_writes = [x for x in w if x[1] == "out"]
    if len(mode_writes) != 128:
        raise ValueError("mode 13h takes %d port writes here, not the 128 that docs/A-VGA-VBE.md says the host tests assert" % len(mode_writes))
    scene = media("vga-scene.rs.txt")
    video = media("vga-video.rs.txt")
    desk16 = re.findall(r"Rgb6::from_rgb8\(0x([0-9a-f]{2}), 0x([0-9a-f]{2}), 0x([0-9a-f]{2})\)", scene.split("pub const DESK16")[1].split("];")[0])
    if len(desk16) != 16 or "(v as u32 * 63 + 127) / 255" not in video:
        raise ValueError("DESK16 or the 8-bit to 6-bit conversion changed in the captured sources")
    amber = [int(x, 16) for x in desk16[8]]
    six = [(v * 63 + 127) // 255 for v in amber]
    eight = [(v << 2) | (v >> 4) for v in six]
    lines, steps_of, count = [], [], 0
    cum = {}
    for st, kind, port, idx, val, note in w:
        if kind == "in":
            txt = "in   al, %03X          ; %s" % (port, note)
        else:
            count += 1
            if val is None:
                txt = "out  %03X, %02X         ; %s" % (port, idx, note)
            else:
                txt = "out  %03X, %02X         ; %s" % (port, val, note)
        lines.append(txt)
        steps_of.append(st)
        cum[st] = count
    dac = ["out  3C8, %02X         ; DAC write index: entry 8, the Desk's amber" % 8, "out  3C9, %02X         ; red   %d of 63" % (six[0], six[0]),
           "out  3C9, %02X         ; green %d of 63" % (six[1], six[1]), "out  3C9, %02X         ; blue  %d of 63" % (six[2], six[2])]
    lines += dac
    steps_of += [8] * 4
    addr = 0xA0000 + 100 * 320 + 160
    cons = console(lines, steps_of, "The port writes of the mode 13h sequence, in order, shown up to the current step")
    counter = "".join('<span class="px-count" data-at="%d"><b>%d</b> of 128 port writes made</span>' % (k, cum.get(k, 0) if k < 8 else 128) for k in range(0, 10))
    cons = '<div class="px-vga"><div class="px-counter">%s</div>%s</div>' % (counter, cons)
    steps = [
        ("Text mode, video on", "The screen is in the BIOS's 80 by 25 text mode. Nothing is written yet: the kernel has no BIOS to ask, so it will write every register itself."),
        ("Video off", "Reset the attribute controller's flip-flop by reading port `3DA`, then write `00` to `3C0`: the palette-address-source bit is clear, so the screen blanks while the registers change."),
        ("Sequencer", "Hold the sequencer in synchronous reset (`3C4`/`3C5`, register 0 = 1) while the miscellaneous output register `3C2` takes `%02X` (the 25.175 MHz clock, I/O at `3Dx`). Registers 1 to 4 follow: `%s`. Register 4 = `%02X` is chain-4: one byte per pixel at consecutive addresses. Then release the reset with `%02X`." % (regs["misc"], " ".join("%02X" % v for v in regs["seq"][1:]), regs["seq"][4], regs["seq"][0])),
        ("Unlock the CRTC", "CRTC register `11` has a protect bit (bit 7) that makes registers 0 to 7 ignore writes. The table says `%02X`; the driver writes `%02X`, bit 7 cleared, first. The host tests include a model that refuses protected writes, so skipping this step fails a test." % (regs["crtc"][0x11], regs["crtc"][0x11] & 0x7F)),
        ("The 25 CRTC registers", "Registers 0 to 24 on `3D4`/`3D5`: the horizontal and vertical timing. `%s`. Register 9 = `%02X` doubles each scan line, so 400 lines show 200. These are the commonly published values; QEMU does not check timing, so no monitor has ever shown them." % (" ".join("%02X" % v for v in regs["crtc"]), regs["crtc"][9])),
        ("Graphics controller", "Nine registers on `3CE`/`3CF`: `%s`. Register 5 = `%02X` selects the 256-colour shift register; register 6 = `%02X` maps 64 KiB of graphics memory at `A0000h`." % (" ".join("%02X" % v for v in regs["gc"]), regs["gc"][5], regs["gc"][6])),
        ("Attribute controller, then video on", "Reset the flip-flop again, then 21 pairs of an index byte and a data byte, both to port `3C0`: the sixteen palette entries (the identity here), the mode control `%02X`, overscan, plane enable `%02X`, pan and colour select. Writing `20` to `3C0` last turns the picture back on." % (regs["attr"][16], regs["attr"][18])),
        ("The DAC mask", "`FF` to port `3C6` makes all 256 palette entries reachable. That is the 128th and last port write of the mode set: %d `out` instructions in all, plus two reads of `3DA`, and no BIOS call." % len(mode_writes)),
        ("Load a colour", "The palette is not part of the mode set. To put the Desk's amber (`#%s%s%s`, entry 8 of its 16 colours) in the DAC, write the entry number to `3C8` and then red, green and blue through `3C9`. The DAC holds six bits per channel, so each 8-bit value is converted with `(v * 63 + 127) / 255`: %s become %s of 63. The index advances after each triple, so a whole 256-entry palette is one index write and 768 data writes." % (desk16[8][0], desk16[8][1], desk16[8][2], ", ".join("%d" % v for v in amber), ", ".join("%d" % v for v in six))),
        ("Draw a pixel", "With chain-4 a pixel is one byte at consecutive addresses, so pixel (160, 100) of the 320 by 200 screen is the byte at `A0000h + 100 * 320 + 160` = `%X`. Store `08` there and the pixel shows entry 8. The DAC turns that into the %s the monitor receives: %s of 255 after widening 6 bits back to 8." % (addr, "amber", ", ".join(str(v) for v in eight))),
    ]
    cap = ("The 128 writes are generated from the <code>MODE_13H</code> table and the sequence in <code>hal/drivers/vga/src/regs.rs</code> (PRs #11 and #14), and the page refuses to build unless there are exactly 128, the number <code>docs/A-VGA-VBE.md</code> says the host tests assert. This is the documented order, not a port trace of a run. The amber is slot 8 of <code>DESK16</code> in the spike's scene file, read from the capture. %s" % cap_note(D, ["vga-regs.rs.txt", "vga-scene.rs.txt", "vga-video.rs.txt"]))
    return stepper("vga13", "Setting mode 13h by writing the VGA registers", cons, steps, cap, ctx, "x-wide")


# ---- the loader's hand-off ------------------------------------------------------------------------------------------------------------

def bootinfo(D, ctx):
    transcript = media("gfx-vbe-expected.txt").splitlines()
    ld = [l for l in transcript if l.startswith("stage2:")]
    ker = [l for l in transcript if l.startswith("video:")]
    if not ld or not ker:
        raise ValueError("the captured graphics transcript has no stage2: or video: lines")
    info = media("boot-info-block.md")
    vb = media("boot-video-block.md")
    for needle in ("`0x820`", "`0x2000`", "version"):
        if needle not in info:
            raise ValueError("the boot information block table no longer says %s" % needle)
    W, H = 900, 250
    o = ['<svg viewBox="0 0 %d %d" role="img" class="diagram" aria-label="The loader runs in real mode, where the BIOS answers: it copies the BIOS font, asks for VBE information and a mode, sets the mode, and writes a boot information block and a video block to fixed low addresses. It then switches to protected mode, and the kernel, which has no BIOS, reads the blocks.">' % (W, H),
         '<defs><marker id="bi-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" class="d-arrow"/></marker></defs>']

    def box(x, y, w, h, kind, a, b="", c=""):
        dash = ' stroke-dasharray="6 4"' if kind == "plan" else ""
        o.append('<rect x="%d" y="%d" width="%d" height="%d" rx="6" class="a-%s" stroke-width="1.6"%s/>' % (x, y, w, h, kind, dash))
        o.append('<text x="%g" y="%g" text-anchor="middle" font-size="13" class="a-t1 a-t1-%s">%s</text>' % (x + w / 2, y + 22, kind, esc(a)))
        for i, t in enumerate([b, c]):
            if t:
                o.append('<text x="%g" y="%g" text-anchor="middle" font-size="11.5" class="a-t2">%s</text>' % (x + w / 2, y + 40 + i * 15, esc(t)))

    def arrow(x1, y1, x2, y2, label="", above=True):
        o.append('<line x1="%g" y1="%g" x2="%g" y2="%g" class="d-line" stroke-width="1.3" marker-end="url(#bi-arrow)"/>' % (x1, y1, x2, y2))
        if label:
            o.append('<text x="%g" y="%g" text-anchor="middle" font-size="11" class="d-muted">%s</text>' % ((x1 + x2) / 2, y1 - 6 if above else y1 + 16, esc(label)))
    o.append('<rect x="8" y="26" width="530" height="212" rx="8" fill="none" class="d-line" stroke-dasharray="4 4"/><text x="18" y="20" font-size="12" class="d-title">Real mode: the BIOS still answers</text>')
    o.append('<rect x="572" y="26" width="320" height="212" rx="8" fill="none" class="d-line" stroke-dasharray="4 4"/><text x="582" y="20" font-size="12" class="d-title">Protected mode: no BIOS</text>')
    box(24, 44, 150, 72, "ext", "BIOS", "INT 10h AX=1130h", "VBE 4F00h, 4F01h, 4F02h")
    box(24, 150, 150, 72, "built", "Stage 2 (loader)", "boot/stage2.asm", "PR #14")
    arrow(99, 150, 99, 118, "asks", False)
    box(214, 44, 150, 72, "review", "Font copy", "8 by 16, 4096 bytes", "at 0x2000")
    box(214, 150, 150, 72, "review", "Video block", "32 bytes at 0x820", "mode, pitch, base")
    box(384, 150, 140, 72, "review", "Boot info block", "at 0x500, version 1", "map, pointers")
    arrow(174, 80, 214, 80)
    arrow(174, 186, 214, 186)
    arrow(364, 186, 384, 186)
    box(612, 150, 130, 72, "review", "Kernel", "video::init", "checks the base")
    arrow(524, 186, 612, 186, "switch to protected mode", True)
    box(756, 150, 120, 72, "review", "desk-vga", "Linear8 over the", "frame buffer")
    arrow(742, 186, 756, 186)
    box(612, 44, 264, 72, "plan", "Not available here", "no INT 10h, no VBE call, no way back to text", "except by card-specific registers")
    o.append("</svg>")
    fig = "".join(o)
    ld_html = console(ld + [""] + ker, [0] * (len(ld) + 1 + len(ker)), "The loader and kernel lines from the expected graphics transcript", False)
    return ('<figure class="boot-info"><div class="chart scroll" tabindex="0" aria-label="Boot information diagram, scrolls sideways"><div style="min-width:760px">%s</div></div>'
            '<div class="two"><div><h4>What the loader writes at 0x820</h4>%s</div><div><h4>The recorded transcript, loader then kernel</h4>%s</div></div>'
            '<figcaption>The hand-off from the loader to the kernel (PR #14). The two field tables and the transcript are captured from the spike; the diagram is the documented flow, drawn to explain it, not to scale. In the transcript, <code>{N:..}</code>, <code>{HEX}</code> and <code>{ANY}</code> are the expected-file patterns the test accepts for numbers that vary by card. %s</figcaption></figure>') % (
        fig, captable("boot-video-block.md", ctx, "The video block at 0x820"), ld_html, cap_note(D, ["gfx-vbe-expected.txt", "boot-video-block.md", "boot-info-block.md"]))


# ---- the cost of std ------------------------------------------------------------------------------------------------------------------

def std_sizes(D, ctx):
    rows = [r for r in media("std-sizes.md").strip().splitlines() if r.startswith("|")]
    head = [c.strip().replace("`", "") for c in rows[0].strip("|").split("|")]
    data = []
    for r in rows[2:]:
        c = [x.strip() for x in r.strip("|").split("|")]
        data.append((c[0], [int(v.replace(",", "")) for v in c[1:]]))
    if len(head) != 6 or len(data) != 3:
        raise ValueError("the captured size table is no longer 3 rows of 5 series")
    series = head[1:]
    base, desk = {k: v[0] for k, v in data}, {k: v[3] for k, v in data}
    mx = max(max(v) for _, v in data)
    L, R, top = 150, 640, 52
    gh, bh, gap = 5 * 20 + 14, 16, 4
    H = top + len(data) * gh + 14
    o = ['<svg viewBox="0 0 800 %d" role="img" class="diagram sz" aria-label="Grouped bars of the executable size in bytes of a hello program on five platform layers under three sets of compiler flags. With default flags: %s">' % (
        H, "; ".join("%s %s bytes" % (s, "{:,}".format(v)) for s, v in zip(series, data[0][1])))]
    for i, s in enumerate(series):
        x = L + i * 128
        o.append('<rect x="%d" y="14" width="12" height="12" rx="2" class="sz-f sz-%d"/><text x="%d" y="25" font-size="12" class="d-text2">%s</text>' % (x, i + 1, x + 17, esc(s)))
    y = top
    for g, (name, vals) in enumerate(data):
        o.append('<text x="%d" y="%d" text-anchor="end" font-size="13" class="a-t1">%s</text>' % (L - 12, y + 44, esc(name)))
        for i, v in enumerate(vals):
            by = y + i * (bh + gap)
            w = v / mx * R
            o.append('<g><title>%s, %s: %s bytes</title><rect x="%d" y="%d" width="%.1f" height="%d" rx="2" class="sz-f sz-%d"/>'
                     '<text x="%.1f" y="%d" font-size="12" class="num d-text2">%s</text></g>' % (esc(name), esc(series[i]), "{:,}".format(v), L, by, w, bh, i + 1, L + w + 6, by + 12, "{:,}".format(v)))
        y += gh
    o.append('<line x1="%d" y1="%d" x2="%d" y2="%d" class="d-line"/>' % (L, top - 4, L, H - 8))
    o.append("</svg>")
    d_add, i_add = desk["default"] - base["default"], desk["imm"] - base["imm"]
    return ('<figure class="szfig"><div class="chart scroll" tabindex="0" aria-label="Bar chart of executable sizes, scrolls sideways"><div style="min-width:640px">%s</div></div>'
            '<details><summary>The same numbers as a table</summary>%s</details>'
            '<figcaption>Executable size in bytes (the 2 KiB stub included) of one hello program, on five platform layers, from the table in <code>docs/E-STD-STUBS.md</code> (PR #12): %s. A patched <code>std</code> adds %s bytes (+%d%%) to a hello with default flags and %s bytes (+%d%%) with <code>-Cpanic=immediate-abort</code>. <code>imm</code> is that flag; <code>imm+loc</code> adds <code>-Zlocation-detail=none -Zfmt-debug=none</code>. The last series adds a ten-entry <code>HashMap</code>, a sort and <code>{:?}</code> output, so it is not the map alone. Measured in DOSBox builds, not on hardware. %s</figcaption></figure>') % (
        "".join(o), captable("std-sizes.md", ctx, "Executable sizes"), "; ".join(series), "{:,}".format(d_add), round(100 * d_add / base["default"]), "{:,}".format(i_add), round(100 * i_add / base["imm"]), cap_note(D, ["std-sizes.md"]))


# ---- reading a VBE mode's attribute word -----------------------------------------------------------------------------------------------

def vbe_attrs(D, ctx):
    ok = media("jsdos-vbe.txt")
    no = media("jsdos-vbe-nolfb.txt")

    def mode(text, num):
        m = re.search(r"^mode %s: (\S+) model (\d+) attrs (0x[0-9a-f]+) lfb (true|false) pitch (\d+) phys (0x[0-9a-f]+)$" % num, text, re.M)
        if not m:
            raise ValueError("mode %s is not in the captured VBE listing" % num)
        return dict(size=m.group(1), model=int(m.group(2)), attrs=int(m.group(3), 16), lfb=m.group(4) == "true", pitch=int(m.group(5)), phys=m.group(6))
    cases = [("js-dos as it ships: mode 0x101, 640 by 480 at 8 bits", mode(ok, "0x101")), ("the same list: mode 0x102, 800 by 600 in 4 planes", mode(ok, "0x102")),
             ("with the machine option `vesa_nolfb`: mode 0x101 again", mode(no, "0x101"))]
    for _, c in cases:
        if bool(c["attrs"] & 0x80) != c["lfb"]:
            raise ValueError("bit 7 of %#x does not agree with the probe's own lfb flag" % c["attrs"])
    label = {0: "supported", 4: "graphics mode", 7: "linear frame buffer"}
    panels = []
    for i, (title, c) in enumerate(cases):
        cells = "".join('<span class="bitcell%s"><b>%d</b><i>%d</i><em>%s</em></span>' % (" on" if (c["attrs"] >> b) & 1 else "", b, (c["attrs"] >> b) & 1, esc(label.get(b, ""))) for b in range(7, -1, -1))
        panels.append('<div class="px-box" data-at="%d"><div class="px-h">%s</div><p class="px-calc">attributes <code>%s</code> = <code>%s</code>; physical base <code>%s</code>; probe says LFB: <b>%s</b></p><div class="bitrow">%s</div></div>' % (
            i, inline(title, ctx), "0x%04x" % c["attrs"], format(c["attrs"] & 0xFF, "08b"), c["phys"], "yes" if c["lfb"] else "no", cells))
    steps = [
        ("The question", "Mode 0x101 is 640 by 480 at 8 bits. The spec had left open whether a browser's emulator would offer it with a linear frame buffer. The probe asks `4F01h` for the mode and prints the attribute word it gets back; bit 7 of that word is the answer."),
        ("A planar mode, for contrast", "Mode 0x102 (800 by 600 in four planes) has `0x001b`: supported and graphics, bit 7 clear. It has no linear frame buffer, in the same listing, and the probe agrees."),
        ("Take the option away", "With `vesa_nolfb` set, the same mode 0x101 reports `0x001b` and a physical base of `0x00000000`: bit 7 is gone and so is the address. The probe stopped with `no 640x480x8 packed-pixel mode with a linear frame buffer offered`. A program that must have the frame buffer has to look at this bit, never assume it."),
    ]
    # step 0 is the shipped 0x101 panel; step 1 the planar panel; step 2 the no-lfb panel
    stage = "".join(panels)
    cap = ("Three real lines of the probe's output in js-dos 8.5.3 running in headless Chromium (PR #13), decoded bit by bit. Only bits 0, 4 and 7 are named, because those are the ones the native loader's own search reads (<code>docs/D-NATIVE-VGA.md</code>); the other set bits are shown unnamed. %s" % cap_note(D, ["jsdos-vbe.txt", "jsdos-vbe-nolfb.txt"]))
    return stepper("vbe-attrs", "Reading the attribute word of a VBE mode", stage, steps, cap, ctx, "x-wide")


# ---- one event, two worlds -------------------------------------------------------------------------------------------------------------------

def pipelines(D, ctx):
    native = media("input-expected.txt").splitlines()
    dos_keys = media("dos-keys.txt").splitlines()
    dos_mouse = media("dos-mouse.txt").splitlines()
    nkeys = [l for l in native if l.startswith("key: ")]
    nmouse = [l for l in native if l.startswith("mouse: ")]
    if "key: A down mods=- char='a'" not in nkeys or "key A down mods=none" not in dos_keys:
        raise ValueError("the two transcripts no longer both contain the key A press the pipelines explainer follows")
    stages = [
        ("A key goes down", "Scancode `1E`", "Scancode `1E`", "same", "The keyboard sends a make code. Under QEMU the controller has already translated to set 1; under DOS the BIOS set the controller up that way, and DOSBox's model delivers set 1."),
        ("IRQ 1 fires", "`KbdIrq` pushes the raw byte into `SpscRing<u8, 64>`", "the same `KbdIrq`, the same ring type", "same", "The handler reads port `60h` and does nothing else. The kernel registers it itself; the DOS runtime registers it through `rt::irq`, which locks the whole image first because a page fault in a handler under DPMI is fatal."),
        ("The end-of-interrupt", "the kernel's dispatcher sends it", "`rt::irq`'s dispatcher sends it", "differs", "In both the handler never sends one: the platform does, after the handler returns. What differs is who the platform is, and that the BIOS handler is replaced, not chained, on DOS by default."),
        ("The core drains the ring", "`desk_kbd::Decoder`, scancode set 1", "the same decoder, started with the BIOS lock state (`with_locks`)", "same", "The decoder is one crate, shared by both tracks. The DOS runtime seeds it from the BIOS lock flags so the first Caps Lock press toggles from what the LED shows."),
        ("A `KeyEvent` comes out", "`key: A down mods=- char='a'`", "`key A down mods=none`", "same", "The same `KeyEvent { code, pressed, modifiers }` type on both tracks. The two transcript lines are formatted by different printing code, so the text differs; the events do not."),
        ("A mouse moves", "IRQ 12: a four-byte PS/2 packet, one `MouseEvent` per packet", "INT 33h function 0Bh: the mickeys since the last poll, one event per poll", "differs", "This is where the tracks really differ. DOS programs do not talk to the PS/2 auxiliary port; they ask the mouse driver, so the hosted track polls once per core iteration and sums the motion between polls."),
        ("A `MouseEvent` comes out", "`mouse: dx=10 dy=-5 buttons=0 wheel=0`", "`mouse dx=40 dy=0 buttons=- wheel=0`", "same", "The same `MouseEvent { dx, dy, buttons, wheel }`. Right and down are positive on both. Buttons are the state now, not changes. A click that begins and ends between two polls is invisible to the hosted adapter, which the spike found and wrote down."),
    ]
    rows = []
    for i, (title, a, b, kind, _) in enumerate(stages):
        rows.append('<div class="pl2-row %s" data-from="%d"><div class="pl2-h">%s</div><div class="pl2-c pl2-n">%s</div><div class="pl2-c pl2-d">%s</div></div>' % (kind, i, inline(title, ctx), inline(a, ctx), inline(b, ctx)))
    head = '<div class="pl2-row pl2-head"><div></div><div class="pl2-c">Native kernel (PR #10, QEMU)</div><div class="pl2-c">Hosted DOS runtime (PR #15, DOSBox)</div></div>'
    nline = "".join('<span class="ln">%s</span>' % esc(l) for l in nkeys[:2] + nmouse[:2])
    dline = "".join('<span class="ln">%s</span>' % esc(l) for l in dos_keys[:2] + [m for m in dos_mouse if m.startswith("mouse")][:2])
    trans = ('<div class="pl2-tr"><div><div class="px-h">Recorded, native</div><pre class="term"><code>%s</code></pre></div><div><div class="px-h">Recorded, hosted</div><pre class="term"><code>%s</code></pre></div></div>' % (nline, dline))
    stage = '<div class="pl2">%s%s%s</div>' % (head, "".join(rows), trans)
    steps = [(t, txt) for t, _, _, _, txt in stages]
    cap = ("The two input pipelines, one row at a time. Blue rows are code both tracks share; amber rows differ. The transcript lines at the bottom are the first lines of two recorded runs: the native kernel under QEMU (<code>expected-input.txt</code>) and <code>INPUT.EXE</code> under DOSBox Staging (<code>keys.txt</code> and the Xvfb mouse run in <code>docs/B-DOS-INPUT.md</code>). The pipeline stages are the documented flow, not a trace. %s" % cap_note(D, ["input-expected.txt", "dos-keys.txt", "dos-mouse.txt"]))
    return stepper("pipelines", "One key and one mouse move through the two input pipelines", stage, steps, cap, ctx, "x-wide")


# ---- the real screenshots -------------------------------------------------------------------------------------------------------------------

IMAGES = {
    "img-d-vbe-window.png": ("The first window on the native kernel: a dark window with an orange title bar reading Desk: first window, the line Move the mouse. Type here. Esc leaves., the typed text desk after a prompt with an orange caret, an arrow cursor inside the window near its lower right, over a purple to orange horizon at the top and black below.",
                             "The first window, drawn by the native kernel into the VBE linear frame buffer at 640 by 480 in 256 colours: an orange title bar, the key echo <code>desk</code>, and the arrow cursor after the script moved the mouse. QEMU 11.1.2, <code>-vga std</code>, a <code>screendump</code> through the monitor."),
    "img-d-mode12h-window.png": ("The same window in 16 colours: the title bar is orange, the body dark, and the horizon behind it has three bands of brown and orange instead of seven.",
                                 "The same window in mode 12h, 16 colours, drawn through the planar driver (<code>Planar16</code>), set by registers with no BIOS call."),
    "img-d-vbe-picture.png": ("The test picture in the VBE frame buffer: seven stepped bands of purple through red to orange, a rainbow ramp of the palette, and a box of fine lines and a grid with six accent swatches.",
                              "The spike's test picture in the VBE linear frame buffer. Its read-back checksum, <code>0x5bc65936</code>, equals the one spike A recorded from DOSBox."),
    "img-d-vbe-fade.png": ("The same test picture at about half intensity: every band, the ramp and the six swatches are darker versions of the colours before.",
                           "The fade at its half-way step, taken with the screenshot hook while the kernel paced a 16-step fade through the DAC. The frame buffer is untouched: only the palette changes."),
    "img-d-mode13h.png": ("The test picture in mode 13h on the native kernel, shown doubled to 640 by 400: seven bands of purple to orange, a rainbow ramp, and the box of lines with six swatches.",
                          "Mode 13h set from protected mode by registers alone, after leaving the VBE mode (QEMU shows the 320 by 200 picture as 640 by 400). Its read-back checksum equals spike A's DOSBox run."),
    "img-d-mode12h.png": ("The test picture in mode 12h: bands of brown and orange at the top, a palette ramp, and the box with the six swatches.",
                          "The same scene in mode 12h by registers: sixteen colours, so the horizon has three bands. Checksum <code>0xfaf8b758</code>, equal to spike A's DOSBox run."),
    "img-a-mode13h.png": ("The test picture in mode 13h shown at double size: seven bands of purple to orange, a rainbow ramp and the box of lines with six swatches.",
                          "The same scene in mode 13h, 320 by 200 shown doubled, as read back from the frame buffer by the DOS program under DOSBox Staging (spike A). Not a DOSBox rendering: the program's own read-back, converted to PNG."),
    "img-d-text-restored.png": ("A black 720 by 400 text screen with two white lines: gfx: text mode restored: registers, palette and the 8x16 font reloaded, and gfx: ready text.",
                                "Text mode after the kernel left graphics without the BIOS: the registers, the sixteen text colours and the 8 by 16 font were reloaded. On QEMU's standard VGA this works."),
    "img-d-cirrus.png": ("A 720 by 400 screen full of scattered coloured fragments of characters on black: unreadable, garbled text.",
                         "The same step on <code>-vga cirrus</code> after a VBE mode: not restored. The register programming that works on the standard card does not undo that card's extended registers."),
    "img-b-screen.png": ("A dark 640 by 480 screen with three small rectangles at the top left, the first orange, and three tiny squares below them, with an arrow cursor near the middle.",
                         "The screen of the DOS input program after the mouse run: the cursor at (375, 290), the left-button square lit, and a key square for each of the keys typed. Read back from the frame buffer by the program, not a DOSBox rendering."),
    "img-s5-vbe.png": ("A 640 by 480 test pattern: a smooth gradient of blue, cyan, green, magenta, yellow and red columns in the top half and a fine woven diagonal pattern in the bottom half.",
                       "The 640 by 480 by 8 pattern written through the linear frame buffer in js-dos, as the browser page's canvas returned it. The probe compares every pixel with a pattern computed in the page: 0 of 307,200 differ."),
}
def png_size(name):
    import struct
    with open(os.path.join(HERE, "media", name), "rb") as f:
        head = f.read(24)
    return struct.unpack(">II", head[16:24])


def shots(D, names):
    """Real screenshots as a row of figures, each with an anchor of its own (shot-NAME) so the gallery can link to it."""
    figs = []
    for n in names:
        if n not in IMAGES:
            raise ValueError("unknown screenshot %s" % n)
        alt, cap = IMAGES[n]
        w, h = png_size(n)
        figs.append('<figure class="shot" id="shot-%s"><div class="screen"><img src="img/%s" alt="%s" width="%d" height="%d" loading="lazy"></div><figcaption>%s</figcaption></figure>' % (
            n[:-4], n, esc(alt), w, h, cap))
    return '<div class="shotrow">%s</div><p class="how">%s</p>' % ("".join(figs), cap_note(D, list(names)))
