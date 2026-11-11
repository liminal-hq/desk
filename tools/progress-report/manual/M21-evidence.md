---
id: m-evidence
n: M21
part: VII
title: Testing and the evidence method
summary: Nothing is promoted without evidence: host tests against the mock bus, expected transcripts, negative checks, key injection, golden frames and their digests, the comparison of the two tracks, CI, the three shelves and the manual's four tags, and what verified never means.
date: 2026-10-23
reviewed: 44f0265
watches: [tools/check.sh, tools/native-check.py, tools/native-inject.py, tools/frames.py, tools/shell-compare.sh, tools/shell-goldens.sh, tests/native/, tests/shell/, hal/platform/mock/, .github/workflows/]
log: [boot, hal, input, firstwindow, archive]
anatomy: []
---
## What it is

This chapter is about how the project knows what it knows. The Desk is built from code that nobody can yet run on the machine it is for, in an archive that has to say which of its claims were checked and which were only written down, so the way a claim is checked is part of the system as much as any driver. There is one rule, and the rest of the chapter is its machinery: **nothing is promoted without evidence**. A piece of work is not called done because it was written, and a box on the map is not called verified because it looks right; it is called verified when something ran and its result was compared with something fixed in advance.

The chapter covers the kinds of check the repository has, what each can and cannot prove, how `tools/check.sh` strings them together and what the CI runs, how the log's three shelves and the manual's four state tags report the result, and the one thing no check here has ever shown: that anything works on a real machine. It leans on [M5](chapter:m-boot) for the transcript checker and the negative checks, on [M13](chapter:m-pixels) for frame digests and on [M1](chapter:m-map) for the state tags, and does not repeat them. After reading it you should be able to say, for any claim in this repository, what kind of [evidence](term:evidence) stands behind it, add a check of each kind, and read a failing run.

## How it works

### The rule

A claim, in this repository, is one of four things: that a piece of code exists, that it was built, that it ran and produced a result, and that the result was the right one. The first is a file in the tree. The second is a build that exits with status zero. The third and fourth need a check, and a check has three parts: a thing to run, a record of what it should produce that was written before the run, and a comparison that can say no. Everything below is a way of making those three parts cheap enough to run on every change.

The rule has a corollary that the repository applies to its own checks: **a check that has never failed has not been shown to work**. A transcript comparison is trusted because a deliberately wrong expectation is rejected; a frame comparison because a screen with one wrong pixel is rejected. The negative checks of [M5](chapter:m-boot) are this corollary in code.

### Host tests against the mock bus

Most of the project's code is Rust that never touches hardware directly: it calls the traits of the HAL ([M9](chapter:m-hal)). That is what makes it testable on an ordinary machine. A driver under test is given a [`MockPortBus`](sym:hal/platform/mock/src/bus.rs::MockPortBus) instead of the real ports, and the test scripts what each device would answer and then checks the exact sequence of reads and writes the driver made. The same crate has a mock interrupt controller, a mock clock, a mock PS/2 controller and a mock video card (`hal/platform/mock/src/`). A [host test](term:host-test) like this runs in a second, in CI, with no emulator, which is why the drivers have most of the project's tests: 179 in the HAL workspace and 161 in the shell's at the commit this chapter was checked against.

What it proves is that the driver does what its author believed the chip does. What it cannot prove is that the chip does it. The mock is the author's model of the hardware, and a mistake in the model and the driver that agree with each other passes. That is the reason the same drivers are run again in an emulator, and the emulator is itself a model; the gap between the two and a real part is the reason for the last section of this chapter.

### Expected transcripts

The native boot is checked by comparing the kernel's serial output with a file of expected lines and patterns. The pattern language, the matcher and the exit statuses are in [M5](chapter:m-boot); the point for this chapter is what the checker is *for*. A transcript ties together several claims at once (the loader ran, the kernel booted, the allocator worked, the timer ticked) and it is cheap to extend: a new line in the kernel and a new line in the expected file. Its weakness is that it checks what was printed, not what was true: a kernel that prints `fpu: results check out` without checking is not caught by the transcript. That is why the lines are printed by code that makes the assertion first (the kernel `assert!`s before it prints), and why a reviewer reads both.

### Negative checks

