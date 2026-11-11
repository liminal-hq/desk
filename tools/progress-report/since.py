# The figures of the chapters on live input, the first hands-on runs, the shell design, the CI image and the two runtimes: a timeline of
# pull requests and a strip of CI runs (both read from the gathered gh data), the roadmap's counts (read from docs/ROADMAP.md), the live
# scenario's marks (read from tests/shell/live.sums and checked against docs/LIVE-INPUT.md), two diagrams whose every box names a path that
# must exist, and screenshots copied from the repository's own image folders and checked byte for byte against their sources.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
import datetime
import os
import re

import gather
from mdlite import esc

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = gather.ROOT


def _read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as f:
        return f.read()


def _fig(svg, caption, cls=""):
    return '<figure class="chartfig %s"><div class="chart scroll" tabindex="0" aria-label="Chart, scrolls sideways when narrow"><div style="min-width:640px">%s</div></div><figcaption>%s</figcaption></figure>' % (cls, svg, caption)


def _when(s):
    return datetime.datetime.fromisoformat(s.replace("Z", "+00:00"))


# ---- the timeline of pull requests -----------------------------------------------------------------------------------------------------

def pr_timeline(D, first=19):
    """One bar per pull request from `first` on, from the day it was opened to the day it merged (UTC, as gh reports it)."""
    prs = [p for p in D["timeline"]["prs"] if p["number"] >= first and p.get("mergedAt")]
    if not prs:
        return '<figure class="chartfig"><p class="how">No pull request dates were available (no gh data and no snapshot).</p></figure>'
    t0 = datetime.datetime(2026, 10, 22, tzinfo=datetime.timezone.utc)
    t1 = max(_when(p["mergedAt"]) for p in prs) + datetime.timedelta(hours=6)
    span = (t1 - t0).total_seconds()
    L, R, rh = 250, 520, 13
    H = 52 + len(prs) * rh + 20
    x = lambda t: L + (t - t0).total_seconds() / span * R
    o = ['<svg viewBox="0 0 800 %d" role="img" class="diagram" aria-label="Bars from the day each of %d pull requests was opened to the day it was merged, from #%d to #%d. The stacks of 23 October waited until 11 November.">' % (H, len(prs), prs[0]["number"], prs[-1]["number"])]
    day = t0
    while day < t1:
        label = day.strftime("%b ") + str(day.day)
        if day.day in (22, 29, 1, 5, 8, 11) or day == t0:
            o.append('<line x1="%.1f" y1="34" x2="%.1f" y2="%d" stroke="currentColor" stroke-opacity=".15"/><text x="%.1f" y="28" text-anchor="middle" font-size="11" class="d-text2">%s</text>' % (x(day), x(day), H - 14, x(day), label))
        day += datetime.timedelta(days=1)
    for i, p in enumerate(prs):
        y = 42 + i * rh
        a, b = x(_when(p["createdAt"])), x(_when(p["mergedAt"]))
        cls = "pg-done-emu" if p["base"] == "main" else "pg-review"
        title = p["title"] if len(p["title"]) <= 36 else p["title"][:35] + "…"
        o.append('<g><title>#%d %s: opened %s, merged %s (base %s)</title><text x="%d" y="%d" text-anchor="end" font-size="10.5" class="d-text2">#%d %s</text><rect x="%.1f" y="%d" width="%.1f" height="9" rx="2" class="pg %s"/></g>' % (
            p["number"], esc(p["title"]), p["createdAt"][:16].replace("T", " "), p["mergedAt"][:16].replace("T", " "), esc(p["base"] or "?"), L - 6, y + 8, p["number"], esc(title), a, y, max(b - a, 2.5), cls))
    o.append('<rect x="%d" y="%d" width="10" height="10" rx="2" class="pg pg-done-emu"/><text x="%d" y="%d" font-size="11" class="d-text2">into main</text><rect x="%d" y="%d" width="10" height="10" rx="2" class="pg pg-review"/><text x="%d" y="%d" font-size="11" class="d-text2">into another pull request\'s branch (a stack)</text></svg>' % (
        L, 4, L + 15, 13, L + 90, 4, L + 105, 13))
    rows = "".join("<tr><td>#%d</td><td>%s</td><td>%s</td><td>%s</td><td>%s</td></tr>" % (p["number"], esc(p["title"]), p["createdAt"][:16].replace("T", " "), p["mergedAt"][:16].replace("T", " "), esc(p["base"] or "")) for p in prs)
    tbl = '<details class="how"><summary>The same dates as a table</summary><div class="tbl"><table><thead><tr><th scope="col">PR</th><th scope="col">Title</th><th scope="col">Opened (UTC)</th><th scope="col">Merged (UTC)</th><th scope="col">Base branch</th></tr></thead><tbody>%s</tbody></table></div></details>' % rows
    cap = "Every pull request from #%d on, from the time it was opened to the time it was merged, in UTC, from <code>gh pr list --state all</code> (source: %s). The stacks opened on 23 October (the live-input stack of ten, #33 to #54, and the Manual's, #38 to #63) are the long bars on the left of the gap: they merged together on 11 November." % (
        prs[0]["number"], D["sources"]["gh"])
    return _fig("".join(o), cap) + tbl


