# The GDT, decoded: the descriptors of the loader's table and of the kernel's own, bit by bit.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Driven by the sources: the loader's three descriptors are the `dq` lines between `gdt:` and `gdt_end:` in `boot/stage2.asm`, the kernel's
# are the entries of `static GDT` in `kernel/src/gdt.rs`, and the selectors are those the code loads. The explainer decodes each with a
# replica of the processor's field layout, and refuses a table that does not decode to a null descriptor followed by a flat 4 GiB ring 0
# code segment at selector 0x08 and a flat 4 GiB ring 0 data segment at 0x10, two tables that disagree, and a source it cannot read.
import os
import re

import gather
from explainers import stepper
from mdlite import Raw, esc

ID = "m-gdt"

# (name, first bit, last bit) of each field in the 64-bit descriptor, as the processor defines them
FIELDS = [("base 31:24", 56, 63, "base"), ("flags", 52, 55, "flags"), ("limit 19:16", 48, 51, "limit"), ("access", 40, 47, "access"),
          ("base 23:0", 16, 39, "base"), ("limit 15:0", 0, 15, "limit")]


def src(path):
    with open(os.path.join(gather.ROOT, path), encoding="utf-8") as f:
        return f.read()


def loader_table(text=None):
    text = text if text is not None else src("boot/stage2.asm")
    m = re.search(r"^gdt:\n(.*?)^gdt_end:", text, flags=re.M | re.S)
    if not m:
        raise ValueError("boot/stage2.asm no longer has a gdt: ... gdt_end: table")
    out = []
    for line in m.group(1).splitlines():
        m2 = re.match(r"\s*dq\s+(0x[0-9A-Fa-f_]+|0)\b", line)
        if m2:
            out.append(int(m2.group(1).replace("_", ""), 0))
    return out


def kernel_table(text=None):
    text = text if text is not None else src("kernel/src/gdt.rs")
    m = re.search(r"static GDT: \[u64; (\d+)\] = \[(.*?)\];", text, flags=re.S)
    if not m:
        raise ValueError("kernel/src/gdt.rs no longer has `static GDT: [u64; N] = [...]`")
    vals = [int(v.strip().replace("_", ""), 0) for v in m.group(2).split(",") if v.strip()]
    if len(vals) != int(m.group(1)):
        raise ValueError("kernel/src/gdt.rs declares %s entries and lists %d" % (m.group(1), len(vals)))
    return vals


def decode(d):
    base = ((d >> 16) & 0xFFFFFF) | (((d >> 56) & 0xFF) << 24)
    limit = (d & 0xFFFF) | (((d >> 48) & 0xF) << 16)
    access, flags = (d >> 40) & 0xFF, (d >> 52) & 0xF
    g = bool(flags & 8)
    out = {"base": base, "limit_field": limit, "access": access, "flags": flags, "g": g, "db": bool(flags & 4), "l": bool(flags & 2), "avl": bool(flags & 1),
           "p": bool(access & 0x80), "dpl": (access >> 5) & 3, "s": bool(access & 0x10), "e": bool(access & 8), "dc": bool(access & 4), "rw": bool(access & 2), "a": bool(access & 1)}
    out["limit_bytes"] = ((limit + 1) << 12) if g else (limit + 1)
    return out


def check(loader, kernel):
    problems = []
    for name, t in (("the loader's table (boot/stage2.asm)", loader), ("the kernel's table (kernel/src/gdt.rs)", kernel)):
        if len(t) != 3:
            problems.append("%s has %d entries, not three" % (name, len(t)))
            continue
        if t[0] != 0:
            problems.append("%s: entry 0 is not the null descriptor" % name)
        code, data = decode(t[1]), decode(t[2])
        for label, d, want_exec in (("code at 0x08", code, True), ("data at 0x10", data, False)):
            flat = d["base"] == 0 and d["limit_bytes"] == 1 << 32 and d["p"] and d["dpl"] == 0 and d["s"] and d["e"] == want_exec and d["g"] and d["db"] and not d["l"]
            if not flat:
                problems.append("%s: the %s descriptor is not a flat 4 GiB ring 0 %s segment" % (name, label, "code" if want_exec else "data"))
    if len(loader) == 3 and len(kernel) == 3 and loader != kernel:
        problems.append("the loader's table and the kernel's table are different")
    return problems


