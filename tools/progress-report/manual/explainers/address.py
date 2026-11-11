# Address translation and the A20 wrap: a real-mode segment and offset made into a linear address, with the gate closed and open, and the
# same address through a protected-mode selector.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Driven by `boot/stage2.asm`: the two addresses the loader's A20 test uses and the segments it points DS and ES at are read from
# `a20_enabled`, the descriptors from the loader's GDT. The explainer refuses a source whose test addresses no longer alias when the gate is
# closed (the test would say "open" for a closed gate), or that alias even with it open (the test would never say "open").
import os
import re

import gather
from explainers import stepper
from mdlite import Raw, esc

import importlib.util

ID = "m-address"
MASK20 = 0xFFFFF


def src(path):
    with open(os.path.join(gather.ROOT, path), encoding="utf-8") as f:
        return f.read()


def test_addresses(text=None):
    """((ds, si), (es, di)) from the loader's `a20_enabled`."""
    text = text if text is not None else src("boot/stage2.asm")
    m = re.search(r"^a20_enabled:\n(.*?)^a20_wait:", text, flags=re.M | re.S)
    if not m:
        raise ValueError("boot/stage2.asm no longer has a20_enabled: ... a20_wait:")
    body = m.group(1)
    if not re.search(r"xor ax, ax\s*\n\s*mov ds, ax\s*\n\s*not ax\s*\n\s*mov es, ax", body):
        raise ValueError("a20_enabled no longer sets DS to 0 and ES to 0xFFFF the way the explainer reads it")
    si = re.search(r"mov si, (0x[0-9A-Fa-f]+)", body)
    di = re.search(r"mov di, (0x[0-9A-Fa-f]+)", body)
    if not si or not di:
        raise ValueError("a20_enabled no longer loads SI and DI with the two test offsets")
    return (0x0000, int(si.group(1), 16)), (0xFFFF, int(di.group(1), 16))


def linear(seg, off):
    return seg * 16 + off


def check(a, b):
    problems = []
    la, lb = linear(*a), linear(*b)
    if la > MASK20:
        problems.append("the first test address %04X:%04X needs more than twenty bits" % (a[0], a[1]))
    if lb <= MASK20:
        problems.append("the second test address %04X:%04X does not reach above 1 MiB, so it cannot tell an open gate from a closed one" % (b[0], b[1]))
    if lb & MASK20 != la:
        problems.append("with the gate closed %04X:%04X (%#x) does not land on %04X:%04X (%#x): the test would report a closed gate as open" % (b[0], b[1], lb & MASK20, a[0], a[1], la))
    if lb == la:
        problems.append("the two test addresses are the same even with the gate open")
    return problems


