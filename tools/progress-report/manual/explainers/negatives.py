# The negative checks: every deliberate failure of the native boot test, the test mode that causes it and what proves that it failed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# Driven by the repository's own files: the test modes are the `TestMode` variants of `kernel/src/main.rs`, their numbers the `case` of
# `tools/native-image.sh`, the lines that must appear are lines of `tests/native/expected-*.txt`, and the checks the test makes are the ones
# `tools/native-test.sh` runs. The explainer refuses a mode that is not in all of those, an expected line that is not in its file, an
# expected file that no test mode accounts for, and a negative mode that `tools/native-test.sh` does not run.
import os
import re

import gather
from explainers import stepper
from mdlite import Raw, esc

ID = "m-negatives"

# mode name (the image tool's), the TestMode variant, how the failure is proved, the expected file (or None), the lines of it that matter,
# the exit status the run must have, and what is said about it.
NEGATIVES = [
    ("div0", "DivideError", "tests/native/expected-div0.txt", ["test: dividing by zero", "EXCEPTION 0x00: divide error", "halting with a failure status"], 1,
     "The kernel executes a `div` by zero. The CPU raises exception 0, the kernel's own handler prints the dump and ends the run through the debug-exit port with the failure value."),
    ("gp", "GeneralProtection", "tests/native/expected-gp.txt", ["test: loading a segment selector beyond the GDT", "EXCEPTION 0x0d: general protection fault", "error code 0x00000028", "halting with a failure status"], 1,
     "The kernel loads selector `0x28` into `ds`, which is beyond the three-entry GDT it set up. The error code in the dump is the selector, `0x28`."),
    ("ud", "InvalidOpcode", "tests/native/expected-ud.txt", ["test: executing ud2", "EXCEPTION 0x06: invalid opcode", "halting with a failure status"], 1,
     "The kernel executes `ud2`, the instruction defined to be undefined, which raises exception 6."),
    ("panic", "Panic", "tests/native/expected-panic.txt", ["test: panicking", "deliberate panic for the negative check"], 1,
     "The kernel calls `panic!`. The panic handler prints the message with its source position (checked as a range, so an edit above it does not break the test) and ends the run with the failure value."),
    ("notick", "NoTick", "tests/native/expected-notick.txt", ["test: timer masked on purpose", "the timer did not tick"], 1,
     "The kernel masks IRQ 0 before it waits for the first tick. The bounded spin in `timer_demo` gives up and panics, which is why that spin is bounded: `hlt` would wait for ever."),
    ("hang", "Hang", None, ["test: hanging with interrupts off"], 124,
     "The kernel stops the CPU with interrupts off. There is no expected file, because the point is that nothing more is printed: the run script's watchdog, five seconds in this test, ends QEMU and the script returns 124."),
    ("input-masked", "InputMasked", "tests/native/expected-input-masked.txt", ["input: no Escape within 500 ticks (0 key events, 8 mouse events)"], 1,
     "The kernel leaves IRQ 1 masked while the run script injects keys. Mouse events arrive and key events do not, and the kernel reports exactly that and panics."),
    ("input-idle", "InputIdle", None, [], 124,
     "The kernel waits for input and nothing is injected. It waits longer than the watchdog, so the run ends with 124."),
    ("shell-corrupt", "ShellCorrupt", None, [], 0,
     "The kernel runs the first window's scenario honestly, prints the true digests, and then changes one pixel of the screen after the `moved` frame. Its transcript passes; its screendump does not, and `frames.py check` must reject it at the `moved` frame. The run itself exits 0: the check that fails is the one on the picture."),
]


def src(path):
    with open(os.path.join(gather.ROOT, path), encoding="utf-8") as f:
        return f.read()


def load():
    main, image, test, run = src("kernel/src/main.rs"), src("tools/native-image.sh"), src("tools/native-test.sh"), src("tools/native-run.sh")
    enum = {m.group(1): int(m.group(2)) for m in re.finditer(r"^\s+(\w+) = (\d+),$", main[main.index("enum TestMode"):main.index("impl TestMode")], flags=re.M)}
    codes = {m.group(1): int(m.group(2)) for m in re.finditer(r"(?:^|\s)([a-z0-9-]+)\) code=(\d+)", image)}
    return main, image, test, run, enum, codes


