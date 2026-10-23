# The floppy image and its reads: the strip of 2,880 sectors, and stage 1's reads of stage 2 and the kernel, one INT 13h call at a time.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Driven by the sources, not by a transcript: the geometry and layout constants are read from `boot/stage1.asm`, the image's size and
# the offsets it patches from `tools/native-image.sh`, the loader's own copy of the layout from `boot/stage2.asm`, and the kernel's
# length from `snapshots/measured.json`. The reads are a replica of `read_sectors` in stage 1 (a read goes to the end of its track, never
# past a 64 KiB physical boundary, never past what is left). The explainer refuses a layout whose regions overlap, a read that crosses
# a 64 KiB boundary, a replica that does not fit its own arithmetic, and sources that disagree with one another.
import json
import math
import os
import re

import gather
from explainers import stepper
from mdlite import Raw, esc

ID = "m-image"


def src(path):
    with open(os.path.join(gather.ROOT, path), encoding="utf-8") as f:
        return f.read()


def equ(text, name):
    m = re.search(r"^%s\s+equ\s+(0x[0-9A-Fa-f]+|\d+)" % re.escape(name), text, flags=re.M)
    if not m:
        raise ValueError("boot/stage1.asm no longer defines %s" % name)
    return int(m.group(1), 0)


def constants():
    s1, s2, tool = src("boot/stage1.asm"), src("boot/stage2.asm"), src("tools/native-image.sh")
    c = {k: equ(s1, k) for k in ("STAGE2_SEG", "STAGE2_LBA", "STAGE2_SECTORS", "KERNEL_LBA", "KERNEL_SEG", "SECTORS_PER_TRACK", "HEADS", "RETRIES")}
    m = re.search(r"size=\$\(\((\d+) \* (\d+)\)\)", tool)
    if not m:
        raise ValueError("tools/native-image.sh no longer computes the image size as SECTORS * BYTES")
    c["TOTAL_SECTORS"], c["SECTOR_BYTES"] = int(m.group(1)), int(m.group(2))
    c["KERNEL_LIMIT"] = int(re.search(r'"\$ksectors" -gt (\d+)', tool).group(1))
    # the image tool's dd offsets must be the layout stage 1 reads
    seeks = {m.group(1): int(m.group(2)) for m in re.finditer(r'dd if="\$in/(stage1|stage2|kernel)\.bin" of="\$out" bs=512 seek=(\d+)', tool)}
    if seeks != {"stage1": 0, "stage2": c["STAGE2_LBA"], "kernel": c["KERNEL_LBA"]}:
        raise ValueError("tools/native-image.sh writes the stages at %s, but stage 1 reads LBA %d and %d" % (seeks, c["STAGE2_LBA"], c["KERNEL_LBA"]))
    if equ(s2, "KERNEL_LOW") != c["KERNEL_SEG"] * 16:
        raise ValueError("stage 2 expects the kernel at %#x but stage 1 reads it to %#x" % (equ(s2, "KERNEL_LOW"), c["KERNEL_SEG"] * 16))
    if equ(s2, "STAGE1_KSECTORS") != 0x7C03:
        raise ValueError("stage 2 no longer reads the kernel length from stage 1 offset 3")
    if not re.search(r"times 17 \* 512", s2) or c["STAGE2_SECTORS"] != 17:
        raise ValueError("stage 2 is no longer padded to the 17 sectors stage 1 reads")
    c["KERNEL_ENTRY"] = equ(s2, "KERNEL_ENTRY")
    return c


def kernel_sectors(c):
    with open(os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "snapshots", "measured.json"), encoding="utf-8") as f:
        k = json.load(f)["nativeKernelBytes"]
    return k, math.ceil(k / c["SECTOR_BYTES"])


def reads(lba, count, seg, c):
    """stage 1's read_sectors: a list of dicts, one per INT 13h call."""
    out = []
    spt, heads = c["SECTORS_PER_TRACK"], c["HEADS"]
    while count:
        sector = lba % spt + 1
        track = lba // spt
        head, cyl = track % heads, track // heads
        track_left = spt + 1 - sector
        boundary = (0x1000 - (seg & 0xFFF)) >> 5
        n = min(track_left, boundary, count)
        start = seg * 16
        end = start + n * c["SECTOR_BYTES"]
        why = "the end of the track" if n == track_left and n != boundary and n != count else ("the 64 KiB boundary" if n == boundary and n != track_left else ("what is left to read" if n == count and n != track_left else "the end of the track"))
        if n == boundary and n == track_left:
            why = "the end of the track, which is also a 64 KiB boundary"
        out.append({"lba": lba, "cyl": cyl, "head": head, "sector": sector, "n": n, "start": start, "end": end, "why": why, "track": track})
        lba += n
        count -= n
        seg += n * 32
    return out


