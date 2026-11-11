# An interrupt from line to handler, and an exception from the instruction to the dump.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Driven by the sources: the remapped vector base from `hal/drivers/pic`, the kernel's `IRQ_BASE`, the stubs and the error-code mask of
# `kernel/src/idt.rs`, the vector names of its `NAMES` table, the order of the handler and the end-of-interrupt in `kernel/src/hal.rs`, and
# the dumps in `tests/native/expected-div0.txt` and `expected-gp.txt`. It refuses sources that disagree with one another (a PIC base that
# is not the kernel's IRQ base, an expected dump that names another exception, an end-of-interrupt sent before the handler).
import os
import re

import gather
from explainers import stepper
from mdlite import Raw, esc

ID = "m-irq"


def src(path):
    with open(os.path.join(gather.ROOT, path), encoding="utf-8") as f:
        return f.read()


def facts(files=None):
    f = files or {}
    pic = f.get("pic") or src("hal/drivers/pic/src/lib.rs")
    idt = f.get("idt") or src("kernel/src/idt.rs")
    hal = f.get("hal") or src("kernel/src/hal.rs")
    div0 = f.get("div0") or src("tests/native/expected-div0.txt")
    gp = f.get("gp") or src("tests/native/expected-gp.txt")
    m = re.search(r"pub const REMAPPED_BASES: \(u8, u8\) = \((0x[0-9A-Fa-f]+), (0x[0-9A-Fa-f]+)\);", pic)
    if not m:
        raise ValueError("hal/drivers/pic no longer defines REMAPPED_BASES")
    base_m, base_s = int(m.group(1), 16), int(m.group(2), 16)
    ib = re.search(r"pub const IRQ_BASE: u8 = (0x[0-9A-Fa-f]+);", idt)
    mask = re.search(r"\(\(1 << \\n\) & (0x[0-9A-Fa-f]+)\) == 0", idt)
    names = re.search(r"const NAMES: \[&str; 32\] = \[(.*?)\];", idt, flags=re.S)
    init = re.search(r"idt\[(0x[0-9A-Fa-f]+)\] = gate\(irq_stubs\[0\]\);", idt)
    if not (ib and mask and names and init):
        raise ValueError("kernel/src/idt.rs no longer has IRQ_BASE, the error-code mask, NAMES and the timer gate the explainer reads")
    stub = re.search(r"irq_stub\\n:\n\s+pushad\n\s+cld\n\s+push \\n\n\s+call irq_dispatch\n\s+add esp, 4\n\s+popad\n\s+iretd", idt)
    common = re.search(r"isr_common:\n\s+pushad\n\s+cld\n\s+push esp\n\s+call exception_handler", idt)
    if not (stub and common):
        raise ValueError("the interrupt stubs of kernel/src/idt.rs no longer have the shape the explainer describes")
    nm = re.findall(r'"([^"]*)"', names.group(1))
    disp = re.search(r"extern \"C\" fn irq_dispatch\(line: u32\) \{(.*?)\n\}", hal, flags=re.S)
    if not disp:
        raise ValueError("kernel/src/hal.rs no longer has irq_dispatch")
    body = disp.group(1)
    return {"base_master": base_m, "base_slave": base_s, "irq_base": int(ib.group(1), 16), "mask": int(mask.group(1), 16), "names": nm, "timer_gate": int(init.group(1), 16),
            "dispatch": body, "div0": div0, "gp": gp}


def check(fa):
    problems = []
    if fa["base_master"] != fa["irq_base"]:
        problems.append("the PIC is remapped to %#x but the kernel's IRQ_BASE is %#x" % (fa["base_master"], fa["irq_base"]))
    if fa["timer_gate"] != fa["irq_base"]:
        problems.append("idt::init installs the timer gate at %#x, not at IRQ_BASE %#x" % (fa["timer_gate"], fa["irq_base"]))
    if len(fa["names"]) != 32:
        problems.append("NAMES has %d entries, not 32" % len(fa["names"]))
    else:
        for vec, label in ((0, "div0"), (13, "gp")):
            want = "EXCEPTION %#04x: %s" % (vec, fa["names"][vec])
            if want not in fa[label].splitlines():
                problems.append("tests/native/expected-%s.txt does not contain %r" % (label, want))
    body = fa["dispatch"]
    if "h.call()" not in body or "PIC.eoi(irq)" not in body or body.index("h.call()") > body.index("PIC.eoi(irq)"):
        problems.append("irq_dispatch no longer runs the handler before it sends the end-of-interrupt")
    if "is_spurious" not in body or body.index("is_spurious") > body.index("h.call()"):
        problems.append("irq_dispatch no longer checks for a spurious interrupt before it runs a handler")
    return problems


