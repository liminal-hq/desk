# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Desk (`liminal-hq/desk`, a working name; see `docs/NAMING.md`) is a graphical shell for MS-DOS: windows, files and small apps on a 486, with any DOS program run full-screen and the Desk resuming afterwards. Rust (`no_std`) for everything above the hardware, C (DJGPP) for the hardware, CWSDPMI underneath. It is the spiritual successor to Lieutenant Fizz for DOS and starts from scratch with what that port taught.

See `AGENTS.md` for the authoritative contributor conventions — most importantly: **Canadian English** spelling everywhere; **Conventional Commits** for commit messages (except the first commit, `Initial Commit`) but **never in PR titles**; **merge commits only** when merging PRs; the licence and copyright header on new source files; **no hard-wrapped markdown**; and **no pushes unless explicitly asked**.

## State of the repository

Design and spikes only: there is no code and no DOS toolchain script yet. `docs/` holds the working design, written before any code, so treat its numbers and formats as targets. `design/` is reference only (do not port its code). When a spike produces a result, record it in `docs/` and let the design follow.

## Commands

```bash
tools/check.sh                           # everything the CI runs
tools/check-headers.sh                   # licence headers
python3 -I tools/check-docs.py           # markdown style and links
python3 -I tools/build-hero.py [name] > assets/hero.svg    # rebuild the README banner
```

Run `tools/check.sh` before opening a PR. Say plainly what you could not verify.