# ---- the strip of CI runs --------------------------------------------------------------------------------------------------------------

def ci_runs(D, day="2026-11-11"):
    runs = [r for r in D["timeline"]["runs"] if (r["createdAt"] or "").startswith(day)]
    if not runs:
        return '<figure class="chartfig"><p class="how">No CI runs of %s were available.</p></figure>' % day
    names = {"success": ("passed", "a-built"), "failure": ("failed", "a-review"), "cancelled": ("cancelled", "a-ext"), "": ("no result yet", "a-plan")}
    rh = 24
    H = 40 + len(runs) * rh
    o = ['<svg viewBox="0 0 800 %d" role="img" class="diagram" aria-label="The %d CI runs on main on %s, in time order, each marked passed, failed, cancelled or without a result.">' % (H, len(runs), day)]
    for i, r in enumerate(runs):
        y = 14 + i * rh
        word, cls = names.get(r["conclusion"], (r["conclusion"], "a-ext"))
        o.append('<g><title>%s %s %s: %s</title><text x="52" y="%d" text-anchor="end" font-size="12" class="num d-text2">%s</text><rect x="62" y="%d" width="86" height="18" rx="3" class="%s"%s/><text x="105" y="%d" text-anchor="middle" font-size="11.5" class="a-t1">%s</text><text x="160" y="%d" font-size="12" class="num d-text2">%s</text><text x="222" y="%d" font-size="12" class="a-t2">%s</text></g>' % (
            r["createdAt"][11:16], r["sha"], esc(r["title"]), word, y + 13, r["createdAt"][11:16], y, cls, ' stroke-dasharray="4 3"' if cls == "a-plan" else "", y + 13, word, y + 13, r["sha"], y + 13, esc(r["title"][:78])))
    o.append('<text x="62" y="%d" font-size="11.5" class="d-muted">Times are UTC (the clock the generator reads from gh). Cancelled runs were replaced by a newer push to the same ref.</text></svg>' % (H - 6))
    cap = "The CI runs on <code>main</code> on %s, from <code>gh run list --branch main</code> (source: %s). A run is only as informative as its conclusion: a cancelled run neither passed nor failed." % (day, D["sources"]["gh"])
    return _fig("".join(o), cap)


# ---- the roadmap's counts ---------------------------------------------------------------------------------------------------------------

STATUS = [("done", "done", "pg-done"), ("in progress", "in progress", "pg-progress"), ("planned", "planned", "pg-planned"), ("idea", "idea", "pg-research"), ("blocked", "blocked", "pg-decision")]


def roadmap_rows():
    """(id, status, group) for every row of docs/ROADMAP.md, the group being the nearest heading above it."""
    out, group = [], ""
    for line in _read("docs/ROADMAP.md").splitlines():
        h = re.match(r"^#{2,3} (.+)$", line)
        if h:
            group = h.group(1).strip()
            continue
        m = re.match(r"^\| (R\d+) \| .*? \| `([a-z ]+)` \|", line)
        if m:
            out.append((m.group(1), m.group(2), group))
    return out


