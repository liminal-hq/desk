---
id: module
n: 5
title: Can Rust be a module?
date: 2026-10-22
summary: Spike s1-dxe3 asked whether an app could be a loadable Rust file that calls back into the core, survives its own crash and leaves no memory behind. Yes, with limits.
prs: [4]
---
The Desk's design wants apps to be separate files that the core loads, runs and unloads, so that a bad app cannot take the desktop with it. Whether Rust could be that kind of file under DJGPP was the first real question, and spike `s1-dxe3` (PR #4, stacked on #3) answered it: **yes, with limits**. A `no_std` Rust crate built with the skeleton's pipeline becomes a DXE3 module with one extra step, `dxe3gen -o X.DXE -E _app_ -U <object> <libgcc.a>`; the skeleton's `tools/coff2go32.py` is enough, unchanged. The core `dlopen`s the module from disk, finds `_app_start`, `_app_event` and `_app_stop` with `dlsym`, and runs the whole life cycle twice.

Rust works inside a module: `Vec`, `String`, `format!` and x87 floats, with the allocator forwarding to the core. Each module carries its own copy of `core` and `alloc`, so two modules loaded together keep separate state with no symbol clashes. They are small — 7.7 KB (hello), 22.9 KB (counter) and 8.2 KB (crash) — but only with link-time optimisation, which is mandatory: without it a module is about 193 KB and fails to load with `unresolved symbol: ___register_frame_info`, and the pull request notes that the underlying cause was not found.

Below is the module life cycle, replayed from the transcripts that CI compares with a real DOSBox run. Step through it, or press Play.

<!-- explainer modules -->

What the replay shows is the property the design needed. A Rust panic inside a module is recovered with `setjmp` and `longjmp`, the core unloads the dead module, and the modules loaded before and after it keep working. A divide error is recovered the same way. Two things are **not** recoverable, and the spike says so plainly: a stack overflow, because CWSDPMI aborts the whole program before any signal handler runs, and — under DOSBox Staging, which aborts itself on any page fault — every fault other than a panic or a divide error. A null write was recovered under QEMU with FreeDOS and CWSDPMI r7 only, by hand.

<!-- facts s1Scenarios transcripts -->

## The shelves

### Catalogued
- Yes, with limits: the {{m.s1Scenarios}} DOSBox scenarios in `spikes/s1-dxe3/run.sh` pass in CI on the merge commit `{{pr4.merge}}`, each transcript compared byte for byte with `spikes/s1-dxe3/expected/`.
- Panic and divide-error recovery, two modules with separate state, and a failed `dlopen` that leaves nothing behind are each a scenario with a transcript.
- 200 rounds of load, run, stop and unload leave memory unchanged, also with a panic each round, and a deliberate 4 KB leak per round is reported as a leak: the control that shows the test can see one.
- A negative check: corrupting one expected transcript fails the run with a clear diff (PR #4 test plan).

### Uncatalogued
- The null-write and stack-overflow runs under QEMU with FreeDOS and CWSDPMI r7 (`spikes/s1-dxe3/qemu-faults.sh`): done by hand, not part of `tools/check.sh`.
- Real hardware, and any DPMI host other than CWSDPMI.
- `dlopen` timing, stack frames over 4 KB, watchdogs for infinite loops, and why a non-LTO build does not load.

### Wish list
- A module format and loader for the native track, which has no DJGPP runtime; the spike lists the properties it must provide (symbol import, per-module state, panic containment, clean unload).
- One module format shared by both tracks, which `docs/DIRECTION.md` calls ideal and not designed.

> **From the stacks — DXE.** DXE is a DJGPP facility for loading code at run time. The DJGPP FAQ describes the original design as "intentionally limited", with no access from a module to the main program's variables or functions and no unloading. The spike used the newer DXE3 interface through `dlopen` and `dlclose`, and in its runs unloading worked: every `dlclose` the transcripts print returned 0.
> Source: [DJGPP FAQ, DXE node](https://djgpp.mirror.garr.it/v2faq/faq22_15.html)