def check(c, stage2_reads, kernel_reads, ksec):
    problems = []
    if ksec > c["KERNEL_LIMIT"]:
        problems.append("the kernel is %d sectors, more than the %d the image tool allows" % (ksec, c["KERNEL_LIMIT"]))
    spans = [("stage 1", 0, 1), ("stage 2", c["STAGE2_LBA"], c["STAGE2_LBA"] + c["STAGE2_SECTORS"]), ("the kernel", c["KERNEL_LBA"], c["KERNEL_LBA"] + ksec)]
    for (an, a0, a1), (bn, b0, b1) in zip(spans, spans[1:]):
        if a1 > b0:
            problems.append("%s (sectors %d to %d) overlaps %s (from %d)" % (an, a0, a1 - 1, bn, b0))
    if spans[-1][2] > c["TOTAL_SECTORS"]:
        problems.append("the kernel runs past the end of the image")
    for r in stage2_reads + kernel_reads:
        if r["start"] // 0x10000 != (r["end"] - 1) // 0x10000:
            problems.append("the read at LBA %d crosses a 64 KiB boundary (%#x to %#x)" % (r["lba"], r["start"], r["end"]))
        if r["n"] < 1:
            problems.append("the read at LBA %d reads nothing" % r["lba"])
    if sum(r["n"] for r in kernel_reads) != ksec or sum(r["n"] for r in stage2_reads) != c["STAGE2_SECTORS"]:
        problems.append("the replica of read_sectors does not read what it was asked to")
    for a, b in zip(stage2_reads + kernel_reads, (stage2_reads + kernel_reads)[1:]):
        if a["end"] != b["start"] and b is not kernel_reads[0]:
            problems.append("the reads at LBA %d and %d are not contiguous in memory" % (a["lba"], b["lba"]))
    return problems


def hx(v):
    return "%#x" % v