def roadmap_status(D):
    rows = roadmap_rows()
    known = {s for s, _, _ in STATUS}
    bad = [r for r in rows if r[1] not in known]
    if bad:
        raise ValueError("docs/ROADMAP.md has a status the chart does not know: %r" % bad[:3])
    ids = [r[0] for r in rows]
    if len(set(ids)) != len(ids):
        raise ValueError("docs/ROADMAP.md lists an item twice")
    groups = []
    for _, st, g in rows:
        if g not in [x[0] for x in groups]:
            groups.append((g, {}))
        dict(groups)[g][st] = dict(groups)[g].get(st, 0) + 1
    L, R, bh = 300, 380, 20
    mx = max(sum(c.values()) for _, c in groups)
    H = 50 + len(groups) * 28 + 30
    total = {s: sum(c.get(s, 0) for _, c in groups) for s, _, _ in STATUS}
    o = ['<svg viewBox="0 0 800 %d" role="img" class="diagram" aria-label="The %d items of the roadmap by section and status: %s.">' % (H, len(rows), ", ".join("%d %s" % (total[s], s) for s, _, _ in STATUS if total[s]))]
    x = 20
    for s, name, cls in STATUS:
        if total[s]:
            o.append('<rect x="%d" y="10" width="12" height="12" rx="2" class="pg %s"/><text x="%d" y="21" font-size="11.5" class="d-text2">%s (%d)</text>' % (x, cls, x + 17, name, total[s]))
            x += 50 + 6.3 * len(name)
    for i, (g, c) in enumerate(groups):
        y = 40 + i * 28
        o.append('<text x="%d" y="%d" text-anchor="end" font-size="12" class="a-t1">%s</text>' % (L - 10, y + 15, esc(g if len(g) < 44 else g[:43] + "…")))
        cx = L
        for s, name, cls in STATUS:
            n = c.get(s, 0)
            if n:
                w = n / mx * R
                o.append('<g><title>%s: %d %s</title><rect x="%.1f" y="%d" width="%.1f" height="%d" class="pg %s"/>%s</g>' % (esc(g), n, name, cx + 1, y, max(w - 2, 2), bh, cls, ('<text x="%.1f" y="%d" text-anchor="middle" font-size="11.5" class="num pg-t">%d</text>' % (cx + w / 2, y + 15, n)) if w > 16 else ""))
                cx += w
        o.append('<text x="%.1f" y="%d" font-size="12" class="num d-text2">%d</text>' % (cx + 8, y + 15, sum(c.values())))
    o.append("</svg>")
    cap = "The %d items of <code>docs/ROADMAP.md</code> counted by the section they sit in and by their status, read from the file on <code>main</code> when this page was built. Every <code>done</code> item is merged; none of them has run on real hardware. The statuses are the roadmap's, not this archive's: the archive's own tags (merged, in review, in progress or planned) still come from <code>gh</code>." % len(rows)
    return _fig("".join(o), cap)


# ---- the live scenario's marks ----------------------------------------------------------------------------------------------------------

def live_marks(D):
    sums = [l.split() for l in _read("tests/shell/live.sums").splitlines() if l.strip()]
    doc = _read("docs/LIVE-INPUT.md")
    by = {}
    for name, depth, _size, _idx, colour in sums:
        by.setdefault(name, {})[depth] = colour
        if "`%s`" % colour not in doc:
            raise ValueError("tests/shell/live.sums has a digest, %s, that docs/LIVE-INPUT.md does not record" % colour)
    rows = "".join("<tr><td><code>%s</code></td><td class=\"num\"><code>%s</code></td><td class=\"num\"><code>%s</code></td></tr>" % (esc(n), by[n].get("8", ""), by[n].get("4", "")) for n in by)
    return ('<div class="tbl"><table><caption class="sr">The eight marks of the live scenario with their colour digests at depth 8 and depth 4</caption><thead><tr><th scope="col">Frame of <code>live.scn</code></th><th scope="col">Depth 8</th><th scope="col">Depth 4</th></tr></thead><tbody>%s</tbody></table></div>'
            '<p class="how">Read from <code>tests/shell/live.sums</code>, the goldens that the host draws; every digest is checked against the Result table of <code>docs/LIVE-INPUT.md</code> when this page is built. The native runs and the DOS runs each had to reproduce these, and did.</p>') % rows


