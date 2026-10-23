# Three targets compared: the fields of the three Rust target files side by side, with the differences marked and the reason where a document gives one.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# The table is the files: `kernel/i486-desk-none.json`, `spikes/d0-rust-dos/dos-elf.json` and `core/dos.json`, read as they are. Where a document
# records why a field has its value (`docs/NATIVE-BOOT.md`, `docs/RUST-TARGETS.md`, `docs/D0-RUST-DOS.md`) the reason is shown with its source;
# otherwise the table says no reason is recorded. The only things it refuses are a file that does not parse and one that stops being a
# 32-bit i486 target without a pointer width and CPU of that kind, because every sentence of the chapter about the three starts there.
import json
import os

import gather
from explainers import stepper
from mdlite import Raw, esc

ID = "m-targets"
FILES = [("kernel/i486-desk-none.json", "kernel"), ("spikes/d0-rust-dos/dos-elf.json", "dos-elf"), ("core/dos.json", "dos.json")]
# (group title, [fields], what the group says)
GROUPS = [
    ("What machine and what system", ["llvm-target", "arch", "os", "vendor", "env", "binary-format", "is-like-windows", "cpu", "target-pointer-width"]),
    ("What instructions", ["features", "max-atomic-width"]),
    ("What the code looks like", ["panic-strategy", "relocation-model", "position-independent-executables", "disable-redzone", "frame-pointer", "stack-probes", "default-uwtable"]),
    ("What the linker makes", ["linker", "linker-flavor", "executables", "dynamic-linking"]),
    ("Layout and ABI", ["data-layout", "target-endian", "target-c-int-width"]),
    ("Notes the kernel's file carries", ["metadata"]),
]
# field -> (reason, source)
WHY = {
    "llvm-target": ("The two ELF targets use LLVM's freestanding ELF triple; core/dos.json borrows the MinGW triple because LLVM has no MS-DOS OS and DJGPP emits COFF like MinGW.", "docs/RUST-TARGETS.md, docs/D0-RUST-DOS.md"),
    "os": ("OS none for the freestanding ELF targets; windows for the borrowed MinGW triple.", "docs/RUST-TARGETS.md"),
    "cpu": ("The machine is a 486.", "docs/NATIVE-BOOT.md, docs/RUST-TARGETS.md"),
    "features": ("A 486 has no MMX or SSE; the FPU is the x87, so there is no soft-float. The two older files switch off every later extension by name as well.", "docs/NATIVE-BOOT.md, docs/RUST-TARGETS.md"),
    "max-atomic-width": ("A 486 has no 64-bit compare-and-swap (no cmpxchg8b).", "docs/NATIVE-BOOT.md, docs/RUST-TARGETS.md"),
    "panic-strategy": ("No unwinding machinery; smaller code.", "docs/NATIVE-BOOT.md, docs/RUST-TARGETS.md"),
    "relocation-model": ("Static: the kernel is linked at a fixed address and nothing relocates it; the DOS skeleton has no dynamic loading.", "docs/NATIVE-BOOT.md, docs/RUST-TARGETS.md"),
    "position-independent-executables": ("False: nothing relocates the program.", "docs/NATIVE-BOOT.md"),
    "disable-redzone": ("An interrupt would otherwise overwrite data below the stack pointer.", "docs/NATIVE-BOOT.md"),
    "stack-probes": ("None: frames over 4 KB would need an _alloca helper under the COFF target, which the ELF target does without.", "docs/RUST-TARGETS.md, docs/D0-RUST-DOS.md"),
    "linker": ("rust-lld ships with rustc, so no GCC or binutils is needed.", "docs/NATIVE-BOOT.md"),
    "executables": ("True for a freestanding static executable; false for core/dos.json, which builds a static library for the C layer to link.", "docs/NATIVE-BOOT.md, docs/RUST-TARGETS.md"),
    "dynamic-linking": ("False: a freestanding static executable.", "docs/NATIVE-BOOT.md"),
}


def src(path):
    with open(os.path.join(gather.ROOT, path), encoding="utf-8") as f:
        return f.read()


def load(texts=None):
    out = []
    for path, label in FILES:
        text = (texts or {}).get(path) or src(path)
        try:
            out.append((path, label, json.loads(text)))
        except ValueError as e:
            raise ValueError("%s is not valid JSON: %s" % (path, e))
    return out


def check(files):
    problems = []
    for path, label, d in files:
        if d.get("cpu") != "i486":
            problems.append("%s: cpu is %r, not i486" % (path, d.get("cpu")))
        if d.get("target-pointer-width") != 32:
            problems.append("%s: the pointer width is %r, not 32" % (path, d.get("target-pointer-width")))
    return problems


def fmt(v):
    if v is None:
        return "(absent)"
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (dict, list)):
        import json as j
        return j.dumps(v, sort_keys=True)
    return str(v)


def render(D, ctx):
    files = load()
    problems = check(files)
    if problems:
        raise ValueError("the targets explainer refuses its sources: " + "; ".join(problems))
    allfields = {k for _, _, d in files for k in d}
    placed = {f for _, fields in GROUPS for f in fields}
    stray = sorted(allfields - placed)
    if stray:
        raise ValueError("the targets explainer has no group for the field(s) %s" % ", ".join(stray))
    rows, steps = [], []
    for gi, (title, fields) in enumerate(GROUPS):
        present = [f for f in fields if any(f in d for _, _, d in files)]
        if not present:
            continue
        step = len(steps)
        text_bits = []
        for f in present:
            vals = [fmt(d.get(f)) for _, _, d in files]
            same = len(set(vals)) == 1
            why = WHY.get(f)
            cls = "tg-same" if same else "tg-diff"
            rows.append('<tr class="%s" data-from="%d"%s><th scope="row"><code>%s</code></th>%s<td>%s</td></tr>' % (
                cls, step, "", esc(f), "".join('<td><code>%s</code></td>' % esc(v) for v in vals), esc(("%s (%s)" % why) if why else "no reason is recorded in the documents")))
            text_bits.append("%s: %s%s" % (f, " | ".join(vals) if not same else vals[0] + " in all three", ""))
        steps.append((title, Raw(esc("Fields: " + "; ".join(text_bits) + ". The table shows the three files in the order kernel, dos-elf, dos.json, the differing rows marked, and the reason where a document gives one."))))
    table = ('<div class="tbl"><table class="tg"><caption class="sr">The fields of the three target files, with the reason where a document records one</caption><thead><tr><th scope="col">Field</th>%s<th scope="col">Why</th></tr></thead><tbody>%s</tbody></table></div>'
             '<p class="how">Rows with a coloured edge differ between the files. The labels are the files: %s.</p>') % (
        "".join('<th scope="col"><code>%s</code></th>' % esc(label) for _, label, _ in files), "".join(rows), ", ".join("<code>%s</code> is <code>%s</code>" % (esc(l), esc(p)) for p, l, _ in files))
    cap = ('The fields are those of <code>kernel/i486-desk-none.json</code>, <code>spikes/d0-rust-dos/dos-elf.json</code> and <code>core/dos.json</code>, as they are; the reasons are quoted from <code>docs/NATIVE-BOOT.md</code>, <code>docs/RUST-TARGETS.md</code> and <code>docs/D0-RUST-DOS.md</code>, and a field with no recorded reason says so. '
           'The build refuses a file that does not parse or that stops being a 32-bit i486 target.')
    return stepper(ID, "Three target files, field by field", table, steps, cap, ctx, "x-wide")