A [negative check](term:negative-check) runs the system in a way that must fail and demands the specific failure. [M5](chapter:m-boot) lists them: a divide error, a protection fault, an invalid opcode, a panic, a masked timer, a hang, a masked keyboard line, a silent one, a screen with one wrong pixel and a wrong expected transcript. Each shows the check is sensitive to something. Two forms are worth naming. The first changes the system (a test mode); the second changes the *expectation* (a transcript with one number altered, sums with one digest altered, the depth 4 sums used for the depth 8 run) and shows the comparison is not satisfied by anything. `tools/native-test.sh` has a small helper, `must_fail`, that runs a command which must exit non-zero and fails the whole test if it does not.

<!-- explainer m-negatives -->

### Key injection

Real input cannot be typed into a headless run, so the test sends it. QEMU has a human monitor, and `tools/native-run.sh --inject SCRIPT` starts it on a Unix socket and runs `tools/native-inject.py` beside the guest. The script format is small: `wait TEXT` waits until the text appears in the serial transcript (so keys are never sent before the kernel is listening), `sleep SECONDS` separates bursts, and any other line is a monitor command such as `sendkey shift-a`, `mouse_move 10 -5` or `screendump {out}/name.ppm`. Each command is sent only after the previous one is answered. This is [key injection](term:key-injection): the keys arrive at the emulated keyboard controller as scancodes, so the whole path from the 8042 to the kernel's decoder is exercised, which a test that called the decoder directly would skip. The cost is that the injection is QEMU's, not a person's: the timing is regular and the mouse moves in whole steps, and the wheel cannot be sent at all.

The first window's scenario shows the other use. `tools/frames.py inject tests/shell/first-window.scn` turns a scenario into a monitor script: wait for `input: ready`, then for each frame wait for the kernel's `shell: ready NAME` line, take a screendump and send a key so that the kernel goes on. The picture is taken by the emulator, not by the program being tested.

### Golden frames and digests

For graphics a transcript is not enough; the question is whether the pixels are right. The shell's first window is checked against [golden frames](term:golden-frame): pictures recorded once, looked at by a person, and compared byte for byte ever after. Each frame and depth has one line in `tests/shell/first-window.sums`, `NAME DEPTH WIDTHxHEIGHT INDEX COLOUR`, where `INDEX` is the CRC-32 of the palette indices row by row and `COLOUR` is the CRC-32 of every pixel's six-bit `r g b` bytes ([M13](chapter:m-pixels)). The goldens are indexed PNGs in `tests/shell/golden/first-window/`, for people to look at and for `frames.py diff` to draw the differing pixels over.

`tools/frames.py` is the one tool the three tracks share: it writes PNGs from raw dumps, computes digests, checks a directory of captures against the sums, checks a transcript's digests, checks that every PNG matches its line, compares two capture sets and writes the QEMU monitor script. The comparison is of the six-bit colour, not of the palette indices, because QEMU's screendump expands six bits to eight and renumbers the palette; shifting each channel right by two gets the six bits back exactly. The host test, the DOS read-back and the QEMU screendumps are all held to the same sums, which is what makes a matching digest a statement about the pixels and not about one program's view of them.

<!-- figure fw-digests -->