# ---- diagrams whose boxes name paths ----------------------------------------------------------------------------------------------------

_N = 0


def _check_paths(paths):
    for p in paths:
        if not os.path.exists(os.path.join(ROOT, p)):
            raise ValueError("a diagram names %s, which is not in the tree" % p)


def _chain(rows, aria, cap, width=800):
    """Rows of boxes joined by arrows. A row is (label, [(title, sub, path)])."""
    paths = [b[2] for _, boxes in rows for b in boxes if b[2]]
    _check_paths(paths)
    global _N
    _N += 1
    mid = "ar%d" % _N
    bw, gap, L, rh = 118, 14, 128, 92
    width = max(width, L + max(len(b) for _, b in rows) * (bw + gap))
    H = 20 + len(rows) * rh
    o = ['<svg viewBox="0 0 %d %d" role="img" class="diagram" aria-label="%s"><defs><marker id="%s" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto"><path d="M0 0L8 4L0 8z" fill="currentColor"/></marker></defs>' % (width, H, esc(aria), mid)]
    for r, (label, boxes) in enumerate(rows):
        y = 10 + r * rh
        o.append('<text x="%d" y="%d" text-anchor="end" font-size="13" class="a-t1">%s</text>' % (L - 12, y + 34, esc(label)))
        for i, (title, sub, path) in enumerate(boxes):
            x = L + i * (bw + gap)
            o.append('<g><title>%s</title><rect x="%d" y="%d" width="%d" height="68" rx="5" class="%s"/><text x="%d" y="%d" text-anchor="middle" font-size="12" class="a-t1">%s</text><text x="%d" y="%d" text-anchor="middle" font-size="10.5" class="a-t2">%s</text>%s</g>' % (
                esc(path or title), x, y, bw, "a-built" if path else "a-ext", x + bw / 2, y + 24, esc(title), x + bw / 2, y + 40, esc(sub),
                ('<text x="%d" y="%d" text-anchor="middle" font-size="9" class="d-muted">%s</text>' % (x + bw / 2, y + 58, esc(path if len(path) < 24 else "…" + path[-22:]))) if path else ""))
            if i:
                o.append('<line x1="%d" y1="%d" x2="%d" y2="%d" stroke="currentColor" stroke-width="1.4" marker-end="url(#%s)"/>' % (x - gap + 1, y + 34, x - 1, y + 34, mid))
    o.append("</svg>")
    return _fig("".join(o), cap)


def live_routes(D):
    rows = [("Hosted track", [("Mouse driver", "INT 33h mickeys", "hal/drivers/int33"), ("Keyboard", "IRQ 1, scancodes", "hal/drivers/i8042"), ("SHELL.EXE loop", "drain, route, present", "shell/dos/src/main.rs"), ("Shared live module", "route: keep, mark, end", "shell/shell/src/live.rs"), ("Shell and wm", "handle, then frame", "shell/wm"), ("Presenter", "linear or planar", "shell/present")]),
            ("Native track", [("PS/2 mouse", "3-byte packets", "hal/drivers/i8042"), ("Keyboard", "IRQ 1, scancodes", "hal/drivers/kbd"), ("Kernel shell-live", "drain, route, present", "kernel/src/shell.rs"), ("Shared live module", "route: keep, mark, end", "shell/shell/src/live.rs"), ("Shell and wm", "handle, then frame", "shell/wm"), ("Presenter", "linear or planar", "shell/present")])]
    cap = "The two routes from a pointer or key to the same frame, as <code>docs/LIVE-INPUT.md</code> sets them out; each box names a file or crate that exists in the tree (the build refuses a path that does not). Columns 4 to 6 are the same code on both rows. Everything on both rows ran in an emulator: DOSBox Staging on the hosted route and QEMU on the native one."
    return _chain(rows, "Two rows of boxes. The hosted route runs from the INT 33h mouse driver and the keyboard through the SHELL.EXE loop to the shared live module, the shell and the presenter; the native route runs from the PS/2 mouse and keyboard through the kernel's shell-live loop to the same three shared boxes.", cap)


