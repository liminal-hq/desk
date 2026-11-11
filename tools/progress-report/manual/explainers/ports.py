# The I/O ports and interrupt lines the code names: a table and a drawing of the two 8259s, read from the drivers' own constants.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Nothing in the table is typed here except which constant to read and the value the chapter's prose relies on. The explainer reads each
# `const NAME: u16 = VALUE;` from the source file that defines it and refuses a constant that is missing or whose value differs from the
# one expected, and it reads the IRQ lines from `hal/hal/src/irq.rs` (refusing one that is missing, numbered differently or past 15) and
# the vector bases from `hal/drivers/pic/src/lib.rs`, refusing a base that differs from the one the chapter states or is not a multiple
# of eight (a controller can only be based on one).
import os
import re

import gather
from mdlite import Raw, esc

ID = "m-ports"

# (device, what the port is, source file, constant, expected value)
PORTS = [
    ("PIT", "channel 0 counter", "hal/drivers/pit/src/lib.rs", "CHANNEL0", 0x40),
    ("PIT", "command (mode) register", "hal/drivers/pit/src/lib.rs", "COMMAND", 0x43),
    ("8042", "data port", "hal/drivers/i8042/src/lib.rs", "DATA_PORT", 0x60),
    ("8042", "status (read) and command (write)", "hal/drivers/i8042/src/lib.rs", "STATUS_PORT", 0x64),
    ("POST", "the I/O delay port", "hal/hal/src/port.rs", "POST_PORT", 0x80),
    ("PIC", "master command", "hal/drivers/pic/src/lib.rs", "MASTER", 0x20),
    ("PIC", "slave command", "hal/drivers/pic/src/lib.rs", "SLAVE", 0xA0),
    ("UART", "COM1 base", "hal/drivers/serial/src/lib.rs", "COM1", 0x3F8),
    ("UART", "COM2 base", "hal/drivers/serial/src/lib.rs", "COM2", 0x2F8),
    ("CMOS", "index port (bit 7 masks NMI)", "kernel/src/bios.rs", "CMOS_INDEX", 0x70),
    ("VGA", "attribute controller address", "hal/drivers/vga/src/regs.rs", "ATTR_ADDR", 0x3C0),
    ("VGA", "miscellaneous output (write)", "hal/drivers/vga/src/regs.rs", "MISC_OUTPUT", 0x3C2),
    ("VGA", "sequencer address", "hal/drivers/vga/src/regs.rs", "SEQ_ADDR", 0x3C4),
    ("VGA", "sequencer data", "hal/drivers/vga/src/regs.rs", "SEQ_DATA", 0x3C5),
    ("VGA", "DAC pixel mask", "hal/drivers/vga/src/dac.rs", "PEL_MASK", 0x3C6),
    ("VGA", "DAC read index", "hal/drivers/vga/src/dac.rs", "DAC_READ_INDEX", 0x3C7),
    ("VGA", "DAC write index", "hal/drivers/vga/src/dac.rs", "DAC_WRITE_INDEX", 0x3C8),
    ("VGA", "DAC data", "hal/drivers/vga/src/dac.rs", "DAC_DATA", 0x3C9),
    ("VGA", "graphics controller address", "hal/drivers/vga/src/regs.rs", "GC_ADDR", 0x3CE),
    ("VGA", "graphics controller data", "hal/drivers/vga/src/regs.rs", "GC_DATA", 0x3CF),
    ("VGA", "CRT controller address", "hal/drivers/vga/src/regs.rs", "CRTC_ADDR", 0x3D4),
    ("VGA", "CRT controller data", "hal/drivers/vga/src/regs.rs", "CRTC_DATA", 0x3D5),
    ("VGA", "input status 1 (bit 3 is vertical retrace)", "hal/drivers/vga/src/regs.rs", "INPUT_STATUS_1", 0x3DA),
    ("Bochs VBE", "index (QEMU's standard card)", "kernel/src/gfx.rs", "DISPI_INDEX", 0x1CE),
    ("Bochs VBE", "data (QEMU's standard card)", "kernel/src/gfx.rs", "DISPI_DATA", 0x1CF),
    ("QEMU", "isa-debug-exit", "kernel/src/exit.rs", "DEBUG_EXIT", 0xF4),
]