def render(D, ctx):
    c = constants()
    kbytes, ksec = kernel_sectors(c)
    r2 = reads(c["STAGE2_LBA"], c["STAGE2_SECTORS"], c["STAGE2_SEG"], c)
    rk = reads(c["KERNEL_LBA"], ksec, c["KERNEL_SEG"], c)
    problems = check(c, r2, rk, ksec)
    if problems:
        raise ValueError("the floppy image explainer refuses its sources: " + "; ".join(problems))
    spt, heads = c["SECTORS_PER_TRACK"], c["HEADS"]
    tracks = c["TOTAL_SECTORS"] // spt
    allreads = r2 + rk
    first_step = 1  # step 0 is the layout
    # ---- the stage: the disk as 160 tracks and the low memory the reads fill
    W, X0, CW = 760, 60, 4.0
    parts = ['<svg viewBox="0 0 %d 250" class="diagram m-image" role="img" aria-label="The 1.44 MB floppy as 160 tracks, and the low memory that stage 1 reads the loader and kernel into">' % W]
    parts.append('<text x="0" y="14" class="d-muted" font-size="12">Disk</text><text x="%d" y="14" class="d-muted" font-size="11">160 tracks: cylinder 0 head 0, cylinder 0 head 1, cylinder 1 head 0, and so on</text>' % X0)
    for t in range(tracks):
        kind = "boot" if t == 0 else ("kern" if c["KERNEL_LBA"] // spt <= t <= (c["KERNEL_LBA"] + ksec - 1) // spt else "free")
        parts.append('<rect x="%g" y="22" width="%g" height="34" class="mi-%s"/>' % (X0 + t * CW, CW - 0.6, kind))
    for i, r in enumerate(allreads):
        step = first_step + i
        parts.append('<rect x="%g" y="19" width="%g" height="40" class="mi-cur" data-at="%d" fill="none" stroke-width="2"/>' % (X0 + r["track"] * CW - 1, CW + 1.4, step))
    parts.append('<text x="%d" y="74" class="d-muted" font-size="11">track 0 holds stage 1 and stage 2; the kernel starts on track 1 and ends on track %d; the rest is zeros</text>' % (X0, (c["KERNEL_LBA"] + ksec - 1) // spt))
    # memory ribbon from 0 to 0x40000
    MX, MW, TOP = 60, 640, 120
    span = 0x40000

    def mx(a):
        return MX + a / span * MW
    parts.append('<text x="0" y="%d" class="d-muted" font-size="12">Memory</text>' % (TOP + 18))
    parts.append('<rect x="%g" y="%d" width="%g" height="34" class="mi-ram"/>' % (MX, TOP, MW))
    for b in (0x10000, 0x20000, 0x30000):
        parts.append('<line x1="%g" y1="%d" x2="%g" y2="%d" class="mi-bound"/><text x="%g" y="%d" text-anchor="middle" class="d-muted" font-size="11">%s</text>' % (mx(b), TOP - 8, mx(b), TOP + 44, mx(b), TOP + 58, hx(b)))
    parts.append('<text x="%g" y="%d" text-anchor="middle" class="d-muted" font-size="11">0x0</text><text x="%g" y="%d" text-anchor="end" class="d-muted" font-size="11">0x40000</text>' % (mx(0), TOP + 58, mx(span), TOP + 58))
    parts.append('<text x="%g" y="%d" class="d-muted" font-size="11">stage 1 at 0x7C00, stage 2 at 0x7E00</text>' % (mx(0x7C00), TOP - 14))
    for i, r in enumerate(allreads):
        step = first_step + i
        kind = "mi-s2" if i < len(r2) else ("mi-ka" if (i - len(r2)) % 2 == 0 else "mi-kb")
        parts.append('<rect x="%g" y="%d" width="%g" height="34" class="%s" data-from="%d"/>' % (mx(r["start"]), TOP, max(1.5, (r["end"] - r["start"]) / span * MW), kind, step))
        parts.append('<rect x="%g" y="%d" width="%g" height="38" class="mi-cur" data-at="%d" fill="none" stroke-width="2"/>' % (mx(r["start"]) - 1, TOP - 2, max(3, (r["end"] - r["start"]) / span * MW + 2), step))
    last = first_step + len(allreads)
    parts.append('<text x="%d" y="%d" class="d-muted" font-size="12" data-from="%d">After the switch to protected mode, stage 2 copies %d dwords from 0x10000 to %s.</text>' % (X0, TOP + 90, last, ksec * c["SECTOR_BYTES"] // 4, hx(c["KERNEL_ENTRY"])))
    parts.append("</svg>")
    key = ('<ul class="m-key"><li><span class="sw mi-boot"></span> stage 1 and stage 2</li><li><span class="sw mi-kern"></span> the kernel</li><li><span class="sw mi-free"></span> zeros</li>'
           '<li><span class="sw mi-ka"></span><span class="sw mi-kb"></span> the kernel as read (alternate reads)</li></ul>')
    stage = "".join(parts) + key
    # ---- the steps
    steps = [("The image and its regions", Raw(esc("The image is %s sectors of %d bytes, %s bytes, which is %d tracks of %d sectors on %d heads (80 cylinders). Sector 0 is stage 1; sectors %d to %d are stage 2; the kernel starts at sector %d, the first sector of track 1, and its %s bytes fill %d sectors. Stage 1 knows the geometry (%d sectors per track, %d heads) and the layout from its own constants; the image tool patched the kernel's length, %d, into stage 1 at offset 3." % (
        "{:,}".format(c["TOTAL_SECTORS"]), c["SECTOR_BYTES"], "{:,}".format(c["TOTAL_SECTORS"] * c["SECTOR_BYTES"]), tracks, spt, heads, c["STAGE2_LBA"], c["STAGE2_LBA"] + c["STAGE2_SECTORS"] - 1, c["KERNEL_LBA"], "{:,}".format(kbytes), ksec, spt, heads, ksec))))]
    for i, r in enumerate(allreads):
        who = "stage 2" if i < len(r2) else "the kernel"
        sect_end = r["sector"] + r["n"] - 1
        text = ("INT 13h AH=02h reads %d sector%s of %s: LBA %d to %d, which is cylinder %d, head %d, sector%s %s. Registers: AL=%d, CH=%d, CL=%d, DH=%d, ES:BX=%04X:0000, so it lands at %s to %s. It stops at %s." % (
            r["n"], "" if r["n"] == 1 else "s", who, r["lba"], r["lba"] + r["n"] - 1, r["cyl"], r["head"], "" if r["n"] == 1 else "s", ("%d" % r["sector"]) if r["n"] == 1 else "%d to %d" % (r["sector"], sect_end),
            r["n"], r["cyl"], r["sector"], r["head"], r["start"] // 16, hx(r["start"]), hx(r["end"] - 1), r["why"]))
        if "64 KiB" in r["why"] and "also" not in r["why"]:
            text += " The floppy's DMA transfer cannot cross a 64 KiB boundary of physical memory, so the rest of the track is read by the next call."
        title = "Read %d: %s, LBA %d%s" % (i + 1, who, r["lba"], " (split)" if "64 KiB" in r["why"] or (i > 0 and allreads[i - 1]["why"].startswith("the 64")) else "")
        steps.append((title, Raw(esc(text))))
    steps.append(("Then stage 2 moves the kernel", Raw(esc("Stage 1 has read %d sectors in %d calls, printing a dot after each (%d dots) and jumping to stage 2 at 0000:7E00. Real mode cannot reach 1 MiB, so the kernel waits at %s until stage 2 is in protected mode; it then copies %d sectors * 512 bytes as %d dwords to %s and jumps there." % (
        len(allreads) and c["STAGE2_SECTORS"] + ksec, len(allreads), len(allreads), hx(c["KERNEL_SEG"] * 16), ksec, ksec * c["SECTOR_BYTES"] // 4, hx(c["KERNEL_ENTRY"]))))))
    cap = ('Replayed from <code>boot/stage1.asm</code> (the geometry, the layout and <code>read_sectors</code>), <code>tools/native-image.sh</code> (the image size and where each stage is written), <code>boot/stage2.asm</code> (where it expects the kernel) and the kernel length in <code>snapshots/measured.json</code>. '
           'This is a replay of the algorithm, not a capture of the floppy controller: the transcript of a boot proves the reads worked, and this figure shows the order and size of each. The build refuses a layout whose regions overlap or a read that crosses a 64 KiB boundary.')
    return stepper(ID, "The floppy image and the reads of the boot sector, one INT 13h call at a time", stage, steps, cap, ctx, "x-wide")