> **From the stacks — Golden masters.** A characterization test is also known as golden master testing, according to the Wikipedia article on the subject, which says the term was coined by Michael Feathers and cites his book "Working Effectively with Legacy Code". The article describes such a test as characterizing the actual behaviour of existing software, and adds that it records what the software does and not whether that is correct. The Desk's goldens work the same way, which is why each was looked at by a person before it was kept.
> Source: [Characterization test, Wikipedia](https://en.wikipedia.org/wiki/Characterization_test)

### Comparing the two tracks

`tools/shell-compare.sh` reads what the DOS check and the native test leave in `dist/` (the DOS read-back of the frame buffer and the DAC, and the QEMU screendumps) and compares the same frames from the two tracks with `frames.py same`, depth 8 with depth 8, depth 4 with depth 4, and the DOS run on a machine with no VBE with the native depth 4 run. Then it runs the comparisons that must fail. This is the strongest check the project has, because the two sides share no code below the shell: different runtimes, different display paths, different emulators. It says the shared core draws the same picture on both tracks. It does not say the picture is what a real card would show.

### CI and `tools/check.sh`

`tools/check.sh` is the pre-pull-request gate and what the CI runs. The CI is one job: a container from the shared `ci-dos` image pinned by digest in `.github/workflows/ci.yml`, a checkout, the font tools, the DOS toolchain, QEMU and NASM if the image lacks them, and `tools/check.sh`. Updating the image is a pull request whose checks run in the new image. The steps, in order, with the opt-in ones last:

| No. | Step |
|---|---|
| 1 | Licence headers |
| 2 | Markdown style and links |
| 3 | Progress report generator: unit tests and a dry run of the data gather |
| 4 | Fonts |
| 5 | Rust format and lints |
| 6 | HAL: host tests and no_std builds |
| 7 | Shell: host tests and no_std builds |
| 8 | Frame tools: PNG, PPM and digest reading, checks and the monitor script |
| 9 | Build DESK.EXE |
| 10 | Run DESK.EXE in headless DOSBox |
| 11 | A Rust panic reports through C and exits non-zero |
| 12 | Spike s1-dxe3: Rust format and lints for the modules |
| 13 | Spike s1-dxe3: build the Rust modules (DXE3) and the host |
| 14 | Spike s1-dxe3: load, call, crash and unload modules in headless DOSBox |
| 15 | Spike d0-rust-dos: pure Rust DOS programs, packed with no C layer |
| 16 | Spike s5-jsdos: Rust format and lints for the probes |
| 17 | Spike e-std-stubs: std on an invented OS, stubs and a patched platform layer |
| 18 | Spike a-vga-vbe: VGA modes 13h and 12h and VBE 2.0 through desk-vga, on the pure Rust DOS runtime |
| 19 | Spike b-dos-input: the keyboard on IRQ 1 and the mouse on INT 33h on the pure Rust DOS runtime, keys typed in by AUTOTYPE |
| 20 | Shell on DOS: the first window's scenario in DOSBox at both depths, read back and compared with the goldens |
| 21 | Native boot: NASM loader, Rust kernel, QEMU as a 486 |
| 22 | Both tracks: the DOS read-back and the QEMU screendumps are the same pictures |

Three more steps run only when asked, because the CI image lacks what they need: the mouse driven by `xdotool` under Xvfb (`INPUT_EXTRA=1`), the native boot test in Bochs (`NATIVE_EXTRA=1`) and the DOS programs in js-dos in headless Chromium (`JSDOS=1`). A record that rests only on one of these is tagged as such ([M20](chapter:m-emulators)).

### The three shelves and the four tags

The log sorts every claim onto one of three shelves: *catalogued* (verified, and the entry says how), *uncatalogued* (built, not verified) and the *wish list* (planned). The manual says the same thing with four tags on every mechanism, `built-verified`, `built-unverified`, `planned` and `external`, and enforces the first one mechanically: a record is built and verified only when its evidence includes a `test:` item (a test or an expected transcript line that a check runs) or a merged pull request whose CI passed. A record that claims more is shown lower, with the reason. The generator's tests are the proof that this rule is applied and not just stated ([M1](chapter:m-map) explains the tags; the generator's `README` the rule).

### What verified never means

In every place this project says *verified*, it means *in an emulator or on the host*. QEMU's 486 model accepts instructions a 486 lacks; its video card hides what a real card does; no emulator here is cycle-accurate; the BIOS images are not the BIOSes of real machines; and the one 1993 BIOS tried, in 86Box, found a real bug ([M20](chapter:m-emulators)). Nothing has booted on a real floppy drive or run on a real 486. A claim about hardware in this repository is therefore either from a document about the hardware (and says so) or a prediction.

## Why it is this way

- **Evidence before promotion.** The archive's rule (`tools/progress-report/README.md`, "Chapters") and the state table's rule are the same rule applied twice: the first stops a chapter overclaiming, the second stops a mechanism doing so. The generator applies it to the archive's own roadmap: a step marked done whose pull request is not merged with CI passing is shown as partly done (`tools/progress-report/README.md`, "How the data is gathered").
- **Checks that run headless.** Every check has to run in a container with no display, no sound and no person: QEMU with `-display none`, DOSBox with a generated configuration, the host's `cargo test`. That is why the framebuffer is read back and the screen dumped through the monitor, and why audio cannot be checked at all (`docs/HAL.md`, "Sound").
- **The same goldens everywhere.** Holding the host test, DOSBox and QEMU to one sums file means a disagreement between tracks is a failure with a name (which frame, which depth), not a difference of opinion (`docs/FIRST-WINDOW.md`, "Goldens and frame digests").
- **Two digests.** The index digest catches a change of palette numbering that the eye cannot see; the colour digest survives an emulator's renumbering. Both are kept because each fails where the other cannot (`tools/frames.py`).
- **Re-recording is deliberate.** A change that moves a pixel re-records the goldens in the same branch and a person looks at the PNGs (`tests/shell/README.md`); the golden's authority is that someone looked.
- **An opt-in tier.** Slow or heavy checks (Bochs, js-dos, Xvfb) are kept out of the default gate so the gate stays quick enough to run before every pull request, and each says in its header what it needs.

