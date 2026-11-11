# The tables of appendix B, "Maps and numbers": read from the files that hold the numbers, not typed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Each table is a list of rows; each row carries the proof of its numbers, a (file, text) pair that must occur in the file, and `check`
# refuses a row whose proof is missing, so a number that moves in the code and not in a table cannot go unnoticed. Where a script can read
# the number (an `equ`, a `const`, an enum, the header comment of a runner) it does; only the descriptions are words.
import glob
import importlib.util
import os
import re

import gather

HERE = os.path.dirname(os.path.abspath(__file__))


def src(root, path):
    with open(os.path.join(root, path), encoding="utf-8") as f:
        return f.read()


def hx(v, w=0):
    return "0x%0*X" % (w, v)


def equ(text, name):
    m = re.search(r"^%s\s+equ\s+(0x[0-9A-Fa-f]+)" % re.escape(name), text, flags=re.M)
    if not m:
        raise ValueError("no `%s equ` line" % name)
    return int(m.group(1), 16), m.group(0)


def rs_const(text, name):
    m = re.search(r"\bconst %s: \w+ = (0x[0-9A-Fa-f_]+|\d+)\b" % re.escape(name), text)
    if not m:
        raise ValueError("no const %s" % name)
    return int(m.group(1).replace("_", ""), 0), m.group(0)