def gdt_module():
    spec = importlib.util.spec_from_file_location("m_gdt_for_address", os.path.join(os.path.dirname(os.path.abspath(__file__)), "gdt.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def ruler(a, b, steps_at):
    la, lb = linear(*a), linear(*b)
    W, X0, MW = 760, 40, 680
    top = 0x110000

    def x(v):
        return X0 + v / top * MW
    o = ['<svg viewBox="0 0 %d 230" class="diagram m-addrfig" role="img" aria-label="A ruler of physical addresses from 0 to just over 1 MiB, with the loader\'s two A20 test addresses marked and where the second lands with the gate closed and open">' % W]
    o.append('<rect x="%g" y="60" width="%g" height="26" class="ad-ram"/>' % (X0, MW))
    o.append('<line x1="%g" y1="50" x2="%g" y2="96" class="ad-bound"/><text x="%g" y="112" text-anchor="middle" font-size="11" class="d-muted">1 MiB = 0x100000</text>' % (x(0x100000), x(0x100000), x(0x100000)))
    o.append('<text x="%g" y="112" text-anchor="middle" font-size="11" class="d-muted">0x0</text><text x="%g" y="130" text-anchor="end" font-size="11" class="d-muted">0x110000</text>' % (x(0), x(top)))
    o.append('<text x="%d" y="22" font-size="12" class="d-muted">Physical addresses; the pair on the left of each mark is the address the program forms</text>' % X0)
    def mark(v, label, cls, y, at_from, at_to=None, anchor="middle"):
        attrs = ' data-from="%d"' % at_from + (' data-to="%d"' % at_to if at_to is not None else "")
        return '<g%s><line x1="%g" y1="%d" x2="%g" y2="60" class="%s"/><circle cx="%g" cy="73" r="5" class="%s"/><text x="%g" y="%d" text-anchor="%s" font-size="12" class="ad-t">%s</text></g>' % (attrs, x(v), y + 12, x(v), cls, x(v), cls, x(v), y, anchor, esc(label))
    o.append(mark(la, "%04X:%04X = %#x" % (a[0], a[1], la), "ad-a", 44, steps_at["a"], None, "start"))
    o.append(mark(lb, "%04X:%04X = %#x" % (b[0], b[1], lb), "ad-open", 150, steps_at["open"], None, "middle"))
    o.append('<g data-from="%d"><line x1="%g" y1="150" x2="%g" y2="86" class="ad-closed"/><circle cx="%g" cy="73" r="5" class="ad-closed"/><text x="%g" y="172" text-anchor="start" font-size="12" class="ad-t">gate closed: bit 20 is lost, so it is %#x, the same byte</text></g>' % (steps_at["closed"], x(la) + 3, x(la) + 3, x(la) + 3, x(la) + 10, la))
    o.append("</svg>")
    return "".join(o)


def render(D, ctx):
    a, b = test_addresses()
    problems = check(a, b)
    if problems:
        raise ValueError("the address explainer refuses its sources: " + "; ".join(problems))
    g = gdt_module()
    loader = g.loader_table()
    if g.check(loader, loader):
        raise ValueError("the loader's GDT no longer decodes to a flat code and data pair")
    la, lb = linear(*a), linear(*b)
    ex = (0xFFFF, 0xFFFF)
    lx = linear(*ex)
    code, data = g.decode(loader[1]), g.decode(loader[2])
    limit = 8 * len(loader) - 1
    steps = [
        ("A real-mode address is 20 bits", Raw(esc("The processor forms a physical address from a 16-bit segment and a 16-bit offset as segment x 16 + offset, a sum of up to 21 bits that real mode on an 8086 truncated to 20. Every pair names one byte, and many pairs name the same one."))),
        ("The first test address", Raw(esc("The loader's A20 test points DS at segment %#06x and reads offset %#06x: %#06x x 16 + %#06x = %#07x. That is the same with the gate open and closed." % (a[0], a[1], a[0], a[1], la)))),
        ("The second test address", Raw(esc("It points ES at segment %#06x and uses offset %#06x: %#06x x 16 + %#06x = %#x + %#x = %#x. That needs bit 20, one megabyte and %#x bytes up. On a 286 or later processor with the gate open, that is where it lands." % (b[0], b[1], b[0], b[1], b[0] * 16, b[1], lb, lb - 0x100000)))),
        ("With the gate closed the bit is lost", Raw(esc("The motherboard's A20 gate holds address line 20 at zero. %#x with bit 20 forced to zero is %#x, which is the first test address: the two pairs now name one byte. The test writes 0x00 at the first, 0xFF at the second and reads the first: it reads 0xFF." % (lb, lb & MASK20)))),
        ("With the gate open the bytes differ", Raw(esc("With the gate open the second address is really %#x, a different byte. The first still holds the 0x00 the test wrote, so the read-back is 0x00. Read-back 0xFF means closed; 0x00 means open. The loader then tries to open the gate, and tests again." % lb))),
        ("The largest pair", Raw(esc("%04X:%04X is %#x x 16 + %#x = %#x, which is %#x past the top of the first megabyte. With the gate closed it wraps to %#x. This is the wrap that some programs of the 8086 era relied on, and that the gate keeps working." % (ex[0], ex[1], ex[0], ex[1], lx, lx - 0x100000, lx & MASK20)))),
        ("In protected mode, a selector instead", Raw(esc("After the switch an address is a selector and an offset, and the selector picks a descriptor in the GDT. The loader's code selector 0x08 is index 1, whose descriptor has base %#x and a limit of %s bytes, so the linear address is base + offset, which is the offset itself: the second test address, %#x, is reached directly, with no wrap at 20 bits. This is why the loader needs A20 open before it switches." % (
            code["base"], "{:,}".format(code["limit_bytes"]), lb)))),
        ("The data selector and a selector that is too big", Raw(esc("The data selector 0x10 (index 2) has the same base and limit, so it gives the same answers. A selector whose index is beyond the table is refused: the table's limit is %d (3 entries x 8 bytes - 1), so selector 0x28 (index 5, offset 40) is outside it, and loading it is a general protection fault, which is what the kernel's gp test mode does on purpose." % limit))),
    ]
    stage = ruler(a, b, {"a": 1, "open": 2, "closed": 3}) + '<p class="how">The two marks are the loader\'s test addresses. The arrow shows where the second lands with the gate closed.</p>'
    cap = ('Computed from the two test addresses of <code>a20_enabled</code> in <code>boot/stage2.asm</code> and the loader\'s GDT. The build refuses a source whose test addresses stop aliasing when the gate is closed, or alias even when it is open. '
           'The wrap itself is the 8086\'s documented behaviour and the hardware gate\'s: it is replayed here, not measured on a real machine.')
    return stepper(ID, "From segment and offset to a linear address, with the A20 gate closed and open", stage, steps, cap, ctx, "x-wide")