## The code

| Where | What it holds |
|---|---|
| [`tools/check.sh`](repo:tools/check.sh) | The pre-pull-request gate: every step above, in order |
| [`.github/workflows/ci.yml`](repo:.github/workflows/ci.yml) | The CI job: the pinned image, the tool set-up and `tools/check.sh` |
| [`hal/platform/mock/`](repo:hal/platform/mock/) | The mock platform: [`MockPortBus`](sym:hal/platform/mock/src/bus.rs::MockPortBus) and its devices |
| [`tools/native-check.py`](repo:tools/native-check.py) | The transcript checker ([M5](chapter:m-boot)) |
| [`tools/native-inject.py`](repo:tools/native-inject.py) | Sends scripted keys, mouse motion and screendump commands through the QEMU monitor |
| [`tools/frames.py`](repo:tools/frames.py) | PNG and PPM reading, digests, checks of captures and transcripts, comparison and the monitor script of a scenario |
| [`tools/test_frames.py`](repo:tools/test_frames.py) | The frame tools' own tests (37) |
| [`tools/shell-goldens.sh`](repo:tools/shell-goldens.sh) | Re-records the goldens from a clean state |
| [`tools/shell-compare.sh`](repo:tools/shell-compare.sh) | The two tracks' frames compared, and the comparisons that must fail |
| [`tools/native-test.sh`](repo:tools/native-test.sh) | The whole native check, with its negative checks |
| [`tests/shell/`](repo:tests/shell/) | The scenario, the sums and the goldens |
| [`tests/native/`](repo:tests/native/) | The expected transcripts and the injection scripts |