def transcript_flow(D):
    rows = [("Before the fix", [("println!", "a shell: line", "shell/dos/src/main.rs"), ("DOS console", "BIOS text output", None), ("Frame buffer", "VBE mode is on", None), ("Window", "text drawn over it", None)]),
            ("After the fix", [("Transcript", "16 KiB buffer", "shell/shell/src/transcript.rs"), ("Clean-up list", "runs at every exit", "shell/dos/src/main.rs"), ("Text mode", "video mode restored", None), ("Console", "lines printed, same order", None)])]
    cap = "How the interactive <code>SHELL.EXE</code>'s transcript lines reached the screen before and after <code>#72</code>, from that pull request's description and <code>docs/LIVE-INPUT.md</code> (\"Where the hosted transcript goes\"). Grey boxes are DOS and the hardware, which this repository does not write."
    return _chain(rows, "Two rows of boxes. Before: a println reaches the DOS console, which draws on the frame buffer while the VBE mode is on, so the text lands over the window. After: the line is filed in a 16 KiB buffer and printed from the clean-up list once text mode is back.", cap)


# ---- screenshots copied from the repository ---------------------------------------------------------------------------------------------

REPO_IMAGES = {
    "img-li-native-8-typed.png": ("docs/images/live-input/native-8-typed.png", "The native kernel's live shell at depth 8 after a drag, Alt+Tab and typing: Notes dimmed behind, Welcome to Desk in front with an orange halo and the word live in its field.",
                                  "Native, depth 8, the <code>typed</code> frame of <code>live.scn</code>: a QEMU screendump, not a photograph of a window."),
    "img-li-dos-8-dragged.png": ("docs/images/live-input/dos-8-dragged.png", "SHELL.EXE at depth 8 after a drag with the mouse: Notes in front with the halo, moved to the right, Welcome dimmed behind it.",
                                 "DOS, depth 8, the <code>dragged</code> frame: the read-back of the emulated frame buffer after <code>xdotool</code> moved the pointer through DOSBox and INT 33h."),
    "img-li-native-4-dragged.png": ("docs/images/live-input/native-4-dragged.png", "The same dragged scene at depth 4, in sixteen colours, drawn by the native kernel on QEMU's Cirrus card.",
                                    "Native, depth 4, <code>dragged</code>: the same scene through the planar presenter."),
    "img-li-dos-4-again.png": ("docs/images/live-input/dos-4-again.png", "SHELL.EXE at depth 4 with the word deok in Welcome's field, a caret and the arrow at the centre.",
                               "DOS, depth 4: the keys d, e, s, k, two Backspaces, o and k, typed by <code>AUTOTYPE</code> through IRQ 1."),
    "img-sd-desktop.png": ("design/shell-og/screenshots/v4-01-desktop.png", "The design prototype's desktop at 960 by 720: icons down the left, a Files window with a stepped orange halo over the Audio Player, a bar with Desk, two window buttons, Jax and the clock.",
                           "The owner's design, prototype v4, at 1.5 times its 640 by 480 screen, rendered by a browser with web fonts. A design, not software that runs on Desk."),
    "img-sd-menu.png": ("design/shell-og/screenshots/v4-03-desk-menu.png", "The Desk menu open over the Files window: Files and Terminal, then Accessories, Entertainment, System and DOS programs as submenus, then Hide Jax, Settings, About and Exit to DOS.",
                        "The design's Desk menu. None of it is built; the first window has no bar and no menu (R96 to R101)."),
    "img-sd-halo.png": ("design/shell-og/screenshots/v4-border-halo.png", "The design's stepped halo border: Calculator in front with three orange rings, Notes behind it with a hairline.",
                        "The stepped halo, the default border of the design, and the one border the first window draws (R20)."),
}


