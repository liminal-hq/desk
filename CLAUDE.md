# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Desk (`liminal-hq/desk`, a working name; see `docs/NAMING.md`) is an operating system, written in Rust, for old hardware: a calm graphical system for 486-class PCs. It is built on two tracks in parallel (see `docs/DIRECTION.md`): a hosted track on top of DOS (Rust `no_std` above the hardware, C with DJGPP for the hardware, CWSDPMI underneath, tested in DOSBox) and a native track, a bootable Rust kernel tested in QEMU, 86Box and Bochs. Both share an OS-independent core behind a hardware abstraction layer, which must not call DOS or DJGPP directly. `docs/SPEC.md` is the hosted track's spec. It is the spiritual successor to Lieutenant Fizz for DOS and starts from scratch with what that port taught.

See `AGENTS.md` for the authoritative contributor conventions — most importantly: **Canadian English** spelling everywhere; **Conventional Commits** for commit messages (except the first commit, `Initial Commit`) but **never in PR titles**; **merge commits only** when merging PRs; the licence and copyright header on new source files; **no hard-wrapped markdown**; and **no pushes unless explicitly asked**.

## State of the repository

Design and spikes, with two walking skeletons: the hosted DOS program built by `tools/build.sh` and run headless in DOSBox, and the native boot sector and Rust kernel run in QEMU by `tools/native-test.sh`. `docs/` holds the working design, written before any code, so treat its numbers and formats as targets. `design/` is reference only (do not port its code). When a spike produces a result, record it in `docs/` and let the design follow.

## Commands

```bash
tools/setup.sh                           # once: the DOS tools into vendor/
tools/check.sh                           # everything the CI runs
tools/build.sh                           # dist/DESK.EXE
tools/check-headers.sh                   # licence headers
python3 -I tools/check-docs.py           # markdown style and links
python3 -I tools/build-hero.py [name] > assets/hero.svg    # rebuild the README banner
```

Run `tools/check.sh` before opening a PR. Say plainly what you could not verify.