def bits(d, lo, hi):
    return "{:0{w}b}".format((d >> lo) & ((1 << (hi - lo + 1)) - 1), w=hi - lo + 1)


def grid(d, at, hi_range=None):
    """The 64 bits as cells, 63 to 0 from the left, grouped by field; `at` maps a step number to the field names it highlights."""
    cw, x0, y0 = 10, 10, 34
    o = ['<svg viewBox="0 0 %d 100" class="diagram m-gdtfig" role="img" aria-label="The 64 bits of the descriptor %s, grouped into base, flags, limit and access fields">' % (x0 * 2 + 64 * cw + 70, "{:#018x}".format(d))]
    for name, lo, hi, kind in FIELDS:
        left = x0 + (63 - hi) * cw + (63 - hi) // 8 * 1.1
        width = (hi - lo + 1) * cw
        o.append('<text x="%g" y="%d" font-size="11" class="d-muted">%s</text>' % (left, y0 - (22 if name == "flags" else 8), esc(name)))
        for b in range(hi, lo - 1, -1):
            x = x0 + (63 - b) * cw + (63 - b) // 8 * 1.1
            o.append('<rect x="%g" y="%d" width="%d" height="22" class="gd-%s"/><text x="%g" y="%d" text-anchor="middle" font-size="11" class="gd-bit">%d</text>' % (x, y0, cw - 1, kind, x + cw / 2 - .5, y0 + 15, (d >> b) & 1))
        for step, names in at.items():
            if name in names:
                o.append('<rect x="%g" y="%d" width="%g" height="28" fill="none" class="gd-cur" data-at="%d" stroke-width="2.2"/>' % (left - 1, y0 - 3, width + 1, step))
    o.append('<text x="%d" y="%d" font-size="11" class="d-muted">bit 63</text><text x="%d" y="%d" text-anchor="end" font-size="11" class="d-muted">bit 0</text>' % (x0, y0 + 40, x0 + 64 * cw + 8, y0 + 40))
    o.append("</svg>")
    return "".join(o)


