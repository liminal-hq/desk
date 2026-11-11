# Frames and the heap: the six entries of the recorded memory map replayed through the kernel's frame bitmap rules, then the heap carved out.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Driven by `media/native-serial.txt` (the recorded QEMU run: the map, the usable-RAM line, the kernel range, the free-frame count and the
# heap address) and the rules of `kernel/src/memory.rs::init` and `heap.rs`, which the replica follows pass by pass. It refuses a transcript
# whose usable-RAM line is not the sum of its usable entries, one whose free-frame count the replica does not reproduce, one whose heap does
# not start where first fit puts it, and a memory.rs whose constants or passes no longer read the way the replica assumes.
import os
import re

import gather
from explainers import stepper
from mdlite import Raw, esc

ID = "m-frames"
FRAME = 4096


def src(path):
    with open(os.path.join(gather.ROOT, path), encoding="utf-8") as f:
        return f.read()


def transcript(text=None):
    text = text if text is not None else src("tools/progress-report/media/native-serial.txt")
    entries = [(int(m.group(1), 16), int(m.group(2), 16), m.group(3).strip()) for m in re.finditer(r"^  map (0x[0-9a-f]+)-(0x[0-9a-f]+) (.+)$", text, flags=re.M)]
    usable = re.search(r"^memory: usable RAM (\d+) KiB$", text, flags=re.M)
    kern = re.search(r"^memory: kernel image (0x[0-9a-f]+)-(0x[0-9a-f]+)$", text, flags=re.M)
    free = re.search(r"^memory: (\d+) frames of 4 KiB free after reservations$", text, flags=re.M)
    heap = re.search(r"^heap: (\d+) KiB at (0x[0-9a-f]+)$", text, flags=re.M)
    if not (entries and usable and kern and free and heap):
        raise ValueError("the recorded transcript no longer has the map, the usable-RAM line, the kernel range, the frame count and the heap line")
    return {"entries": entries, "usable_kib": int(usable.group(1)), "kernel": (int(kern.group(1), 16), int(kern.group(2), 16)), "free": int(free.group(1)),
            "heap_kib": int(heap.group(1)), "heap_at": int(heap.group(2), 16)}