For depth: [`docs/FIRST-WINDOW.md`](doc:docs/FIRST-WINDOW.md#goldens-and-frame-digests) and [`docs/NATIVE-BOOT.md`](doc:docs/NATIVE-BOOT.md#the-checks).

## Worked example

**One frame, followed through every check.** The second frame of the first window's scenario is `moved`, where the pointer has moved and a cursor is drawn. Its line in `tests/shell/first-window.sums` is

```text
moved 8 640x480 10201b69 03faf8b3
```

That is the name, the depth, the size, the index digest and the colour digest. The same two numbers are in the golden PNG: `python3 -I tools/frames.py digest tests/shell/golden/first-window/moved-8.png` prints `640x480 10201b69 03faf8b3`. They are in the native transcript, which prints `shell: frame moved 8 640x480 10201b69 03faf8b3` from the kernel's own digest, and `frames.py transcript` checks that line against the sums. They are what the QEMU screendump must reproduce for the colour digest, and the DOS read-back of the frame buffer and DAC. The damage line `shell: damage moved: 0 pixels repainted, 456 presented in 2 rectangles` in the same transcript is a separate check that the cursor was drawn by touching two small rectangles and not by repainting the screen. Four independent paths reach the same eight hexadecimal digits, which is why the digest is evidence of the picture: the host test and the DOS program compute it from memory they own, and the QEMU run from the emulator's own picture.

Change one pixel of the golden and `frames.py goldens` reports `moved` as the frame that no longer matches its line; change one digit of the sums and the host test, the transcript check and the screendump check all say which frame. The `shell-corrupt` test mode does the harder thing and changes one pixel of the screen after the kernel has printed the right digests, so that only the screendump comparison can notice, and the test requires that it does.

## State of this layer

```status
id: evidence.hosttests
label: Host tests of the drivers and the core against the mock platform
state: built-verified
evidence: sym:hal/platform/mock/src/bus.rs::MockPortBus, test:tools/check.sh::cargo test --locked --quiet, test:.github/workflows/ci.yml::tools/check.sh
note: Run by CI on every pull request. The mock is the author's model of the hardware.

id: evidence.transcripts
label: Expected transcripts and the checker that compares them
state: built-verified
evidence: test:tools/native-test.sh::wrong-expected.txt, sym:tools/native-check.py::line_matches

id: evidence.negative
label: Negative checks, including a wrong expectation that must be rejected
state: built-verified
evidence: test:tools/native-test.sh::must_fail, test:tools/native-test.sh::hang
note: Described in M5, with the one weakness found in wrong-expected.txt.

id: evidence.inject
label: Key and mouse injection through the QEMU monitor
state: built-verified
evidence: test:tools/native-test.sh::run_input, sym:tools/native-inject.py::read_prompt
note: QEMU only; whole-step mouse motion and no wheel.

id: evidence.goldens
label: Golden frames and their digests, held to the host test, the DOS read-back and the QEMU screendumps
state: built-verified
evidence: test:tools/check.sh::frames.py goldens, test:tools/native-test.sh::frames check, sym:tools/frames.py::FramesError
note: The DOS read-back rests on the CI run; it was not rerun for this chapter.

id: evidence.compare
label: The two tracks' frames compared, and the comparisons that must fail
state: built-verified
evidence: test:tools/check.sh::tools/shell-compare.sh, sym:tools/shell-compare.sh::frames
note: Run by CI after the DOS and native steps; not rerun for this chapter.

id: evidence.ci
label: The CI job: a pinned image and tools/check.sh
state: built-verified
evidence: test:.github/workflows/ci.yml::tools/check.sh, doc:docs/NATIVE-BOOT.md#what-was-and-was-not-verified

id: evidence.tags
label: The manual's four tags and the rule that a verified record needs run evidence
state: built-verified
evidence: test:tools/progress-report/tests/test_report.py::test_a_verified_claim_without_a_passing_merged_pull_request_is_shown_as_unverified, sym:tools/progress-report/manual.py::effective_state

id: evidence.optional
label: The opt-in checks: Bochs, js-dos and the mouse under Xvfb
state: built-unverified
evidence: test:tools/check.sh::NATIVE_EXTRA, test:tools/check.sh::JSDOS
note: Not run by CI, which lacks what they need; run by hand and recorded in documents.

id: evidence.hardware
label: Any check on a real machine
state: planned
evidence: doc:docs/NATIVE-BOOT.md#what-was-and-was-not-verified, repo:docs/DIRECTION.md
```

The negative checks are {{state.evidence.negative}} and the host tests {{state.evidence.hosttests}}; the claim that any of it describes a real machine is {{state.evidence.hardware}}.

## Try it yourself

These commands run the cheap checks of this chapter on the host: the frame tools' tests, the goldens against their sums, one frame's digest, the monitor script that a scenario makes and the generator's tests. They need only Python 3. Output is from a run on 2026-10-23 at `44f0265`.

```try
needs: host: python3
ran: 2026-10-23 on the host
$ python3 -I tools/test_frames.py 2>&1 | tail -3
Ran 37 tests in 0.091s

OK
$ python3 -I tools/frames.py goldens tests/shell/first-window.sums tests/shell/golden/first-window
ok: 12 golden PNGs match tests/shell/first-window.sums
$ python3 -I tools/frames.py digest tests/shell/golden/first-window/moved-8.png
640x480 10201b69 03faf8b3
$ python3 -I tools/frames.py inject tests/shell/first-window.scn | head -6
wait input: ready
wait shell: ready open
screendump {out}/open.ppm
sendkey spc
wait shell: ready moved
screendump {out}/moved.ppm
```

The digest in the second command is the one in the `moved 8` line of `tests/shell/first-window.sums`. To see a check refuse, copy the sums, change the last hexadecimal digit of one line, and run `frames.py goldens` on the copy.

**Exercises.** (1) Predict which of the 12 goldens `frames.py goldens` would name if you changed the colour digest of the `typed` frame at depth 4 in a copy of the sums; then try it. (2) The native test's `shell-corrupt` mode prints the right digests and changes a pixel. Say which of the checks in the worked example would notice, and which would not, and why the test needs the one that would. (3) Pick a claim in this manual that a status record tags built-verified and find its evidence by following the links; say what kind of evidence it is (host test, transcript line, golden, merged pull request) and what it cannot show.

## Proved in the log

<!-- manual proved -->

## Where next

<!-- manual next -->

```glossary
term: Evidence
def: What stands behind a claim: a test or expected transcript that a check runs and whose result was compared with something fixed in advance, or a merged pull request whose CI passed. A claim with none is built, not verified.

term: Host test
def: A test that runs on the development machine against a model of the hardware (the mock platform), with no emulator, so that it can run in a second on every change.

term: Key injection
def: Sending keys and mouse motion to a running emulator through its monitor, so that headless runs exercise the whole input path from the emulated controller upward.

term: Golden frame
def: A picture recorded once, looked at by a person and kept, that every later run of the same scenario must reproduce exactly, checked by its digests.
```

```figures
id: m21-digests
reuse: first-window-digests
at: figure fw-digests

id: m21-negatives
reuse: m5-negatives
at: explainer m-negatives
```

## Revisions

- 2026-10-23: first written, against `44f0265`. The step table matches the `step` lines of `tools/check.sh` in order (a test checks it); the commands in the try block were run on the host. No disagreement was found between the documents and the scripts for this layer.