def check(main, image, test, run, enum, codes, negatives=NEGATIVES, listing=None):
    problems = []
    modes = set(codes)
    for mode, variant, expected, lines, status, _ in negatives:
        if variant not in enum:
            problems.append("kernel/src/main.rs has no TestMode::%s" % variant)
        elif codes.get(mode) != enum[variant]:
            problems.append("tools/native-image.sh gives %s the code %s, but TestMode::%s is %s" % (mode, codes.get(mode), variant, enum[variant]))
        if not re.search(r"(?<![\w-])%s(?![\w-])" % re.escape(mode), test):
            problems.append("tools/native-test.sh does not run the negative mode %s" % mode)
        if expected:
            text = src(expected) if os.path.exists(os.path.join(gather.ROOT, expected)) else None
            if text is None:
                problems.append("%s does not exist" % expected)
            else:
                have = text.splitlines()
                for l in lines:
                    if l not in have:
                        problems.append("%s has no line %r" % (expected, l))
        for l in lines:
            if l.startswith("test: ") and l[6:] not in main:
                problems.append("kernel/src/main.rs does not print %r" % l)
        if status == 1 and ("35)" not in run or "0x11" not in src("kernel/src/exit.rs")):
            problems.append("the failure status 1 (QEMU 35, value 0x11) is no longer how tools/native-run.sh and kernel/src/exit.rs end a failed run")
        if status == 124 and "124)" not in run:
            problems.append("tools/native-run.sh no longer returns 124 for a timeout")
    names = listing if listing is not None else sorted(f for f in os.listdir(os.path.join(gather.ROOT, "tests/native")) if re.match(r"expected-.*\.txt$", f))
    for f in names:
        stem = f[len("expected-"):-len(".txt")]
        if not any(stem == m or stem.startswith(m + "-") for m in modes):
            problems.append("tests/native/%s is an expected transcript that no test mode accounts for" % f)
    return problems


def render(D, ctx):
    main, image, test, run, enum, codes = load()
    problems = check(main, image, test, run, enum, codes)
    if problems:
        raise ValueError("the negative checks explainer refuses its sources: " + "; ".join(problems))
    steps, panels = [], []
    for i, (mode, variant, expected, lines, status, text) in enumerate(NEGATIVES):
        end = {1: "exit status 1: the guest reported failure (QEMU exited with 35)", 124: "exit status 124: the watchdog ended the run", 0: "exit status 0: the run succeeded and the picture was rejected"}[status]
        how = ("The transcript must match <code>%s</code>, with these lines among them." % esc(expected)) if expected else "There is no expected transcript for this mode."
        body = "%s Select it with <code>tools/native-image.sh --test-mode %s</code> (the image's test-mode byte becomes %d, <code>TestMode::%s</code>). %s Proof: %s." % (
            esc(re.sub(r"`([^`]+)`", r"\1", text)), esc(mode), codes[mode], esc(variant), how, esc(end))
        if lines:
            body += " Lines: " + "; ".join("<code>%s</code>" % esc(l) for l in lines) + "."
        steps.append(("%s: %s" % (mode, variant), Raw(body)))
        pre = "\n".join(lines) if lines else "(nothing more is printed)" if status == 124 else "(the transcript is the normal one)"
        panels.append('<div class="mn-panel" data-at="%d"><div class="mn-head"><b>--test-mode %s</b> <span class="mn-exit">%s</span></div><pre class="term mn-pre"><code>%s</code></pre></div>' % (
            i, esc(mode), esc(end), esc(pre)))
    wrong = "tests/native/wrong-expected.txt"
    i = len(NEGATIVES)
    steps.append(("wrong-expected.txt: the checker itself", Raw(esc("Before any of the above, the test proves that its own comparison can fail: %s is run against the normal boot's transcript with native-check.py and must be rejected. The file is a stale copy of the normal transcript, so it is rejected at the first line it lacks rather than at the one number it was meant to differ in; it still proves the checker says no." % wrong))))
    panels.append('<div class="mn-panel" data-at="%d"><div class="mn-head"><b>python3 -I tools/native-check.py %s</b> <span class="mn-exit">exit status 1: the transcript does not match</span></div><pre class="term mn-pre"><code>%s</code></pre></div>' % (i, esc(wrong), esc("transcript does not match tests/native/wrong-expected.txt")))
    cap = ('Each step is one deliberate failure of the native boot test. The modes are the variants of <code>TestMode</code> in <code>kernel/src/main.rs</code>, their numbers come from <code>tools/native-image.sh</code>, the lines are lines of <code>tests/native/expected-*.txt</code> and the runs are those of <code>tools/native-test.sh</code>. '
           'The build refuses an expected file no test mode accounts for and a negative mode the test does not run.')
    return stepper(ID, "The negative checks, one deliberate failure at a time", '<div class="mn-stage">%s</div>' % "".join(panels), steps, cap, ctx, "x-wide")
