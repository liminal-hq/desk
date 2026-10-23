---
id: wheels
n: 4
title: The first wheels
date: 2026-10-22
summary: The hosted walking skeleton: a Rust program and a little C, packed into one DOS executable that runs headless in CI and prints one line.
prs: [3]
---
A walking skeleton is the smallest thing that goes all the way through the machinery, and PR #3 built one for DOS. Rust `no_std` with `alloc` holds the logic (`core/`), a small C layer for DJGPP provides the runtime (`c/`), and the two are linked into one 32-bit protected-mode program, `DESK.EXE`, which runs under CWSDPMI. `tools/check.sh` builds it and runs it in headless DOSBox Staging, and the whole pipeline is written for this project: `tools/setup.sh`, `fetch-tools.sh` (with SHA-256 checks), `build.sh`, `dosrun.sh` and a fresh `coff2go32.py`.

What it prints is the point. The Rust core allocates with `alloc` through a `malloc` and `free` shim, does integer maths, does one x87 floating-point operation, and returns:

```
DESK-SKELETON ok: squares=10 sum=385 big=385001155 sqrt=19.621417
```

The check compares that line byte for byte, requires the same line in `OUT.TXT`, and also checks the unhappy path: a Rust panic reports through C and the program exits with status 3, which has to survive DOSBox (it does not expand `%ERRORLEVEL%`, so `dosrun.sh` recovers the status another way).

One field note from the toolchain. `coff2go32.py` was written to convert rustc's COFF objects for DJGPP's linker, and PR #3 found it was **still needed** with Rust 1.98.0: without its REL32 addend patch the program crashes, and without folding sections it prints nothing. Hold on to that: chapter 8 shows a route that does without it, though retiring it is a later decision.

The CI workflow runs `tools/setup.sh` and then `tools/check.sh` in the `ci-dos` image, with the image digest unchanged, and needs network access for the crates that `-Zbuild-std` depends on.

## The shelves

### Catalogued
- The expected line and the panic path are checked in headless DOSBox Staging by `tools/check.sh`, and CI on the merge commit `{{pr3.merge}}` is green.
- On a clean clone in the pinned `ci-dos` image, setup and check took about 10 s from a clean tree (PR #3 test plan).
- Negative checks (PR #3): a wrong expected line fails with a clear message, a broken `Vec` sum fails, a hung program ends with status 124, and a Pentium-only instruction faults under DOSBox's `cputype = 486`.

### Uncatalogued
- Real hardware, an older DOS, or any DPMI host other than CWSDPMI.
- Stack frames over 4 KB, which would need an `_alloca` helper.
- The x87 control word at start: nothing in the skeleton needs bit-exact results.

### Wish list
- Retiring the C layer and `coff2go32.py`: spike d0 (chapter 8) recommends it, and it is an open decision in the roadmap.
- Frame checksums of a drawn desktop, which `AGENTS.md` says will join the check "as the code that needs them arrives".

> **From the stacks — Extenders and DJGPP.** DJGPP, "DJ's GNU Programming Platform", is the GNU compiler for DOS, begun by DJ Delorie in 1989. A DOS extender lets a program run in 32-bit protected mode on top of a DOS that only knows real mode, and DPMI is the interface it uses to ask for memory and interrupts; its 1.0 specification is dated 12 March 1991. CWSDPMI is a DPMI host that DJGPP programs load when none is running, which is why the Desk's executables travel with a `CWSDPMI.EXE`.
> Source: [DJGPP, Wikipedia](https://en.wikipedia.org/wiki/DJGPP) and [DOS extender, Wikipedia](https://en.wikipedia.org/wiki/DOS_extender)