# The vector bases the chapter's prose and worked examples state: (BIOS_BASES, REMAPPED_BASES).
BASES = {"BIOS_BASES": (0x08, 0x70), "REMAPPED_BASES": (0x20, 0x28)}

# The IRQ lines the HAL names, with what sits on each on a PC (the second column is the constant's name in `irq.rs`).
LINES = [(0, "TIMER", "PIT channel 0"), (1, "KEYBOARD", "8042, keyboard"), (2, "CASCADE", "the slave 8259"), (3, "COM2", "second serial port"),
         (4, "COM1", "first serial port"), (6, "FLOPPY", "floppy controller"), (7, "LPT1_OR_SPURIOUS", "printer or spurious"),
         (8, "RTC", "real-time clock"), (12, "MOUSE", "8042, mouse"), (13, "FPU", "x87 error"), (14, "IDE_PRIMARY", "primary IDE"),
         (15, "IDE_SECONDARY_OR_SPURIOUS", "IDE 2 or spurious")]


def src(path):
    with open(os.path.join(gather.ROOT, path), encoding="utf-8") as f:
        return f.read()


def read_const(text, name):
    m = re.search(r"\bconst %s: u16 = (0x[0-9A-Fa-f_]+|\d+);" % re.escape(name), text)
    return int(m.group(1).replace("_", ""), 0) if m else None


def read_lines(text):
    return {m.group(1): int(m.group(2)) for m in re.finditer(r"pub const (\w+): Irq = Irq\((\d+)\);", text)}


def read_bases(text):
    out = {}
    for name in ("BIOS_BASES", "REMAPPED_BASES"):
        m = re.search(r"pub const %s: \(u8, u8\) = \((0x[0-9A-Fa-f]+), (0x[0-9A-Fa-f]+)\);" % name, text)
        out[name] = (int(m.group(1), 16), int(m.group(2), 16)) if m else None
    return out


def vector(line, bases):
    return bases[0] + line if line < 8 else bases[1] + line - 8


def check(ports=PORTS, lines=LINES, files=None, irq_text=None, pic_text=None):
    """The problems with the table: a constant the source no longer has or defines differently, a line the HAL does not name or
    numbers differently, a vector base that is not a multiple of eight."""
    files = files or {}
    problems = []
    for dev, what, path, name, want in ports:
        text = files.get(path) if path in files else src(path)
        got = read_const(text, name)
        if got is None:
            problems.append("%s has no u16 constant %s" % (path, name))
        elif got != want:
            problems.append("%s defines %s as %#x, but the chapter says %#x" % (path, name, got, want))
    found = read_lines(irq_text if irq_text is not None else src("hal/hal/src/irq.rs"))
    for line, name, _ in lines:
        if not 0 <= line <= 15:
            problems.append("IRQ %d (%s) is out of range: the PC has lines 0 to 15" % (line, name))
        elif name not in found:
            problems.append("hal/hal/src/irq.rs has no Irq constant %s" % name)
        elif found[name] != line:
            problems.append("hal/hal/src/irq.rs numbers %s as %d, but the chapter says %d" % (name, found[name], line))
    bases = read_bases(pic_text if pic_text is not None else src("hal/drivers/pic/src/lib.rs"))
    for n, b in bases.items():
        if b is None:
            problems.append("hal/drivers/pic/src/lib.rs has no %s" % n)
        elif b[0] % 8 or b[1] % 8:
            problems.append("%s %r is not a pair of multiples of eight" % (n, b))
        elif n == "REMAPPED_BASES" and b[1] != b[0] + 8:
            problems.append("%s %r puts the slave's vectors somewhere other than just after the master's eight" % (n, b))
        elif b != BASES[n]:
            problems.append("hal/drivers/pic/src/lib.rs defines %s as %r, but the chapter says %r" % (n, b, BASES[n]))
    return problems


def hx(v):
    return "0x%X" % v if v > 0xFF else "0x%02X" % v


