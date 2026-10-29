# Roadmap

A trackable list of what the project has done, what is in flight and what is planned or only an idea, with the evidence for each. It is the one place that says where everything stands; [`DIRECTION.md`](DIRECTION.md) says why the project is shaped this way and [`SPEC.md`](SPEC.md) says what the hosted track will be.

> **Status.** Written on 2026-10-29 against `main` at `44f0265` and the open pull requests as they stood that day. Every `done` item is merged into `main`. Nothing here has run on real hardware, and Bochs, 86Box and js-dos were not part of the runs behind the live-input and Manual work (their earlier results are in [`NATIVE-EMULATORS.md`](NATIVE-EMULATORS.md) and [`S5-JSDOS.md`](S5-JSDOS.md)). Where the repository does not establish a fact, the entry says so.

## Contents

- [Path to the first OS milestone](#path-to-the-first-os-milestone)
- [How to read it](#how-to-read-it)
- [Past: merged into main](#past-merged-into-main)
- [Current: in flight on open pull requests](#current-in-flight-on-open-pull-requests)
- [Future: planned, in the owner's order](#future-planned-in-the-owners-order)
- [Future: the path to an OS](#future-the-path-to-an-os)
- [Reuse and vendoring](#reuse-and-vendoring)
- [Future: ideas and blocked items](#future-ideas-and-blocked-items)
- [Decisions waiting on the owner](#decisions-waiting-on-the-owner)
- [How to keep this current](#how-to-keep-this-current)

## Path to the first OS milestone

The first OS milestone is: the native kernel boots, opens a window, and runs a separate program loaded from disk (R80). Until that works the native track is a kernel with a shell inside it, not an operating system. The order below is the owner's; each step names its items.

1. Finish and verify live input (R47).
2. The module loader spike: an executable format and loaders for both tracks (R48).
3. A floppy or IDE driver and a read-only FAT filesystem (R74, R75; spike R50).
4. A system-call boundary between the kernel and apps (R77).
5. The first separate app running from disk (R76, R79, R80).

Paging and per-task address spaces (R51, R78), sound and timing for Fizz (R81), DOS programs under the kernel (R49) and real hardware (R64) follow or run beside this path; none of them blocks it.

## How to read it

Each item has an id (R1, R2 and so on, never reused), a title, a status, the outcome in one line and the evidence: pull requests (`#NN`), documents and commits.

| Status | Meaning |
|---|---|
| `done` | Merged into `main`. The outcome says what was verified and how, and what was not |
| `in progress` | Built or being built on an open pull request that is not merged |
| `planned` | Decided, in a known order, not started |
| `idea` | Mentioned in a document or by the owner; not decided and not ordered |
| `blocked` | Cannot proceed until something outside the work happens |

Three limits apply to every `done` item unless it says otherwise. No code has run on a real 486, a real floppy drive, a real VGA card or a real mouse; "verified" means on the host, in DOSBox Staging, in QEMU (as a 486) or, where stated, in Bochs and 86Box. The emulators model the hardware and are not the hardware. And the design documents were written before the code, so where a document and the code disagree the code wins (see R55).

## Past: merged into main

Every pull request from #1 to #32 in the list is merged (there is no #22 in it). Merge commits are given where a pull request merged on its own; the first-window stack (#20 to #28) merged together in `c9612b4`.

### Foundations: fonts, emoji, toolchain, direction

| Id | Title | Status | Outcome | Evidence |
|---|---|---|---|---|
| R1 | Fonts | `done` | The four bitmap faces and the three modern outline families (Desk Sans, Mono, Serif) build into OTF, TTF and `.DFN` files. Checked by the font build's own tests; not seen on a DOS screen by the fonts work itself (the first window later drew them, R20 and R23) | #1 (`eb2bd1a`), [`FONTS.md`](FONTS.md) |
| R2 | Emoji | `done` | The Desk emoji set, colour fonts and the `.DEM` format. 164 of 244 requested sequences are drawn, 80 are not (R60) | #2 (`af0b171`), [`EMOJI.md`](EMOJI.md), [`EMOJI-ROADMAP.md`](EMOJI-ROADMAP.md) |
| R3 | Hosted walking skeleton | `done` | A DOS toolchain (Rust target `core/dos.json`, DJGPP, CWSDPMI) and a headless run in DOSBox Staging that prints a result line, checked by `tools/check.sh`. Not run on a DOS machine | #3 (`12c1c9a`), [`TOOLCHAIN.md`](TOOLCHAIN.md) |
| R4 | Direction and Rust targets | `done` | The decision (2026-10-22) to build a native Rust OS beside the hosted track, with a shared core behind a HAL, and the tracking document for how Rust is built for each application space | #5 (`f550bcb`), [`DIRECTION.md`](DIRECTION.md), [`RUST-TARGETS.md`](RUST-TARGETS.md) |
| R5 | Progress report | `done` | A generator for a static multi-page archive of the project (`tools/progress-report/`), with chapters for each spike, a gallery, and the first-window chapter and two anatomy pages. Published by hand as an artifact; the publishing is not repeatable from CI | #9, #18, #32 (`44f0265`), [`tools/progress-report/README.md`](../tools/progress-report/README.md) |

### Spikes

| Id | Title | Status | Outcome | Evidence |
|---|---|---|---|---|
| R6 | s1-dxe3: Rust modules as DXE3 files | `done` | Yes, with limits: a Rust module loads with `dlopen` and calls back into the core, run in headless DOSBox Staging with CWSDPMI r7. Some build variants were built and not run | #4 (`6a3c564`), [`S1-DXE3.md`](S1-DXE3.md) |
| R7 | d0: a pure-Rust DOS program | `done` | A DOS program with its own `_start`, DPMI wrappers and INT 21h output, linked by `rust-lld` and packed with CWSDPMI's stub, runs headless in DOSBox with no C layer. Whether the C layer and `coff2go32.py` retire is still the owner's call (see Decisions) | #7 (`6c563f0`), [`D0-RUST-DOS.md`](D0-RUST-DOS.md) |
| R8 | s5: DOS programs and VBE in js-dos 8 | `done` | The programs and the VBE linear frame buffer work in headless Chromium through js-dos 8; sound was measured from the emulator's samples. Chromium only; Firefox, Safari, phones and the opt-in `JSDOS` check were not run for later work | #13 (`662544b`), [`S5-JSDOS.md`](S5-JSDOS.md) |
| R9 | H: the HAL and shared drivers | `done` | The `hal/` workspace (port I/O, interrupts, time, ring buffer, a mock platform) with 16550, 8254 and 8259 drivers, tested on the host and built for the kernel and DOS targets. The design beyond that was a design; the migration steps are tracked at R53 and R58 | #8 (`fe419f3`), [`HAL.md`](HAL.md) |
| R10 | A: VGA and VBE from Rust | `done` | A driver for VGA modes 13h and 12h by registers and VBE 2.0 over a `Bios` trait, with 82 host tests and a program that proves every stage in DOSBox Staging on an S3 card. Not run on a real card, QEMU, Bochs or 86Box by this spike | #11 (`16ac88e`), [`A-VGA-VBE.md`](A-VGA-VBE.md) |
| R11 | C: kernel keyboard and PS/2 mouse | `done` | The native kernel reads the keyboard and the PS/2 mouse through shared drivers, checked by QEMU input tests with injected keys. Real 8042 behaviour may differ | #10 (`547478b`), [`C-INPUT.md`](C-INPUT.md) |
| R12 | B: DOS input | `done` | The keyboard on a protected-mode IRQ 1 handler and the mouse on INT 33h from Rust on the DOS runtime, checked in DOSBox with `AUTOTYPE` and, opt-in, with `xdotool` under Xvfb. No real mouse driver or hardware | #15 (`b754da9`), [`B-DOS-INPUT.md`](B-DOS-INPUT.md) |
| R13 | D: native VGA | `done` | The kernel draws: VBE asked for in the loader, VGA modes by registers, a software cursor and a window, checked by QEMU screendumps. Speed figures are emulated-speed numbers only | #14 (`3f00838`), [`D-NATIVE-VGA.md`](D-NATIVE-VGA.md) |
| R14 | n3: a real BIOS path for the kernel | `done` | A real-mode trampoline lets the protected-mode kernel call the BIOS (VBE, INT 13h, E820, RTC), run in QEMU on three video cards and in Bochs. Not run on real BIOSes. V86 stays the later upgrade | #17 (`0d9868b`), [`N3-BIOS-PATH.md`](N3-BIOS-PATH.md) |
| R15 | G: boot in Bochs and 86Box | `done` | The native image boots unchanged in Bochs (a 486 build, three BIOS images) and in 86Box (an AMIBIOS 486 board) after one loader bug that 86Box found was fixed. Not on real hardware | #16 (`ca3c37d`), [`NATIVE-EMULATORS.md`](NATIVE-EMULATORS.md) |
| R16 | E: `std` stubs | `done` | `std` builds for an invented OS name and a patched platform layer over the runtime makes `println!`, `HashMap`, `Instant` and `exit` work in DOSBox. Nothing ran on a 486 or under other DOS hosts | #12 (`564105d`), [`E-STD-STUBS.md`](E-STD-STUBS.md) |

### Native track

| Id | Title | Status | Outcome | Evidence |
|---|---|---|---|---|
| R17 | n0: native boot skeleton | `done` | A 512-byte NASM boot sector, a loader and a protected-mode Rust kernel boot a floppy image in QEMU as a 486 and print to serial; exceptions, a timer and a memory map are covered by `tools/check.sh`. Not run on a real floppy or a real 486 | #6 (`491ed55`), [`NATIVE-BOOT.md`](NATIVE-BOOT.md) |
| R18 | Kernel on the shared drivers | `done` | The kernel's serial, PIT, PIC and keyboard code run on the `hal/` drivers (migration step 1, done with R11). The kernel-side `kernel::hal` platform that [`HAL.md`](HAL.md) still lists as planned is out of date (see R55) | #10, #17, [`HAL.md`](HAL.md) |

### The first window

| Id | Title | Status | Outcome | Evidence |
|---|---|---|---|---|
| R19 | First-window design and plan | `done` | The design for the shared core's first frame on both tracks and a twelve-task build plan | #19 (`b45b252`), [`FIRST-WINDOW.md`](FIRST-WINDOW.md), [`FIRST-WINDOW-PLAN.md`](FIRST-WINDOW-PLAN.md) |
| R20 | The shared `shell/` core | `done` | The crates: the compositor `desk-gfx`, `desk-font` (reads `.DFN`), `desk-present` (linear and planar presenters), `desk-ui`, `desk-wm`, the scenario parser and `tools/frames.py`, and the runtime. Host-tested against golden frames at both depths | #20, #21, #23 to #27 (`c9612b4`) |
| R21 | The first-window scenario and goldens | `done` | A six-frame scenario (`open`, `moved`, `focus`, `dragged`, `typed`, `closed`) with `.sums` digests and golden PNGs at 8 and 4 bits, run through all three presenters | #28 (`c9612b4`), `tests/shell/` |
| R22 | The window on the native kernel | `done` | The kernel's `shell` test mode runs the scenario and QEMU's screendumps match the goldens (std and Cirrus cards). Scripted events, not live input | #29 (`81c85f4`) |
| R23 | The window on DOS as `SHELL.EXE` | `done` | The hosted program draws the scenario on the pure-Rust DOS runtime and holds to the goldens in DOSBox Staging; keys reach it through IRQ 1 in the interactive run | #30 (`2164e7f`) |
| R24 | Direct comparison of the two tracks | `done` | Every frame of the scenario has the same six-bit colour digest on DOS and native, at both depths. The DOS capture reads back the emulated card's memory, not the emulator's rendered screen. Not verified: the mouse path, real hardware, Bochs, 86Box | #31 (`24cc044`), [`FIRST-WINDOW.md`](FIRST-WINDOW.md#result) |
| R25 | Progress report: first-window chapter | `done` | A chapter and two anatomy pages (DOS and native) in the progress report. The DOS anatomy page's opening wording is corrected in #36 (R36) | #32 (`44f0265`) |

## Current: in flight on open pull requests

None of the items below is merged. Each pull request's checks were passing on 2026-10-29. No pull request in either stack has been built from a clean clone of the whole stack by anyone other than its author, and the owner has not merged any of them.

### Live input (L1 to L9)

A linear stack: each pull request's base is the branch of the one below it, starting at #33. The plan is in #33 (`docs/LIVE-INPUT-PLAN.md` and `docs/LIVE-INPUT.md` exist only on the branches until it merges). The builder stopped partway, so the statuses below follow what each pull request contains, not what the plan promised. The result write-up in #54 reports measured runs on 2026-10-23 on the tip of the stack; those runs were by the author and have not been repeated from clean clones, and #54's own "Not verified" section lists the real desktop, real hardware, other emulators and drivers, `INPUT_EXTRA=1 tools/check.sh` in the CI image, and the right and middle buttons.

| Id | Title | Status | Outcome | Evidence |
|---|---|---|---|---|
| R26 | Live-input design and plan | `in progress` | `LIVE-INPUT.md` (design, key table, eight questions with defaults) and `LIVE-INPUT-PLAN.md` (tasks L1 to L9). Docs only | #33 (`docs/live-input-plan`, base `main`) |
| R27 | L1: cursor coalescing and the `live` module | `in progress` | A burst of cursor moves presents at most two sprite rectangles (the test failed before the change: 2440 pixels against 456); `Shell::is_dirty`; `desk_shell::live` (`route`, `Tally`, mark lines). Host tests | #34 (`feat/live-cursor`) |
| R28 | L2: window-manager keys | `in progress` | Alt+Tab, Shift+Alt+Tab, Alt+F4, Tab and Shift+Tab in `desk-wm`, the caret drawn only in the focused field, and a ten-frame `keys.scn` at each depth. Host tests | #37 (`feat/live-wm-keys`) |
| R29 | L3: live scenarios and frame tooling | `in progress` | `live.scn` (8 frames) and `live-keys.scn` (4 frames); `frames.py live` turns them into QEMU monitor commands, `AUTOTYPE` lines or `xdotool` lines, and `frames.py marks` checks a run's marks | #39 (`feat/live-frames`) |
| R30 | L4: INT 33h press and release counters | `in progress` | `INPUT.EXE slowpoll` against six 5 ms clicks saw none of them without the counters and all six with them (DOSBox's built-in driver only). The pull request's premise changed: a click with no gap never reaches DOSBox's driver, so spike B's explanation of the lost clicks was wrong | #43 (`feat/live-int33-clicks`), [`B-DOS-INPUT.md`](B-DOS-INPUT.md) |
| R31 | L5: native live run | `in progress` | The kernel's `shell-live` mode (code 15) on the PS/2 drivers, paced by the PIT, marks on F12, Escape to end; per #54, QEMU played `live.scn` with 8 of 8 marks equal to the goldens at both depths. QEMU only | #44 (`feat/live-native`) |
| R32 | L6: DOS live run | `in progress` | `SHELL.EXE`'s live loop with `marks`; per #54, `AUTOTYPE` and `xdotool` runs matched the goldens, and the native and DOS dumps of `live.scn` are pixel-equal. DOSBox's built-in mouse driver only | #49 (`feat/live-dos`) |
| R33 | L7: launchers | `in progress` | `tools/run-shell.sh` (native or dos), `native-run.sh --window` and `DOS_WINDOW=1 tools/dosrun.sh` open the shell in an emulator window. Not tried on a Wayland desktop or by a person | #50 (`feat/live-launchers`) |
| R34 | L8: the DOS mouse under Xvfb | `in progress` | `shell/dos/check-xvfb.sh` runs `SHELL.EXE` under Xvfb with `xdotool` and compares the marks with the goldens; opt-in (`INPUT_EXTRA=1`), needs Docker | #53 (`test/live-dos-xvfb`) |
| R35 | L9: the result | `in progress` | `LIVE-INPUT.md`'s Result section: what was built, matched, differs from the design, and is not verified, with four screenshots from the runs | #54 (`docs/live-input-result`) |
| R36 | DOS anatomy wording fix | `in progress` | A one-line fix so the Rust DOS anatomy page says plainly what it shows. Not part of the live-input stack (base `main`) | #36 (`docs/anatomy-dos-opening`) |

### The Manual (M1 to M22)

A second linear stack on #38, with the state of the work in `MANUAL-STATUS.md` (on branch `docs/manual-appendices`, #63). The generator adds a Manual group to the progress report; the chapters are checked by their own tests (239 in all, per the status file) and by `build.py --check`. The status file's list of what could not be verified applies to all of them: nothing on real hardware, Bochs and 86Box not run for the chapters, js-dos and the opt-in checks not run, the DOS side run only for M17 and M19, and `tools/check.sh` not run in full locally (CI ran it on #40 to #57).

| Id | Title | Status | Outcome | Evidence |
|---|---|---|---|---|
| R37 | Manual plan | `in progress` | `MANUAL-PLAN.md`: 22 chapters, explainers, appendices and eight questions with defaults. Docs only | #38 |
| R38 | W1: the Manual generator | `in progress` | The Manual group, `manual.html`, pager, backlinks, glossary, commands, sources and staleness report in the progress-report generator, with tests. Code-reviewed | #40 |
| R39 | M4, M5: boot sector, loader, the whole boot | `in progress` | The model chapters and the `image` and `negatives` explainers. Code-reviewed | #41 |
| R40 | M1, M2, M3: map, processor, devices | `in progress` | Chapters on the memory map, the CPU and the devices of a 486 PC, and the ports explainer. Code-reviewed | #45, #46 |
| R41 | M6 to M8: tables, interrupts, memory, BIOS | `in progress` | Chapters on the kernel's tables, interrupts and timer, kernel memory and calling the BIOS. Code-reviewed | #47, #48 |
| R42 | M9 to M14: HAL, drivers, surfaces, text, chrome | `in progress` | Chapters on the HAL, the first drivers, keyboard, mouse, VGA and VBE, surfaces and damage, text and window chrome. Code-reviewed | #51, #52, #55 |
| R43 | M15 to M17, M19: shell, platforms, runtime, toolchain | `in progress` | Chapters on windows and scenarios, the two platforms, the DOS runtime and the toolchain; the DOS side of M17 and M19 was run in the CI image. Code-reviewed | #56, #57 |
| R44 | M18, M20 to M22: fonts, emulators, evidence, frontier | `in progress` | Chapters on fonts and formats, emulators, the evidence method and what is not built. Not code-reviewed (#58's review did not run; #59 was read by its author alone) | #58, #59 |
| R45 | Explainers X1 to X9 and appendices | `in progress` | The address, GDT, frames, interrupt, crate graph, `.DFN` and target explainers; appendix B generated from the files that hold the numbers; the editorial pass. Not code-reviewed; checked by their own tests and by rendering | #60, #61, #62, #63 |
| R46 | Progress report with the Manual, published | `in progress` | Archive v10 is reported as published (the artifact's own version 10). That comes from the project transcript, passed on by the coordinator; `MANUAL-STATUS.md` on #63 does not mention a version, and the artifact was not read for this change, so it is not confirmed here. The publishing recipe is in [`tools/progress-report/README.md`](../tools/progress-report/README.md) | #63, [`tools/progress-report/README.md`](../tools/progress-report/README.md#publishing) |

## Future: planned, in the owner's order

| Id | Title | Status | Outcome | Evidence |
|---|---|---|---|---|
| R47 | Finish and verify live input | `planned` | The live-input stack is merged in order, and its checks pass from clean clones of `main` and in the CI image; the Not verified list of #54 is re-read and shortened where it can be | R26 to R36, #33 to #54 |
| R48 | Module loader spike | `planned` | Our own executable format and a loader: a packer that turns the linked Rust ELF into it, one loader for the native kernel and one DOS stub, so a module runs on both tracks without DJGPP's loader. Ends with a result document in `docs/` | [`DIRECTION.md`](DIRECTION.md) (native milestone 6; "What stays valid and what changes"), [`S1-DXE3.md`](S1-DXE3.md) |
| R49 | Candidate spike: FreeDOS in a V86 task under the native kernel | `planned` | The kernel runs FreeDOS in a V86 task or boots into it, which is how DOS programs run on the native OS. Candidate; the order among the four is open | [`DIRECTION.md`](DIRECTION.md) ("DOS programs: bundle FreeDOS"), [`N3-BIOS-PATH.md`](N3-BIOS-PATH.md) |
| R50 | Candidate spike: native floppy and IDE | `planned` | Own floppy and IDE drivers in the kernel instead of INT 13h through the trampoline, the first step to a filesystem (follow-on work: R74, R75). Candidate | [`DIRECTION.md`](DIRECTION.md) (native milestone 5, "Early disk access") |
| R51 | Candidate spike: paging | `planned` | The kernel turns on paging, which memory protection and V86 both need. Candidate | [`N3-BIOS-PATH.md`](N3-BIOS-PATH.md), [`DIRECTION.md`](DIRECTION.md) ("Memory protection") |
| R52 | Candidate spike: Lieutenant Fizz Episode 2 as a native Rust DOS game | `planned` | A game as a plain Rust DOS program on the shared runtime and drivers, with no gcc-built C layer. Candidate | [`DIRECTION.md`](DIRECTION.md) ("Rust for DOS: shared machinery") |
| R53 | Promote `rt` to a top-level crate | `planned` | `spikes/d0-rust-dos/rt` becomes `rt/` with `rt::hal` as the hosted platform (HAL migration step 2) | [`HAL.md`](HAL.md) (migration plan), [`D0-RUST-DOS.md`](D0-RUST-DOS.md) |
| R54 | Follow-up fix for the d0 runtime | `planned` | A fix pull request for four findings from the code review of the merged d0 runtime, never fixed: a recursion guard in `die` (`spikes/d0-rust-dos/rt/src/dos.rs`, line 72), `saturating_add` where the arithmetic can overflow (the sums in `rt/src/heap.rs` at lines 99 to 102 and 134 are the candidates), a reset of the IRQ 0 tick counter when the hook is installed (`rt/src/timer.rs`, `RT_IRQ0_TICKS` at line 17; the install code is at lines 56 to 70), and removal of dead code (not located). Whether to do the follow-up pull request is the owner's decision. The findings come from the project transcript, passed on by the coordinator; the repository does not record them, and the line numbers are the places the findings point to on `main` at `44f0265`, not quoted from the review | #7, [`D0-RUST-DOS.md`](D0-RUST-DOS.md) |
| R55 | Correct the stale design documents | `planned` | A documentation pull request that fixes the documents the Manual found out of date: `HAL.md` (still says no binary uses the HAL and marks `kernel::hal` and `rt::hal` planned), `D0-RUST-DOS.md` (line counts and sizes), `NATIVE-BOOT.md` (the `wrong-expected.txt` description and where the EOI is sent), `RUST-TARGETS.md` (`elf2bin.py` is not a linker), `FIRST-WINDOW.md` (the platforms and `expect-damage`) | `MANUAL-STATUS.md` on #63 ("Known gaps") |
| R56 | Refresh `tests/native/wrong-expected.txt` | `planned` | The negative-test expectation is rejected at its fifth line rather than at the number it was meant to differ in; refreshing it is a change to a test | `MANUAL-STATUS.md` on #63, [`tests/native/`](../tests/native/) |
| R57 | Merge the Manual stack and review #58 to #63 | `planned` | Wait for CI on #58 to #63, run the code review on each, merge in the status file's order when the owner says so, then re-run `build.py --manual-report` on `main` and re-publish | `MANUAL-STATUS.md` on #63 ("The next step") |
| R58 | HAL migration steps 3 to 5 | `planned` | The hosted skeleton and shell written against `Platform`, new drivers landing in `hal/drivers/`, and the core running on both platforms and the mock | [`HAL.md`](HAL.md) (migration plan) |
| R59 | Tests the Manual found missing | `planned` | A script that runs the `bios-solo` test mode, and a test that makes a floppy read fail so stage 1's retry path runs (it has never run) | `MANUAL-STATUS.md` on #63 |
| R60 | Remaining emoji | `planned` | 80 sequences still to do: 23 single, 2 VS16 forms, 7 ZWJ sequences and 48 tone families (240 variants by palette substitution), in the order set out in the emoji roadmap | [`EMOJI-ROADMAP.md`](EMOJI-ROADMAP.md) |
| R61 | Hosted track: handoff, core apps, release | `planned` | The handoff to DOS programs and back (`DESK.COM`, `DESK.RUN`), then Files, Programs and Settings, then optional apps and a release | [`SPEC.md`](SPEC.md) (milestones), [`HANDOFF.md`](HANDOFF.md), [`DIRECTION.md`](DIRECTION.md) |
| R62 | Native track: filesystem, shared core with apps, sound | `planned` | A filesystem the kernel reads, the shared core with the core apps on the native HAL, and OPL and Sound Blaster sound | [`DIRECTION.md`](DIRECTION.md) (native milestones 5, 7, 8) |
| R63 | The native track in the browser | `planned` | The same disk image booting in v86 | [`DIRECTION.md`](DIRECTION.md) (native milestone 9) |

## Future: the path to an OS

Items that the first OS milestone needs and that no earlier section tracks. Candidate spikes already listed (R48 to R52) and the real-hardware run (R64) are linked, not repeated. Each item says what it depends on and whether something could be reused; see [Reuse and vendoring](#reuse-and-vendoring), where every candidate is still to be assessed.

| Id | Title | Status | Outcome | Depends on | Could we vendor or reuse something? |
|---|---|---|---|---|---|
| R74 | Block device trait and the floppy and IDE/ATA drivers | `planned` | A block-device trait in the HAL and floppy and IDE/ATA drivers in the native kernel, tested in QEMU against a disk image; the spike is R50, and INT 13h through the trampoline (R14) is the stopgap | R50, [`HAL.md`](HAL.md) | Possibly: an ATA driver from a Rust OS project; to be assessed |
| R75 | A filesystem | `planned` | Read-only FAT12 and FAT16 first, so DOS-compatible disks work and FreeDOS can share them, then a writable one; whether a filesystem of our own is wanted for the system volume stays open | R74 | Possibly: a Rust FAT crate; to be assessed |
| R76 | Load a separate program from disk | `planned` | The native kernel reads an executable from a disk image and runs it, using the format and loader of R48 | R48, R74, R75 | Only the format is ours; see R48 |
| R77 | Tasks, a scheduler and a system-call boundary | `planned` | A task model, a cooperative scheduler first, and a system-call interface between the kernel and apps; preemption is an open question in [`DIRECTION.md`](DIRECTION.md) | R76 | Design ideas only; to be assessed |
| R78 | Per-task address spaces | `idea` | Each task gets its own address space on the 486's MMU; flat and shared memory is the alternative. The paging spike is R51 | R51, R77 | Possibly: paging code in a Rust OS project; to be assessed |
| R79 | The app interface made real | `planned` | [`SDK.md`](SDK.md) is design only; this turns it into a stable API so an app can be built separately from the kernel and run through the system calls | R77, R48 | No; it is the project's own interface |
| R80 | First separate app from disk (the first OS milestone) | `planned` | Boot, open a window, and run a program loaded from disk. This is the definition of the first OS milestone | R47, R48, R74 to R77, R79 | Not applicable |
| R81 | Sound and timing for Fizz Episode 2 | `planned` | A Sound Blaster or PC speaker driver and a stable timer that the Fizz game needs, then the game itself (R52). The broader sound work is in R62 | R52, R62 | Possibly: the OPL and Sound Blaster notes in [`S5-JSDOS.md`](S5-JSDOS.md) are ours; any driver code to be assessed |

DOS programs under the native kernel (V86 mode with FreeDOS bundled) stay at R49 and are optional for the first milestone.

## Reuse and vendoring

Nothing is vendored, and nothing here has been checked for compatibility. The project's licence is Apache-2.0 OR MIT; for each candidate the first thing to check is its licence and whether it is compatible with that dual licence, and every candidate below is **to be assessed**. A decision to vendor or depend on outside code needs the owner, a `NOTICE` entry (`NOTICE` already lists third-party material), and an exception to the rule in `AGENTS.md` that `shell/` takes no external crates (the kernel and `hal/` are not under that rule, but the same owner decision applies).

Where licences were looked up, on 2026-10-29 with a web fetch of each project's own page:

| Candidate | For | Licence as stated on its page | Notes |
|---|---|---|---|
| [`rust-fatfs`](https://github.com/rafalh/rust-fatfs) (the `fatfs` crate) | R75 | MIT (its README links `LICENSE.txt`) | States `no_std` support with `default-features = false` and FAT12, FAT16 and FAT32. Maintenance and whether it fits a kernel with no OS I/O traits not checked; to be assessed |
| [Theseus](https://github.com/theseus-os/Theseus) | R74, R77, R78 | MIT (its README) | A Rust OS. Whether it has a usable ATA driver crate was not established from the page; to be assessed |
| [Redox](https://gitlab.redox-os.org/redox-os/redox) | R74, R75, R77 | MIT (stated on its page) | A Rust OS. Which components could be reused, and under what licence each one is, was not checked; to be assessed |
| [FreeDOS kernel](https://github.com/FDOS/kernel) | R49 | GPL v2 (its README says "GPL v2"; its About says "GPL v2 or later") | Copyleft; bundling it affects the licence of what ships, as [`DIRECTION.md`](DIRECTION.md) already warns. Other FreeDOS programs have their own licences, not checked |

I am not confident of any other project's existence or licence, so none is listed. A source for the floppy controller and a Sound Blaster driver was not looked up.

## Future: ideas and blocked items

| Id | Title | Status | Outcome | Evidence |
|---|---|---|---|---|
| R64 | Run on a real 486 | `blocked` | Boot, draw and take input on real hardware, which is the only way to settle every "not verified on real hardware" in the documents. No real hardware is in the loop yet | [`DIRECTION.md`](DIRECTION.md) (risks, native milestone 11) |
| R65 | Name the project | `blocked` | Choose a product name from the candidates and set the repository's description and topics. Waiting on the owner (see below) | [`NAMING.md`](NAMING.md) |
| R66 | Live input: pointing without capture | `idea` | An absolute pointer in QEMU through the `vmmouse` back door, so no capture is needed; needs a small driver with its own `unsafe` | #54, `LIVE-INPUT.md` (question 4) |
| R67 | Alt+Tab switcher overlay | `idea` | Show the windows while Alt is held; the live-input default is release-to-pick with nothing shown | `LIVE-INPUT.md` (question 3) |
| R68 | Own the DOS extender | `idea` | An extender of our own in place of CWSDPMI, which would need VCPI, XMS and DPMI-compatibility work. Research, not planned | [`DIRECTION.md`](DIRECTION.md) ("Rust for DOS") |
| R69 | The machinery in its own repository | `idea` | The runtime, packer and shared drivers move to a repository such as `liminal-hq/dos-rs` once stable | [`DIRECTION.md`](DIRECTION.md) |
| R70 | A TrueType reader | `idea` | The Desk reads `.TTF` files so apps can use any font; not needed for 0.1 | [`OPEN-QUESTIONS.md`](OPEN-QUESTIONS.md) |
| R71 | A Rust target upstream | `idea` | A published prebuilt sysroot or an upstream Tier 3 target for 32-bit DOS | [`DIRECTION.md`](DIRECTION.md), [`RUST-TARGETS.md`](RUST-TARGETS.md) |
| R72 | A v86 boot inside the Manual | `idea` | A page that boots the real image in the browser; needs a large dependency and a hosting decision | `MANUAL-PLAN.md` on #38 |
| R73 | Native bench build of the core | `idea` | The core built for `i586-unknown-linux-gnu` as a third consumer of the HAL, which costs little | [`DIRECTION.md`](DIRECTION.md), [`SPEC.md`](SPEC.md) |

## Decisions waiting on the owner

- **The project name** ([`NAMING.md`](NAMING.md)) and the GitHub description and topics (R65).
- **Desk Mono Bold's "a".** It still has a pinhole counter, a near-solid blob. Desk Sans Bold's was fixed separately and Desk Mono Regular is loved and stays unchanged. The choice is to open up the Bold "a" only, leaving Regular untouched, or to leave it. (From the project transcript; see [`FONTS.md`](FONTS.md) for the faces.)
- **Yellow in the 16-colour set.** The 16-colour fallback (slots 0 to 15, also the only colours in mode 12h) has no yellow, so the yellow circle and the orange circle emoji differ only by a band of brown; the Afterglow ramps in slots 16 to 96 do include yellow ([`PALETTE.md`](PALETTE.md)). The choices are to change the 16-colour set, accept it, or re-colour the oranges. A related open question: whether to draw lower-quality vector versions of animals and food or leave them to the pixel set. (From the project transcript.)
- **Two older artifacts.** "Desk Fonts" (https://claude.ai/artifact/8Env6h8TMiFgiHgEmGBLSD) and "Desk Emoji" (https://claude.ai/artifact/BB2icTX4maR22ZBrcBLSAZ) are redundant now that the font and emoji pages live inside the new site, but the owner likes the emoji page as it is. Deleting either is the owner's call, and nothing has been deleted. (From the project transcript.)
- **The d0 runtime follow-up** (R54): whether to open the fix pull request for the four review findings.
- **The defaults taken for live input.** Eight questions in `LIVE-INPUT.md` (on #33 and #54): Escape ends a session, F12 always marks, Alt+Tab with no overlay, no absolute pointer, left-button counters only, one PIT tick of pacing, no windowed QEMU from Docker, one launcher.
- **The defaults taken for the Manual.** Eight questions in `MANUAL-PLAN.md` (on #38): where it lives, M-numbers, a textbook voice with Gregory as editor, CI failing on broken pointers, the pager, the legacy hosted skeleton in one section of M17, planned formats in a short table, and writing the boot pair first.
- **Retiring the hosted skeleton's C layer and `coff2go32.py`** now that d0 works (R7).
- **The order of the candidate spikes** (R49 to R52) after the module loader (R48).
- **Merging the live-input and Manual stacks** (R47 and R57), and which is first.

## How to keep this current

- **Who.** The agent that lands a change updates this file in the same pull request; the owner reviews it with the change. A pull request that completes, starts or abandons an item says so here.
- **When.** At least when a pull request merges, when a spike starts, and when a result document says something this file claims otherwise. A fresh read against `gh pr list --state all` before each release or progress-report publication catches drift.
- **Status changes need evidence.** Every change of status cites a pull request number (`#NN`), a document or a commit, and a `done` item states what was verified and how, and what was not. An item never goes to `done` before its pull request is merged into `main`.
- **Ids are stable.** New items take the next number; a dropped item stays with the status `idea` and a note on why, instead of being deleted.
- **Check it.** Run `python3 -I tools/check-docs.py` before committing.