def rules(text=None):
    """The constants and passes of memory.rs that the replica assumes; refuses a file that no longer reads that way."""
    text = text if text is not None else src("kernel/src/memory.rs")
    h = src("kernel/src/heap.rs")
    for needle, why in (("pub const FRAME_SIZE: u32 = 4096;", "the frame size"), ("const MAX_BYTES: u64 = 256 * 1024 * 1024;", "the 256 MiB bitmap limit"),
                        ("let first = (r.base + fs - 1) / fs;", "usable regions rounded inward at the start"), ("let last = (r.base + r.len) / fs;", "usable regions rounded inward at the end"),
                        ("let first = r.base / fs;", "other regions rounded outward at the start"), ("let last = (r.base + r.len + fs - 1) / fs;", "other regions rounded outward at the end"),
                        ("for f in 0..(0x100000 / FRAME_SIZE) as usize {", "the first megabyte reserved"), ("end.div_ceil(FRAME_SIZE)", "the kernel range rounded outward")):
        if needle not in text:
            raise ValueError("kernel/src/memory.rs no longer has %s (%r), so the replica no longer follows it" % (why, needle))
    m = re.search(r"const HEAP_FRAMES: usize = (\d+);", h)
    if not m:
        raise ValueError("kernel/src/heap.rs no longer has HEAP_FRAMES")
    return {"heap_frames": int(m.group(1)), "max_frames": (256 * 1024 * 1024) // FRAME}


def replay(t, r):
    limit = r["max_frames"]
    free = set()
    log = []
    freed = 0
    for base, end, kind in t["entries"]:
        if kind == "usable":
            first, last = -(-base // FRAME), end // FRAME
            for f in range(first, min(last, limit)):
                free.add(f)
            log.append(("usable", base, end, first, last))
    after1 = len(free)
    reserved_other = []
    for base, end, kind in t["entries"]:
        if kind != "usable" and end != base:
            first, last = base // FRAME, -(-end // FRAME)
            before = len(free)
            for f in range(first, min(last, limit)):
                free.discard(f)
            reserved_other.append((kind, base, end, first, last, before - len(free)))
    after2 = len(free)
    before = len(free)
    for f in range(0x100000 // FRAME):
        free.discard(f)
    first_mib = before - len(free)
    after3 = len(free)
    ks, ke = t["kernel"]
    before = len(free)
    for f in range(ks // FRAME, -(-ke // FRAME)):
        free.discard(f)
    kernel = before - len(free)
    heap_first = min(f for f in free if all((f + i) in free for i in range(r["heap_frames"])))
    return {"after1": after1, "after2": after2, "first_mib": first_mib, "after3": after3, "kernel": kernel, "final": len(free), "heap_first": heap_first, "usable_log": log,
            "reserved_other": reserved_other, "after_heap": len(free) - r["heap_frames"]}


def check(t, r, out):
    problems = []
    total = sum((e - b) // 1024 for b, e, k in t["entries"] if k == "usable")
    if total != t["usable_kib"]:
        problems.append("the usable-RAM line says %d KiB but the usable entries sum to %d KiB" % (t["usable_kib"], total))
    if out["final"] != t["free"]:
        problems.append("the transcript says %d frames are free and the replica of memory::init leaves %d" % (t["free"], out["final"]))
    if out["heap_first"] * FRAME != t["heap_at"]:
        problems.append("the transcript puts the heap at %#x but first fit puts it at %#x" % (t["heap_at"], out["heap_first"] * FRAME))
    if t["heap_kib"] * 1024 != r["heap_frames"] * FRAME:
        problems.append("the heap is %d KiB in the transcript and %d frames in heap.rs" % (t["heap_kib"], r["heap_frames"]))
    return problems


def render(D, ctx):
    t, r = transcript(), rules()
    out = replay(t, r)
    problems = check(t, r, out)
    if problems:
        raise ValueError("the frames explainer refuses its sources: " + "; ".join(problems))
    top = max(e for b, e, k in t["entries"] if e < 0x2000000)
    SPAN, X0, W = 0x1000000, 40, 680

    def x(a):
        return X0 + a / SPAN * W
    o = ['<svg viewBox="0 0 760 150" class="diagram m-framesfig" role="img" aria-label="The first 16 MiB of physical memory as the BIOS map and then as the kernel\'s frame bitmap leaves it: usable frames, reserved frames, the kernel and the heap">']
    o.append('<text x="0" y="16" font-size="12" class="d-muted">0</text><text x="%d" y="16" text-anchor="end" font-size="12" class="d-muted">16 MiB</text>' % (X0 + W + 20))
    # row 1: the map as the BIOS gave it, always shown
    o.append('<text x="0" y="46" font-size="12" class="d-muted">Map</text>')
    for b, e, k in t["entries"]:
        if b < SPAN:
            o.append('<rect x="%g" y="30" width="%g" height="22" class="%s"/>' % (x(b), max(1.2, x(min(e, SPAN)) - x(b)), "fr-use" if k == "usable" else "fr-res"))
    # row 2: the bitmap, built by steps
    o.append('<text x="0" y="96" font-size="12" class="d-muted">Frames</text>')
    for b, e, k in t["entries"]:
        if k == "usable" and b < SPAN:
            first, last = -(-b // FRAME), e // FRAME
            o.append('<rect x="%g" y="80" width="%g" height="22" class="fr-free" data-from="1"/>' % (x(first * FRAME), x(last * FRAME) - x(first * FRAME)))
    for kind, b, e, first, last, n in out["reserved_other"]:
        if b < SPAN:
            o.append('<rect x="%g" y="80" width="%g" height="22" class="fr-res" data-from="2"/>' % (x(first * FRAME), max(1.5, x(min(last * FRAME, SPAN)) - x(first * FRAME))))
    o.append('<rect x="%g" y="80" width="%g" height="22" class="fr-res" data-from="3"/>' % (x(0), x(0x100000) - x(0)))
    ks, ke = t["kernel"]
    o.append('<rect x="%g" y="80" width="%g" height="22" class="fr-kern" data-from="4"/>' % (x(ks), x(-(-ke // FRAME) * FRAME) - x(ks)))
    o.append('<rect x="%g" y="80" width="%g" height="22" class="fr-heap" data-from="5"/>' % (x(t["heap_at"]), x(t["heap_at"] + r["heap_frames"] * FRAME) - x(t["heap_at"])))
    for step, label, a in ((3, "first MiB", 0x80000), (4, "kernel", ks), (5, "heap", t["heap_at"] + 0x100000)):
        o.append('<text x="%g" y="124" text-anchor="middle" font-size="11" class="d-muted" data-at="%d">%s</text>' % (x(a), step, esc(label)))
    o.append("</svg>")
    key = ('<ul class="m-key"><li><span class="sw fr-use"></span> usable in the map</li><li><span class="sw fr-res"></span> reserved</li><li><span class="sw fr-free"></span> a free frame</li>'
           '<li><span class="sw fr-kern"></span> the kernel</li><li><span class="sw fr-heap"></span> the heap</li></ul>')
    ents = "; ".join("%#010x to %#010x %s" % e for e in t["entries"])
    kib = [(b, e) for b, e, k in t["entries"] if k == "usable"]
    steps = [
        ("The map the BIOS gave", Raw(esc("The loader stored %d entries: %s. The kernel never sees the raw BIOS calls, only this list." % (len(t["entries"]), ents)))),
        ("Pass 1: usable regions, rounded inward", Raw(esc("Each usable entry frees whole 4 KiB frames, rounding its start up and its end down: %s. That frees %s frames in all (%s)." % (
            "; ".join("%#x to %#x is frames %d to %d (%d frames)" % (b, e, f, l - 1, l - f) for _, b, e, f, l in out["usable_log"]), "{:,}".format(out["after1"]), " + ".join(str(l - f) for _, b, e, f, l in out["usable_log"]))))),
        ("Pass 2: every other region, rounded outward", Raw(esc("Each other entry reserves whole frames, rounding its start down and its end up, so an overlap with usable memory resolves to reserved. Here these entries free nothing that pass 1 had not already left reserved (frames freed again: %d), because the usable entries stop at 0x9fc00 and 0xfe0000. The frame count is still %s. The 4,096 frames of the entry above 4 GiB are past the 256 MiB the bitmap covers, so they are ignored." % (
            sum(n for *_, n in out["reserved_other"]), "{:,}".format(out["after2"]))))),
        ("The first megabyte stays reserved", Raw(esc("Frames 0 to 255 are marked used whatever the map says: interrupt vectors, the BIOS data area, the loader, video memory and option ROMs live there. Pass 1 had freed %d of them (the 159 frames below 0x9fc00), so the free count falls from %s to %s." % (
            out["first_mib"], "{:,}".format(out["after2"]), "{:,}".format(out["after3"]))))),
        ("The kernel image", Raw(esc("The kernel's range %#x to %#x (its code, data, .bss and 64 KiB stack) is %d frames, rounded outward. The free count is now %s, which is the number the kernel prints: %s frames of 4 KiB free after reservations." % (
            ks, ke, out["kernel"], "{:,}".format(out["final"]), "{:,}".format(t["free"]))))),
        ("The heap", Raw(esc("heap::init asks for %d contiguous frames (%d KiB). First fit takes the lowest run that is long enough, which starts at frame %d, address %#x: just after the kernel, exactly where the transcript says (heap: %d KiB at %#x). %s frames remain for everything else." % (
            r["heap_frames"], t["heap_kib"], out["heap_first"], out["heap_first"] * FRAME, t["heap_kib"], t["heap_at"], "{:,}".format(out["after_heap"]))))),
    ]
    cap = ('Replayed from the recorded run in <code>media/native-serial.txt</code> by a replica of <code>memory::init</code> and the first-fit rule of <code>heap::init</code>. The build refuses a transcript whose usable-RAM line is not the sum of its entries, '
           'whose free-frame count the replica does not reproduce or whose heap is not where first fit puts it. The picture shows the first 16 MiB; the map\'s last entry is above 4 GiB.')
    return stepper(ID, "From the memory map to the frame bitmap and the heap", "".join(o) + key, steps, cap, ctx, "x-wide")