def ports_module():
    spec = importlib.util.spec_from_file_location("m_ports_for_maps", os.path.join(HERE, "manual", "explainers", "ports.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def tables(root=None):
    root = root or gather.ROOT
    s1, s2 = src(root, "boot/stage1.asm"), src(root, "boot/stage2.asm")
    bios_rs, idt = src(root, "kernel/src/bios.rs"), src(root, "kernel/src/idt.rs")
    out = []

    # ---- physical memory
    rows = []
    for name, what in (("BOOTINFO", "Boot information block: the 32-byte header"), ("MAP", "The memory map, 24 bytes an entry, up to 32 entries"), ("VIDEO", "The video block, 32 bytes"),
                       ("FONT_BUF", "The BIOS 8x16 font, 4,096 bytes"), ("VBE_INFO", "VBE controller information, while stage 2 runs"), ("VBE_MODE", "One VBE mode information block, while stage 2 runs"),
                       ("STACK32", "Top of stage 2's protected-mode stack")):
        v, line = equ(s2, name)
        rows.append([hx(v), what, "boot/stage2.asm: " + name, ("boot/stage2.asm", line)])
    rows.append([hx(0x7C00), "Stage 1, the boot sector, where the BIOS puts it", "boot/stage1.asm: org", ("boot/stage1.asm", "org 0x7C00")])
    rows.append([hx(0x7E00), "Stage 2, straight after the boot sector (17 sectors)", "boot/stage2.asm: org", ("boot/stage2.asm", "org 0x7E00")])
    v, line = equ(s2, "KERNEL_LOW")
    rows.append([hx(v), "The kernel's staging copy while booting; after boot, the BIOS trampoline", "boot/stage2.asm: KERNEL_LOW; kernel/src/bios.rs: LOW_BASE", ("boot/stage2.asm", line)])
    v, line = rs_const(bios_rs, "BOUNCE_BASE")
    rows.append([hx(v), "The bounce buffer for BIOS calls (8,192 bytes)", "kernel/src/bios.rs: BOUNCE_BASE", ("kernel/src/bios.rs", line)])
    v, line = rs_const(bios_rs, "STACK_BASE")
    rows.append([hx(v), "The real-mode stack for BIOS calls", "kernel/src/bios.rs: STACK_BASE", ("kernel/src/bios.rs", line)])
    v, line = equ(s2, "KERNEL_ENTRY")
    rows.append([hx(v), "The kernel, linked and started here", "boot/stage2.asm: KERNEL_ENTRY; kernel/link.ld", ("boot/stage2.asm", line)])
    out.append({"id": "memory", "title": "Physical memory while booting and after", "source": "boot/stage2.asm, boot/stage1.asm, kernel/src/bios.rs",
                "head": ["Address", "What is there", "Defined at"], "rows": rows})

    # ---- ports
    pm = ports_module()
    rows = []
    for dev, what, path, name, want in pm.PORTS:
        v, line = rs_const(src(root, path), name)
        rows.append([hx(v), dev + ": " + what, path + ": " + name, (path, line)])
    out.append({"id": "ports", "title": "I/O ports the code names", "source": "the drivers and the kernel", "head": ["Port", "What it is", "Defined at"], "rows": sorted(rows, key=lambda r: int(r[0], 16))})

    # ---- IRQ lines and vectors
    pic = src(root, "hal/drivers/pic/src/lib.rs")
    bases = pm.read_bases(pic)
    irq = src(root, "hal/hal/src/irq.rs")
    found = pm.read_lines(irq)
    rows = []
    for line, name, dev in pm.LINES:
        proof = re.search(r"pub const %s: Irq = Irq\(%d\);" % (name, line), irq)
        if not proof:
            raise ValueError("irq.rs has no %s = %d" % (name, line))
        rows.append([str(line), name + ": " + dev, hx(pm.vector(line, bases["REMAPPED_BASES"]), 2), hx(pm.vector(line, bases["BIOS_BASES"]), 2), ("hal/hal/src/irq.rs", proof.group(0))])
    out.append({"id": "irq", "title": "Interrupt lines and their vectors", "source": "hal/hal/src/irq.rs, hal/drivers/pic/src/lib.rs",
                "head": ["IRQ", "Line", "Vector under the kernel's remap", "Vector under the BIOS's default"], "rows": rows})

    # ---- exception vectors
    names = re.search(r"const NAMES: \[&str; 32\] = \[(.*?)\];", idt, flags=re.S)
    mask = re.search(r"\(\(1 << \\n\) & (0x[0-9A-Fa-f]+)\) == 0", idt)
    if not (names and mask):
        raise ValueError("idt.rs no longer has NAMES and the error-code mask")
    nm = re.findall(r'"([^"]*)"', names.group(1))
    m = int(mask.group(1), 16)
    rows = [[hx(v, 2), nm[v], "yes" if (1 << v) & m else "no", ("kernel/src/idt.rs", '"%s"' % nm[v])] for v in range(32) if nm[v] != "reserved"]
    out.append({"id": "exceptions", "title": "CPU exception vectors the kernel names", "source": "kernel/src/idt.rs", "head": ["Vector", "Name", "The CPU pushes an error code"], "rows": rows})

    # ---- BIOS calls
    calls = [("INT 13h AH=02h", "Read sectors by CHS", "boot/stage1.asm", "stage 1", "int 0x13", "mov ah, 0x02"), ("INT 13h AH=00h", "Reset the disk system after an error", "boot/stage1.asm", "stage 1", "int 0x13", "xor ax, ax"),
             ("INT 10h AH=0Eh", "Teletype output to the screen", "boot/stage1.asm", "stage 1 and stage 2", "int 0x10", "mov ah, 0x0E"),
             ("INT 15h AX=E820h", "The memory map, one entry per call", "boot/stage2.asm", "stage 2", "int 0x15", "mov eax, 0xE820"), ("INT 15h AX=E801h", "Memory between 1 and 16 MiB, and above", "boot/stage2.asm", "stage 2", "int 0x15", "mov ax, 0xE801"),
             ("INT 15h AH=88h", "Extended memory in KiB", "boot/stage2.asm", "stage 2", "int 0x15", "mov ah, 0x88"), ("INT 12h", "Conventional memory in KiB", "boot/stage2.asm", "stage 2", "int 0x12", "int 0x12"),
             ("INT 10h AX=1130h", "The address of the 8x16 font", "boot/stage2.asm", "stage 2", "int 0x10", "mov ax, 0x1130"),
             ("INT 10h AX=4F00h / 4F01h / 4F02h", "VBE controller information, mode information, set mode", "boot/stage2.asm", "stage 2", "int 0x10", "mov ax, 0x4F02")]
    rows = [[c[0], c[1], c[3], c[2], (c[2], c[4]), (c[2], c[5])] for c in calls]
    rows += [["INT 10h, 13h, 15h, 1Ah by a real-mode trampoline", "The kernel's bios::call, for VBE, disk, memory and time", "the kernel", "kernel/src/bios.rs", ("kernel/src/bios.rs", "pub fn call"), ("kernel/bios16.asm", "int")]]
    out.append({"id": "bios", "title": "The BIOS calls made, and by whom", "source": "boot/stage1.asm, boot/stage2.asm, kernel/src/bios.rs", "head": ["Call", "What for", "Made by", "In"], "rows": rows})

    # ---- exit statuses
    run = src(root, "tools/native-run.sh")
    header = re.findall(r"^#\s{2,}(\d+)\s{2,}(.+)$", run, flags=re.M)
    exit_rs = src(root, "kernel/src/exit.rs")
    ok, okl = rs_const(exit_rs, "SUCCESS")
    bad, badl = rs_const(exit_rs, "FAILURE")
    rows = [[str(n), what.strip(), "tools/native-run.sh", ("tools/native-run.sh", "#   %s " % n)] for n, what in header]
    rows.append([str((ok << 1) | 1), "What QEMU exits with when the guest writes %s to the debug-exit port: success" % hx(ok), "kernel/src/exit.rs: SUCCESS", ("kernel/src/exit.rs", okl)])
    rows.append([str((bad << 1) | 1), "What QEMU exits with when the guest writes %s: failure" % hx(bad), "kernel/src/exit.rs: FAILURE", ("kernel/src/exit.rs", badl)])
    dos = src(root, "tools/dosrun.sh")
    for n in re.findall(r"(\d+) when", dos)[:2]:
        what = "the command outlived DOS_TIMEOUT" if n == "124" else "DOSBox itself failed"
        rows.append([n, what, "tools/dosrun.sh", ("tools/dosrun.sh", "%s when" % n)])
    out.append({"id": "exit", "title": "Exit statuses of the runners", "source": "tools/native-run.sh, kernel/src/exit.rs, tools/dosrun.sh", "head": ["Status", "Meaning", "Read from"], "rows": rows})

    # ---- test modes
    main, image = src(root, "kernel/src/main.rs"), src(root, "tools/native-image.sh")
    enum = re.findall(r"^\s+(\w+) = (\d+),$", main[main.index("enum TestMode"):main.index("impl TestMode")], flags=re.M)
    codes = {int(n): m for m, n in re.findall(r"([a-z0-9-]+)\) code=(\d+)", image)}
    expected = sorted(os.path.basename(p) for p in glob.glob(os.path.join(root, "tests/native/expected-*.txt")))
    rows = []
    for variant, n in enum:
        name = codes[int(n)]
        longer = [m for m in codes.values() if len(m) > len(name) and m.startswith(name + "-")]
        files = [f for f in expected if (f[len("expected-"):-4] == name or f[len("expected-"):-4].startswith(name + "-"))
                 and not any(f[len("expected-"):-4] == m or f[len("expected-"):-4].startswith(m + "-") for m in longer)]
        rows.append([n, name, variant, ", ".join(files) if files else ("none" if name != "none" else "expected.txt"), ("kernel/src/main.rs", "%s = %s," % (variant, n))])
    out.append({"id": "modes", "title": "Test modes of the native kernel", "source": "kernel/src/main.rs, tools/native-image.sh, tests/native/", "head": ["No.", "Name", "Variant", "Expected transcript"], "rows": rows})
    return out


def check(tbls, root=None):
    """Every row's proof must occur in its file."""
    root = root or gather.ROOT
    problems = []
    for t in tbls:
        for r in t["rows"]:
            for proof in [x for x in r if isinstance(x, tuple)]:
                path, needle = proof
                text = src(root, path)
                if needle not in text:
                    problems.append("%s: %r does not occur in %s" % (t["id"], needle, path))
    return problems


def html(tbls, esc, link):
    """The appendix's body: one captioned table each, with the file the numbers were read from."""
    out = []
    for t in tbls:
        head = "".join('<th scope="col">%s</th>' % esc(h) for h in t["head"])
        body = "".join("<tr>%s</tr>" % "".join(("<td><code>%s</code></td>" if i == 0 and t["id"] != "modes" else "<td>%s</td>") % esc(c) for i, c in enumerate(r) if not isinstance(c, tuple)) for r in t["rows"])
        out.append('<section aria-labelledby="map-%s"><h2 id="map-%s">%s</h2><div class="tbl"><table><caption class="sr">%s</caption><thead><tr>%s</tr></thead><tbody>%s</tbody></table></div>'
                   '<p class="how">Read by the generator from %s. A test checks that every number in the table occurs in the file it names.</p></section>' % (t["id"], t["id"], esc(t["title"]), esc(t["title"]), head, body, esc(t["source"])))
    return "".join(out)