def render(D, ctx):
    loader, kernel = loader_table(), kernel_table()
    problems = check(loader, kernel)
    if problems:
        raise ValueError("the GDT explainer refuses its sources: " + "; ".join(problems))
    code, data = loader[1], loader[2]
    dc, dd = decode(code), decode(data)
    # one picture per descriptor, each shown for its own steps; the steps are numbered in the order they are listed
    steps = []
    steps.append(("The table", Raw(esc("The loader's table has three eight-byte entries, %s. The processor reads a selector as an index in its upper thirteen bits, so the entries are selectors 0x00, 0x08 and 0x10. The kernel loads a table of its own with the same three values (kernel/src/gdt.rs, which the build compares with this one) because the loader's own copy lives in memory the kernel may overwrite." % ", ".join("{:#018x}".format(v) for v in loader)))))
    steps.append(("Entry 0: the null descriptor", Raw(esc("Entry 0 is eight zero bytes. The processor requires it, and a selector with index 0 (0x00) is the 'null selector': loading it into a data segment register is allowed, using it to access memory raises a general protection fault."))))
    steps.append(("Entry 1, selector 0x08: the bytes", Raw(esc("%s is stored least significant byte first, so in memory it is %s. Read as a 64-bit number the fields are scattered: the base and the limit are each in two pieces." % ("{:#018x}".format(code), " ".join("%02X" % ((code >> (8 * i)) & 0xFF) for i in range(8)))))))
    steps.append(("Limit: bits 0 to 15 and 48 to 51", Raw(esc("Bits 0 to 15 are %s (%#06x) and bits 48 to 51 are %s (%#x), so the 20-bit limit field is %#07x. Its unit is set by the granularity flag below." % (bits(code, 0, 15), code & 0xFFFF, bits(code, 48, 51), (code >> 48) & 0xF, dc["limit_field"])))))
    steps.append(("Base: bits 16 to 39 and 56 to 63", Raw(esc("Bits 16 to 39 are %s and bits 56 to 63 are %s, so the 32-bit base is %#010x: the segment starts at address 0, which is what makes the segment 'flat'." % (bits(code, 16, 39), bits(code, 56, 63), dc["base"])))))
    steps.append(("Access byte: bits 40 to 47", Raw(esc("The access byte is %#04x = %s. Present = %d. Privilege level (ring) = %d. Descriptor type S = %d (a code or data segment, not a system one). Executable = %d, so this is a code segment. Conforming/direction = %d. Readable = %d. Accessed = %d (the processor sets it on first use)." % (
        dc["access"], bits(code, 40, 47), dc["p"], dc["dpl"], dc["s"], dc["e"], dc["dc"], dc["rw"], dc["a"])))))
    steps.append(("Flags: bits 52 to 55", Raw(esc("The flags nibble is %#x = %s. Granularity G = %d: the limit counts 4 KiB units. Default size D/B = %d: 32-bit code. Long mode L = %d (a 64-bit flag; clear). Available AVL = %d (software's)." % (dc["flags"], bits(code, 52, 55), dc["g"], dc["db"], dc["l"], dc["avl"])))))
    steps.append(("Result: a flat 4 GiB code segment", Raw(esc("With G set, the limit is (%#x + 1) x 4,096 = %s bytes, which is 4 GiB: the segment covers every 32-bit address, from base 0, in ring 0, readable and executable. This is the segment that the far jump to 0x08:pm_entry loads into CS." % (dc["limit_field"], "{:,}".format(dc["limit_bytes"]))))))
    steps.append(("Entry 2, selector 0x10: the data segment", Raw(esc("%s differs from the code descriptor in one byte, the access byte: %#04x = %s instead of %#04x. Executable = %d, so it is data; direction = %d (grows up); writable = %d. Base 0 and limit 4 GiB are the same, so with this selector in DS, ES, FS, GS and SS every address is the same number as a physical address." % (
        "{:#018x}".format(data), dd["access"], bits(data, 40, 47), dc["access"], dd["e"], dd["dc"], dd["rw"])))))
    steps.append(("The kernel's own table", Raw(esc("kernel/src/gdt.rs declares %d entries, %s: the same three values. Its init function loads them with lgdt and reloads every segment register, so from then on the loader's copy can be overwritten." % (len(kernel), ", ".join("{:#018x}".format(v) for v in kernel))))))
    # which step highlights which field of the code descriptor
    at = {3: ["limit 15:0", "limit 19:16"], 4: ["base 23:0", "base 31:24"], 5: ["access"], 6: ["flags"], 7: ["flags", "access", "limit 15:0", "limit 19:16", "base 23:0", "base 31:24"]}
    stage = grid(code, at) + ('<ul class="m-key"><li><span class="sw gd-base"></span> base</li><li><span class="sw gd-limit"></span> limit</li><li><span class="sw gd-access"></span> access byte</li><li><span class="sw gd-flags"></span> flags</li></ul>'
                              '<p class="how">The picture shows the code descriptor %s; the data descriptor differs only in its access byte.</p>' % "{:#018x}".format(code))
    cap = ('Decoded from the <code>dq</code> lines of <code>boot/stage2.asm</code> and the <code>GDT</code> array of <code>kernel/src/gdt.rs</code> by a replica of the processor\'s field layout. The build refuses a table that is not a null descriptor, a flat 4 GiB ring 0 code segment at selector 0x08 and a flat 4 GiB ring 0 data segment at 0x10, '
           'and refuses a loader table that differs from the kernel\'s.')
    return stepper(ID, "The descriptors of the loader's GDT, bit by bit", stage, steps, cap, ctx, "x-wide")