def repo_images(D, names):
    from explainers_hw import png_size
    head = D.get("head") or "?"
    figs = []
    for n in names:
        if n not in REPO_IMAGES:
            raise ValueError("unknown repository image %s" % n)
        src, alt, cap = REPO_IMAGES[n]
        with open(os.path.join(HERE, "media", n), "rb") as a, open(os.path.join(ROOT, src), "rb") as b:
            if a.read() != b.read():
                raise ValueError("media/%s differs from %s: copy it again" % (n, src))
        w, h = png_size(n)
        figs.append('<figure class="shot" id="shot-%s"><div class="screen"><img src="img/%s" alt="%s" width="%d" height="%d" loading="lazy"></div><figcaption>%s <span class="how">Copied from <code>%s</code>.</span></figcaption></figure>' % (n[:-4], n, esc(alt), w, h, cap, esc(src)))
    return '<div class="shotrow">%s</div>' % "".join(figs)


# ---- the layout of a go32 executable ----------------------------------------------------------------------------------------------------

def go32_layout(D):
    """The file layout `tools/elf2go32.py` writes, from the facts in its own header comment and the comparison's measurements."""
    packer = _read("tools/elf2go32.py")
    comp = _read("docs/FIZZ2-COMPARISON.md")
    for needle, where, text in (("168 bytes in all", "tools/elf2go32.py", packer), ("exactly three section headers", "tools/elf2go32.py", packer), ("multiple of 512", "tools/elf2go32.py", packer),
                                ("The first 2,048 bytes (the stub) are byte-identical", "docs/FIZZ2-COMPARISON.md", comp), ("271,360-byte `.bss`", "docs/FIZZ2-COMPARISON.md", comp)):
        if needle not in text:
            raise ValueError("%s no longer says %r; the go32 diagram needs updating" % (where, needle))
    parts = [("DJGPP stub", "2,048 bytes", "real-mode code that gets a DPMI\nhost (autoload CWSDPMI.EXE)", "a-ext", 170),
             ("COFF headers", "168 bytes", "20 + 28 + 3 × 40: the stub\nreads these and checks 0x014C", "a-built", 150),
             (".text", "512-byte aligned", "code and read-only data:\nthe first PT_LOAD", "a-built", 170),
             (".data", "512-byte aligned", "the second PT_LOAD's\nfile-backed part", "a-built", 130),
             (".bss", "271,360 bytes in memory", "not in the file: the stub\nzeroes it in dwords", "a-plan", 150)]
    o = ['<svg viewBox="0 0 800 190" role="img" class="diagram" aria-label="A DOS executable built by elf2go32: the 2,048-byte DJGPP stub, then 168 bytes of COFF headers, then the text and data sections, with the bss existing only in memory.">']
    x = 10
    for name, size, text, cls, w in parts:
        o.append('<g><rect x="%d" y="40" width="%d" height="70" rx="5" class="%s"%s/><text x="%.1f" y="66" text-anchor="middle" font-size="13" class="a-t1">%s</text><text x="%.1f" y="86" text-anchor="middle" font-size="11" class="a-t2">%s</text></g>' % (
            x, w - 8, cls, ' stroke-dasharray="4 3"' if cls == "a-plan" else "", x + (w - 8) / 2, esc(name), x + (w - 8) / 2, esc(size)))
        for j, line in enumerate(text.split("\n")):
            o.append('<text x="%.1f" y="%d" text-anchor="middle" font-size="9.5" class="d-muted">%s</text>' % (x + (w - 8) / 2, 128 + j * 12, esc(line)))
        x += w
    o.append('<text x="10" y="24" font-size="12" class="d-text2">File layout of SHELL.EXE, left to right (not to scale). Grey is DJGPP\'s; solid is what the packer writes; dashed exists only in memory.</text></svg>')
    cap = "What <code>tools/elf2go32.py</code> writes, from the requirements recorded in its header comment (found by reading the stub's code, which <code>stubify -g</code> writes out) and the sizes measured in <code>docs/FIZZ2-COMPARISON.md</code>. The build refuses to draw this if either document stops saying what it says here."
    return _fig("".join(o), cap)