def render(D, ctx):
    problems = check()
    if problems:
        raise ValueError("the ports explainer refuses its sources: " + "; ".join(problems))
    bases = read_bases(src("hal/drivers/pic/src/lib.rs"))
    bios, kern = bases["BIOS_BASES"], bases["REMAPPED_BASES"]
    # the drawing: two controllers, eight lines each, the slave hanging off IRQ 2
    names = {l: (n, d) for l, n, d in LINES}
    parts = ['<svg class="diagram" viewBox="0 0 700 250" role="img" aria-label="The two 8259 interrupt controllers: the master takes IRQ 0 to 7 and the slave takes IRQ 8 to 15 through the master\'s IRQ 2">']
    for ci, (x0, title, first) in enumerate(((20, "Master 8259, base %s" % hx(kern[0]), 0), (370, "Slave 8259, base %s" % hx(kern[1]), 8))):
        parts.append('<rect x="%d" y="30" width="310" height="200" rx="6" style="fill:var(--panel);stroke:var(--line-strong)"/>' % x0)
        parts.append('<text x="%d" y="22" class="d-text2" font-size="13">%s</text>' % (x0, esc(title)))
        for i in range(8):
            line = first + i
            y = 50 + i * 22
            n = names.get(line)
            label = "IRQ %d  %s" % (line, esc(n[1]) if n else "")
            parts.append('<text x="%d" y="%d" font-size="12" class="%s">%s</text>' % (x0 + 10, y + 4, "d-text2" if n else "d-muted", label))
            parts.append('<text x="%d" y="%d" font-size="12" text-anchor="end" class="d-muted">vector %s</text>' % (x0 + 300, y + 4, hx(vector(line, kern))))
    parts.append('<path d="M 330 92 L 370 92" class="d-line" stroke-width="2"/><text x="350" y="84" text-anchor="middle" font-size="11" class="d-muted">cascade</text>')
    parts.append("</svg>")
    drawing = "".join(parts)
    rows = "".join("<tr><td>%s</td><td style=\"white-space:nowrap\"><code>%s</code></td><td>%s</td><td><code>%s</code></td></tr>" % (esc(d), hx(v), esc(w), esc(p) + "::" + esc(n)) for d, w, p, n, v in PORTS)
    lrows = "".join("<tr><td>%d</td><td>%s</td><td><code>%s</code></td><td><code>%s</code></td><td><code>Irq::%s</code></td></tr>" % (
        l, esc(d), hx(vector(l, kern)), hx(vector(l, bios)), esc(n)) for l, n, d in LINES)
    t1 = ('<div class="tbl"><table><caption class="sr">I/O ports the code names</caption><thead><tr><th scope="col">Device</th><th scope="col">Port</th><th scope="col">What it is</th>'
          '<th scope="col">Defined in</th></tr></thead><tbody>%s</tbody></table></div>' % rows)
    t2 = ('<div class="tbl"><table><caption class="sr">IRQ lines the HAL names</caption><thead><tr><th scope="col">IRQ</th><th scope="col">Device</th><th scope="col">Kernel vector</th>'
          '<th scope="col">BIOS vector</th><th scope="col">HAL name</th></tr></thead><tbody>%s</tbody></table></div>' % lrows)
    cap = ('Read from the drivers: each port is a constant in the file named, the lines are the <code>Irq</code> constants of <code>hal/hal/src/irq.rs</code>, and the vectors are the line plus the bases '
           '<code>REMAPPED_BASES</code> (%s and %s) and <code>BIOS_BASES</code> (%s and %s) in <code>hal/drivers/pic/src/lib.rs</code>. The build refuses a constant that is missing or has another value. '
           'The assignment of devices to lines is the PC convention, which the HAL names; the table does not claim that a given machine wires them so.' % (hx(kern[0]), hx(kern[1]), hx(bios[0]), hx(bios[1])))
    return Raw('<figure class="m-ports"><div class="chart scroll" tabindex="0" aria-label="The two interrupt controllers, scrolls sideways"><div style="min-width:700px">%s</div></div>%s%s<figcaption>%s</figcaption></figure>' % (drawing, t1, t2, cap))