def boxes(labels, row_y, at_offset, w=118, gap=16, x0=10):
    o = []
    for i, label in enumerate(labels):
        x = x0 + i * (w + gap)
        o.append('<rect x="%d" y="%d" width="%d" height="46" rx="6" class="ir-box"/><text x="%d" y="%d" text-anchor="middle" font-size="11.5" class="ir-t">%s</text>' % (x, row_y, w, x + w // 2, row_y + 27, esc(label)))
        if i:
            o.append('<line x1="%d" y1="%d" x2="%d" y2="%d" class="ir-arrow"/>' % (x - gap + 2, row_y + 23, x - 2, row_y + 23))
        o.append('<rect x="%d" y="%d" width="%d" height="52" rx="8" fill="none" class="ir-cur" data-at="%d" stroke-width="2.4"/>' % (x - 3, row_y - 3, w + 6, at_offset + i))
    return "".join(o)


def render(D, ctx):
    fa = facts()
    problems = check(fa)
    if problems:
        raise ValueError("the interrupt explainer refuses its sources: " + "; ".join(problems))
    vec = fa["irq_base"]
    names = fa["names"]
    err_vectors = [v for v in range(32) if (1 << v) & fa["mask"]]
    hw = [("1. The timer", Raw(esc("The 8254's channel 0 output goes high every 10 ms (a divisor of 11,932 on a 1,193,182 Hz input). It is wired to line 0 of the master 8259, which raises IRQ 0 to the CPU if the line is not masked. The kernel's PIC set-up leaves only this line open.")), "PIT output, line IRQ 0"),
          ("2. The remapped vector", Raw(esc("With interrupts enabled (the sti in timer_demo), the CPU asks the PIC which vector this is. The PIC answers its base plus the line: %#x + 0 = %#x. The BIOS default base would be 0x08, on top of the CPU's own exceptions; hal/drivers/pic remaps the master to %#x and the slave to %#x." % (fa["base_master"], vec, fa["base_master"], fa["base_slave"]))), "PIC gives vector %#x" % vec),
          ("3. The gate", Raw(esc("The CPU reads entry %#x of the IDT, an interrupt gate that idt::init pointed at irq_stubs[0], and pushes EFLAGS, CS and EIP on the kernel's stack. Using an interrupt gate clears the interrupt flag, so the handler cannot be interrupted itself." % vec)), "IDT gate %#x" % vec),
          ("4. The stub", Raw(esc("irq_stub0 saves every register with pushad, clears the direction flag, pushes its line number (0) and calls irq_dispatch. One short stub exists for each of the 16 lines, and each differs only in the number it pushes.")), "irq_stub0"),
          ("5. The dispatcher and the handler", Raw(esc("irq_dispatch first asks the PIC whether a spurious IRQ 7 or 15 is the cause (it is not) and then runs the handler registered for the line, here the timer's, which adds one to an atomic tick counter. The registry is written with interrupts off and read here with them off.")), "irq_dispatch, handler"),
          ("6. The end-of-interrupt and the return", Raw(esc("Only after the handler returns does the dispatcher send the end-of-interrupt to the PIC, which tells it that it may deliver the next IRQ 0. The stub then pops the registers and executes iretd, which restores EIP, CS and EFLAGS (and so the interrupt flag): the interrupted code resumes, and timer_demo sees the counter has moved.")), "EOI, popad, iretd")]
    ex = [("7. An exception: the instruction", Raw(esc("In the divide-error test mode the kernel runs div with a zero divisor. No device is involved: the CPU itself raises vector 0 (%s). The vector numbers below 32 are the CPU's own." % names[0])), "div by zero"),
          ("8. Its stub", Raw(esc("Exception stubs are one per vector, in idt.rs. Vector 0 pushes no error code, so its stub pushes a zero so that every frame has the same shape, then its vector number, and joins isr_common, which saves the registers and calls exception_handler with a pointer to them. The vectors where the CPU pushes an error code of its own are %s (the mask %#x); vector 13, the general protection fault of the gp mode, is one, and its error code is the selector, 0x28." % (", ".join(str(v) for v in err_vectors), fa["mask"]))), "isr0, isr_common"),
          ("9. The dump and the exit", Raw(esc("exception_handler looks the vector up in NAMES, prints the name, the error code, EIP, CS, EFLAGS, the general registers and CR0, CR2 and CR3, then exits through the debug-exit port with the failure value. The expected file tests/native/expected-div0.txt holds the line EXCEPTION 0x00: %s, and expected-gp.txt holds EXCEPTION 0x0d: %s; the run must end with exit status 1." % (names[0], names[13]))), "exception_handler, exit::failure")]
    allsteps = hw + ex
    steps = [(re.sub(r"^\d+\. ", "", t), b) for t, b, _ in allsteps]
    stage = ['<svg viewBox="0 0 800 220" class="diagram m-irqfig" role="img" aria-label="Two paths through the kernel: a hardware interrupt from the timer to the handler and the end-of-interrupt, and a CPU exception to the dump">']
    stage.append('<text x="10" y="18" font-size="12" class="d-muted">A hardware interrupt</text>')
    stage.append(boxes(["PIT, IRQ 0", "PIC, vector %#x" % vec, "IDT gate", "irq_stub0", "dispatch, handler", "EOI, iretd"][:6], 26, 0, w=110, gap=22))
    stage.append('<text x="10" y="130" font-size="12" class="d-muted">An exception</text>')
    stage.append(boxes(["div by zero", "isr0, isr_common", "handler, dump, exit"], 138, 6, w=150, gap=40))
    stage.append("</svg>")
    cap = ('Read from <code>hal/drivers/pic</code> (the remapped base), <code>kernel/src/idt.rs</code> (the gates, the stubs, the error-code mask and the exception names), <code>kernel/src/hal.rs</code> (the dispatcher) and the dumps of <code>tests/native/expected-div0.txt</code> and <code>expected-gp.txt</code>. '
           'The build refuses sources that disagree: a PIC base that is not the kernel\'s IRQ base, an expected dump that names another exception, an end-of-interrupt sent before the handler.')
    return stepper(ID, "An interrupt from the line to the handler, and an exception to the dump", "".join(stage), steps, cap, ctx, "x-wide")
